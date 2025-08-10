/*
 * Copyright (C) 2025 Jakub Kruszona-Zawadzki, Saglabs SA
 * 
 * This file is part of MooseFS.
 * 
 * MooseFS is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 2 (only).
 * 
 * MooseFS is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with MooseFS; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA 02111-1301, USA
 * or visit http://www.gnu.org/licenses/gpl-2.0.html
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif

#include <stdio.h>
#include <stdlib.h>
#include <inttypes.h>

#include "MFSCommunication.h"

#include "csdb.h"
#include "bio.h"
#include "changelog.h"
#include "datapack.h"
#include "mfslog.h"
#include "hashfn.h"
#include "massert.h"
#include "matocsserv.h"
#include "cfg.h"
#include "main.h"
#include "metadata.h"
#include "multilan.h"
#include "sockets.h"
#ifdef ENABLE_IPV6
#include "sockets_ipv6.h"
#endif

#define CSDB_OP_ADD 0
#define CSDB_OP_DEL 1
#define CSDB_OP_NEWIPPORT 2
#define CSDB_OP_NEWID 3
#define CSDB_OP_MAINTENANCEON 4
#define CSDB_OP_MAINTENANCEOFF 5
#define CSDB_OP_MAINTENANCETMP 6
// #define CSDB_OP_FASTREPLICATIONON 6
// #define CSDB_OP_FASTREPLICATIONOFF 7

static uint32_t HeavyLoadGracePeriod;
static uint32_t HeavyLoadThreshold;
static double HeavyLoadRatioThreshold;
static uint32_t MaintenanceModeTimeout;
static uint32_t TempMaintenanceModeTimeout;
static uint32_t SecondsToRemoveUnusedCS;

#define CSDBHASHSIZE 256
#define CSDBHASHFN(ip,port) (hash32((ip)^((port)<<16))%(CSDBHASHSIZE))

#define MAINTENANCE_OFF 0
#define MAINTENANCE_ON 1
#define MAINTENANCE_TMP 2

typedef struct csdbentry {
	uint32_t ip;			// IPv4 address (legacy/backward compatibility)
	uint16_t port;
	uint16_t csid;
	uint16_t number;
	uint32_t heavyloadts;		// last timestamp of heavy load state (load > thresholds)
	uint32_t load;
	uint32_t maintenance_timeout;
	uint32_t disconnection_time;
	uint8_t maintenance;
//	uint8_t fastreplication;
#ifdef ENABLE_IPV6
	mfs_ip ip_addr;			// Unified IPv4/IPv6 address
	uint8_t use_ipv6;		// Flag indicating IPv6 is being used
#endif
	void *eptr;
	struct csdbentry *next;
} csdbentry;

static csdbentry *csdbhash[CSDBHASHSIZE];
static csdbentry **csdbtab;
static uint32_t nextid;
static uint32_t disconnected_servers;
static uint32_t disconnected_servers_in_maintenance;
static uint32_t servers;
// static uint32_t disconnecttime;
static uint32_t loadsum;

/*
void csdb_disconnect_check(void) {
	static uint8_t laststate=0;
	if (disconnected_servers && laststate==0) {
		disconnecttime = main_time();
		laststate = 1;
	} else if (disconnected_servers==0) {
		disconnecttime = 0;
		laststate = 0;
	}
}
*/

void csdb_self_check(void) {
	uint32_t hash,now;
	csdbentry *csptr;
	uint32_t ds,dsm,s;

	now = main_time();

	ds = 0;
	dsm = 0;
	s = 0;
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
			if ((csptr->maintenance==MAINTENANCE_TMP && csptr->eptr!=NULL) || (csptr->maintenance!=MAINTENANCE_OFF && csptr->maintenance_timeout>0 && now>csptr->maintenance_timeout)) {
				if (csptr->eptr==NULL) {
					disconnected_servers_in_maintenance--;
				}
				csptr->maintenance = MAINTENANCE_OFF;
				csptr->maintenance_timeout = 0;
				changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_MAINTENANCEOFF,csptr->ip,csptr->port);
			}
			s++;
			if (csptr->eptr==NULL) {
				ds++;
				if (csptr->maintenance!=MAINTENANCE_OFF) {
					dsm++;
				}
			}
		}
	}
	if (s!=servers) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"csdb: servers counter mismatch - fixing (%"PRIu32"->%"PRIu32")",servers,s);
		servers = s;
	}
	if (ds!=disconnected_servers) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"csdb: disconnected servers counter mismatch - fixing (%"PRIu32"->%"PRIu32")",disconnected_servers,ds);
		disconnected_servers = ds;
	}
	if (dsm!=disconnected_servers_in_maintenance) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"csdb: disconnected and being maintained servers counter mismatch - fixing (%"PRIu32"->%"PRIu32")",disconnected_servers_in_maintenance,dsm);
		disconnected_servers_in_maintenance = dsm;
	}
//	csdb_disconnect_check();
}

uint16_t csdb_newid(void) {
	while (nextid<65536 && csdbtab[nextid]!=NULL) {
		nextid++;
	}
	return nextid;
}

void csdb_delid(uint16_t csid) {
	csdbtab[csid] = NULL;
	if (csid<nextid) {
		nextid = csid;
	}
}

void* csdb_new_connection(uint32_t ip,uint16_t port,uint16_t csid,void *eptr) {
	uint32_t hash,hashid;
	csdbentry *csptr,**cspptr,*csidptr;
	char strip[STRIPSIZE];
	char strtmpip[STRIPSIZE];

	univmakestrip(strip,ip);
	if (csid>0) {
		csidptr = csdbtab[csid];
	} else {
		csidptr = NULL;
	}
	if (csidptr && csidptr->ip == ip && csidptr->port == port) { // fast find using csid
		if (csidptr->eptr!=NULL) {
			mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using ip:port and csid (%s:%"PRIu16",%"PRIu16"), but server is still connected",strip,port,csid);
			return NULL;
		}
		csidptr->eptr = eptr;
		disconnected_servers--;
		if (csidptr->maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance--;
		}
		mfs_log(MFSLOG_SYSLOG,MFSLOG_INFO,"csdb: found cs using ip:port and csid (%s:%"PRIu16",%"PRIu16")",strip,port,csid);
		return csidptr;
	}
	hash = CSDBHASHFN(ip,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) { // slow find using (ip+port)
		if (csptr->ip == ip && csptr->port == port) {
			if (csptr->eptr!=NULL) {
				mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using ip:port (%s:%"PRIu16",%"PRIu16"), but server is still connected",strip,port,csid);
				return NULL;
			}
			csptr->eptr = eptr;
			disconnected_servers--;
			if (csptr->maintenance!=MAINTENANCE_OFF) {
				disconnected_servers_in_maintenance--;
			}
			return csptr;
		}
	}
	if (csidptr && csidptr->eptr==NULL) { // ip+port not found, but found csid - change ip+port
		univmakestrip(strtmpip,csidptr->ip);
		mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using csid (%s:%"PRIu16",%"PRIu16") - previous ip:port (%s:%"PRIu16")",strip,port,csid,strtmpip,csidptr->port);
		hashid = CSDBHASHFN(csidptr->ip,csidptr->port);
		cspptr = csdbhash + hashid;
		while ((csptr=*cspptr)) {
			if (csptr == csidptr) {
				*cspptr = csptr->next;
				csptr->next = csdbhash[hash];
				csdbhash[hash] = csptr;
				break;
			} else {
				cspptr = &(csptr->next);
			}
		}
		csidptr->ip = ip;
		csidptr->port = port;
#ifdef ENABLE_IPV6
		// Update IPv6 fields for existing entry
		ipv4_to_mfs_ip(&csidptr->ip_addr, ip);
		csidptr->use_ipv6 = 0;
#endif
			changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu16")",main_time(),CSDB_OP_NEWIPPORT,ip,port,csidptr->csid);
		csidptr->eptr = eptr;
		disconnected_servers--;
		if (csidptr->maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance--;
		}
		return csidptr;
	}
	mfs_log(MFSLOG_SYSLOG,MFSLOG_INFO,"csdb: server not found (%s:%"PRIu16",%"PRIu16"), add it to database",strip,port,csid);
	csptr = malloc(sizeof(csdbentry));
	passert(csptr);
	csptr->ip = ip;
	csptr->port = port;
#ifdef ENABLE_IPV6
	// Initialize IPv6 fields for backward compatibility
	ipv4_to_mfs_ip(&csptr->ip_addr, ip);
	csptr->use_ipv6 = 0;
#endif
	if (csid>0) {
		if (csdbtab[csid]==NULL) {
			csdbtab[csid] = csptr;
		} else {
			csid = 0;
		}
	}
	csptr->csid = csid;
	csptr->heavyloadts = 0;
	csptr->maintenance_timeout = 0;
	csptr->maintenance = MAINTENANCE_OFF;
	csptr->disconnection_time = main_time();
//	csptr->fastreplication = 1;
	csptr->load = 0;
	csptr->eptr = eptr;
	csptr->next = csdbhash[hash];
	csdbhash[hash] = csptr;
	servers++;
	changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu16")",main_time(),CSDB_OP_ADD,ip,port,csptr->csid);
	return csptr;
}

#ifdef ENABLE_IPV6
void* csdb_new_connection_v6(const mfs_ip *ip_addr,uint16_t port,uint16_t csid,void *eptr) {
	uint32_t hash;
	csdbentry *csptr,**cspptr,*csidptr;
	char strip[STRIPSIZE_V6];
	char strtmpip[STRIPSIZE_V6];
	uint32_t ipv4_for_hash;

	univ6makestrip(strip, ip_addr);
	
	// For backward compatibility, extract IPv4 for hashing if it's an IPv4 address
	if (ip_addr->family == AF_INET) {
		ipv4_for_hash = ip_addr->addr.v4;
	} else {
		// For IPv6, use a simple hash of first 4 bytes
		ipv4_for_hash = ((uint32_t*)ip_addr->addr.v6)[0];
	}

	if (csid>0) {
		csidptr = csdbtab[csid];
	} else {
		csidptr = NULL;
	}
	
	// Check if found using csid and addresses match
	if (csidptr && mfs_ip_equal(&csidptr->ip_addr, ip_addr) && csidptr->port == port) {
		if (csidptr->eptr!=NULL) {
			mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using ip:port and csid (%s:%"PRIu16",%"PRIu16"), but server is still connected",strip,port,csid);
			return NULL;
		}
		csidptr->eptr = eptr;
		disconnected_servers--;
		if (csidptr->maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance--;
		}
		mfs_log(MFSLOG_SYSLOG,MFSLOG_INFO,"csdb: found cs using ip:port and csid (%s:%"PRIu16",%"PRIu16")",strip,port,csid);
		return csidptr;
	}
	
	hash = CSDBHASHFN(ipv4_for_hash,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) { // slow find using (ip+port)
		if (csptr->use_ipv6 && mfs_ip_equal(&csptr->ip_addr, ip_addr) && csptr->port == port) {
			if (csptr->eptr!=NULL) {
				mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using ip:port (%s:%"PRIu16",%"PRIu16"), but server is still connected",strip,port,csid);
				return NULL;
			}
			csptr->eptr = eptr;
			disconnected_servers--;
			if (csptr->maintenance!=MAINTENANCE_OFF) {
				disconnected_servers_in_maintenance--;
			}
			return csptr;
		}
	}
	
	if (csidptr && csidptr->eptr==NULL) { // ip+port not found, but found csid - change ip+port
		if (csidptr->use_ipv6) {
			univ6makestrip(strtmpip, &csidptr->ip_addr);
		} else {
			univmakestrip(strtmpip, csidptr->ip);
		}
		mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"csdb: found cs using csid (%s:%"PRIu16",%"PRIu16") - previous ip:port (%s:%"PRIu16")",strip,port,csid,strtmpip,csidptr->port);
		
		// Remove from old hash bucket
		uint32_t old_hash = csidptr->use_ipv6 ? 
			CSDBHASHFN(((uint32_t*)csidptr->ip_addr.addr.v6)[0], csidptr->port) :
			CSDBHASHFN(csidptr->ip, csidptr->port);
		cspptr = csdbhash + old_hash;
		while ((csptr=*cspptr)) {
			if (csptr == csidptr) {
				*cspptr = csptr->next;
				csptr->next = csdbhash[hash];
				csdbhash[hash] = csptr;
				break;
			} else {
				cspptr = &(csptr->next);
			}
		}
		
		// Update address information
		csidptr->ip_addr = *ip_addr;
		csidptr->use_ipv6 = 1;
		csidptr->ip = mfs_ip_to_ipv4(ip_addr); // For backward compatibility
		csidptr->port = port;
		
		changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu16")",main_time(),CSDB_OP_NEWIPPORT,csidptr->ip,port,csidptr->csid);
		csidptr->eptr = eptr;
		disconnected_servers--;
		if (csidptr->maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance--;
		}
		return csidptr;
	}
	
	mfs_log(MFSLOG_SYSLOG,MFSLOG_INFO,"csdb: server not found (%s:%"PRIu16",%"PRIu16"), add it to database",strip,port,csid);
	csptr = malloc(sizeof(csdbentry));
	passert(csptr);
	
	// Set both IPv6 and legacy IPv4 fields
	csptr->ip_addr = *ip_addr;
	csptr->use_ipv6 = 1;
	csptr->ip = mfs_ip_to_ipv4(ip_addr); // For backward compatibility
	csptr->port = port;
	
	if (csid>0) {
		if (csdbtab[csid]==NULL) {
			csdbtab[csid] = csptr;
		} else {
			csid = 0;
		}
	}
	csptr->csid = csid;
	csptr->heavyloadts = 0;
	csptr->maintenance_timeout = 0;
	csptr->maintenance = MAINTENANCE_OFF;
	csptr->disconnection_time = main_time();
	csptr->load = 0;
	csptr->eptr = eptr;
	csptr->next = csdbhash[hash];
	csdbhash[hash] = csptr;
	servers++;
	changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu16")",main_time(),CSDB_OP_ADD,csptr->ip,port,csptr->csid);
	return csptr;
}
#endif

void csdb_temporary_maintenance_mode(void *v_csptr) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	if (csptr!=NULL && csptr->eptr!=NULL && csptr->maintenance==MAINTENANCE_OFF) {
		csptr->maintenance = MAINTENANCE_TMP;
		if (TempMaintenanceModeTimeout>0) {
			csptr->maintenance_timeout = main_time() + TempMaintenanceModeTimeout;
		} else {
			csptr->maintenance_timeout = 0;
		}
		changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu32")",main_time(),CSDB_OP_MAINTENANCETMP,csptr->ip,csptr->port,csptr->maintenance_timeout);
	}
}

void csdb_lost_connection(void *v_csptr) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	if (csptr!=NULL) {
		csptr->disconnection_time = main_time();
		csptr->eptr = NULL;
		disconnected_servers++;
		if (csptr->maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance++;
		}
	}
//	csdb_disconnect_check();
}

void csdb_server_load(void *v_csptr,uint32_t load) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	double loadavg;
	char strip[STRIPSIZE];
	loadsum -= csptr->load;
	if (servers>1) {
		loadavg = loadsum / (servers-1);
	} else {
		loadavg = load;
	}
	csptr->load = load;
	loadsum += load;
	if (load>HeavyLoadThreshold && load>loadavg*HeavyLoadRatioThreshold) { // cs is in 'heavy load state'
		univmakestrip(strip,csptr->ip);
		mfs_log(MFSLOG_SYSLOG,MFSLOG_NOTICE,"Heavy load server detected (%s:%u); load: %"PRIu32" ; threshold: %"PRIu32" ; loadavg (without this server): %.2lf ; ratio_threshold: %.2lf",strip,csptr->port,csptr->load,HeavyLoadThreshold,loadavg,HeavyLoadRatioThreshold);
		csptr->heavyloadts = main_time();
	}
}


uint16_t csdb_get_csid(void *v_csptr) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	char strip[STRIPSIZE];
	if (csptr->csid==0) {
		csptr->csid = csdb_newid();
		csdbtab[csptr->csid] = csptr;
		changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu16")",main_time(),CSDB_OP_NEWID,csptr->ip,csptr->port,csptr->csid);
		univmakestrip(strip,csptr->ip);
		mfs_log(MFSLOG_SYSLOG,MFSLOG_INFO,"csdb: generate new server id for (%s:%"PRIu16"): %"PRIu16,strip,csptr->port,csptr->csid);
	}
	return csptr->csid;
}

uint8_t csdb_server_is_overloaded(void *v_csptr,uint32_t now) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	return (csptr->heavyloadts+HeavyLoadGracePeriod<=now)?0:1;
}

uint8_t csdb_server_is_being_maintained(void *v_csptr) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	return (csptr->maintenance!=MAINTENANCE_OFF)?1:0;
}


uint32_t csdb_servlist_data(uint8_t mode,uint8_t *ptr,uint32_t clientip) {
	uint32_t hash;
	uint32_t now = main_time();
	uint32_t gracetime,maintenance_timeout;
	uint8_t *p;
	csdbentry *csptr;
	uint32_t i;
	uint32_t recsize;

#ifdef ENABLE_IPV6
	if (mode == 2) {
		// IPv6 extended mode: add 17 bytes (16 for IPv6 address + 1 for address family)
		recsize = 94; // 77 + 17
	} else {
		recsize = (mode==0)?73:77;
	}
#else
	recsize = (mode==0)?73:77;
#endif
	i=0;
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
			if (ptr!=NULL) {
				if (csptr->heavyloadts+HeavyLoadGracePeriod>now) {
					gracetime = csptr->heavyloadts+HeavyLoadGracePeriod-now; // seconds to be turned back to work
				} else {
					gracetime = 0; // Server is working properly and never was in heavy load state
				}
				if (csptr->maintenance_timeout>=now) {
					maintenance_timeout = csptr->maintenance_timeout - now;
				} else if (csptr->maintenance_timeout>0) {
					maintenance_timeout = 0;
				} else {
					maintenance_timeout = 0xFFFFFFFF;
				}
				p = ptr;
				if (csptr->eptr) {
					uint32_t version,chunkscount,tdchunkscount,errorcounter,load,labelmask;
					uint64_t usedspace,totalspace,tdusedspace,tdtotalspace;
					uint8_t hlstatus,mfrstatus;
					matocsserv_getservdata(csptr->eptr,&version,&usedspace,&totalspace,&chunkscount,&tdusedspace,&tdtotalspace,&tdchunkscount,&errorcounter,&load,&hlstatus,&labelmask,&mfrstatus);
					if (hlstatus==HLSTATUS_OK) {
						gracetime = 0;
					} else if (hlstatus==HLSTATUS_OVERLOADED) {
						gracetime = 0xC0000000;
					} else if (hlstatus==HLSTATUS_LSREBALANCE) {
						gracetime = 0x80000000;
					} else if (hlstatus==HLSTATUS_HSREBALANCE) {
						gracetime = 0x40000000;
					}
					put32bit(&ptr,version&0xFFFFFF);
					put32bit(&ptr,csptr->ip);
					put32bit(&ptr,multilan_map(csptr->ip,clientip));
					put16bit(&ptr,csptr->port);
					put16bit(&ptr,csptr->csid);
					put64bit(&ptr,usedspace);
					put64bit(&ptr,totalspace);
					put32bit(&ptr,chunkscount);
					put64bit(&ptr,tdusedspace);
					put64bit(&ptr,tdtotalspace);
					put32bit(&ptr,tdchunkscount);
					put32bit(&ptr,errorcounter);
					put32bit(&ptr,load);
					put32bit(&ptr,gracetime);
					put32bit(&ptr,labelmask);
					put8bit(&ptr,mfrstatus);
				} else {
					put32bit(&ptr,0);
					put32bit(&ptr,csptr->ip);
					put32bit(&ptr,multilan_map(csptr->ip,clientip));
					put16bit(&ptr,csptr->port);
					put16bit(&ptr,csptr->csid);
					put64bit(&ptr,0);
					put64bit(&ptr,0);
					put32bit(&ptr,0);
					put64bit(&ptr,0);
					put64bit(&ptr,0);
					put32bit(&ptr,0);
					put32bit(&ptr,0);
					put32bit(&ptr,0);
					put32bit(&ptr,gracetime);
					put32bit(&ptr,0);
					put8bit(&ptr,0);
				}
				if (mode>0) {
					put32bit(&ptr,maintenance_timeout);
				}
#ifdef ENABLE_IPV6
				if (mode == 2) {
					// Add IPv6 extension fields
					if (csptr->use_ipv6) {
						put8bit(&ptr, csptr->ip_addr.family); // Address family (AF_INET or AF_INET6)
						if (csptr->ip_addr.family == AF_INET6) {
							// Put 16 bytes of IPv6 address
							for (int j = 0; j < 16; j++) {
								put8bit(&ptr, csptr->ip_addr.addr.v6[j]);
							}
						} else {
							// Put IPv4 address followed by 12 zeros
							put32bit(&ptr, csptr->ip_addr.addr.v4);
							for (int j = 0; j < 12; j++) {
								put8bit(&ptr, 0);
							}
						}
					} else {
						// Legacy IPv4-only entry
						put8bit(&ptr, AF_INET);
						put32bit(&ptr, csptr->ip);
						for (int j = 0; j < 12; j++) {
							put8bit(&ptr, 0);
						}
					}
				}
#endif
				if (csptr->eptr==NULL) {
					*p |= CSERV_FLAG_DISCONNECTED;
				}
				if (csptr->maintenance!=MAINTENANCE_OFF) {
					*p |= CSERV_FLAG_MAINTENANCE;
				}
				if (csptr->maintenance==MAINTENANCE_TMP) {
					*p |= CSERV_FLAG_TMPMAINTENANCE;
				}
			}
			i++;
		}
	}
	return i*recsize;
}

uint8_t csdb_remove_server(uint32_t ip,uint16_t port) {
	uint32_t hash;
	csdbentry *csptr,**cspptr;

	hash = CSDBHASHFN(ip,port);
	cspptr = csdbhash + hash;
	while ((csptr=*cspptr)) {
		if (csptr->ip == ip && csptr->port == port) {
			if (csptr->eptr!=NULL) {
				return MFS_ERROR_ACTIVE;
			}
			if (csptr->csid>0) {
				csdb_delid(csptr->csid);
			}
			if (csptr->maintenance!=MAINTENANCE_OFF) {
				disconnected_servers_in_maintenance--;
			}
			*cspptr = csptr->next;
			free(csptr);
			servers--;
			disconnected_servers--;
			changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_DEL,ip,port);
			return MFS_STATUS_OK;
		} else {
			cspptr = &(csptr->next);
		}
	}
	return MFS_ERROR_NOTFOUND;
}

void csdb_remove_unused(void) {
	uint32_t hash,now;
	csdbentry *csptr,**cspptr;

	if (SecondsToRemoveUnusedCS>0) {
		now = main_time();
		for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
			cspptr = csdbhash + hash;
			while ((csptr=*cspptr)) {
				if (csptr->eptr==NULL && csptr->disconnection_time+SecondsToRemoveUnusedCS<now) {
					uint32_t ip = csptr->ip;
					uint16_t port = csptr->port;
					if (csptr->csid>0) {
						csdb_delid(csptr->csid);
					}
					if (csptr->maintenance!=MAINTENANCE_OFF) {
						disconnected_servers_in_maintenance--;
					}
					*cspptr = csptr->next;
					free(csptr);
					servers--;
					disconnected_servers--;
					changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_DEL,ip,port);
				} else {
					cspptr = &(csptr->next);
				}
			}
		}
	}
}

uint8_t csdb_mr_op(uint8_t op,uint32_t ip,uint16_t port, uint32_t arg) {
	uint32_t hash,hashid;
	csdbentry *csptr,**cspptr,*csidptr;

	switch (op) {
		case CSDB_OP_ADD:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					return MFS_ERROR_MISMATCH;
				}
			}
			if (arg>65535 || (arg>0 && csdbtab[arg]!=NULL)) {
				return MFS_ERROR_MISMATCH;
			}
			csptr = malloc(sizeof(csdbentry));
			passert(csptr);
			csptr->ip = ip;
			csptr->port = port;
			csptr->csid = arg;
			csdbtab[arg] = csptr;
			csptr->heavyloadts = 0;
			csptr->maintenance_timeout = 0;
			csptr->maintenance = MAINTENANCE_OFF;
			csptr->disconnection_time = main_time();
//			csptr->fastreplication = 1;
			csptr->load = 0;
			csptr->eptr = NULL;
			csptr->next = csdbhash[hash];
			csdbhash[hash] = csptr;
			servers++;
			disconnected_servers++;
			meta_version_inc();
			return MFS_STATUS_OK;
		case CSDB_OP_DEL:
			hash = CSDBHASHFN(ip,port);
			cspptr = csdbhash + hash;
			while ((csptr=*cspptr)) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->eptr!=NULL) {
						return MFS_ERROR_MISMATCH;
					}
					if (csptr->csid>0) {
						csdb_delid(csptr->csid);
					}
					if (csptr->maintenance!=MAINTENANCE_OFF) {
						disconnected_servers_in_maintenance--;
					}
					*cspptr = csptr->next;
					free(csptr);
					servers--;
					disconnected_servers--;
					meta_version_inc();
					return MFS_STATUS_OK;
				} else {
					cspptr = &(csptr->next);
				}
			}
			return MFS_ERROR_MISMATCH;
		case CSDB_OP_NEWIPPORT:
			if (arg>65535 || arg==0 || csdbtab[arg]==NULL) {
				return MFS_ERROR_MISMATCH;
			}
			csidptr = csdbtab[arg];

			hashid = CSDBHASHFN(csidptr->ip,csidptr->port);
			hash = CSDBHASHFN(ip,port);
			cspptr = csdbhash + hashid;
			while ((csptr=*cspptr)) {
				if (csptr == csidptr) {
					*cspptr = csptr->next;
					csptr->next = csdbhash[hash];
					csdbhash[hash] = csptr;
					break;
				} else {
					cspptr = &(csptr->next);
				}
			}
			if (csptr==NULL) {
				return MFS_ERROR_MISMATCH;
			}
			csptr->ip = ip;
			csptr->port = port;
			meta_version_inc();
			return MFS_STATUS_OK;
		case CSDB_OP_NEWID:
			if (arg>65535 || arg==0 || csdbtab[arg]!=NULL) {
				return MFS_ERROR_MISMATCH;
			}
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->csid!=arg) {
						if (csptr->csid>0) {
							csdb_delid(csptr->csid);
						}
						csptr->csid = arg;
						csdbtab[arg] = csptr;
					}
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
		case CSDB_OP_MAINTENANCEON:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->maintenance==MAINTENANCE_OFF) {
						if (csptr->eptr==NULL) {
							disconnected_servers_in_maintenance++;
						}
					}
					csptr->maintenance = MAINTENANCE_ON;
					csptr->maintenance_timeout = arg;
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
		case CSDB_OP_MAINTENANCEOFF:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->maintenance!=MAINTENANCE_OFF) {
						if (csptr->eptr==NULL) {
							disconnected_servers_in_maintenance--;
						}
					}
					csptr->maintenance = MAINTENANCE_OFF;
					csptr->maintenance_timeout = arg;
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
		case CSDB_OP_MAINTENANCETMP:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->maintenance==MAINTENANCE_OFF) {
						if (csptr->eptr==NULL) {
							disconnected_servers_in_maintenance++;
						}
					}
					csptr->maintenance = MAINTENANCE_TMP;
					csptr->maintenance_timeout = arg;
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
/*		case CSDB_OP_FASTREPLICATIONON:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->fastreplication!=0) {
						return MFS_ERROR_MISMATCH;
					}
					csptr->fastreplication = 1;
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
		case CSDB_OP_FASTREPLICATIONOFF:
			hash = CSDBHASHFN(ip,port);
			for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
				if (csptr->ip == ip && csptr->port == port) {
					if (csptr->fastreplication!=1) {
						return MFS_ERROR_MISMATCH;
					}
					csptr->fastreplication = 0;
					meta_version_inc();
					return MFS_STATUS_OK;
				}
			}
			return MFS_ERROR_MISMATCH;
*/
	}
	return MFS_ERROR_MISMATCH;
}


uint8_t csdb_back_to_work(uint32_t ip,uint16_t port) {
	uint32_t hash;
	csdbentry *csptr;

	hash = CSDBHASHFN(ip,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
		if (csptr->ip == ip && csptr->port == port) {
			csptr->heavyloadts = 0;
			return MFS_STATUS_OK;
		}
	}
	return MFS_ERROR_NOTFOUND;
}

uint8_t csdb_maintenance(uint32_t ip,uint16_t port,uint8_t onoff) {
	uint32_t hash;
	csdbentry *csptr;

	if (onoff!=0 && onoff!=1) {
		return MFS_ERROR_EINVAL;
	}
	hash = CSDBHASHFN(ip,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
		if (csptr->ip == ip && csptr->port == port) {
			if ((csptr->maintenance!=MAINTENANCE_OFF && onoff==0) || (csptr->maintenance==MAINTENANCE_OFF && onoff==1)) {
				csptr->maintenance = onoff?MAINTENANCE_ON:MAINTENANCE_OFF;
				if (csptr->maintenance==MAINTENANCE_ON && MaintenanceModeTimeout>0) {
					csptr->maintenance_timeout = main_time()+MaintenanceModeTimeout;
				} else {
					csptr->maintenance_timeout = 0;
				}
				if (onoff) {
					changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",%"PRIu32")",main_time(),CSDB_OP_MAINTENANCEON,ip,port,csptr->maintenance_timeout);
				} else {
					changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_MAINTENANCEOFF,ip,port);
				}
				if (csptr->eptr==NULL) {
					if (onoff) {
						disconnected_servers_in_maintenance++;
					} else {
						disconnected_servers_in_maintenance--;
					}
				}
			}
			return MFS_STATUS_OK;
		}
	}
	return MFS_ERROR_NOTFOUND;
}
/*
uint8_t csdb_fastreplication(uint32_t ip,uint16_t port,uint8_t onoff) {
	uint32_t hash;
	csdbentry *csptr;

	if (onoff!=0 && onoff!=1) {
		return MFS_ERROR_EINVAL;
	}
	hash = CSDBHASHFN(ip,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
		if (csptr->ip == ip && csptr->port == port) {
			if (csptr->fastreplication!=onoff) {
				csptr->fastreplication = onoff;
				if (onoff) {
					changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_FASTREPLICATIONON,ip,port);
				} else {
					changelog("%"PRIu32"|CSDBOP(%u,%"PRIu32",%"PRIu16",0)",main_time(),CSDB_OP_FASTREPLICATIONOFF,ip,port);
				}
			}
			return MFS_STATUS_OK;
		}
	}
	return MFS_ERROR_NOTFOUND;
}
*/
/*
uint8_t csdb_find(uint32_t ip,uint16_t port,uint16_t csid) {
	uint32_t hash;
	csdbentry *csptr;

	hash = CSDBHASHFN(ip,port);
	for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
		if (csptr->ip == ip && csptr->port == port) {
			return 1;
		}
	}
	if (csid>0 && csdbtab[csid]!=NULL) {
		return 2;
	}
	return 0;
}
*/

void csdb_get_server_counters(uint32_t *servers_ptr,uint32_t *disconnected_servers_ptr,uint32_t *disconnected_servers_in_maintenance_ptr) {
	if (servers_ptr!=NULL) {
		*servers_ptr = servers;
	}
	if (disconnected_servers_ptr!=NULL) {
		*disconnected_servers_ptr = disconnected_servers;
	}
	if (disconnected_servers_in_maintenance_ptr!=NULL) {
		*disconnected_servers_in_maintenance_ptr = disconnected_servers_in_maintenance;
	}
}

uint8_t csdb_have_all_servers(void) {
	return (disconnected_servers>0)?0:1;
}

uint8_t csdb_stop_chunk_jobs(void) {
	return (disconnected_servers>0 && disconnected_servers==disconnected_servers_in_maintenance)?1:0;
}

int csdb_compare(const void *a,const void *b) {
	const csdbentry *aa = *((const csdbentry**)a);
	const csdbentry *bb = *((const csdbentry**)b);
	if (aa->ip < bb->ip) {
		return -1;
	} else if (aa->ip > bb->ip) {
		return 1;
	} else if (aa->port < bb->port) {
		return -1;
	} else if (aa->port > bb->port) {
		return 1;
	}
	return 0;
}

uint16_t csdb_sort_servers(void) {
	csdbentry **stab,*csptr;
	uint32_t i,hash;

	stab = malloc(sizeof(csdbentry*)*(servers));
	i = 0;
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
			if (i<servers) {
				stab[i] = csptr;
				i++;
			} else {
				mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"internal error: wrong chunk servers count !!!");
				csptr->number = 0;
			}
		}
	}
	qsort(stab,servers,sizeof(csdbentry*),csdb_compare);
	for (i=0 ; i<(servers) ; i++) {
		stab[i]->number = i+1;
	}
	free(stab);

	return servers;
}

uint16_t csdb_servers_count(void) {
	return servers;
}

uint16_t csdb_getnumber(void *v_csptr) {
	csdbentry *csptr = (csdbentry*)v_csptr;
	if (csptr!=NULL) {
		return csptr->number;
	}
	return 0;
}

uint8_t csdb_store(bio *fd) {
	uint32_t hash;
	uint8_t wbuff[13],*ptr;
	csdbentry *csptr;
	uint32_t l;
	l=0;

	if (fd==NULL) {
		return 0x13;
	}
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
			l++;
		}
	}
	ptr = wbuff;
	put32bit(&ptr,l);
	if (bio_write(fd,wbuff,4)!=4) {
		return 0xFF;
	}
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
			ptr=wbuff;
			put32bit(&ptr,csptr->ip);
			put16bit(&ptr,csptr->port);
			put16bit(&ptr,csptr->csid);
			put8bit(&ptr,csptr->maintenance);
			put32bit(&ptr,csptr->maintenance_timeout);
//			put8bit(&ptr,csptr->fastreplication);
			if (bio_write(fd,wbuff,13)!=13) {
				return 0xFF;
			}
		}
	}
	return 0;
}

int csdb_load(bio *fd,uint8_t mver,int ignoreflag) {
	uint8_t rbuff[13];
	const uint8_t *ptr;
	csdbentry *csptr;
	uint32_t hash;
	uint32_t l,ip;
	uint16_t port,csid;
	uint8_t maintenance;
	uint32_t maintenance_timeout;
	uint8_t nl=1;
	uint32_t bsize;

	if (bio_read(fd,rbuff,4)!=4) {
		int err = errno;
		if (nl) {
			fputc('\n',stderr);
			// nl=0;
		}
		errno = err;
		mfs_log(MFSLOG_ERRNO_SYSLOG_STDERR,MFSLOG_ERR,"loading chunkservers: read error");
		return -1;
	}
	ptr=rbuff;
	l = get32bit(&ptr);
	if (mver<=0x10) {
		bsize = 6;
	} else if (mver<=0x11) {
		bsize = 8;
	} else if (mver<=0x12) {
		bsize = 9;
	} else {
		bsize = 13;
	}
	while (l>0) {
		if (bio_read(fd,rbuff,bsize)!=bsize) {
			int err = errno;
			if (nl) {
				fputc('\n',stderr);
				// nl=0;
			}
			errno = err;
			mfs_log(MFSLOG_ERRNO_SYSLOG_STDERR,MFSLOG_ERR,"loading chunkservers: read error");
			return -1;
		}
		ptr = rbuff;
		ip = get32bit(&ptr);
		port = get16bit(&ptr);
		if (mver>=0x11) {
			csid = get16bit(&ptr);
		} else {
			csid = 0;
		}
		if (mver>=0x12) {
			maintenance = get8bit(&ptr);
		} else {
			maintenance = MAINTENANCE_OFF;
		}
		if (mver>=0x13) {
			maintenance_timeout = get32bit(&ptr);
		} else {
			maintenance_timeout = 0;
		}
		hash = CSDBHASHFN(ip,port);
		for (csptr = csdbhash[hash] ; csptr ; csptr = csptr->next) {
#ifdef ENABLE_IPV6
			// Proper duplicate detection for both IPv4 and IPv6
			int is_duplicate = 0;
			if (ip != 0) {
				// IPv4 entry - check IPv4 address and port
				if (csptr->ip == ip && csptr->port == port) {
					is_duplicate = 1;
				}
			} else {
				// IPv6 entry (ip == 0) - check port and ensure different CSID
				// Since IPv6 addresses aren't stored in metadata, we rely on CSID uniqueness
				if (csptr->ip == 0 && csptr->port == port && csptr->csid == csid) {
					is_duplicate = 1;
				}
			}
			if (is_duplicate) {
#else
			if (csptr->ip == ip && csptr->port == port) {
#endif
				if (nl) {
					fputc('\n',stderr);
					nl=0;
				}
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_ERR,"repeated chunkserver entry (ip:%"PRIu32",port:%"PRIu16")",ip,port);
				if (ignoreflag==0) {
					fprintf(stderr,"use '-i' option to remove this chunkserver definition");
					return -1;
				}
			}
		}
		if (csid>0) {
			csptr = csdbtab[csid];
			if (csptr!=NULL) {
				if (nl) {
					fputc('\n',stderr);
					nl=0;
				}
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_ERR,"repeated chunkserver entry (csid:%"PRIu16")",csid);
				if (ignoreflag==0) {
					fprintf(stderr,"use '-i' option to remove this chunkserver definition");
					return -1;
				}
			}
		}
		csptr = malloc(sizeof(csdbentry));
		passert(csptr);
		csptr->ip = ip;
		csptr->port = port;
		csptr->csid = csid;
		if (csid>0) {
			csdbtab[csid] = csptr;
		}
		csptr->number = 0;
		csptr->heavyloadts = 0;
		csptr->load = 0;
		csptr->eptr = NULL;
		csptr->maintenance = maintenance;
		csptr->maintenance_timeout = maintenance_timeout;
		csptr->disconnection_time = main_time();
//		csptr->fastreplication = fastreplication;
#ifdef ENABLE_IPV6
		// Initialize IPv6 fields for entries loaded from metadata
		if (ip == 0) {
			csptr->use_ipv6 = 1;
			// IPv6 address will be set when chunkserver connects
			memset(&csptr->ip_addr, 0, sizeof(csptr->ip_addr));
		} else {
			csptr->use_ipv6 = 0;
			csptr->ip_addr.family = AF_INET;
			csptr->ip_addr.addr.v4 = ip;
		}
#endif
		csptr->next = csdbhash[hash];
		csdbhash[hash] = csptr;
		servers++;
		disconnected_servers++;
		if (maintenance!=MAINTENANCE_OFF) {
			disconnected_servers_in_maintenance++;
		}
		l--;
	}
	return 0;
}

void csdb_cleanup(void) {
	uint32_t hash;
	csdbentry *csptr,*csnptr;

	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		csptr = csdbhash[hash];
		while (csptr) {
			csnptr = csptr->next;
			free(csptr);
			csptr = csnptr;
		}
		csdbhash[hash]=NULL;
	}
	for (hash=0 ; hash<65536 ; hash++) {
		csdbtab[hash] = NULL;
	}
	nextid = 1;
	disconnected_servers = 0;
	disconnected_servers_in_maintenance = 0;
	servers = 0;
}
/*
uint32_t csdb_getdisconnecttime(void) {
	return disconnecttime;
}
*/
void csdb_reload(void) {
	uint32_t dtr;
	HeavyLoadGracePeriod = cfg_getuint32("CS_HEAVY_LOAD_GRACE_PERIOD",900);
	HeavyLoadThreshold = cfg_getuint32("CS_HEAVY_LOAD_THRESHOLD",150);
	HeavyLoadRatioThreshold = cfg_getdouble("CS_HEAVY_LOAD_RATIO_THRESHOLD",3.0);
	MaintenanceModeTimeout = cfg_getsperiod("CS_MAINTENANCE_MODE_TIMEOUT","0");
	TempMaintenanceModeTimeout = cfg_getsperiod("CS_TEMP_MAINTENANCE_MODE_TIMEOUT","30m");
	dtr = cfg_getuint32("CS_DAYS_TO_REMOVE_UNUSED",7);
	if (dtr>365) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"CS_DAYS_TO_REMOVE_UNUSED - value is too big (max=365) - use zero for infinite value");
		dtr = 0;
	}
	SecondsToRemoveUnusedCS = dtr * 86400;
}

int csdb_init(void) {
	uint32_t hash;
	csdb_reload();
	for (hash=0 ; hash<CSDBHASHSIZE ; hash++) {
		csdbhash[hash]=NULL;
	}
	csdbtab = malloc(sizeof(csdbentry*)*65536);
	passert(csdbtab);
	for (hash=0 ; hash<65536 ; hash++) {
		csdbtab[hash] = NULL;
	}
	nextid = 1;
	disconnected_servers = 0;
	disconnected_servers_in_maintenance = 0;
	servers = 0;
//	disconnecttime = 0;
	loadsum = 0;
	main_reload_register(csdb_reload);
	main_time_register(1,0,csdb_self_check);
	main_time_register(600,300,csdb_remove_unused);
	return 0;
}
