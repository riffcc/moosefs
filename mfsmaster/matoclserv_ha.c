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

#include "matoclserv.h"
#include "mfslog.h"
#include "cfg.h"

/* High Availability support */

// Track if this node is a leader or follower
static uint8_t is_ha_leader = 1; // Default to leader if HA is not enabled

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
    char *leader_ip;
    uint16_t leader_port;
    uint8_t status;
    
    // Get leader IP and port
    status = matoclserv_get_node_ip_port(leader_id, &leader_ip, &leader_port);
    if (status == 0) {
        mfs_log(0, MFSLOG_ERR, "HA redirect failed - could not get leader address");
        return;
    }
    
    mfs_log(0, MFSLOG_NOTICE, "HA redirecting clients to leader (node ID: %u, address: %s:%u)",
           leader_id, leader_ip, leader_port);
    
    // TODO: Redirect each client to the new leader
    // This will need to be implemented based on the MooseFS client protocol
    // For now, we just log that we would redirect
    
    free(leader_ip);
}

// Get IP and port for a node by ID (from HA configuration)
uint8_t matoclserv_get_node_ip_port(uint32_t node_id, char **ip_ptr, uint16_t *port_ptr) {
    char *config_key;
    char *config_value;
    
    // For now, use configuration to get node addresses
    // Format the config key for the node
    config_key = malloc(24);
    if (config_key == NULL) {
        return 0;
    }
    snprintf(config_key, 24, "HA_NODE_%u_ADDRESS", node_id);
    
    // Get the address from config
    config_value = cfg_getstr(config_key, NULL);
    free(config_key);
    
    if (config_value == NULL) {
        return 0;
    }
    
    // Parse IP:port
    char *colon = strchr(config_value, ':');
    if (colon == NULL) {
        free(config_value);
        return 0;
    }
    
    *colon = '\0';
    *ip_ptr = strdup(config_value);
    *port_ptr = atoi(colon + 1);
    
    free(config_value);
    return 1;
} 