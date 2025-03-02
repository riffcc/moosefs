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

#ifndef _REPLICATION_H_
#define _REPLICATION_H_

#include <inttypes.h>

// Replication modes
typedef enum {
    REPL_MODE_SYNC = 0,    // Synchronous replication (strong consistency)
    REPL_MODE_ASYNC = 1,   // Asynchronous replication (eventual consistency)
    REPL_MODE_ACTIVE_ACTIVE = 2,  // Active-active (multi-master) replication
} repl_mode_t;

// Consistency level for operations
typedef enum {
    CONSISTENCY_EVENTUAL = 0,  // Eventually consistent
    CONSISTENCY_SESSION = 1,   // Session consistency (read your writes)
    CONSISTENCY_STRONG = 2,    // Strong consistency
} consistency_level_t;

// Replication configuration
typedef struct _repl_config {
    repl_mode_t mode;
    uint32_t sync_timeout_ms;
    uint32_t max_batch_size;
    uint32_t sync_interval_ms;
    char *data_dir;
} repl_config_t;

// WAL entry structure
typedef struct _wal_entry {
    uint64_t seq_num;
    uint64_t timestamp;
    uint32_t node_id;
    uint32_t data_size;
    uint8_t *data;
} wal_entry_t;

// Replication callbacks
typedef struct _repl_callbacks {
    // Called when a WAL entry is received and needs to be applied
    int (*apply_entry)(wal_entry_t *entry);
    
    // Called to resolve conflicts in active-active mode
    int (*resolve_conflict)(wal_entry_t *local_entry, wal_entry_t *remote_entry, wal_entry_t **resolved_entry);
    
    // Called to get the current timestamp for conflict resolution
    uint64_t (*get_timestamp)(void);
    
    // Called when a node becomes active
    void (*on_node_active)(uint32_t node_id);
    
    // Called when a node becomes inactive
    void (*on_node_inactive)(uint32_t node_id);
} repl_callbacks_t;

// Initialize the replication subsystem
int repl_init(repl_config_t *config, repl_callbacks_t *callbacks);

// Add a node to the replication topology
int repl_add_node(uint32_t node_id, const char *hostname, uint16_t port);

// Remove a node from the replication topology
int repl_remove_node(uint32_t node_id);

// Start the replication subsystem
int repl_start(void);

// Stop the replication subsystem
int repl_stop(void);

// Submit an entry to the WAL
int repl_submit_entry(uint8_t *data, uint32_t data_size, consistency_level_t consistency);

// Wait for all entries to be replicated (synchronous mode only)
int repl_sync(uint32_t timeout_ms);

// Get the current replication lag for a node (in entries)
uint64_t repl_get_lag(uint32_t node_id);

// Set the replication mode
int repl_set_mode(repl_mode_t mode);

// Register a new callback for node state changes
void repl_set_node_state_callback(void (*on_active)(uint32_t), void (*on_inactive)(uint32_t));

// Clean up resources used by replication
void repl_term(void);

#endif /* _REPLICATION_H_ */ 