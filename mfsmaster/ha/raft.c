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
#include <fcntl.h>
#include <pthread.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
#include <inttypes.h>
#include <errno.h>
#include <poll.h>
#include <netinet/in.h>
#include <arpa/inet.h>

#include "raft.h"
#include "crc.h"
#include "datapack.h"
#include "mfslog.h"
#include "random.h"
#include "sockets.h"
#include "mfsalloc.h"
#include "clocks.h"
#include "massert.h"

#define MAX_NODES 64
#define MAX_PENDING_REQS 1024
#define RAFT_LOG_MAGIC 0x524146544C4F4700ULL  // "RAFTLOG\0" in hex
#define RAFT_SNAPSHOT_MAGIC 0x52414654534E4150ULL  // "RAFTSNAP" in hex

#define LOG_ERR MFSLOG_ERROR
#define LOG_WARNING MFSLOG_WARNING

// Raft message types
typedef enum {
    RAFT_MSG_VOTE_REQUEST = 1,
    RAFT_MSG_VOTE_RESPONSE = 2,
    RAFT_MSG_APPEND_ENTRIES_REQUEST = 3,
    RAFT_MSG_APPEND_ENTRIES_RESPONSE = 4,
    RAFT_MSG_INSTALL_SNAPSHOT_REQUEST = 5,
    RAFT_MSG_INSTALL_SNAPSHOT_RESPONSE = 6,
} raft_message_type_t;

// Vote request message
typedef struct {
    uint64_t term;
    uint32_t candidate_id;
    uint64_t last_log_index;
    uint64_t last_log_term;
} raft_vote_request_t;

// Vote response message
typedef struct {
    uint64_t term;
    uint8_t vote_granted;
} raft_vote_response_t;

// Append entries request message
typedef struct {
    uint64_t term;
    uint32_t leader_id;
    uint64_t prev_log_index;
    uint64_t prev_log_term;
    uint64_t leader_commit;
    // Followed by entries (packed format)
} raft_append_entries_request_t;

// Append entries response message
typedef struct {
    uint64_t term;
    uint8_t success;
    uint64_t match_index;
} raft_append_entries_response_t;

// Install snapshot request message
typedef struct {
    uint64_t term;
    uint32_t leader_id;
    uint64_t last_included_index;
    uint64_t last_included_term;
    uint32_t offset;
    uint8_t done;
    // Followed by snapshot data (raw bytes)
} raft_install_snapshot_request_t;

// Install snapshot response message
typedef struct {
    uint64_t term;
    uint8_t success;
} raft_install_snapshot_response_t;

// Raft log structure
typedef struct {
    uint64_t first_index;
    uint64_t last_index;
    raft_log_entry_t *entries;   // Dynamic array of entries
    uint32_t capacity;
    char *log_path;
    int log_fd;
} raft_log_t;

// Pending request structure
typedef struct {
    uint64_t index;
    void (*callback)(void *ctx, int success);
    void *ctx;
} raft_pending_req_t;

// Raft state structure
typedef struct {
    // Persistent state
    uint64_t current_term;
    uint32_t voted_for;
    raft_log_t log;
    
    // Volatile state
    raft_state_t state;
    uint32_t leader_id;
    uint64_t commit_index;
    uint64_t last_applied;
    uint32_t node_id;  // This node's ID
    
    // Leader state
    uint64_t *next_index;    // Array indexed by node_id
    uint64_t *match_index;   // Array indexed by node_id
    
    // Configuration
    raft_config_t config;
    raft_callbacks_t callbacks;
    
    // Cluster membership
    raft_node_t *nodes;
    uint32_t node_count;
    
    // Timer state
    uint64_t election_timeout;
    uint64_t last_heartbeat;
    
    // Network state
    int server_socket;
    struct pollfd *poll_fds;
    int poll_count;
    
    // Threading
    pthread_t main_thread;
    pthread_mutex_t mutex;
    int running;
    
    // Pending client requests
    raft_pending_req_t pending_reqs[MAX_PENDING_REQS];
    uint32_t pending_req_count;
    
    // Snapshot state
    uint64_t last_snapshot_index;
    uint64_t last_snapshot_term;
    char *snapshot_path;
} raft_state;

// Global state
static raft_state rs = {0};
static pthread_mutex_t g_mutex = PTHREAD_MUTEX_INITIALIZER;

// Forward declarations
static int raft_log_append(raft_log_t *log, uint64_t term, uint8_t *data, uint32_t data_size);
static int raft_become_follower(uint64_t term, uint32_t leader_id);
static int raft_become_candidate(void);
static int raft_become_leader(void);
static int raft_send_append_entries(uint32_t node_id, int heartbeat);
static int raft_send_vote_request(uint32_t node_id);
static int raft_handle_vote_request(uint32_t sender_id, const uint8_t *data, uint32_t data_size);
static int raft_handle_vote_response(uint32_t sender_id, const uint8_t *data, uint32_t data_size);
static int raft_handle_append_entries(uint32_t sender_id, const uint8_t *data, uint32_t data_size);
static int raft_handle_append_entries_response(uint32_t sender_id, const uint8_t *data, uint32_t data_size);
static int raft_apply_log_entries(void);
static void *raft_main_loop(void *arg);
static int raft_handle_message(int sock, uint32_t sender_id);
static int raft_log_init(raft_log_t *log, const char *dir);
static int raft_log_load(raft_log_t *log);
static int raft_log_save_entry(raft_log_t *log, raft_log_entry_t *entry);
static int raft_save_state(void);
static int raft_load_state(void);
static void raft_reset_election_timeout(void);

// Initialize the Raft subsystem
int raft_init(raft_config_t *config, raft_callbacks_t *callbacks) {
    if (config == NULL || callbacks == NULL) {
        return -1;
    }
    
    memset(&rs, 0, sizeof(rs));
    
    // Copy config and callbacks
    rs.config = *config;
    rs.callbacks = *callbacks;
    
    // Initialize persistent state
    rs.current_term = 0;
    rs.voted_for = 0;
    
    // Initialize volatile state
    rs.state = RAFT_STATE_FOLLOWER;
    rs.leader_id = 0;
    rs.commit_index = 0;
    rs.last_applied = 0;
    
    // Initialize log
    if (raft_log_init(&rs.log, config->data_dir) < 0) {
        mfs_log(MFSLOG_SYSLOG,LOG_ERR, "Failed to initialize Raft log");
        return -1;
    }
    
    // Load persistent state
    if (raft_load_state() < 0) {
        mfs_log(MFSLOG_SYSLOG,LOG_WARNING, "Failed to load Raft state, starting fresh");
    }
    
    // Initialize networking
    rs.server_socket = -1;
    rs.poll_fds = NULL;
    rs.poll_count = 0;
    
    // Initialize mutex
    pthread_mutex_init(&rs.mutex, NULL);
    
    return 0;
}

// Add a node to the Raft cluster
int raft_add_node(uint32_t node_id, const char *hostname, uint16_t port) {
    raft_node_t *node, *curr;
    
    if (node_id == 0 || hostname == NULL) {
        return -1;
    }
    
    pthread_mutex_lock(&rs.mutex);
    
    // Check for existing node with same ID
    curr = rs.nodes;
    while (curr) {
        if (curr->node_id == node_id) {
            // Update existing node
            free(curr->hostname);
            curr->hostname = strdup(hostname);
            curr->port = port;
            pthread_mutex_unlock(&rs.mutex);
            return 0;
        }
        curr = curr->next;
    }
    
    // Add new node
    node = malloc(sizeof(raft_node_t));
    if (node == NULL) {
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    node->node_id = node_id;
    node->hostname = strdup(hostname);
    node->port = port;
    node->next = rs.nodes;
    rs.nodes = node;
    rs.node_count++;
    
    // If we're the leader, initialize leader state for this node
    if (rs.state == RAFT_STATE_LEADER) {
        rs.next_index[node_id] = rs.log.last_index + 1;
        rs.match_index[node_id] = 0;
    }
    
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Remove a node from the Raft cluster
int raft_remove_node(uint32_t node_id) {
    raft_node_t *prev = NULL, *curr;
    
    if (node_id == 0) {
        return -1;
    }
    
    pthread_mutex_lock(&rs.mutex);
    
    // Find node
    curr = rs.nodes;
    while (curr) {
        if (curr->node_id == node_id) {
            // Remove node
            if (prev) {
                prev->next = curr->next;
            } else {
                rs.nodes = curr->next;
            }
            free(curr->hostname);
            free(curr);
            rs.node_count--;
            pthread_mutex_unlock(&rs.mutex);
            return 0;
        }
        prev = curr;
        curr = curr->next;
    }
    
    pthread_mutex_unlock(&rs.mutex);
    return -1;  // Node not found
}

// Start the Raft subsystem
int raft_start(void) {
    pthread_mutex_lock(&rs.mutex);
    
    if (rs.running) {
        pthread_mutex_unlock(&rs.mutex);
        return 0;  // Already running
    }
    
    // Initialize leader state arrays
    rs.next_index = calloc(MAX_NODES + 1, sizeof(uint64_t));
    rs.match_index = calloc(MAX_NODES + 1, sizeof(uint64_t));
    
    if (!rs.next_index || !rs.match_index) {
        free(rs.next_index);
        free(rs.match_index);
        rs.next_index = NULL;
        rs.match_index = NULL;
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    // Initialize random seed
    rnd_init();
    
    // Initial election timeout
    raft_reset_election_timeout();
    rs.last_heartbeat = monotonic_useconds();
    
    // Start main thread
    rs.running = 1;
    if (pthread_create(&rs.main_thread, NULL, raft_main_loop, NULL) != 0) {
        rs.running = 0;
        free(rs.next_index);
        free(rs.match_index);
        rs.next_index = NULL;
        rs.match_index = NULL;
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Stop the Raft subsystem
int raft_stop(void) {
    pthread_mutex_lock(&rs.mutex);
    
    if (!rs.running) {
        pthread_mutex_unlock(&rs.mutex);
        return 0;  // Not running
    }
    
    rs.running = 0;
    pthread_mutex_unlock(&rs.mutex);
    
    // Wait for main thread to exit
    pthread_join(rs.main_thread, NULL);
    
    // Clean up
    free(rs.next_index);
    free(rs.match_index);
    rs.next_index = NULL;
    rs.match_index = NULL;
    
    return 0;
}

// Get current Raft state
raft_state_t raft_get_state(void) {
    raft_state_t state;
    
    pthread_mutex_lock(&rs.mutex);
    state = rs.state;
    pthread_mutex_unlock(&rs.mutex);
    
    return state;
}

// Get current Raft term
uint64_t raft_get_current_term(void) {
    uint64_t term;
    
    pthread_mutex_lock(&rs.mutex);
    term = rs.current_term;
    pthread_mutex_unlock(&rs.mutex);
    
    return term;
}

// Get current leader ID (0 if no leader)
uint32_t raft_get_leader_id(void) {
    uint32_t leader_id;
    
    pthread_mutex_lock(&rs.mutex);
    leader_id = rs.leader_id;
    pthread_mutex_unlock(&rs.mutex);
    
    return leader_id;
}

// Check if this node is the leader
int raft_is_leader(void) {
    int is_leader;
    
    pthread_mutex_lock(&rs.mutex);
    is_leader = (rs.state == RAFT_STATE_LEADER);
    pthread_mutex_unlock(&rs.mutex);
    
    return is_leader;
}

// Submit a new log entry to the Raft log (only succeeds on leader)
int raft_submit_entry(uint8_t *data, uint32_t data_size) {
    int result;
    
    pthread_mutex_lock(&rs.mutex);
    
    if (rs.state != RAFT_STATE_LEADER) {
        pthread_mutex_unlock(&rs.mutex);
        return -1;  // Not leader
    }
    
    // Append to log
    result = raft_log_append(&rs.log, rs.current_term, data, data_size);
    if (result < 0) {
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    // Set replication state for this entry
    uint64_t index = rs.log.last_index;
    for (raft_node_t *node = rs.nodes; node != NULL; node = node->next) {
        // Skip self
        if (node->node_id == rs.node_id) {
            rs.match_index[node->node_id] = index;
            continue;
        }
        
        // Send append entries RPC immediately
        raft_send_append_entries(node->node_id, 0);
    }
    
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Force a leadership transfer (only succeeds on leader)
int raft_transfer_leadership(uint32_t target_node_id) {
    int result = -1;
    
    pthread_mutex_lock(&rs.mutex);
    
    if (rs.state != RAFT_STATE_LEADER) {
        pthread_mutex_unlock(&rs.mutex);
        return -1;  // Not leader
    }
    
    // TODO: Implement leadership transfer
    
    pthread_mutex_unlock(&rs.mutex);
    return result;
}

// Get the latest committed log index
uint64_t raft_get_commit_index(void) {
    uint64_t commit_index;
    
    pthread_mutex_lock(&rs.mutex);
    commit_index = rs.commit_index;
    pthread_mutex_unlock(&rs.mutex);
    
    return commit_index;
}

// Register a new callback for leader change notifications
void raft_set_leader_change_callback(void (*callback)(uint32_t node_id)) {
    pthread_mutex_lock(&rs.mutex);
    rs.callbacks.on_leader_change = callback;
    pthread_mutex_unlock(&rs.mutex);
}

// Clean up resources used by Raft
void raft_term(void) {
    raft_node_t *node, *next;
    
    // Stop if running
    raft_stop();
    
    // Free nodes
    node = rs.nodes;
    while (node) {
        next = node->next;
        free(node->hostname);
        free(node);
        node = next;
    }
    rs.nodes = NULL;
    
    // Close log file
    if (rs.log.log_fd >= 0) {
        close(rs.log.log_fd);
        rs.log.log_fd = -1;
    }
    
    // Free log entries
    free(rs.log.entries);
    rs.log.entries = NULL;
    
    // Free paths
    free(rs.log.log_path);
    free(rs.snapshot_path);
    
    // Free leader state
    free(rs.next_index);
    free(rs.match_index);
    
    // Destroy mutex
    pthread_mutex_destroy(&rs.mutex);
}

// Main Raft thread loop
static void *raft_main_loop(void *arg) {
    uint64_t now, elapsed;
    int timeout;
    
    while (1) {
        pthread_mutex_lock(&rs.mutex);
        
        if (!rs.running) {
            pthread_mutex_unlock(&rs.mutex);
            break;
        }
        
        // Get current time
        now = monotonic_useconds();
        
        // Check for timeout
        elapsed = now - rs.last_heartbeat;
        if (rs.state == RAFT_STATE_FOLLOWER || rs.state == RAFT_STATE_CANDIDATE) {
            if (elapsed >= rs.election_timeout) {
                // Start election
                raft_become_candidate();
                rs.last_heartbeat = now;
            }
        } else if (rs.state == RAFT_STATE_LEADER) {
            if (elapsed >= rs.config.heartbeat_interval_ms * 1000) {
                // Send heartbeats
                for (raft_node_t *node = rs.nodes; node != NULL; node = node->next) {
                    if (node->node_id != rs.node_id) {
                        raft_send_append_entries(node->node_id, 1);
                    }
                }
                rs.last_heartbeat = now;
            }
        }
        
        // Apply committed log entries
        raft_apply_log_entries();
        
        // Calculate poll timeout
        if (rs.state == RAFT_STATE_LEADER) {
            timeout = rs.config.heartbeat_interval_ms;
        } else {
            timeout = (rs.election_timeout - elapsed) / 1000;
            if (timeout < 0) {
                timeout = 0;
            }
        }
        
        // TODO: Handle network I/O
        
        pthread_mutex_unlock(&rs.mutex);
        
        // Sleep a bit to avoid busy waiting
        usleep(10000);  // 10ms
    }
    
    return NULL;
}

// Initialize log
static int raft_log_init(raft_log_t *log, const char *dir) {
    // Initialize log structure
    log->first_index = 1;
    log->last_index = 0;
    log->capacity = 1000;  // Initial capacity
    log->entries = malloc(log->capacity * sizeof(raft_log_entry_t));
    if (!log->entries) {
        return -1;
    }
    
    // Set log path
    if (dir) {
        size_t len = strlen(dir);
        log->log_path = malloc(len + 10);
        if (!log->log_path) {
            free(log->entries);
            log->entries = NULL;
            return -1;
        }
        strcpy(log->log_path, dir);
        if (dir[len - 1] != '/') {
            strcat(log->log_path, "/");
        }
        strcat(log->log_path, "raft.log");
    } else {
        log->log_path = strdup("raft.log");
        if (!log->log_path) {
            free(log->entries);
            log->entries = NULL;
            return -1;
        }
    }
    
    // Open log file
    log->log_fd = open(log->log_path, O_RDWR | O_CREAT, 0644);
    if (log->log_fd < 0) {
        free(log->entries);
        free(log->log_path);
        log->entries = NULL;
        log->log_path = NULL;
        return -1;
    }
    
    // Load existing log
    if (raft_log_load(log) < 0) {
        // Failed to load, but we can continue with an empty log
        mfs_log(MFSLOG_SYSLOG,LOG_WARNING, "Failed to load Raft log");
    }
    
    return 0;
}

// Load log entries from disk
static int raft_log_load(raft_log_t *log) {
    // TODO: Implement loading log entries from disk
    return 0;
}

// Append an entry to the log
static int raft_log_append(raft_log_t *log, uint64_t term, uint8_t *data, uint32_t data_size) {
    raft_log_entry_t *entry;
    
    // Ensure capacity
    if (log->last_index - log->first_index + 1 >= log->capacity) {
        uint32_t new_capacity = log->capacity * 2;
        raft_log_entry_t *new_entries = realloc(log->entries, new_capacity * sizeof(raft_log_entry_t));
        if (!new_entries) {
            return -1;
        }
        log->entries = new_entries;
        log->capacity = new_capacity;
    }
    
    // Create new entry
    log->last_index++;
    entry = &log->entries[log->last_index - log->first_index];
    entry->term = term;
    entry->index = log->last_index;
    entry->data_size = data_size;
    entry->data = malloc(data_size);
    if (!entry->data) {
        log->last_index--;
        return -1;
    }
    memcpy(entry->data, data, data_size);
    
    // Save entry to disk
    if (raft_log_save_entry(log, entry) < 0) {
        free(entry->data);
        log->last_index--;
        return -1;
    }
    
    return 0;
}

// Save log entry to disk
static int raft_log_save_entry(raft_log_t *log, raft_log_entry_t *entry) {
    // TODO: Implement saving log entry to disk
    return 0;
}

// Save persistent state to disk
static int raft_save_state(void) {
    // TODO: Implement saving persistent state to disk
    return 0;
}

// Load persistent state from disk
static int raft_load_state(void) {
    // TODO: Implement loading persistent state from disk
    return 0;
}

// Transition to follower state
static int raft_become_follower(uint64_t term, uint32_t leader_id) {
    // Update state
    rs.state = RAFT_STATE_FOLLOWER;
    rs.current_term = term;
    rs.voted_for = 0;
    rs.leader_id = leader_id;
    
    // Reset election timeout
    raft_reset_election_timeout();
    
    // Save state
    raft_save_state();
    
    // Notify of leader change
    if (rs.callbacks.on_leader_change) {
        rs.callbacks.on_leader_change(leader_id);
    }
    
    return 0;
}

// Transition to candidate state
static int raft_become_candidate(void) {
    // Update state
    rs.state = RAFT_STATE_CANDIDATE;
    rs.current_term++;
    rs.voted_for = rs.node_id;
    rs.leader_id = 0;
    
    // Reset election timeout
    raft_reset_election_timeout();
    
    // Save state
    raft_save_state();
    
    // Request votes from all peers
    for (raft_node_t *node = rs.nodes; node != NULL; node = node->next) {
        if (node->node_id != rs.node_id) {
            raft_send_vote_request(node->node_id);
        }
    }
    
    return 0;
}

// Transition to leader state
static int raft_become_leader(void) {
    // Update state
    rs.state = RAFT_STATE_LEADER;
    rs.leader_id = rs.node_id;
    
    // Initialize leader state
    uint64_t next_index = rs.log.last_index + 1;
    for (raft_node_t *node = rs.nodes; node != NULL; node = node->next) {
        rs.next_index[node->node_id] = next_index;
        rs.match_index[node->node_id] = 0;
    }
    
    // Send initial empty AppendEntries RPCs (heartbeats) to each server
    for (raft_node_t *node = rs.nodes; node != NULL; node = node->next) {
        if (node->node_id != rs.node_id) {
            raft_send_append_entries(node->node_id, 1);
        }
    }
    
    // Notify of leader change
    if (rs.callbacks.on_leader_change) {
        rs.callbacks.on_leader_change(rs.node_id);
    }
    
    return 0;
}

// Reset election timeout
static void raft_reset_election_timeout(void) {
    // Random timeout between min and max
    uint32_t range = rs.config.election_timeout_max_ms - rs.config.election_timeout_min_ms;
    uint32_t timeout = rs.config.election_timeout_min_ms + (rndu32_ranged(range) * 1000);
    rs.election_timeout = timeout * 1000;  // Convert to microseconds
    rs.last_heartbeat = monotonic_useconds();
}

// Send AppendEntries RPC to a node
static int raft_send_append_entries(uint32_t node_id, int heartbeat) {
    // TODO: Implement sending AppendEntries RPC
    return 0;
}

// Send RequestVote RPC to a node
static int raft_send_vote_request(uint32_t node_id) {
    // TODO: Implement sending RequestVote RPC
    return 0;
}

// Handle incoming vote request
static int raft_handle_vote_request(uint32_t sender_id, const uint8_t *data, uint32_t data_size) {
    // TODO: Implement handling vote request
    return 0;
}

// Handle incoming vote response
static int raft_handle_vote_response(uint32_t sender_id, const uint8_t *data, uint32_t data_size) {
    // TODO: Implement handling vote response
    return 0;
}

// Handle incoming append entries request
static int raft_handle_append_entries(uint32_t sender_id, const uint8_t *data, uint32_t data_size) {
    // TODO: Implement handling append entries request
    return 0;
}

// Handle incoming append entries response
static int raft_handle_append_entries_response(uint32_t sender_id, const uint8_t *data, uint32_t data_size) {
    // TODO: Implement handling append entries response
    return 0;
}

// Apply committed log entries to state machine
static int raft_apply_log_entries(void) {
    // Apply all newly committed entries
    while (rs.last_applied < rs.commit_index) {
        rs.last_applied++;
        
        // Get the entry
        uint64_t idx = rs.last_applied - rs.log.first_index;
        if (idx >= rs.log.last_index - rs.log.first_index + 1) {
            // This shouldn't happen if our log is consistent
            mfs_log(MFSLOG_SYSLOG,LOG_ERR, "Raft log inconsistency detected");
            return -1;
        }
        
        raft_log_entry_t *entry = &rs.log.entries[idx];
        
        // Apply to state machine
        if (rs.callbacks.apply_log) {
            rs.callbacks.apply_log(entry);
        }
    }
    
    return 0;
}

// Handle an incoming Raft message
static int raft_handle_message(int sock, uint32_t sender_id) {
    // TODO: Implement message handling
    return 0;
} 