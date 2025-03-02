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

#ifndef _RAFT_H_
#define _RAFT_H_

#include <inttypes.h>

// Raft node states
typedef enum {
    RAFT_STATE_FOLLOWER = 0,
    RAFT_STATE_CANDIDATE = 1,
    RAFT_STATE_LEADER = 2,
} raft_state_t;

// Raft node structure
typedef struct _raft_node {
    uint32_t node_id;
    char *hostname;
    uint16_t port;
    struct _raft_node *next;
} raft_node_t;

// Raft log entry
typedef struct _raft_log_entry {
    uint64_t term;
    uint64_t index;
    uint32_t data_size;
    uint8_t *data;
} raft_log_entry_t;

// Raft configuration
typedef struct _raft_config {
    uint32_t election_timeout_min_ms;
    uint32_t election_timeout_max_ms;
    uint32_t heartbeat_interval_ms;
    char *data_dir;
} raft_config_t;

// Raft callbacks
typedef struct _raft_callbacks {
    // Called when a node becomes a leader
    void (*on_leader_change)(uint32_t node_id);
    
    // Called to apply committed log entries to state machine
    int (*apply_log)(raft_log_entry_t *entry);
    
    // Called to serialize state machine for snapshots
    int (*serialize_state)(uint8_t **data, uint32_t *data_size);
    
    // Called to deserialize and restore state machine from snapshot
    int (*deserialize_state)(uint8_t *data, uint32_t data_size);
} raft_callbacks_t;

// Initialize the Raft subsystem
int raft_init(raft_config_t *config, raft_callbacks_t *callbacks);

// Add a node to the Raft cluster
int raft_add_node(uint32_t node_id, const char *hostname, uint16_t port);

// Remove a node from the Raft cluster
int raft_remove_node(uint32_t node_id);

// Start the Raft subsystem
int raft_start(void);

// Stop the Raft subsystem
int raft_stop(void);

// Get current Raft state
raft_state_t raft_get_state(void);

// Get current Raft term
uint64_t raft_get_current_term(void);

// Get current leader ID (0 if no leader)
uint32_t raft_get_leader_id(void);

// Check if this node is the leader
int raft_is_leader(void);

// Submit a new log entry to the Raft log (only succeeds on leader)
int raft_submit_entry(uint8_t *data, uint32_t data_size);

// Force a leadership transfer (only succeeds on leader)
int raft_transfer_leadership(uint32_t target_node_id);

// Get the latest committed log index
uint64_t raft_get_commit_index(void);

// Register a new callback for leader change notifications
void raft_set_leader_change_callback(void (*callback)(uint32_t node_id));

// Clean up resources used by Raft
void raft_term(void);

#endif /* _RAFT_H_ */ 