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
#include <string.h>
#include <inttypes.h>

#include "MFSCommunication.h"
#include "sharedpointer.h"
#include "filesystem.h"
#include "sessions.h"
#include "openfiles.h"
#include "flocklocks.h"
#include "posixlocks.h"
#include "csdb.h"
#include "chunks.h"
#include "storageclass.h"
#include "patterns.h"
#include "metadata.h"
#include "mfslog.h"
#include "massert.h"
#include "mfsstrerr.h"

#define EAT(clptr,fn,vno,c) { \
	if (*(clptr)!=(c)) { \
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": '%c' expected",(fn),(vno),(c)); \
		return -1; \
	} \
	(clptr)++; \
}

#define NEXTSEPARATOR(c,clptr) { \
	const char *_tmp_clptr = (const char *)clptr; \
	char _tmp_c; \
	(c) = '#'; \
	while ((c)=='#') { \
		_tmp_c = *(_tmp_clptr); \
		(_tmp_clptr)++; \
		if (_tmp_c<32 || _tmp_c>=127 || _tmp_c==',' || _tmp_c=='(' || _tmp_c==')') { \
			(c) = _tmp_c; \
		} \
	} \
}

#define GETNAME(name,clptr,fn,vno,c) { \
	uint32_t _tmp_i; \
	char _tmp_c,_tmp_h1,_tmp_h2; \
	memset((void*)(name),0,256); \
	_tmp_i = 0; \
	while ((_tmp_c=*((clptr)++))!=c && _tmp_i<255) { \
		if (_tmp_c=='\0' || _tmp_c=='\r' || _tmp_c=='\n') { \
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": '%c' expected",(fn),(vno),(c)); \
			return -1; \
		} \
		if (_tmp_c=='%') { \
			_tmp_h1 = *((clptr)++); \
			_tmp_h2 = *((clptr)++); \
			if (_tmp_h1>='0' && _tmp_h1<='9') { \
				_tmp_h1-='0'; \
			} else if (_tmp_h1>='A' && _tmp_h1<='F') { \
				_tmp_h1-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			if (_tmp_h2>='0' && _tmp_h2<='9') { \
				_tmp_h2-='0'; \
			} else if (_tmp_h2>='A' && _tmp_h2<='F') { \
				_tmp_h2-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			_tmp_c = _tmp_h1*16+_tmp_h2; \
		} \
		name[_tmp_i++] = _tmp_c; \
	} \
	(clptr)--; \
	name[_tmp_i]=0; \
}

#define GETPATH(path,size,clptr,fn,vno,c) { \
	uint32_t _tmp_i; \
	char _tmp_c,_tmp_h1,_tmp_h2; \
	_tmp_i = 0; \
	while ((_tmp_c=*((clptr)++))!=c) { \
		if (_tmp_c=='\0' || _tmp_c=='\r' || _tmp_c=='\n') { \
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": '%c' expected",(fn),(vno),(c)); \
			return -1; \
		} \
		if (_tmp_c=='%') { \
			_tmp_h1 = *((clptr)++); \
			_tmp_h2 = *((clptr)++); \
			if (_tmp_h1>='0' && _tmp_h1<='9') { \
				_tmp_h1-='0'; \
			} else if (_tmp_h1>='A' && _tmp_h1<='F') { \
				_tmp_h1-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			if (_tmp_h2>='0' && _tmp_h2<='9') { \
				_tmp_h2-='0'; \
			} else if (_tmp_h2>='A' && _tmp_h2<='F') { \
				_tmp_h2-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			_tmp_c = _tmp_h1*16+_tmp_h2; \
		} \
		if ((_tmp_i)>=(size)) { \
			(size) = _tmp_i+1000; \
			if ((path)==NULL) { \
				(path) = malloc(size); \
			} else { \
				uint8_t *_tmp_path = (path); \
				(path) = realloc((path),(size)); \
				if ((path)==NULL) { \
					free(_tmp_path); \
				} \
			} \
			passert(path); \
		} \
		(path)[_tmp_i++]=_tmp_c; \
	} \
	if ((_tmp_i)>=(size)) { \
		(size) = _tmp_i+1000; \
		if ((path)==NULL) { \
			(path) = malloc(size); \
		} else { \
			uint8_t *_tmp_path = (path); \
			(path) = realloc((path),(size)); \
			if ((path)==NULL) { \
				free(_tmp_path); \
			} \
		} \
		passert(path); \
	} \
	(clptr)--; \
	(path)[_tmp_i]=0; \
}

#define GETDATA(buff,leng,size,clptr,fn,vno,c) { \
	char _tmp_c,_tmp_h1,_tmp_h2; \
	(leng) = 0; \
	while ((_tmp_c=*((clptr)++))!=c) { \
		if (_tmp_c=='\0' || _tmp_c=='\r' || _tmp_c=='\n') { \
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": '%c' expected",(fn),(vno),(c)); \
			return -1; \
		} \
		if (_tmp_c=='%') { \
			_tmp_h1 = *((clptr)++); \
			_tmp_h2 = *((clptr)++); \
			if (_tmp_h1>='0' && _tmp_h1<='9') { \
				_tmp_h1-='0'; \
			} else if (_tmp_h1>='A' && _tmp_h1<='F') { \
				_tmp_h1-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			if (_tmp_h2>='0' && _tmp_h2<='9') { \
				_tmp_h2-='0'; \
			} else if (_tmp_h2>='A' && _tmp_h2<='F') { \
				_tmp_h2-=('A'-10); \
			} else { \
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": hex expected",(fn),(vno)); \
				return -1; \
			} \
			_tmp_c = _tmp_h1*16+_tmp_h2; \
		} \
		if ((leng)>=(size)) { \
			(size) = (leng)+1000; \
			if ((buff)==NULL) { \
				(buff) = malloc(size); \
			} else { \
				uint8_t *_tmp_buff = (buff); \
				(buff) = realloc((buff),(size)); \
				if ((buff)==NULL) { \
					free(_tmp_buff); \
				} \
			} \
			passert(buff); \
		} \
		(buff)[(leng)++]=_tmp_c; \
	} \
	(clptr)--; \
}

#define GETARRAYU32(buff,leng,size,clptr,fn,vno) { \
	char _tmp_c; \
	char *eptr; \
	(leng) = 0; \
	_tmp_c = *((clptr)++); \
	if (_tmp_c!='[') { \
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": '[' expected",(fn),(vno)); \
		return -1; \
	} \
	while ((_tmp_c=*((clptr)++))!=']') { \
		if (_tmp_c=='\0' || _tmp_c=='\r' || _tmp_c=='\n') { \
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": ']' expected",(fn),(vno)); \
			return -1; \
		} \
		if (_tmp_c>='0' && _tmp_c<='9') { \
			(clptr)--; \
			if ((leng)>=(size)) { \
				(size) = (leng)+32; \
				if ((buff)==NULL) { \
					(buff) = malloc((size)*sizeof(uint32_t)); \
				} else { \
					uint32_t *_tmp_buff = (buff); \
					(buff) = realloc((buff),(size)*sizeof(uint32_t)); \
					if ((buff)==NULL) { \
						free(_tmp_buff); \
					} \
				} \
				passert(buff); \
			} \
			(buff)[(leng)++] = strtoul((clptr),&eptr,10); \
			(clptr) = (const char*)eptr; \
		} else if (_tmp_c!=',') { \
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": number or ',' expected",(fn),(vno)); \
			return -1; \
		} \
	} \
}

#define GETHEX(buff,leng,size,clptr,fn,vno) { \
	char _tmp_h1,_tmp_h2; \
	(leng) = 0; \
	while (1) { \
		_tmp_h1 = *((clptr)++); \
		if (_tmp_h1>='0' && _tmp_h1<='9') { \
			_tmp_h1-='0'; \
		} else if (_tmp_h1>='A' && _tmp_h1<='F') { \
			_tmp_h1-=('A'-10); \
		} else { \
			(clptr)--; \
			break; \
		} \
		_tmp_h2 = *((clptr)++); \
		if (_tmp_h2>='0' && _tmp_h2<='9') { \
			_tmp_h2-='0'; \
		} else if (_tmp_h2>='A' && _tmp_h2<='F') { \
			_tmp_h2-=('A'-10); \
		} else { \
			(clptr)--; \
			break; \
		} \
		_tmp_h1 = _tmp_h1*16+_tmp_h2; \
		if ((leng)>=(size)) { \
			(size) = (leng)+1000; \
			if ((buff)==NULL) { \
				(buff) = malloc(size); \
			} else { \
				uint8_t *_tmp_buff = (buff); \
				(buff) = realloc((buff),(size)); \
				if ((buff)==NULL) { \
					free(_tmp_buff); \
				} \
			} \
			passert(buff); \
		} \
		(buff)[(leng)++]=_tmp_h1; \
	} \
}

#define GETU8(data,clptr) { \
	uint32_t tmp; \
	char *eptr; \
	tmp=strtoul(clptr,&eptr,10); \
	clptr = (const char*)eptr; \
	if (tmp>255) { \
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"value too big (%"PRIu32" - 0-255 expected)",tmp); \
		return -1; \
	} \
	(data)=tmp; \
}

#define GETU16(data,clptr) { \
	uint32_t tmp; \
	char *eptr; \
	tmp=strtoul(clptr,&eptr,10); \
	clptr = (const char*)eptr; \
	if (tmp>65535) { \
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"value too big (%"PRIu32" - 0-65535 expected)",tmp); \
		return -1; \
	} \
	(data)=tmp; \
}

#define GETU32(data,clptr) { \
	char *eptr; \
	(data)=strtoul(clptr,&eptr,10); \
	clptr = (const char*)eptr; \
}

#define GETX32(data,clptr) { \
	char *eptr; \
	(data)=strtoul(clptr,&eptr,16); \
	clptr = (const char*)eptr; \
}

#define GETU64(data,clptr) { \
	char *eptr; \
	(data)=strtoull(clptr,&eptr,10); \
	clptr = (const char*)eptr; \
}

int do_idle(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	(void)ts;
	EAT(ptr,filename,lv,'(');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	meta_version_inc(); // no-operation - just increase meta version and return OK.
	return MFS_STATUS_OK;
}

int do_access(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_access(ts,inode);
}

int do_addattr(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,attrblobleng;
	uint8_t flags;
	static uint8_t *attrblob = NULL;
	static uint32_t attrblobsize = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(flags,ptr);
	EAT(ptr,filename,lv,',');
	GETDATA(attrblob,attrblobleng,attrblobsize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_additionalattr(ts,inode,flags,attrblob,attrblobleng);
}

int do_append(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,inode_src;
	uint32_t slice_from,slice_to;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(inode_src,ptr);
	if (*ptr==')') {
		slice_from = 0xFFFFFFFF;
		slice_to = 0;
	} else {
		EAT(ptr,filename,lv,',');
		GETU32(slice_from,ptr);
		EAT(ptr,filename,lv,',');
		GETU32(slice_to,ptr);
	}
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_append_slice(ts,inode,inode_src,slice_from,slice_to);
}

int do_acquire(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,cuid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(cuid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return of_mr_acquire(inode,cuid);
}

int do_archchg(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,uid,nsinodes;
	uint8_t flags;
	uint64_t chgchunks,notchgchunks;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(flags,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU64(chgchunks,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(notchgchunks,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(nsinodes,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_archchg(ts,inode,uid,flags,chgchunks,notchgchunks,nsinodes);
}

int do_amtime(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,xatime,xmtime,xctime;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(xatime,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(xmtime,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(xctime,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_amtime(inode,ts,xatime,xmtime,xctime);
}

int do_autoarch(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,archreftime;
	uint32_t archchgchunks,trashchgchunks;
	uint8_t intrash;
	uint8_t format;
	(void)ts;
	format = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(archreftime,ptr);
	if (*ptr==',') {
		format = 1;
		EAT(ptr,filename,lv,',');
		GETU8(intrash,ptr);
	} else {
		intrash = 1; // old metadata - do not clear trash flag
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(archchgchunks,ptr);
	if (format==1) {
		EAT(ptr,filename,lv,',');
		GETU32(trashchgchunks,ptr);
	} else {
		trashchgchunks = 0;
	}
	(void)ptr; // silence cppcheck warnings
	return fs_mr_autoarch(inode,archreftime,intrash,archchgchunks,trashchgchunks);
}

int do_attr(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,mode,uid,gid,atime,mtime,winattr,aclmode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(gid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(atime,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mtime,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(aclmode,ptr);
		if (*ptr==',') {
			EAT(ptr,filename,lv,',');
			winattr = aclmode;
			GETU32(aclmode,ptr);
		} else {
			winattr = 0;
		}
	} else {
		aclmode = mode;
		winattr = 0;
	}
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_attr(ts,inode,mode,uid,gid,atime,mtime,winattr,aclmode);
}

/*
int do_copy(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent;
	uint8_t name[256];
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	return fs_mr_copy(ts,inode,parent,strlen(name),name);
}
*/

int do_create(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t parent,uid,gid,rdev,inode;
	uint16_t mode,cumask;
	uint8_t type,name[256];
	EAT(ptr,filename,lv,'(');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	if (*ptr>='0' && *ptr<='9') {
		GETU8(type,ptr);
	} else {
		type = *ptr;
		ptr++;
	}
	EAT(ptr,filename,lv,',');
	GETU16(mode,ptr);
	EAT(ptr,filename,lv,',');
	if (type<16) {
		GETU16(cumask,ptr);
		EAT(ptr,filename,lv,',');
	} else {
		cumask = 0;
	}
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(gid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(rdev,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(inode,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_create(ts,parent,strlen((char*)name),name,type,mode,cumask,uid,gid,rdev,inode);
}

int do_csdbop(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t op,ip,port,arg;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU8(op,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(ip,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(port,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(arg,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return csdb_mr_op(op,ip,port,arg);
}

/* deprecated since version 1.7.25 */
int do_csadd(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t ip,port;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(ip,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(port,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return csdb_mr_csadd(ip,port);
}

/* deprecated since version 1.7.25 */
int do_csdel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t ip,port;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(ip,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(port,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return csdb_mr_csdel(ip,port);
}

int do_chunkadd(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	uint32_t version,lockedto;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(version,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(lockedto,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_chunkadd(ts,chunkid,version,lockedto);
}

int do_chunkdel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	uint32_t version;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(version,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_chunkdel(ts,chunkid,version);
}

int do_chunkflagsclr(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_flagsclr(ts,chunkid);
}

int do_emptytrash(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sustainedinodes,freeinodes,trashflaginodes,inode_chksum,bid;
	EAT(ptr,filename,lv,'(');
	if (*ptr!=')') {
		GETU32(bid,ptr);
	} else {
		bid = 0xFFFFFFFF;
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(freeinodes,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sustainedinodes,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(inode_chksum,ptr);
		if (*ptr==',') {
			EAT(ptr,filename,lv,',');
			trashflaginodes = inode_chksum;
			GETU32(inode_chksum,ptr);
		} else {
			trashflaginodes = UINT32_C(0xFFFFFFFF);
		}
	} else {
		inode_chksum = 0;
		trashflaginodes = UINT32_C(0xFFFFFFFF);
	}
	(void)ptr; // silence cppcheck warnings
	return fs_mr_emptytrash(ts,bid,freeinodes,sustainedinodes,trashflaginodes,inode_chksum);
}

int do_emptysustained(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t freeinodes,inode_chksum,bid;
	EAT(ptr,filename,lv,'(');
	if (*ptr!=')') {
		GETU32(bid,ptr);
	} else {
		bid = 0xFFFFFFFF;
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(freeinodes,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(inode_chksum,ptr);
	} else {
		inode_chksum = 0;
	}
	(void)ptr; // silence cppcheck warnings
	return fs_mr_emptysustained(ts,bid,freeinodes,inode_chksum);
}

int do_flock(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,sessionid;
	uint64_t owner;
	char cmd;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(owner,ptr);
	EAT(ptr,filename,lv,',');
	cmd = *ptr;
	ptr++;
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return flock_mr_change(inode,sessionid,owner,cmd);
}

int do_freeinodes(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sustainedinodes,freeinodes,inode_chksum;
	uint32_t inodereusedelay;
	EAT(ptr,filename,lv,'(');
	if (*ptr==')') {
		inodereusedelay = 86400;
	} else {
		GETU32(inodereusedelay,ptr);
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(freeinodes,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(sustainedinodes,ptr);
	} else {
		sustainedinodes = 0;
	}
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(inode_chksum,ptr);
	} else {
		inode_chksum = 0;
	}
	(void)ptr; // silence cppcheck warnings
	return fs_mr_freeinodes(ts,inodereusedelay,freeinodes,sustainedinodes,inode_chksum);
}

int do_incversion(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) { // depreciated - replaced by 'setversion'
	uint64_t chunkid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_increase_version(chunkid);
}

int do_setversion(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	uint32_t version;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(version,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_set_version(chunkid,version);
}


int do_link(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent;
	uint8_t name[256];
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_link(ts,inode,parent,strlen((char*)name),name);
}

int do_length(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	uint64_t length;
	uint8_t canmodmtime;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(length,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU8(canmodmtime,ptr);
	} else {
		canmodmtime = 1;
	}
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_length(ts,inode,length,canmodmtime);
}

int do_move(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent_src,parent_dst;
	uint8_t name_src[256],name_dst[256];
	uint8_t rmode;
	char s;
	EAT(ptr,filename,lv,'(');
	GETU32(parent_src,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name_src,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(parent_dst,ptr);
	EAT(ptr,filename,lv,',');
	NEXTSEPARATOR(s,ptr);
	if (s==',') {
		GETNAME(name_dst,ptr,filename,lv,',');
		EAT(ptr,filename,lv,',');
		GETU8(rmode,ptr);
	} else {
		GETNAME(name_dst,ptr,filename,lv,')');
		rmode = 0;
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(inode,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_move(ts,parent_src,strlen((char*)name_src),name_src,parent_dst,strlen((char*)name_dst),name_dst,rmode,inode);
}

int do_nextchunkid(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return chunk_mr_nextchunkid(chunkid);
}

int do_patadd(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t gname[256];
	uint32_t euid,egid;
	uint8_t priority,omask,scid,seteattr,clreattr;
	uint16_t trashretention;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETNAME(gname,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(euid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(egid,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(priority,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(omask,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(scid,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(trashretention,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(seteattr,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU8(clreattr,ptr);
	} else {
		clreattr = 0;
	}
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return patterns_mr_add(strlen((char*)gname),gname,euid,egid,priority,omask,scid,trashretention,seteattr,clreattr);
}

int do_patdel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t gname[256];
	uint32_t euid,egid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETNAME(gname,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(euid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(egid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return patterns_mr_delete(strlen((char*)gname),gname,euid,egid);
}

int do_posixlock(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,sessionid,pid;
	uint64_t owner,start,end;
	char cmd;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(owner,ptr);
	EAT(ptr,filename,lv,',');
	cmd = *ptr;
	ptr++;
	EAT(ptr,filename,lv,',');
	GETU64(start,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(end,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(pid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return posix_lock_mr_change(inode,sessionid,owner,cmd,start,end,pid);
}

int do_purge(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_purge(ts,inode);
}

int do_quota(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,stimestamp,sinodes,hinodes;
	uint64_t slength,ssize,srealsize;
	uint64_t hlength,hsize,hrealsize;
	uint32_t flags,exceeded,timelimit;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(exceeded,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(flags,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(stimestamp,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sinodes,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(hinodes,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(slength,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(hlength,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(ssize,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(hsize,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(srealsize,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(hrealsize,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(timelimit,ptr);
	} else {
		timelimit = 0;
	}
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_quota(ts,inode,exceeded,flags,stimestamp,sinodes,hinodes,slength,hlength,ssize,hsize,srealsize,hrealsize,timelimit);
}

/*
int do_reinit(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx;
	uint64_t chunkid;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU64(chunkid,ptr);
	return fs_mr_reinit(ts,inode,indx,chunkid);
}
*/
int do_release(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,cuid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(cuid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return of_mr_release(inode,cuid);
}

int do_repair(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx;
	uint32_t version;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(version,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_repair(ts,inode,indx,version);
}

int do_renumedges(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t enextedgeid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU64(enextedgeid,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_renumerate_edges(enextedgeid);
}
/*
int do_remove(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,')');
	return fs_mr_remove(ts,inode);
}
*/
int do_session(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sessionid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(sessionid,ptr);
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_session(sessionid);
}

int do_sesadd(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t rootinode,sesflags,peerip,sessionid;
	uint32_t rootuid,rootgid,mapalluid,mapallgid;
	uint32_t mingoal,maxgoal,sclassgroups,mintrashretention,maxtrashretention;
	uint32_t disables;
	uint16_t umaskval;
	uint64_t exportscsum;
	uint32_t ileng;
	static uint8_t *info = NULL;
	static uint32_t infosize = 0;

	(void)ts;
	EAT(ptr,filename,lv,'(');
	if (*ptr=='#') {
		EAT(ptr,filename,lv,'#');
		GETU64(exportscsum,ptr);
		EAT(ptr,filename,lv,',');
	} else {
		exportscsum = 0;
	}
	GETU32(rootinode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sesflags,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='0') {
		if (ptr[1]<'0' || ptr[1]>'7' || ptr[2]<'0' || ptr[2]>'7' || ptr[3]<'0' || ptr[3]>'7') {
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"wrong session umask ('%c%c%c' - octal number expected)",ptr[1],ptr[2],ptr[3]);
			return -1;
		}
		umaskval = (ptr[1]-'0') * 64 + (ptr[2]-'0') * 8 + (ptr[3]-'0');
		ptr+=4;
		EAT(ptr,filename,lv,',');
	} else {
		umaskval=0;
	}
	GETU32(rootuid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(rootgid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mapalluid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mapallgid,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='0' && ptr[1]=='x') {
		ptr+=2;
		GETX32(sclassgroups,ptr);
		if (sclassgroups>0xFFFF) {
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"wrong sclassgroups (0x%"PRIX32")",sclassgroups);
			return -1;
		}
	} else {
		GETU32(mingoal,ptr);
		EAT(ptr,filename,lv,',');
		GETU32(maxgoal,ptr);
		sclassgroups = 1;
		while (mingoal<=maxgoal) {
			sclassgroups |= (1<<mingoal);
			mingoal++;
		}
	}
	EAT(ptr,filename,lv,',');
	GETU32(mintrashretention,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(maxtrashretention,ptr);
	EAT(ptr,filename,lv,',');
	if (ptr[0]=='0' && ptr[1]=='x') {
		ptr+=2;
		GETX32(disables,ptr);
		EAT(ptr,filename,lv,',');
	} else {
		disables = 0;
	}
	GETU32(peerip,ptr);
	EAT(ptr,filename,lv,',');
	GETDATA(info,ileng,infosize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(sessionid,ptr);
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_sesadd(exportscsum,rootinode,sesflags,umaskval,rootuid,rootgid,mapalluid,mapallgid,sclassgroups,mintrashretention,maxtrashretention,disables,peerip,info,ileng,sessionid);
}

int do_seschanged(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t rootinode,sesflags,peerip,sessionid;
	uint32_t rootuid,rootgid,mapalluid,mapallgid;
	uint32_t mingoal,maxgoal,sclassgroups,mintrashretention,maxtrashretention;
	uint32_t disables;
	uint16_t umaskval;
	uint64_t exportscsum;
	uint32_t ileng;
	static uint8_t *info = NULL;
	static uint32_t infosize = 0;

	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='#') {
		EAT(ptr,filename,lv,'#');
		GETU64(exportscsum,ptr);
		EAT(ptr,filename,lv,',');
	} else {
		exportscsum = 0;
	}
	GETU32(rootinode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sesflags,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='0') {
		if (ptr[1]<'0' || ptr[1]>'7' || ptr[2]<'0' || ptr[2]>'7' || ptr[3]<'0' || ptr[3]>'7') {
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"wrong session umask ('%c%c%c' - octal number expected)",ptr[1],ptr[2],ptr[3]);
			return -1;
		}
		umaskval = (ptr[1]-'0') * 64 + (ptr[2]-'0') * 8 + (ptr[3]-'0');
		ptr+=4;
		EAT(ptr,filename,lv,',');
	} else {
		umaskval=0;
	}
	GETU32(rootuid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(rootgid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mapalluid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(mapallgid,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='0' && ptr[1]=='x') {
		ptr+=2;
		GETX32(sclassgroups,ptr);
		if (sclassgroups>0xFFFF) {
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"wrong sclassgroups (0x%"PRIX32")",sclassgroups);
			return -1;
		}
	} else {
		GETU32(mingoal,ptr);
		EAT(ptr,filename,lv,',');
		GETU32(maxgoal,ptr);
		sclassgroups = 1;
		while (mingoal<=maxgoal) {
			sclassgroups |= (1<<mingoal);
			mingoal++;
		}
	}
	EAT(ptr,filename,lv,',');
	GETU32(mintrashretention,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(maxtrashretention,ptr);
	EAT(ptr,filename,lv,',');
	if (ptr[0]=='0' && ptr[1]=='x') {
		ptr+=2;
		GETX32(disables,ptr);
		EAT(ptr,filename,lv,',');
	} else {
		disables = 0;
	}
	GETU32(peerip,ptr);
	EAT(ptr,filename,lv,',');
	GETDATA(info,ileng,infosize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_seschanged(sessionid,exportscsum,rootinode,sesflags,umaskval,rootuid,rootgid,mapalluid,mapallgid,sclassgroups,mintrashretention,maxtrashretention,disables,peerip,info,ileng);
}

int do_sesdel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sessionid;

	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_sesdel(sessionid);
}

int do_sesconnected(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sessionid;

	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_connected(sessionid);
}

int do_sesdisconnected(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t sessionid;

	EAT(ptr,filename,lv,'(');
	GETU32(sessionid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sessions_mr_disconnected(sessionid,ts);
}

int do_rollback(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx;
	uint64_t prevchunkid,chunkid;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(prevchunkid,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_rollback(ts,inode,indx,prevchunkid,chunkid);
}

int do_seteattr(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,uid,ci,nci,npi;
	uint8_t eattr,smode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(eattr,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(smode,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(ci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(nci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(npi,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_seteattr(ts,inode,uid,eattr,smode,ci,nci,npi);
}

int do_setfilechunk(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx;
	uint64_t chunkid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	EAT(ptr,filename,lv,',');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_set_file_chunk(inode,indx,chunkid);
}

// deprecated
int do_setgoal(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,uid,ci,nci,npi;
	uint8_t sclassid,smode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sclassid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(smode,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(ci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(nci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(npi,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_setsclass(ts,inode,uid,sclassid,sclassid,smode,ci,nci,npi);
}

int do_setsclass(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,uid,ci,nci,npi;
	uint8_t src_sclassid,dst_sclassid,smode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(src_sclassid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(dst_sclassid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(smode,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(ci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(nci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(npi,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_setsclass(ts,inode,uid,src_sclassid,dst_sclassid,smode,ci,nci,npi);
}

int do_setmetaid(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t metaid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU64(metaid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return meta_mr_setmetaid(metaid);
}

int do_setpath(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	static uint8_t *path = NULL;
	static uint32_t pathsize = 0;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETPATH(path,pathsize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_setpath(inode,path);
}

int do_settrashretention(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,uid,ci,nci,npi;
	uint32_t trashretention;
	uint8_t smode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(trashretention,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(smode,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(ci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(nci,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(npi,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_settrashretention(ts,inode,uid,trashretention,smode,ci,nci,npi);
}

int do_setxattr(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,valueleng,mode;
	uint8_t name[256];
	static uint8_t *value = NULL;
	static uint32_t valuesize = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETDATA(value,valueleng,valuesize,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(mode,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_setxattr(ts,inode,strlen((char*)name),name,valueleng,value,mode);
}

int do_setacl(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,aclblobleng;
	uint8_t acltype,changectime;
	uint16_t mode,userperm,groupperm,otherperm,mask,namedusers,namedgroups;
	static uint8_t *aclblob = NULL;
	static uint32_t aclblobsize = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(mode,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(changectime,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(acltype,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(userperm,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(groupperm,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(otherperm,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(mask,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(namedusers,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(namedgroups,ptr);
	EAT(ptr,filename,lv,',');
	GETDATA(aclblob,aclblobleng,aclblobsize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	if (aclblobleng!=6U*(namedusers+namedgroups)) {
		return MFS_ERROR_MISMATCH;
	}
	return fs_mr_setacl(ts,inode,mode,changectime,acltype,userperm,groupperm,otherperm,mask,namedusers,namedgroups,aclblob);
}

int do_snapshot(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent,smode,sesflags,uid,gids,umask;
	uint32_t inodecheck,removed,same,existing,hardlinks,new;
	static uint8_t *gidstr = NULL;
	static uint32_t gidstrsize = 0;
	static uint32_t *gid = NULL;
	static uint32_t gidsize = 0;
	uint32_t gidleng;
	uint32_t *gidtab;
	uint8_t name[256];
	uint8_t mode;
	uint32_t i;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(smode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(sesflags,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='[') {
		mode=2;
	} else {
		GETU32(gids,ptr);
		EAT(ptr,filename,lv,',');
		mode = 0;
		for (i=0 ; ptr[i] && ptr[i]!=')' ; i++) {
			if (ptr[i]<'0' || ptr[i]>'9') {
				mode=1;
				break;
			}
		}
	}
	if (mode>=1) {
		if (mode==2) {
			GETARRAYU32(gid,gids,gidsize,ptr,filename,lv);
			gidtab = gid;
		} else {
			GETDATA(gidstr,gidleng,gidstrsize,ptr,filename,lv,',');
			if (gids*4!=gidleng) {
				return MFS_ERROR_MISMATCH;
			}
			gidtab = (uint32_t*)gidstr;
		}
		EAT(ptr,filename,lv,',');
		GETU32(umask,ptr);
		EAT(ptr,filename,lv,')');
		if (*ptr==':') {
			EAT(ptr,filename,lv,':');
			GETU32(inodecheck,ptr);
			EAT(ptr,filename,lv,',');
			GETU32(removed,ptr);
			EAT(ptr,filename,lv,',');
			GETU32(same,ptr);
			EAT(ptr,filename,lv,',');
			GETU32(existing,ptr);
			EAT(ptr,filename,lv,',');
			GETU32(hardlinks,ptr);
			EAT(ptr,filename,lv,',');
			GETU32(new,ptr);
		} else {
			inodecheck=0;
			removed=0;
			same=0;
			existing=0;
			hardlinks=0;
			new=0;
		}
		(void)ptr; // silence cppcheck warnings
		return fs_mr_snapshot(ts,inode,parent,strlen((char*)name),name,smode,sesflags,uid,gids,gidtab,umask,inodecheck,removed,same,existing,hardlinks,new);
	} else {
		GETU32(umask,ptr);
		EAT(ptr,filename,lv,')');
		(void)ptr; // silence cppcheck warnings
		return fs_mr_snapshot(ts,inode,parent,strlen((char*)name),name,smode,sesflags,uid,1,&gids,umask,0,0,0,0,0,0);
	}
}

int do_symlink(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t parent,uid,gid,inode;
	uint8_t name[256];
	static uint8_t *path = NULL;
	static uint32_t pathsize = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETPATH(path,pathsize,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(gid,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(inode,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_symlink(ts,parent,strlen((char*)name),name,path,uid,gid,inode);
}

int do_scecon(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	(void)ts;
	EAT(ptr,filename,lv,'(');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_ec_version(1);
}

int do_scecversion(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t ec_new_version;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETU8(ec_new_version,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_ec_version(ec_new_version);
}

int do_scdel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t name[256];
	uint16_t spid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETNAME(name,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU16(spid,ptr);
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_delete_entry(strlen((char*)name),name,spid);
}

int do_scdup(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t sname[256];
	uint8_t dname[256];
	uint16_t sspid,dspid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETNAME(sname,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETNAME(dname,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU16(sspid,ptr);
	EAT(ptr,filename,lv,',');
	GETU16(dspid,ptr);
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_duplicate_entry(strlen((char*)sname),sname,strlen((char*)dname),dname,sspid,dspid);
}

int do_scren(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t sname[256];
	uint8_t dname[256];
	uint16_t spid;
	(void)ts;
	EAT(ptr,filename,lv,'(');
	GETNAME(sname,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETNAME(dname,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU16(spid,ptr);
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_rename_entry(strlen((char*)sname),sname,strlen((char*)dname),dname,spid);
}

int do_scset(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint8_t name[256];
	uint8_t desc[256];
	uint16_t spid;
	uint16_t arch_delay,min_trashretention;
	uint64_t arch_min_size;
	uint8_t version;
	uint8_t new_flag,export_group,adminonly,labels_mode,arch_mode,i;
	uint32_t priority;
	storagemode create,keep,arch,trash;
//	uint8_t create_labelscnt,keep_labelscnt,arch_labelscnt;
	uint32_t old_labelmasks[MAXLABELSCNT*MASKORGROUP];
	uint32_t labelexpr_leng;
	static uint8_t *labelexpr_buff = NULL;
	static uint32_t labelexpr_size = 0;

	(void)ts;
	version = 0;
	memset(&create,0,sizeof(storagemode));
	memset(&keep,0,sizeof(storagemode));
	memset(&arch,0,sizeof(storagemode));
	memset(&trash,0,sizeof(storagemode));
	create.labels_mode = LABELS_MODE_GLOBAL;
	keep.labels_mode = LABELS_MODE_GLOBAL;
	arch.labels_mode = LABELS_MODE_GLOBAL;
	trash.labels_mode = LABELS_MODE_GLOBAL;
	EAT(ptr,filename,lv,'(');
	GETNAME(name,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU8(new_flag,ptr);
	EAT(ptr,filename,lv,',');
	if (*ptr=='W') { // MFS < 4.0
		EAT(ptr,filename,lv,'W');
		GETU8(create.labelscnt,ptr);
		EAT(ptr,filename,lv,',');
		EAT(ptr,filename,lv,'K');
		GETU8(keep.labelscnt,ptr);
		EAT(ptr,filename,lv,',');
		EAT(ptr,filename,lv,'A');
		GETU8(arch.labelscnt,ptr);
		EAT(ptr,filename,lv,',');
		GETU8(labels_mode,ptr);
		EAT(ptr,filename,lv,',');
		GETU16(arch_delay,ptr);
		EAT(ptr,filename,lv,',');
		GETU8(adminonly,ptr);
		if (create.labelscnt>MAXLABELSCNT || keep.labelscnt>MAXLABELSCNT || arch.labelscnt>MAXLABELSCNT) {
			return MFS_ERROR_EINVAL;
		}
		if (create.labelscnt+keep.labelscnt+arch.labelscnt==0) {
			EAT(ptr,filename,lv,',');
			EAT(ptr,filename,lv,'-');
		} else {
			for (i=0 ; i<create.labelscnt*MASKORGROUP ; i++) {
				EAT(ptr,filename,lv,',');
				GETU32(old_labelmasks[i],ptr);
			}
			sclass_maskorgroup_to_labelexpr(create.labelexpr,old_labelmasks,create.labelscnt);
			for (i=0 ; i<keep.labelscnt*MASKORGROUP ; i++) {
				EAT(ptr,filename,lv,',');
				GETU32(old_labelmasks[i],ptr);
			}
			sclass_maskorgroup_to_labelexpr(keep.labelexpr,old_labelmasks,keep.labelscnt);
			for (i=0 ; i<arch.labelscnt*MASKORGROUP ; i++) {
				EAT(ptr,filename,lv,',');
				GETU32(old_labelmasks[i],ptr);
			}
			sclass_maskorgroup_to_labelexpr(arch.labelexpr,old_labelmasks,arch.labelscnt);
			arch_delay *= 24;
		}
		desc[0] = 0;
		priority = 0;
		export_group = 0;
		min_trashretention = 0;
		arch_min_size = 0;
		arch_mode = SCLASS_ARCH_MODE_CTIME;
	} else if (*ptr=='C' || *ptr=='1') { // MFS >= 4.0
		if (*ptr=='1') { // MFS >= 4.57.0
			EAT(ptr,filename,lv,'1');
			EAT(ptr,filename,lv,',');
			GETNAME(desc,ptr,filename,lv,',');
			EAT(ptr,filename,lv,',');
			GETU32(priority,ptr);
			EAT(ptr,filename,lv,',');
			GETU8(export_group,ptr);
			EAT(ptr,filename,lv,',');
			version = 1;
		} else {
			desc[0] = 0;
			priority = 0;
			export_group = 0;
		}
		EAT(ptr,filename,lv,'C');
		EAT(ptr,filename,lv,'(');
		GETU8(create.labelscnt,ptr);
		EAT(ptr,filename,lv,':');
		GETU32(create.uniqmask,ptr);
		if (*ptr==':') {
			EAT(ptr,filename,lv,':');
			GETU8(create.labels_mode,ptr);
		}
		EAT(ptr,filename,lv,')');
		EAT(ptr,filename,lv,',');
		EAT(ptr,filename,lv,'K');
		EAT(ptr,filename,lv,'(');
		GETU8(keep.labelscnt,ptr);
		EAT(ptr,filename,lv,':');
		GETU32(keep.uniqmask,ptr);
		if (*ptr==':') {
			EAT(ptr,filename,lv,':');
			GETU8(keep.labels_mode,ptr);
		}
		EAT(ptr,filename,lv,')');
		EAT(ptr,filename,lv,',');
		EAT(ptr,filename,lv,'A');
		EAT(ptr,filename,lv,'(');
		GETU8(arch.labelscnt,ptr);
		EAT(ptr,filename,lv,':');
		GETU32(arch.uniqmask,ptr);
		EAT(ptr,filename,lv,':');
		GETU8(arch.ec_data_chksum_parts,ptr);
		if (*ptr==':') {
			EAT(ptr,filename,lv,':');
			GETU8(arch.labels_mode,ptr);
		}
		EAT(ptr,filename,lv,')');
		EAT(ptr,filename,lv,',');
		EAT(ptr,filename,lv,'T');
		EAT(ptr,filename,lv,'(');
		GETU8(trash.labelscnt,ptr);
		EAT(ptr,filename,lv,':');
		GETU32(trash.uniqmask,ptr);
		EAT(ptr,filename,lv,':');
		GETU8(trash.ec_data_chksum_parts,ptr);
		if (*ptr==':') {
			EAT(ptr,filename,lv,':');
			GETU8(trash.labels_mode,ptr);
		}
		EAT(ptr,filename,lv,')');
		EAT(ptr,filename,lv,',');
		GETU8(labels_mode,ptr);
		EAT(ptr,filename,lv,',');
		if (*ptr=='(') {
			EAT(ptr,filename,lv,'(');
			GETU8(arch_mode,ptr);
			EAT(ptr,filename,lv,':');
			GETU16(arch_delay,ptr);
			if (*ptr==':') {
				EAT(ptr,filename,lv,':');
				GETU64(arch_min_size,ptr);
			} else {
				arch_min_size = 0;
			}
			EAT(ptr,filename,lv,')');
		} else {
			GETU16(arch_delay,ptr);
			arch_mode = SCLASS_ARCH_MODE_CTIME;
			arch_min_size = 0;
		}
		EAT(ptr,filename,lv,',');
		GETU16(min_trashretention,ptr);
		EAT(ptr,filename,lv,',');
		GETU8(adminonly,ptr);
		if (create.labelscnt>MAXLABELSCNT || keep.labelscnt>MAXLABELSCNT || arch.labelscnt>MAXLABELSCNT || trash.labelscnt>MAXLABELSCNT) {
			return MFS_ERROR_EINVAL;
		}
		if (create.labelscnt+keep.labelscnt+arch.labelscnt+trash.labelscnt==0) {
			EAT(ptr,filename,lv,',');
			EAT(ptr,filename,lv,'-');
		} else {
			for (i=0 ; i<create.labelscnt ; i++) {
				EAT(ptr,filename,lv,',');
				GETHEX(labelexpr_buff,labelexpr_leng,labelexpr_size,ptr,filename,lv);
				if (labelexpr_leng>SCLASS_EXPR_MAX_SIZE) {
					return MFS_ERROR_EINVAL;
				}
				memcpy(create.labelexpr[i],labelexpr_buff,labelexpr_leng);
			}
			for (i=0 ; i<keep.labelscnt ; i++) {
				EAT(ptr,filename,lv,',');
				GETHEX(labelexpr_buff,labelexpr_leng,labelexpr_size,ptr,filename,lv);
				if (labelexpr_leng>SCLASS_EXPR_MAX_SIZE) {
					return MFS_ERROR_EINVAL;
				}
				memcpy(keep.labelexpr[i],labelexpr_buff,labelexpr_leng);
			}
			for (i=0 ; i<arch.labelscnt ; i++) {
				EAT(ptr,filename,lv,',');
				GETHEX(labelexpr_buff,labelexpr_leng,labelexpr_size,ptr,filename,lv);
				if (labelexpr_leng>SCLASS_EXPR_MAX_SIZE) {
					return MFS_ERROR_EINVAL;
				}
				memcpy(arch.labelexpr[i],labelexpr_buff,labelexpr_leng);
			}
			for (i=0 ; i<trash.labelscnt ; i++) {
				EAT(ptr,filename,lv,',');
				GETHEX(labelexpr_buff,labelexpr_leng,labelexpr_size,ptr,filename,lv);
				if (labelexpr_leng>SCLASS_EXPR_MAX_SIZE) {
					return MFS_ERROR_EINVAL;
				}
				memcpy(trash.labelexpr[i],labelexpr_buff,labelexpr_leng);
			}
		}
	} else {
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"wrong storage class format ('%c' found - 'W' or 'C' expected)",*ptr);
		return -1;
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU16(spid,ptr);
	if (version==0) {
		if (spid<10) {
			export_group = spid;
		}
	}
	(void)ptr; // silence cppcheck warnings
	return sclass_mr_set_entry(strlen((char*)name),name,spid,new_flag,strlen((char*)desc),desc,priority,export_group,adminonly,labels_mode,arch_mode,arch_delay,arch_min_size,min_trashretention,&create,&keep,&arch,&trash);
}

int do_trash_recover(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent;
	uint16_t cumask;
	uint32_t uid,gid;
	uint8_t copysgid;
	static uint8_t *path = NULL;
	static uint32_t pathsize = 0;
	static uint8_t *created_path = NULL;
	static uint32_t created_pathsize = 0;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETPATH(path,pathsize,ptr,filename,lv,',');
	EAT(ptr,filename,lv,',');
	GETU16(cumask,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(uid,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(gid,ptr);
	EAT(ptr,filename,lv,',');
	GETU8(copysgid,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	EAT(ptr,filename,lv,'(');
	GETPATH(created_path,created_pathsize,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_trash_recover(ts,inode,parent,strlen((char*)path),path,cumask,uid,gid,copysgid,strlen((char*)created_path),created_path);
}

int do_trash_remove(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_trash_remove(ts,inode);
}

int do_undel(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_undel(ts,inode);
}

int do_unlink(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,parent;
	uint8_t name[256];
	EAT(ptr,filename,lv,'(');
	GETU32(parent,ptr);
	EAT(ptr,filename,lv,',');
	GETNAME(name,ptr,filename,lv,')');
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU32(inode,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_unlink(ts,parent,strlen((char*)name),name,inode);
}

int do_unlock(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint64_t chunkid;
	EAT(ptr,filename,lv,'(');
	GETU64(chunkid,ptr);
	EAT(ptr,filename,lv,')');
	(void)ptr; // silence cppcheck warnings
	return fs_mr_unlock(ts,chunkid);
}

int do_trunc(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx;
	uint64_t chunkid;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU64(chunkid,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_trunc(ts,inode,indx,chunkid);
}

int do_write(const char *filename,uint64_t lv,uint32_t ts,const char *ptr) {
	uint32_t inode,indx,opflag;
	uint64_t chunkid;
	uint8_t canmodmtime;
	EAT(ptr,filename,lv,'(');
	GETU32(inode,ptr);
	EAT(ptr,filename,lv,',');
	GETU32(indx,ptr);
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU32(opflag,ptr);
	} else {
		opflag=1;
	}
	if (*ptr==',') {
		EAT(ptr,filename,lv,',');
		GETU8(canmodmtime,ptr);
	} else {
		canmodmtime=1;
	}
	EAT(ptr,filename,lv,')');
	EAT(ptr,filename,lv,':');
	GETU64(chunkid,ptr);
	(void)ptr; // silence cppcheck warnings
	return fs_mr_write(ts,inode,indx,opflag,canmodmtime,chunkid);
}

#define HASHCODESTR(str) (((((uint8_t*)(str))[0]*256U+((uint8_t*)(str))[1])*256U+((uint8_t*)(str))[2])*256U+((uint8_t*)(str))[3])
#define HASHCODE(a,b,c,d) (((((uint8_t)a)*256U+(uint8_t)b)*256U+(uint8_t)c)*256U+(uint8_t)d)

int restore_line(const char *filename,uint64_t lv,const char *line,uint32_t *rts) {
	const char *ptr;
	uint32_t ts;
	uint32_t hc;
	int status;
//	char* errormsgs[]={ MFS_ERROR_STRINGS };

	status = MFS_ERROR_MISMATCH;
	ptr = line;

//	EAT(ptr,filename,lv,':');
//	EAT(ptr,filename,lv,' ');
	GETU32(ts,ptr);
	if (rts!=NULL) {
		*rts = ts;
	}
	EAT(ptr,filename,lv,'|');
	hc = HASHCODESTR(ptr);
	switch (hc) {
		case HASHCODE('I','D','L','E'):
			return do_idle(filename,lv,ts,ptr+4);
		case HASHCODE('A','C','C','E'):
			if (strncmp(ptr,"ACCESS",6)==0) {
				return do_access(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('A','D','D','A'):
			if (strncmp(ptr,"ADDATTR",7)==0) {
				return do_addattr(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('A','T','T','R'):
			return do_attr(filename,lv,ts,ptr+4);
		case HASHCODE('A','P','P','E'):
			if (strncmp(ptr,"APPEND",6)==0) {
				return do_append(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('A','C','Q','U'):
			if (strncmp(ptr,"ACQUIRE",7)==0) {
				return do_acquire(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('A','Q','U','I'):
			if (strncmp(ptr,"AQUIRE",6)==0) {
				return do_acquire(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('A','R','C','H'):
			if (strncmp(ptr,"ARCHCHG",7)==0) {
				return do_archchg(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('A','M','T','I'):
			if (strncmp(ptr,"AMTIME",6)==0) {
				return do_amtime(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('A','U','T','O'):
			if (strncmp(ptr,"AUTOARCH",8)==0) {
				return do_autoarch(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('C','R','E','A'):
			if (strncmp(ptr,"CREATE",6)==0) {
				return do_create(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('C','H','U','N'):
			if (strncmp(ptr,"CHUNKADD",8)==0) {
				return do_chunkadd(filename,lv,ts,ptr+8);
			} else if (strncmp(ptr,"CHUNKDEL",8)==0) {
				return do_chunkdel(filename,lv,ts,ptr+8);
			} else if (strncmp(ptr,"CHUNKFLAGSCLR",13)==0) {
				return do_chunkflagsclr(filename,lv,ts,ptr+13);
			}
			break;
		case HASHCODE('C','S','A','D'):
			if (strncmp(ptr,"CSADD",5)==0) {		// deprecated
				return do_csadd(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('C','S','D','B'):
			if (strncmp(ptr,"CSDBOP",6)==0) {
				return do_csdbop(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('C','S','D','E'):
			if (strncmp(ptr,"CSDEL",5)==0) {		// deprecated
				return do_csdel(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('C','U','S','T'):
			if (strncmp(ptr,"CUSTOMER",8)==0) {	// deprecated
				return do_session(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('E','M','P','T'):
			if (strncmp(ptr,"EMPTYTRASH",10)==0) {
				return do_emptytrash(filename,lv,ts,ptr+10);
			} else if (strncmp(ptr,"EMPTYSUSTAINED",14)==0) {
				return do_emptysustained(filename,lv,ts,ptr+14);
			} else if (strncmp(ptr,"EMPTYRESERVED",13)==0) {
				return do_emptysustained(filename,lv,ts,ptr+13);
			}
			break;
		case HASHCODE('F','L','O','C'):
			if (strncmp(ptr,"FLOCK",5)==0) {
				return do_flock(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('F','R','E','E'):
			if (strncmp(ptr,"FREEINODES",10)==0) {
				return do_freeinodes(filename,lv,ts,ptr+10);
			}
			break;
		case HASHCODE('I','N','C','V'):
			if (strncmp(ptr,"INCVERSION",10)==0) {
				return do_incversion(filename,lv,ts,ptr+10); // deprecated -> SETVERSION
			}
			break;
		case HASHCODE('L','E','N','G'):
			if (strncmp(ptr,"LENGTH",6)==0) {
				return do_length(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('L','I','N','K'):
			return do_link(filename,lv,ts,ptr+4);
		case HASHCODE('M','O','V','E'):
			return do_move(filename,lv,ts,ptr+4);
		case HASHCODE('N','E','X','T'):
			if (strncmp(ptr,"NEXTCHUNKID",11)==0) {
				return do_nextchunkid(filename,lv,ts,ptr+11); // deprecated
			}
			break;
		case HASHCODE('P','A','T','A'):
			if (strncmp(ptr,"PATADD",6)==0) {
				return do_patadd(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('P','A','T','D'):
			if (strncmp(ptr,"PATDEL",6)==0) {
				return do_patdel(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('P','O','S','I'):
			if (strncmp(ptr,"POSIXLOCK",9)==0) {
				return do_posixlock(filename,lv,ts,ptr+9);
			}
			break;
		case HASHCODE('P','U','R','G'):
			if (strncmp(ptr,"PURGE",5)==0) {
				return do_purge(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('Q','U','O','T'):
			if (strncmp(ptr,"QUOTA",5)==0) {
				return do_quota(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('R','E','L','E'):
			if (strncmp(ptr,"RELEASE",7)==0) {
				return do_release(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('R','E','N','U'):
			if (strncmp(ptr,"RENUMERATEEDGES",15)==0) {
				return do_renumedges(filename,lv,ts,ptr+15);
			}
			break;
		case HASHCODE('R','E','P','A'):
			if (strncmp(ptr,"REPAIR",6)==0) {
				return do_repair(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('R','O','L','L'):
			if (strncmp(ptr,"ROLLBACK",8)==0) {
				return do_rollback(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('S','C','D','E'):
			if (strncmp(ptr,"SCDEL",5)==0) {
				return do_scdel(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('S','C','D','U'):
			if (strncmp(ptr,"SCDUP",5)==0) {
				return do_scdup(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('S','C','E','C'):
			if (strncmp(ptr,"SCECVERSION",11)==0) {
				return do_scecversion(filename,lv,ts,ptr+11);
			} else if (strncmp(ptr,"SCECON",6)==0) { // deprecated
				return do_scecon(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('S','C','R','A'):
			if (strncmp(ptr,"SCRAIDON",8)==0) { // deprecated
				return do_scecon(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('S','C','R','E'):
			if (strncmp(ptr,"SCREN",5)==0) {
				return do_scren(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('S','C','S','E'):
			if (strncmp(ptr,"SCSET",5)==0) {
				return do_scset(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('S','E','S','A'):
			if (strncmp(ptr,"SESADD",6)==0) {
				return do_sesadd(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('S','E','S','C'):
			if (strncmp(ptr,"SESCHANGED",10)==0) {
				return do_seschanged(filename,lv,ts,ptr+10);
			} else if (strncmp(ptr,"SESCONNECTED",12)==0) {
				return do_sesconnected(filename,lv,ts,ptr+12);
			}
			break;
		case HASHCODE('S','E','S','D'):
			if (strncmp(ptr,"SESDEL",6)==0) {
				return do_sesdel(filename,lv,ts,ptr+6);
			} else if (strncmp(ptr,"SESDISCONNECTED",15)==0) {
				return do_sesdisconnected(filename,lv,ts,ptr+15);
			}
			break;
		case HASHCODE('S','E','S','S'):
			if (strncmp(ptr,"SESSION",7)==0) { // deprecated
				return do_session(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('S','E','T','A'):
			if (strncmp(ptr,"SETACL",6)==0) {
				return do_setacl(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('S','E','T','E'):
			if (strncmp(ptr,"SETEATTR",8)==0) {
				return do_seteattr(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('S','E','T','F'):
			if (strncmp(ptr,"SETFILECHUNK",12)==0) {
				return do_setfilechunk(filename,lv,ts,ptr+12);
			}
			break;
		case HASHCODE('S','E','T','G'):
			if (strncmp(ptr,"SETGOAL",7)==0) {
				return do_setgoal(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('S','E','T','M'):
			if (strncmp(ptr,"SETMETAID",9)==0) {
				return do_setmetaid(filename,lv,ts,ptr+9);
			}
			break;
		case HASHCODE('S','E','T','P'):
			if (strncmp(ptr,"SETPATH",7)==0) {
				return do_setpath(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('S','E','T','S'):
			if (strncmp(ptr,"SETSCLASS",9)==0) {
				return do_setsclass(filename,lv,ts,ptr+9);
			}
			break;
		case HASHCODE('S','E','T','T'):
			if (strncmp(ptr,"SETTRASHTIME",12)==0) {
				return do_settrashretention(filename,lv,ts,ptr+12);
			}
			break;
		case HASHCODE('S','E','T','V'):
			if (strncmp(ptr,"SETVERSION",10)==0) {
				return do_setversion(filename,lv,ts,ptr+10);
			}
			break;
		case HASHCODE('S','E','T','X'):
			if (strncmp(ptr,"SETXATTR",8)==0) {
				return do_setxattr(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('S','N','A','P'):
			if (strncmp(ptr,"SNAPSHOT",8)==0) {
				return do_snapshot(filename,lv,ts,ptr+8);
			}
			break;
		case HASHCODE('S','Y','M','L'):
			if (strncmp(ptr,"SYMLINK",7)==0) {
				return do_symlink(filename,lv,ts,ptr+7);
			}
			break;
		case HASHCODE('T','R','A','S'):
			if (strncmp(ptr,"TRASH_RECOVER",13)==0) {
				return do_trash_recover(filename,lv,ts,ptr+13);
			} else if (strncmp(ptr,"TRASH_REMOVE",12)==0) {
				return do_trash_remove(filename,lv,ts,ptr+12);
			}
			break;
		case HASHCODE('T','R','U','N'):
			if (strncmp(ptr,"TRUNC",5)==0) {
				return do_trunc(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('U','N','D','E'):
			if (strncmp(ptr,"UNDEL",5)==0) {
				return do_undel(filename,lv,ts,ptr+5);
			}
			break;
		case HASHCODE('U','N','L','I'):
			if (strncmp(ptr,"UNLINK",6)==0) {
				return do_unlink(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('U','N','L','O'):
			if (strncmp(ptr,"UNLOCK",6)==0) {
				return do_unlock(filename,lv,ts,ptr+6);
			}
			break;
		case HASHCODE('W','R','I','T'):
			if (strncmp(ptr,"WRITE",5)==0) {
				return do_write(filename,lv,ts,ptr+5);
			}
			break;
	}
	mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": unknown entry '%s'\n",filename,lv,ptr);
	return status;
}

int restore_net(uint64_t lv,const char *ptr,uint32_t *rts) {
	int status;
	if (lv!=meta_version()) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"desync - invalid meta version (version in packet: %"PRIu64" / expected: %"PRIu64" / packet data: %s)",lv,meta_version(),ptr);
		return -1;
	}
	status = restore_line("NET",lv,ptr,rts);
	if (status<0) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"desync - operation (%s) parse error",ptr);
		return -1;
	}
	if (status!=MFS_STATUS_OK) {
		mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"desync - operation (%s) error: %d (%s)",ptr,status,mfsstrerr(status));
		return -1;
	}
	if (lv+1!=meta_version()) {
		if (lv+1>meta_version()) {
			mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"desync - meta version has not been increased after the operation (%s)",ptr);
		} else {
			mfs_log(MFSLOG_SYSLOG,MFSLOG_WARNING,"desync - meta version has been increased more then once after the operation (%s)",ptr);
		}
		return -1;
	}
	return 0;
}

static uint64_t v=0,lastv=0;
static void *lastshfn = NULL;

int restore_file(void *shfilename,uint64_t lv,const char *ptr,uint8_t vlevel) {
	int status;
	char *lastfn;
	char *filename = (char*)shp_get(shfilename);
	if (lastv==0 || v==0 || lastshfn==NULL) {
		v = meta_version();
		lastv = lv-1;
		lastfn = "(no file)";
	} else {
		lastfn = (char*)shp_get(lastshfn);
	}
	if (vlevel>1) {
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_INFO,"filename: %s ; current meta version: %"PRIu64" ; previous changeid: %"PRIu64" ; current changeid: %"PRIu64" ; change data%s",filename,v,lastv,lv,ptr);
	}
	if (lv<lastv) {
		mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"merge error - possibly corrupted input file - ignore entry (filename: %s)\n",filename);
		return 0;
	} else if (lv>=v) {
		if (lv==lastv) {
			if (vlevel>1) {
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_INFO,"duplicated entry: %"PRIu64" (previous file: %s, current file: %s)\n",lv,lastfn,filename);
			}
		} else if (lv>lastv+1) {
			mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"hole in change files (entries from %s:%"PRIu64" to %s:%"PRIu64" are missing) - add more files\n",lastfn,lastv+1,filename,lv-1);
			return -2;
		} else {
			if (vlevel>0) {
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_INFO,"%s: change%s",filename,ptr);
			}
			status = restore_line(filename,lv,ptr,NULL);
			if (status<0) { // parse error - just ignore this line
				return 0;
			}
			if (status>0) { // other errors - stop processing data
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": operation (%s) error: %d (%s)",filename,lv,ptr,status,mfsstrerr(status));
				return -1;
			}
			v = meta_version();
			if (lv+1!=v) {
				mfs_log(MFSLOG_SYSLOG_STDERR,MFSLOG_WARNING,"%s:%"PRIu64": version mismatch\n",filename,lv);
				return -1;
			}
		}
	}
	lastv = lv;
	if (shfilename!=lastshfn) {
		shp_inc(shfilename);
		if (lastshfn!=NULL) {
			shp_dec(lastshfn);
		}
		lastshfn = shfilename;
	}
	return 0;
}
