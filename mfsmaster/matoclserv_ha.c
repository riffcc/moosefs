/*
 * Copyright (C) 2025 MooseFS
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

#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <inttypes.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <ifaddrs.h>
#include <ctype.h>

#include "matoclserv.h"
#include "mfslog.h"
#include "cfg.h"

/* High Availability support */

// Track if this node is a leader or follower
static uint8_t is_ha_leader = 1; // Default to leader if HA is not enabled

// List of discovered master nodes
#define MAX_HA_NODES 16
static char *ha_node_addresses[MAX_HA_NODES];
static uint32_t ha_node_count = 0;
static uint32_t ha_local_node_id = 0;
static uint16_t ha_default_port = 9426; // Default HA port

// Parse a host:port string, returning the host part and setting port if specified
static char* parse_host_port(const char *hostport, uint16_t *port) {
    char *result, *colon;
    size_t len;
    
    // Find the last colon (to handle IPv6 addresses correctly)
    colon = strrchr(hostport, ':');
    
    if (colon && isdigit(*(colon+1))) {
        // Port specified
        len = colon - hostport;
        *port = atoi(colon + 1);
    } else {
        // No port, use default
        len = strlen(hostport);
    }
    
    result = malloc(len + 1);
    if (result) {
        memcpy(result, hostport, len);
        result[len] = '\0';
    }
    
    return result;
}

// Parse MASTER_HOST string to get list of masters or DNS name
static void parse_master_host(void) {
    char *master_host, *host_copy, *saveptr, *token, *hostname;
    uint16_t port;
    
    // Clean up any existing addresses
    for (uint32_t i = 0; i < ha_node_count; i++) {
        if (ha_node_addresses[i]) {
            free(ha_node_addresses[i]);
            ha_node_addresses[i] = NULL;
        }
    }
    ha_node_count = 0;
    
    master_host = cfg_getstr("MASTER_HOST", "mfsmaster");
    if (!master_host) {
        mfs_log(0, MFSLOG_NOTICE, "MASTER_HOST not configured, using default");
        master_host = strdup("mfsmaster");
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Using MASTER_HOST: %s", master_host);
    
    // Make a copy for strtok_r which modifies the string
    host_copy = strdup(master_host);
    if (!host_copy) {
        free(master_host);
        return;
    }
    
    // Check if it's a comma-separated list
    if (strchr(host_copy, ',')) {
        // Parse as comma-separated list of hosts
        token = strtok_r(host_copy, ",", &saveptr);
        while (token && ha_node_count < MAX_HA_NODES) {
            port = ha_default_port;
            hostname = parse_host_port(token, &port);
            
            if (hostname) {
                size_t len = strlen(hostname) + 10; // Space for :port
                ha_node_addresses[ha_node_count] = malloc(len);
                if (ha_node_addresses[ha_node_count]) {
                    snprintf(ha_node_addresses[ha_node_count], len, "%s:%u", hostname, port);
                    ha_node_count++;
                }
                free(hostname);
            }
            
            token = strtok_r(NULL, ",", &saveptr);
        }
        
        mfs_log(0, MFSLOG_NOTICE, "Parsed %u master nodes from list", ha_node_count);
    } else {
        // Single hostname or IP - could be a DNS name with multiple addresses
        port = ha_default_port;
        hostname = parse_host_port(host_copy, &port);
        
        // Will call discover_ha_nodes with this hostname later
        if (hostname) {
            // Store initial hostname for DNS resolution
            size_t len = strlen(hostname) + 10;
            ha_node_addresses[0] = malloc(len);
            if (ha_node_addresses[0]) {
                snprintf(ha_node_addresses[0], len, "%s:%u", hostname, port);
                ha_node_count = 1;
            }
            free(hostname);
        }
    }
    
    free(host_copy);
    free(master_host);
}

// Discovers HA master nodes using DNS and identifies local node
static void discover_ha_nodes(void) {
    struct addrinfo hints, *res, *p;
    struct ifaddrs *ifaddr, *ifa;
    char host[NI_MAXHOST];
    int status;
    char port_str[8];
    uint32_t node_id = 1;
    char *hostname, *portstr;
    uint16_t port;
    char **temp_addresses = NULL;
    uint32_t temp_count = 0;
    
    // If we have multiple explicit addresses already, no need for DNS
    if (ha_node_count > 1) {
        goto identify_local_node;
    }
    
    // We have either 0 or 1 addresses
    if (ha_node_count == 0) {
        mfs_log(0, MFSLOG_NOTICE, "No master hosts configured");
        return;
    }
    
    // Extract hostname and port from the first node
    hostname = strdup(ha_node_addresses[0]);
    portstr = strchr(hostname, ':');
    if (portstr) {
        *portstr = '\0';
        portstr++;
        port = atoi(portstr);
    } else {
        port = ha_default_port;
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Discovering HA nodes via DNS: %s:%u", hostname, port);
    
    // Allocate temporary space for discovered addresses
    temp_addresses = malloc(sizeof(char*) * MAX_HA_NODES);
    if (!temp_addresses) {
        free(hostname);
        return;
    }
    memset(temp_addresses, 0, sizeof(char*) * MAX_HA_NODES);
    
    // Set up lookup hints
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;    // Allow IPv4 or IPv6
    hints.ai_socktype = SOCK_STREAM;
    
    // Convert port to string for getaddrinfo
    snprintf(port_str, sizeof(port_str), "%d", port);
    
    // Lookup DNS name
    status = getaddrinfo(hostname, port_str, &hints, &res);
    if (status != 0) {
        mfs_log(0, MFSLOG_ERR, "DNS lookup failed: %s", gai_strerror(status));
        free(hostname);
        free(temp_addresses);
        return;
    }
    
    free(hostname);
    
    // Process each address returned
    for (p = res; p != NULL && temp_count < MAX_HA_NODES; p = p->ai_next) {
        char ip_str[INET6_ADDRSTRLEN];
        void *addr;
        
        // Get IP address string
        if (p->ai_family == AF_INET) {
            struct sockaddr_in *ipv4 = (struct sockaddr_in *)p->ai_addr;
            addr = &(ipv4->sin_addr);
        } else {
            struct sockaddr_in6 *ipv6 = (struct sockaddr_in6 *)p->ai_addr;
            addr = &(ipv6->sin6_addr);
        }
        
        inet_ntop(p->ai_family, addr, ip_str, sizeof(ip_str));
        
        // Create full address with port
        size_t len = strlen(ip_str) + 10;
        temp_addresses[temp_count] = malloc(len);
        if (temp_addresses[temp_count]) {
            snprintf(temp_addresses[temp_count], len, "%s:%u", ip_str, port);
            temp_count++;
        }
    }
    
    freeaddrinfo(res);
    
    // If we found multiple addresses, replace the original list
    if (temp_count > 1) {
        // Free original addresses
        for (uint32_t i = 0; i < ha_node_count; i++) {
            if (ha_node_addresses[i]) {
                free(ha_node_addresses[i]);
                ha_node_addresses[i] = NULL;
            }
        }
        
        // Copy temp addresses to main list
        for (uint32_t i = 0; i < temp_count; i++) {
            ha_node_addresses[i] = temp_addresses[i];
        }
        
        ha_node_count = temp_count;
        free(temp_addresses); // Just free the array
    } else {
        // Free temp addresses
        for (uint32_t i = 0; i < temp_count; i++) {
            free(temp_addresses[i]);
        }
        free(temp_addresses);
    }
    
    if (ha_node_count == 0) {
        mfs_log(0, MFSLOG_NOTICE, "No master nodes found");
        return;
    }
    
    // Log discovered nodes
    for (uint32_t i = 0; i < ha_node_count; i++) {
        mfs_log(0, MFSLOG_NOTICE, "Discovered HA node %u: %s", i+1, ha_node_addresses[i]);
    }
    
identify_local_node:
    // Get list of local addresses to identify which node is us
    if (getifaddrs(&ifaddr) == -1) {
        mfs_log(0, MFSLOG_ERR, "Failed to get local interfaces for HA node identification");
        return;
    }
    
    // Try to identify our node
    for (uint32_t i = 0; i < ha_node_count; i++) {
        char *ip_str = strdup(ha_node_addresses[i]);
        char *colon = strchr(ip_str, ':');
        if (colon) {
            *colon = '\0';
        }
        
        // Check if this is our address
        for (ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
            if (ifa->ifa_addr == NULL)
                continue;
                
            if (ifa->ifa_addr->sa_family == AF_INET) {
                struct sockaddr_in *ipv4 = (struct sockaddr_in *)ifa->ifa_addr;
                char local_ip[INET_ADDRSTRLEN];
                inet_ntop(AF_INET, &(ipv4->sin_addr), local_ip, sizeof(local_ip));
                
                if (strcmp(local_ip, ip_str) == 0) {
                    ha_local_node_id = i + 1;
                    mfs_log(0, MFSLOG_NOTICE, "Local node identified as HA node %d", ha_local_node_id);
                    free(ip_str);
                    freeifaddrs(ifaddr);
                    return;
                }
            } else if (ifa->ifa_addr->sa_family == AF_INET6) {
                struct sockaddr_in6 *ipv6 = (struct sockaddr_in6 *)ifa->ifa_addr;
                char local_ip[INET6_ADDRSTRLEN];
                inet_ntop(AF_INET6, &(ipv6->sin6_addr), local_ip, sizeof(local_ip));
                
                if (strcmp(local_ip, ip_str) == 0) {
                    ha_local_node_id = i + 1;
                    mfs_log(0, MFSLOG_NOTICE, "Local node identified as HA node %d", ha_local_node_id);
                    free(ip_str);
                    freeifaddrs(ifaddr);
                    return;
                }
            }
        }
        
        free(ip_str);
    }
    
    freeifaddrs(ifaddr);
    
    // If we couldn't identify ourselves, use hostname resolution as fallback
    if (ha_local_node_id == 0) {
        char hostname[256];
        if (gethostname(hostname, sizeof(hostname)) == 0) {
            memset(&hints, 0, sizeof(hints));
            hints.ai_family = AF_UNSPEC;
            hints.ai_socktype = SOCK_STREAM;
            
            if (getaddrinfo(hostname, NULL, &hints, &res) == 0) {
                for (p = res; p != NULL; p = p->ai_next) {
                    char host_ip[INET6_ADDRSTRLEN];
                    
                    if (p->ai_family == AF_INET) {
                        struct sockaddr_in *ipv4 = (struct sockaddr_in *)p->ai_addr;
                        inet_ntop(AF_INET, &(ipv4->sin_addr), host_ip, sizeof(host_ip));
                    } else if (p->ai_family == AF_INET6) {
                        struct sockaddr_in6 *ipv6 = (struct sockaddr_in6 *)p->ai_addr;
                        inet_ntop(AF_INET6, &(ipv6->sin6_addr), host_ip, sizeof(host_ip));
                    } else {
                        continue;
                    }
                    
                    for (uint32_t i = 0; i < ha_node_count; i++) {
                        char *node_ip = strdup(ha_node_addresses[i]);
                        char *colon = strchr(node_ip, ':');
                        if (colon) {
                            *colon = '\0';
                        }
                        
                        if (strcmp(node_ip, host_ip) == 0) {
                            ha_local_node_id = i + 1;
                            mfs_log(0, MFSLOG_NOTICE, "Local node identified as HA node %d via hostname", ha_local_node_id);
                            free(node_ip);
                            break;
                        }
                        
                        free(node_ip);
                    }
                    
                    if (ha_local_node_id > 0) {
                        break;
                    }
                }
                
                freeaddrinfo(res);
            }
        }
    }
    
    mfs_log(0, MFSLOG_NOTICE, "HA cluster configured with %d nodes, local node ID: %d", 
           ha_node_count, ha_local_node_id);
}

// Called when HA role changes
void matoclserv_ha_state_changed(uint8_t is_leader) {
    is_ha_leader = is_leader;
    mfs_log(0, MFSLOG_NOTICE, "HA state changed - this node is now %s", is_leader ? "leader" : "follower");
    
    // TODO: Update all active client connections about the role change
    // This may involve sending notifications to clients or changing how
    // requests are processed
}

// Redirect clients to the new leader
void matoclserv_ha_redirect(uint32_t leader_id) {
    char *leader_ip = NULL;
    uint16_t leader_port = 0;
    
    if (leader_id == 0 || leader_id > ha_node_count) {
        mfs_log(0, MFSLOG_ERR, "HA redirect failed - invalid leader ID: %u", leader_id);
        return;
    }
    
    // Extract IP and port from node address
    char *node_addr = ha_node_addresses[leader_id - 1];
    char *colon = strchr(node_addr, ':');
    
    if (colon == NULL) {
        mfs_log(0, MFSLOG_ERR, "HA redirect failed - malformed leader address: %s", node_addr);
        return;
    }
    
    // Allocate memory for IP
    size_t ip_len = colon - node_addr;
    leader_ip = malloc(ip_len + 1);
    if (leader_ip == NULL) {
        mfs_log(0, MFSLOG_ERR, "HA redirect failed - memory allocation error");
        return;
    }
    
    // Copy IP portion
    memcpy(leader_ip, node_addr, ip_len);
    leader_ip[ip_len] = '\0';
    
    // Extract port
    leader_port = atoi(colon + 1);
    
    mfs_log(0, MFSLOG_NOTICE, "HA redirecting clients to leader (node ID: %u, address: %s:%u)",
           leader_id, leader_ip, leader_port);
    
    // TODO: Redirect each client to the new leader
    // This will need to be implemented based on the MooseFS client protocol
    
    free(leader_ip);
}

// Get IP and port for a node by ID (from HA configuration)
uint8_t matoclserv_get_node_ip_port(uint32_t node_id, char **ip_ptr, uint16_t *port_ptr) {
    if (node_id == 0 || node_id > ha_node_count) {
        return 0;
    }
    
    char *node_addr = ha_node_addresses[node_id - 1];
    char *colon = strchr(node_addr, ':');
    
    if (colon == NULL) {
        return 0;
    }
    
    // Allocate memory for IP
    size_t ip_len = colon - node_addr;
    *ip_ptr = malloc(ip_len + 1);
    if (*ip_ptr == NULL) {
        return 0;
    }
    
    // Copy IP portion
    memcpy(*ip_ptr, node_addr, ip_len);
    (*ip_ptr)[ip_len] = '\0';
    
    // Extract port
    *port_ptr = atoi(colon + 1);
    
    return 1;
}

// Initialize HA subsystem using MASTER_HOST configuration
void matoclserv_ha_init(void) {
    // Load configuration
    ha_default_port = cfg_get16("HA_PORT", 9426);
    
    // Parse MASTER_HOST to get list of masters or DNS name
    parse_master_host();
    
    // Discover nodes if needed and identify local node
    discover_ha_nodes();
    
    if (ha_node_count > 1) {
        mfs_log(0, MFSLOG_NOTICE, "HA cluster enabled with %u nodes", ha_node_count);
    } else if (ha_node_count == 1) {
        mfs_log(0, MFSLOG_NOTICE, "Only one master node found, running in single-master mode");
    } else {
        mfs_log(0, MFSLOG_NOTICE, "No master nodes configured, using defaults");
    }
} 