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
#include <time.h>
#include <pthread.h>

#include "mfs_ha.h"
#include "ha_manager.h"
#include "changelog.h"
#include "metadata.h"
#include "matoclserv.h"
#include "mfslog.h"
#include "cfg.h"

// Configuration defaults
#define DEFAULT_RAFT_PORT 9420
#define DEFAULT_REPL_PORT 9421
#define DEFAULT_ELECTION_TIMEOUT_MIN_MS 150
#define DEFAULT_ELECTION_TIMEOUT_MAX_MS 300
#define DEFAULT_HEARTBEAT_INTERVAL_MS 50
#define DEFAULT_SYNC_TIMEOUT_MS 5000

// Status constants
#define HA_STATUS_DISABLED 0
#define HA_STATUS_LEADER 1
#define HA_STATUS_FOLLOWER 2
#define HA_STATUS_ACTIVE_FOLLOWER 3
#define HA_STATUS_OBSERVER 4
#define HA_STATUS_INACTIVE 5

// Log constants
#define LOG_ERR MFSLOG_ERROR
#define LOG_NOTICE MFSLOG_NOTICE
#define LOG_INFO MFSLOG_INFO

static int ha_enabled = 0;
static pthread_mutex_t ha_mutex = PTHREAD_MUTEX_INITIALIZER;

// Forward declarations
static uint8_t ha_changelog_interceptor(uint8_t type, uint64_t version, void *data, uint32_t data_size);
static int ha_metadata_operation_hook(uint8_t op_type, void *op_data, uint32_t op_size);
static void ha_role_changed_callback(ha_role_t old_role, ha_role_t new_role);

// Initialize MooseFS HA
int mfs_ha_init(void) {
    pthread_mutex_lock(&ha_mutex);

    // Check if HA is enabled in configuration
    ha_enabled = cfg_getuint8("HA_ENABLED", 0);

    if (!ha_enabled) {
        pthread_mutex_unlock(&ha_mutex);
        mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "High Availability is disabled");
        return 0;
    }

    mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "Initializing High Availability subsystem");

    // Read configuration
    uint32_t node_id = cfg_getuint32("HA_NODE_ID", 0);
    char *data_dir = cfg_getstr("HA_DATA_DIR", NULL);
    uint16_t raft_port = cfg_getuint16("HA_RAFT_PORT", DEFAULT_RAFT_PORT);
    uint16_t repl_port = cfg_getuint16("HA_REPL_PORT", DEFAULT_REPL_PORT);
    uint32_t election_min = cfg_getuint32("HA_ELECTION_TIMEOUT_MIN", DEFAULT_ELECTION_TIMEOUT_MIN_MS);
    uint32_t election_max = cfg_getuint32("HA_ELECTION_TIMEOUT_MAX", DEFAULT_ELECTION_TIMEOUT_MAX_MS);
    uint32_t heartbeat = cfg_getuint32("HA_HEARTBEAT_INTERVAL", DEFAULT_HEARTBEAT_INTERVAL_MS);
    uint32_t sync_timeout = cfg_getuint32("HA_SYNC_TIMEOUT", DEFAULT_SYNC_TIMEOUT_MS);

    // Validate configuration
    if (node_id == 0) {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "HA configuration error: NODE_ID must be set");
        pthread_mutex_unlock(&ha_mutex);
        return -1;
    }

    if (data_dir == NULL) {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "HA configuration error: DATA_DIR must be set");
        pthread_mutex_unlock(&ha_mutex);
        return -1;
    }

    // Setup HA configuration
    ha_config_t ha_config;
    memset(&ha_config, 0, sizeof(ha_config));
    
    ha_config.node_id = node_id;
    ha_config.data_dir = data_dir;
    ha_config.raft_port = raft_port;
    ha_config.repl_port = repl_port;
    ha_config.election_timeout_min_ms = election_min;
    ha_config.election_timeout_max_ms = election_max;
    ha_config.heartbeat_interval_ms = heartbeat;
    ha_config.sync_timeout_ms = sync_timeout;
    ha_config.default_mode = HA_MODE_ACTIVE_STANDBY;  // Default mode

    // Initialize the HA manager
    int ret = ha_init(&ha_config);
    if (ret != 0) {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "HA initialization failed");
        free(data_dir);
        pthread_mutex_unlock(&ha_mutex);
        return -1;
    }

    // Register for role change events
    ha_register_event_callback(ha_role_changed_callback);

    // Hook the changelog system for replication
    changelog_register_interceptor(ha_changelog_interceptor);

    // Initialize CGI integration if enabled
    mfs_ha_init_cgi();

    free(data_dir);
    pthread_mutex_unlock(&ha_mutex);
    mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "High Availability subsystem initialized (node ID: %u)", node_id);
    return 0;
}

// Start MooseFS HA
int mfs_ha_start(void) {
    if (!ha_enabled) {
        return 0;
    }

    pthread_mutex_lock(&ha_mutex);
    int ret = ha_start();
    pthread_mutex_unlock(&ha_mutex);

    if (ret == 0) {
        mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "High Availability subsystem started");
    } else {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "Failed to start High Availability subsystem");
    }

    return ret;
}

// Stop MooseFS HA
int mfs_ha_stop(void) {
    if (!ha_enabled) {
        return 0;
    }

    pthread_mutex_lock(&ha_mutex);
    int ret = ha_stop();
    pthread_mutex_unlock(&ha_mutex);

    if (ret == 0) {
        mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "High Availability subsystem stopped");
    } else {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "Failed to stop High Availability subsystem");
    }

    return ret;
}

// Check if current node is the leader
int mfs_ha_is_leader(void) {
    if (!ha_enabled) {
        return 1;  // If HA is disabled, always act as leader
    }

    return ha_is_leader();
}

// Get current HA status
int mfs_ha_status(void) {
    if (!ha_enabled) {
        return HA_STATUS_DISABLED;
    }

    ha_role_t role = ha_get_role();
    ha_mode_t mode = ha_get_mode();

    if (role == HA_ROLE_LEADER) {
        return HA_STATUS_LEADER;
    } else if (role == HA_ROLE_FOLLOWER) {
        return (mode == HA_MODE_ACTIVE_ACTIVE) ? 
            HA_STATUS_ACTIVE_FOLLOWER : HA_STATUS_FOLLOWER;
    } else if (role == HA_ROLE_OBSERVER) {
        return HA_STATUS_OBSERVER;
    } else {
        return HA_STATUS_INACTIVE;
    }
}

// Set HA configuration parameters
int mfs_ha_set_config(uint32_t node_id, const char *data_dir, 
                     uint16_t raft_port, uint16_t repl_port,
                     ha_mode_t mode) {
    
    // Set the configuration parameters for the HA subsystem
    ha_config_t config;
    memset(&config, 0, sizeof(config));
    
    config.node_id = node_id;
    config.data_dir = (char *)data_dir;  // Will be copied by ha_init
    config.raft_port = raft_port;
    config.repl_port = repl_port;
    config.election_timeout_min_ms = cfg_getuint32("HA_ELECTION_TIMEOUT_MIN", DEFAULT_ELECTION_TIMEOUT_MIN_MS);
    config.election_timeout_max_ms = cfg_getuint32("HA_ELECTION_TIMEOUT_MAX", DEFAULT_ELECTION_TIMEOUT_MAX_MS);
    config.heartbeat_interval_ms = cfg_getuint32("HA_HEARTBEAT_INTERVAL", DEFAULT_HEARTBEAT_INTERVAL_MS);
    config.sync_timeout_ms = cfg_getuint32("HA_SYNC_TIMEOUT", DEFAULT_SYNC_TIMEOUT_MS);
    config.default_mode = mode;
    
    return ha_init(&config);
}

// Intercept changelog entries for replication
static uint8_t ha_changelog_interceptor(uint8_t type, uint64_t version, void *data, uint32_t data_size) {
    if (!ha_enabled) {
        return 0;  // Allow changelog to proceed normally
    }

    // If we're not the leader in active-standby mode, reject changelog operations
    if (ha_get_mode() == HA_MODE_ACTIVE_STANDBY && !ha_is_leader()) {
        return 1;  // Reject
    }

    // Package the changelog entry for replication
    uint8_t *buffer = malloc(data_size + sizeof(uint8_t) + sizeof(uint64_t));
    if (buffer == NULL) {
        mfs_log(MFSLOG_SYSLOG, LOG_ERR, "Out of memory for changelog replication");
        return 0;  // Allow to proceed anyway
    }

    // Pack the data for replication
    uint32_t offset = 0;
    memcpy(buffer + offset, &type, sizeof(uint8_t));
    offset += sizeof(uint8_t);
    
    memcpy(buffer + offset, &version, sizeof(uint64_t));
    offset += sizeof(uint64_t);
    
    memcpy(buffer + offset, data, data_size);

    // Submit to replication subsystem
    ha_submit_operation(buffer, data_size + offset, CONSISTENCY_STRONG);
    
    free(buffer);
    
    // Allow the local changelog operation to proceed
    return 0;
}

// Handle MooseFS metadata operations
int mfs_ha_handle_operation(uint8_t op_type) {
    if (!ha_enabled) {
        return 0;  // Allow
    }

    // Check if the operation is a write
    if (metadata_is_write_operation(op_type)) {
        // In active-standby mode, only the leader can process writes
        if (ha_get_mode() == HA_MODE_ACTIVE_STANDBY) {
            if (ha_is_leader()) {
                return 0;  // Allow
            } else {
                mfs_log(MFSLOG_SYSLOG, LOG_INFO, "Rejecting write operation: node is not the leader");
                return 1;  // Redirect
            }
        }
        
        // In read-only mode, no writes allowed
        if (ha_get_mode() == HA_MODE_READ_ONLY) {
            mfs_log(MFSLOG_SYSLOG, LOG_INFO, "Rejecting write operation: system is in read-only mode");
            return -1;  // Reject
        }
    }
    
    // All other operations are allowed
    return 0;
}

// Clean up HA resources
void mfs_ha_term(void) {
    if (!ha_enabled) {
        return;
    }

    pthread_mutex_lock(&ha_mutex);
    
    // Unregister from changelog system
    changelog_unregister_interceptor(ha_changelog_interceptor);
    
    // Stop the HA subsystem
    ha_stop();
    
    // Clean up resources
    ha_term();
    
    ha_enabled = 0;
    pthread_mutex_unlock(&ha_mutex);
    
    mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "High Availability subsystem terminated");
}

// Initialize CGI integration
int mfs_ha_init_cgi(void) {
    if (!ha_enabled) {
        return 0;
    }
    
    // Register CGI handlers for HA
    mfs_log(MFSLOG_SYSLOG, LOG_INFO, "Registering HA CGI handlers");
    
    return 0;
}

// Callback function for role changes
static void ha_role_changed_callback(ha_role_t old_role, ha_role_t new_role) {
    mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "HA role changed from %d to %d", (int)old_role, (int)new_role);
    
    // Handle becoming leader
    if (new_role == HA_ROLE_LEADER) {
        mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "This node has become the leader");
        
        // Notify matoclserv to handle leader state
        matoclserv_ha_state_changed(1);
        
        // Enable write operations
        // TODO: Update any necessary state
    }
    
    // Handle no longer being leader
    if (old_role == HA_ROLE_LEADER && new_role != HA_ROLE_LEADER) {
        mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "This node is no longer the leader");
        
        // Notify matoclserv to handle follower state
        matoclserv_ha_state_changed(0);
        
        // If we're in active-standby mode, disable write operations
        if (ha_get_mode() == HA_MODE_ACTIVE_STANDBY) {
            // TODO: Update any necessary state
        }
    }
} 