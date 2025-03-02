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
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/time.h>
#include <time.h>
#include <inttypes.h>
#include <errno.h>
#include <pthread.h>

#include "ha_manager.h"
#include "raft.h"
#include "replication.h"
#include "mfslog.h"
#include "clocks.h"
#include "massert.h"
#include "datapack.h"

#define LOG_ERR MFSLOG_ERROR
#define LOG_WARNING MFSLOG_WARNING

// HA manager state structure
typedef struct {
    // Configuration
    ha_config_t config;
    
    // Current state
    ha_role_t role;
    ha_mode_t mode;
    uint32_t leader_id;
    
    // Callbacks
    ha_event_callback_t event_callback;
    
    // Threading
    pthread_mutex_t mutex;
    int initialized;
} ha_state;

// Global state
static ha_state hs = {0};

// Forward declarations
static void ha_on_leader_change(uint32_t node_id);
static int ha_apply_log(raft_log_entry_t *entry);
static int ha_resolve_conflict(wal_entry_t *local_entry, wal_entry_t *remote_entry, wal_entry_t **resolved_entry);
static uint64_t ha_get_timestamp(void);
static void ha_on_node_active(uint32_t node_id);
static void ha_on_node_inactive(uint32_t node_id);

// Initialize the HA Manager
int ha_init(ha_config_t *config) {
    raft_config_t raft_config;
    raft_callbacks_t raft_callbacks;
    repl_config_t repl_config;
    repl_callbacks_t repl_callbacks;
    
    if (config == NULL) {
        return -1;
    }
    
    // Initialize state
    memset(&hs, 0, sizeof(hs));
    
    // Copy config
    hs.config = *config;
    if (config->data_dir) {
        hs.config.data_dir = strdup(config->data_dir);
    }
    
    // Set initial state
    hs.role = HA_ROLE_FOLLOWER;
    hs.mode = config->default_mode;
    hs.leader_id = 0;
    
    // Initialize mutex
    pthread_mutex_init(&hs.mutex, NULL);
    
    // Initialize Raft
    memset(&raft_config, 0, sizeof(raft_config));
    raft_config.election_timeout_min_ms = config->election_timeout_min_ms;
    raft_config.election_timeout_max_ms = config->election_timeout_max_ms;
    raft_config.heartbeat_interval_ms = config->heartbeat_interval_ms;
    raft_config.data_dir = config->data_dir;
    
    memset(&raft_callbacks, 0, sizeof(raft_callbacks));
    raft_callbacks.on_leader_change = ha_on_leader_change;
    raft_callbacks.apply_log = ha_apply_log;
    
    if (raft_init(&raft_config, &raft_callbacks) < 0) {
        mfs_log(MFSLOG_SYSLOG,LOG_ERR, "Failed to initialize Raft");
        return -1;
    }
    
    // Initialize replication
    memset(&repl_config, 0, sizeof(repl_config));
    repl_config.mode = (hs.mode == HA_MODE_ACTIVE_STANDBY) ? REPL_MODE_SYNC : REPL_MODE_ASYNC;
    repl_config.sync_timeout_ms = config->sync_timeout_ms;
    repl_config.max_batch_size = 1024;
    repl_config.sync_interval_ms = 500;
    repl_config.data_dir = config->data_dir;
    
    memset(&repl_callbacks, 0, sizeof(repl_callbacks));
    repl_callbacks.apply_entry = NULL;  // Will be set up through Raft apply_log callback
    repl_callbacks.resolve_conflict = ha_resolve_conflict;
    repl_callbacks.get_timestamp = ha_get_timestamp;
    repl_callbacks.on_node_active = ha_on_node_active;
    repl_callbacks.on_node_inactive = ha_on_node_inactive;
    
    if (repl_init(&repl_config, &repl_callbacks) < 0) {
        mfs_log(MFSLOG_SYSLOG,LOG_ERR, "Failed to initialize replication");
        raft_term();
        return -1;
    }
    
    hs.initialized = 1;
    return 0;
}

// Add a node to the HA cluster
int ha_add_node(uint32_t node_id, const char *hostname, uint16_t raft_port, uint16_t repl_port) {
    int result;
    
    if (!hs.initialized || node_id == 0 || hostname == NULL) {
        return -1;
    }
    
    // Add to Raft cluster
    result = raft_add_node(node_id, hostname, raft_port);
    if (result < 0) {
        return result;
    }
    
    // Add to replication topology
    result = repl_add_node(node_id, hostname, repl_port);
    if (result < 0) {
        raft_remove_node(node_id);
        return result;
    }
    
    return 0;
}

// Remove a node from the HA cluster
int ha_remove_node(uint32_t node_id) {
    if (!hs.initialized || node_id == 0) {
        return -1;
    }
    
    // Remove from Raft cluster
    raft_remove_node(node_id);
    
    // Remove from replication topology
    repl_remove_node(node_id);
    
    return 0;
}

// Start the HA subsystem
int ha_start(void) {
    int result;
    
    if (!hs.initialized) {
        return -1;
    }
    
    // Start Raft
    result = raft_start();
    if (result < 0) {
        return result;
    }
    
    // Start replication
    result = repl_start();
    if (result < 0) {
        raft_stop();
        return result;
    }
    
    return 0;
}

// Stop the HA subsystem
int ha_stop(void) {
    if (!hs.initialized) {
        return -1;
    }
    
    // Stop replication
    repl_stop();
    
    // Stop Raft
    raft_stop();
    
    return 0;
}

// Get current node role
ha_role_t ha_get_role(void) {
    ha_role_t role;
    
    pthread_mutex_lock(&hs.mutex);
    role = hs.role;
    pthread_mutex_unlock(&hs.mutex);
    
    return role;
}

// Get current HA mode
ha_mode_t ha_get_mode(void) {
    ha_mode_t mode;
    
    pthread_mutex_lock(&hs.mutex);
    mode = hs.mode;
    pthread_mutex_unlock(&hs.mutex);
    
    return mode;
}

// Set HA mode
int ha_set_mode(ha_mode_t mode) {
    if (!hs.initialized) {
        return -1;
    }
    
    pthread_mutex_lock(&hs.mutex);
    
    // Update mode
    hs.mode = mode;
    
    // Update replication mode
    if (mode == HA_MODE_ACTIVE_STANDBY) {
        repl_set_mode(REPL_MODE_SYNC);
    } else if (mode == HA_MODE_ACTIVE_ACTIVE) {
        repl_set_mode(REPL_MODE_ACTIVE_ACTIVE);
    } else {
        // Read-only mode - no changes to replication mode
    }
    
    pthread_mutex_unlock(&hs.mutex);
    
    return 0;
}

// Get the leader node ID
uint32_t ha_get_leader_id(void) {
    uint32_t leader_id;
    
    pthread_mutex_lock(&hs.mutex);
    leader_id = hs.leader_id;
    pthread_mutex_unlock(&hs.mutex);
    
    return leader_id;
}

// Check if this node is the leader
int ha_is_leader(void) {
    int is_leader;
    
    pthread_mutex_lock(&hs.mutex);
    is_leader = (hs.role == HA_ROLE_LEADER);
    pthread_mutex_unlock(&hs.mutex);
    
    return is_leader;
}

// Get node information list
ha_node_info_t *ha_get_nodes(void) {
    // TODO: Implement collecting node information from Raft and replication subsystems
    return NULL;
}

// Free node information list
void ha_free_nodes(ha_node_info_t *nodes) {
    ha_node_info_t *node, *next;
    
    node = nodes;
    while (node) {
        next = node->next;
        free(node->hostname);
        free(node);
        node = next;
    }
}

// Register for HA events
void ha_register_event_callback(ha_event_callback_t callback) {
    pthread_mutex_lock(&hs.mutex);
    hs.event_callback = callback;
    pthread_mutex_unlock(&hs.mutex);
}

// Submit an operation to be replicated
int ha_submit_operation(uint8_t *data, uint32_t data_size, consistency_level_t consistency) {
    if (!hs.initialized) {
        return -1;
    }
    
    // In active-standby mode, only the leader can submit operations
    if (hs.mode == HA_MODE_ACTIVE_STANDBY && hs.role != HA_ROLE_LEADER) {
        return -1;
    }
    
    // In read-only mode, no operations can be submitted
    if (hs.mode == HA_MODE_READ_ONLY) {
        return -1;
    }
    
    // Submit to replication system
    return repl_submit_entry(data, data_size, consistency);
}

// Clean up resources used by the HA Manager
void ha_term(void) {
    if (!hs.initialized) {
        return;
    }
    
    // Stop subsystems
    ha_stop();
    
    // Clean up Raft
    raft_term();
    
    // Clean up replication
    repl_term();
    
    // Free config strings
    free(hs.config.data_dir);
    
    // Clean up mutex
    pthread_mutex_destroy(&hs.mutex);
    
    hs.initialized = 0;
}

// Raft callback: leader change
static void ha_on_leader_change(uint32_t node_id) {
    ha_role_t old_role, new_role;
    
    pthread_mutex_lock(&hs.mutex);
    
    // Save old role
    old_role = hs.role;
    
    // Update leader ID
    hs.leader_id = node_id;
    
    // Update role based on new leader
    if (node_id == hs.config.node_id) {
        new_role = HA_ROLE_LEADER;
    } else {
        new_role = HA_ROLE_FOLLOWER;
    }
    
    // Update role
    hs.role = new_role;
    
    // Call event callback if role changed
    if (old_role != new_role && hs.event_callback) {
        hs.event_callback(new_role, node_id);
    }
    
    pthread_mutex_unlock(&hs.mutex);
}

// Raft callback: apply log entry
static int ha_apply_log(raft_log_entry_t *entry) {
    // TODO: Implement log entry application
    return 0;
}

// Replication callback: resolve conflict
static int ha_resolve_conflict(wal_entry_t *local_entry, wal_entry_t *remote_entry, wal_entry_t **resolved_entry) {
    // TODO: Implement conflict resolution logic
    
    // For now, use last-writer-wins based on timestamp
    if (local_entry->timestamp >= remote_entry->timestamp) {
        *resolved_entry = local_entry;
    } else {
        *resolved_entry = remote_entry;
    }
    
    return 0;
}

// Replication callback: get timestamp
static uint64_t ha_get_timestamp(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (uint64_t)tv.tv_sec * 1000000 + tv.tv_usec;
}

// Replication callback: node active
static void ha_on_node_active(uint32_t node_id) {
    // TODO: Implement node active handling
}

// Replication callback: node inactive
static void ha_on_node_inactive(uint32_t node_id) {
    // TODO: Implement node inactive handling
} 