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
#include <sys/stat.h>
#include <fcntl.h>
#include <pthread.h>
#include <inttypes.h>
#include <errno.h>

#include "mfs_ha.h"
#include "ha_manager.h"
#include "mfslog.h"
#include "datacachemgr.h"
#include "changelog.h"
#include "metadata.h"
#include "matomlserv.h"
#include "matoclserv.h"
#include "cfg.h"

// Configuration defaults
#define DEFAULT_RAFT_PORT 9420
#define DEFAULT_REPL_PORT 9421
#define DEFAULT_ELECTION_TIMEOUT_MIN_MS 150
#define DEFAULT_ELECTION_TIMEOUT_MAX_MS 300
#define DEFAULT_HEARTBEAT_INTERVAL_MS 50
#define DEFAULT_SYNC_TIMEOUT_MS 5000

#define LOG_ERR MFSLOG_ERROR
#define LOG_WARNING MFSLOG_WARNING
#define LOG_NOTICE MFSLOG_NOTICE

// MFS HA state structure
typedef struct {
    uint8_t enabled;
    uint8_t initialized;
    ha_config_t config;
    pthread_mutex_t mutex;
} mfs_ha_state;

// Global state
static mfs_ha_state mfs_ha = {0};

// Forward declarations
static void mfs_ha_on_role_change(ha_role_t new_role, uint32_t leader_id);
static void mfs_ha_redirect_clients(uint32_t leader_id);
static void mfs_ha_update_master_state(ha_role_t role);

// Initialize MFS High Availability
int mfs_ha_init(void) {
    char *config_value;
    
    // Check if HA is enabled
    config_value = cfg_getstr("HA_ENABLED", "0");
    if (strcmp(config_value, "1") != 0) {
        mfs_ha.enabled = 0;
        free(config_value);
        return 0;  // HA is disabled, not an error
    }
    free(config_value);
    
    mfs_ha.enabled = 1;
    
    // Initialize mutex
    pthread_mutex_init(&mfs_ha.mutex, NULL);
    
    // Read configuration values
    uint32_t node_id = cfg_getuint32("HA_NODE_ID", 0);
    if (node_id == 0) {
        mfs_log(MFSLOG_SYSLOG_STDERR, LOG_ERR, "HA: NODE_ID must be set to a positive value in configuration");
        return -1;
    }
    
    char *data_dir = cfg_getstr("HA_DATA_DIR", NULL);
    if (data_dir == NULL) {
        mfs_log(MFSLOG_SYSLOG_STDERR, LOG_ERR, "HA: DATA_DIR must be set in configuration");
        return -1;
    }
    
    uint16_t raft_port = cfg_getuint16("HA_RAFT_PORT", DEFAULT_RAFT_PORT);
    uint16_t repl_port = cfg_getuint16("HA_REPL_PORT", DEFAULT_REPL_PORT);
    char *mode_str = cfg_getstr("HA_MODE", "active-standby");
    
    ha_mode_t mode;
    if (strcmp(mode_str, "active-active") == 0) {
        mode = HA_MODE_ACTIVE_ACTIVE;
    } else if (strcmp(mode_str, "read-only") == 0) {
        mode = HA_MODE_READ_ONLY;
    } else {
        mode = HA_MODE_ACTIVE_STANDBY;
    }
    free(mode_str);
    
    // Set HA configuration
    int result = mfs_ha_set_config(node_id, data_dir, raft_port, repl_port, mode);
    free(data_dir);
    
    if (result < 0) {
        return result;
    }
    
    // Initialize CGI integration
    mfs_ha_init_cgi();
    
    mfs_ha.initialized = 1;
    return 0;
}

// Start MFS High Availability
int mfs_ha_start(void) {
    int result;
    
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 0;  // Not enabled or not initialized, not an error
    }
    
    // Start HA manager
    result = ha_start();
    if (result < 0) {
        mfs_log(MFSLOG_SYSLOG_STDERR, LOG_ERR, "Failed to start HA subsystem");
        return result;
    }
    
    // Register for role change events
    ha_register_event_callback(mfs_ha_on_role_change);
    
    // Update master state based on initial role
    mfs_ha_update_master_state(ha_get_role());
    
    return 0;
}

// Stop MFS High Availability
int mfs_ha_stop(void) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 0;  // Not enabled or not initialized, not an error
    }
    
    return ha_stop();
}

// Check if this node is the leader and can service write operations
int mfs_ha_is_leader(void) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 1;  // Not in HA mode, always allowed
    }
    
    // In active-active mode, all nodes can service writes
    if (ha_get_mode() == HA_MODE_ACTIVE_ACTIVE) {
        return 1;
    }
    
    return ha_is_leader();
}

// Get current HA status
int mfs_ha_status(void) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 0;  // Not in HA mode
    }
    
    ha_role_t role = ha_get_role();
    ha_mode_t mode = ha_get_mode();
    
    if (role == HA_ROLE_LEADER) {
        return 1;  // Active leader
    } else if (role == HA_ROLE_FOLLOWER && mode == HA_MODE_ACTIVE_ACTIVE) {
        return 2;  // Active follower
    } else {
        return 0;  // Inactive
    }
}

// Set HA configuration
int mfs_ha_set_config(uint32_t node_id, const char *data_dir, uint16_t raft_port, 
                     uint16_t repl_port, ha_mode_t mode) {
    ha_config_t config;
    
    if (node_id == 0 || data_dir == NULL) {
        return -1;
    }
    
    // Initialize config
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
    
    pthread_mutex_lock(&mfs_ha.mutex);
    mfs_ha.config = config;
    pthread_mutex_unlock(&mfs_ha.mutex);
    
    // Initialize HA manager
    return ha_init(&config);
}

// Intercept metadata changes for replication
int mfs_ha_changelog_interceptor(const char *logstring) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 0;  // Not in HA mode, proceed with normal changelog
    }
    
    // In active-standby mode, only leaders can make changes
    if (ha_get_mode() == HA_MODE_ACTIVE_STANDBY && !ha_is_leader()) {
        return -1;  // Not leader, reject change
    }
    
    // In read-only mode, no changes allowed
    if (ha_get_mode() == HA_MODE_READ_ONLY) {
        return -1;  // Read-only mode, reject change
    }
    
    // Submit the change to the replication system
    uint8_t *data = (uint8_t *)strdup(logstring);
    if (!data) {
        return -1;  // Out of memory
    }
    
    int result = ha_submit_operation(data, strlen(logstring) + 1, CONSISTENCY_STRONG);
    free(data);
    
    return result;
}

// Handle a MooseFS metadata operation
int mfs_ha_handle_operation(uint8_t op_type) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return 0;  // Not in HA mode, allow operation
    }
    
    // Read operations are always allowed
    if (op_type == 0) {
        return 0;
    }
    
    // Write operations
    if (op_type == 1) {
        // In active-standby mode, only the leader can process writes
        if (ha_get_mode() == HA_MODE_ACTIVE_STANDBY) {
            if (ha_is_leader()) {
                return 0;  // Leader, allow write
            } else {
                return 1;  // Not leader, redirect to leader
            }
        }
        
        // In active-active mode, all nodes can process writes
        if (ha_get_mode() == HA_MODE_ACTIVE_ACTIVE) {
            return 0;  // Allow write
        }
        
        // In read-only mode, no writes allowed
        if (ha_get_mode() == HA_MODE_READ_ONLY) {
            return -1;  // Reject write
        }
    }
    
    return -1;  // Unknown operation type
}

// Clean up MFS HA resources
void mfs_ha_term(void) {
    if (!mfs_ha.enabled || !mfs_ha.initialized) {
        return;
    }
    
    ha_term();
    pthread_mutex_destroy(&mfs_ha.mutex);
    mfs_ha.initialized = 0;
}

// Handler for role changes
static void mfs_ha_on_role_change(ha_role_t new_role, uint32_t leader_id) {
    // Update master state
    mfs_ha_update_master_state(new_role);
    
    // If we're not the leader, redirect clients
    if (new_role != HA_ROLE_LEADER && leader_id != 0) {
        mfs_ha_redirect_clients(leader_id);
    }
    
    // Log the role change
    const char *role_str = "unknown";
    switch (new_role) {
        case HA_ROLE_FOLLOWER:
            role_str = "follower";
            break;
        case HA_ROLE_CANDIDATE:
            role_str = "candidate";
            break;
        case HA_ROLE_LEADER:
            role_str = "leader";
            break;
        case HA_ROLE_OBSERVER:
            role_str = "observer";
            break;
        default:
            break;
    }
    
    mfs_log(MFSLOG_SYSLOG, LOG_NOTICE, "HA role changed to %s (leader ID: %u)", role_str, leader_id);
}

// Redirect clients to the leader
static void mfs_ha_redirect_clients(uint32_t leader_id) {
    // In active-standby mode, tell clients to reconnect to the leader
    if (ha_get_mode() != HA_MODE_ACTIVE_ACTIVE) {
        matoclserv_ha_redirect(leader_id);
    }
}

// Update master state based on role
static void mfs_ha_update_master_state(ha_role_t role) {
    ha_mode_t mode = ha_get_mode();
    
    // In active-standby mode, only the leader can make changes
    if (mode == HA_MODE_ACTIVE_STANDBY) {
        if (role == HA_ROLE_LEADER) {
            // Enable write operations
            meta_setignoreflag();
        } else {
            // Disable write operations, but continue serving reads
            // TODO: Implement
        }
    } 
    // In active-active mode, all nodes can make changes
    else if (mode == HA_MODE_ACTIVE_ACTIVE) {
        // Enable write operations
        meta_setignoreflag();
    }
    // In read-only mode, no changes allowed
    else if (mode == HA_MODE_READ_ONLY) {
        // Disable write operations
        // TODO: Implement
    }
}

// Initialize mfscgi integration with HA
int mfs_ha_init_cgi(void) {
    // TODO: Register CGI handlers
    return 0;
}

// CGI callback for HA status page
void mfs_ha_status_cgi(void) {
    // TODO: Implement status page
}

// CGI callback for HA admin operations
void mfs_ha_admin_cgi(void) {
    // TODO: Implement admin operations
} 