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

#ifndef _HA_MANAGER_H_
#define _HA_MANAGER_H_

#include <inttypes.h>
#include "replication.h"

// HA Manager node role
typedef enum {
    HA_ROLE_UNKNOWN = 0,
    HA_ROLE_FOLLOWER = 1,
    HA_ROLE_CANDIDATE = 2,
    HA_ROLE_LEADER = 3,
    HA_ROLE_OBSERVER = 4   // Non-voting observer
} ha_role_t;

// HA Manager operating mode
typedef enum {
    HA_MODE_ACTIVE_STANDBY = 0,  // Active-standby mode with strong consistency
    HA_MODE_ACTIVE_ACTIVE = 1,   // Active-active mode with eventual consistency
    HA_MODE_READ_ONLY = 2,       // Read-only mode for special operations
} ha_mode_t;

// HA Manager configuration
typedef struct _ha_config {
    uint32_t node_id;
    char *data_dir;
    uint16_t raft_port;
    uint16_t repl_port;
    uint32_t election_timeout_min_ms;
    uint32_t election_timeout_max_ms;
    uint32_t heartbeat_interval_ms;
    uint32_t sync_timeout_ms;
    ha_mode_t default_mode;
} ha_config_t;

// HA Node information
typedef struct _ha_node_info {
    uint32_t node_id;
    char *hostname;
    uint16_t raft_port;
    uint16_t repl_port;
    ha_role_t role;
    uint64_t replication_lag;
    struct _ha_node_info *next;
} ha_node_info_t;

// Initialize the HA Manager
int ha_init(ha_config_t *config);

// Add a node to the HA cluster
int ha_add_node(uint32_t node_id, const char *hostname, uint16_t raft_port, uint16_t repl_port);

// Remove a node from the HA cluster
int ha_remove_node(uint32_t node_id);

// Start the HA subsystem
int ha_start(void);

// Stop the HA subsystem
int ha_stop(void);

// Get current node role
ha_role_t ha_get_role(void);

// Get current HA mode
ha_mode_t ha_get_mode(void);

// Set HA mode
int ha_set_mode(ha_mode_t mode);

// Get the leader node ID
uint32_t ha_get_leader_id(void);

// Check if this node is the leader
int ha_is_leader(void);

// Get node information list
ha_node_info_t *ha_get_nodes(void);

// Free node information list
void ha_free_nodes(ha_node_info_t *nodes);

// Register for HA events
typedef void (*ha_event_callback_t)(ha_role_t new_role, uint32_t leader_id);
void ha_register_event_callback(ha_event_callback_t callback);

// Submit an operation to be replicated (returns immediately)
int ha_submit_operation(uint8_t *data, uint32_t data_size, consistency_level_t consistency);

// Clean up resources used by the HA Manager
void ha_term(void);

#endif /* _HA_MANAGER_H_ */ 