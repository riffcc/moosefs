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

#include "replication.h"
#include "crc.h"
#include "datapack.h"
#include "mfslog.h"
#include "random.h"
#include "sockets.h"
#include "mfsalloc.h"
#include "clocks.h"
#include "massert.h"

#define MAX_NODES 64
#define MAX_BATCH_SIZE 1024
#define WAL_MAGIC 0x57414C4D4753ULL  // "WALMGS" in hex

#define LOG_ERR MFSLOG_ERROR
#define LOG_WARNING MFSLOG_WARNING

// Message types
typedef enum {
    MSG_WAL_ENTRY = 1,
    MSG_WAL_ACK = 2,
    MSG_NODE_STATUS = 3,
    MSG_SYNC_REQUEST = 4,
    MSG_SYNC_RESPONSE = 5,
} message_type_t;

// Node status
typedef enum {
    NODE_STATUS_ACTIVE = 1,
    NODE_STATUS_INACTIVE = 2,
} node_status_t;

// Node structure
typedef struct _repl_node {
    uint32_t node_id;
    char *hostname;
    uint16_t port;
    int sock;
    uint64_t last_seq_num;
    uint64_t last_ack_seq_num;
    node_status_t status;
    struct _repl_node *next;
} repl_node_t;

// WAL file header
typedef struct {
    uint64_t magic;
    uint64_t version;
    uint64_t first_seq_num;
    uint64_t entry_count;
} wal_header_t;

// WAL structure
typedef struct {
    uint64_t next_seq_num;
    uint64_t first_seq_num;
    char *wal_path;
    int wal_fd;
} wal_t;

// Pending entry structure
typedef struct _pending_entry {
    wal_entry_t entry;
    uint64_t submit_time;
    consistency_level_t consistency;
    struct _pending_entry *next;
} pending_entry_t;

// Replication state structure
typedef struct {
    // Configuration
    repl_config_t config;
    repl_callbacks_t callbacks;
    
    // Node management
    repl_node_t *nodes;
    uint32_t node_count;
    uint32_t node_id;  // This node's ID
    
    // WAL
    wal_t wal;
    pending_entry_t *pending_head;
    pending_entry_t *pending_tail;
    uint32_t pending_count;
    
    // Network state
    int server_socket;
    struct pollfd *poll_fds;
    int poll_count;
    
    // Threading
    pthread_t main_thread;
    pthread_mutex_t mutex;
    int running;
} repl_state;

// Global state
static repl_state rs = {0};
static pthread_mutex_t g_mutex = PTHREAD_MUTEX_INITIALIZER;

// Forward declarations
static int wal_init(wal_t *wal, const char *dir);
static int wal_append_entry(wal_t *wal, wal_entry_t *entry);
static int wal_load(wal_t *wal);
static int repl_send_wal_entry(repl_node_t *node, wal_entry_t *entry);
static int repl_send_wal_ack(repl_node_t *node, uint64_t seq_num);
static int repl_handle_wal_entry(repl_node_t *node, const uint8_t *data, uint32_t data_size);
static int repl_handle_wal_ack(repl_node_t *node, const uint8_t *data, uint32_t data_size);
static int repl_handle_node_status(repl_node_t *node, const uint8_t *data, uint32_t data_size);
static int repl_handle_sync_request(repl_node_t *node, const uint8_t *data, uint32_t data_size);
static int repl_handle_sync_response(repl_node_t *node, const uint8_t *data, uint32_t data_size);
static void *repl_main_loop(void *arg);
static int repl_handle_message(int sock);
static repl_node_t *repl_find_node_by_sock(int sock);
static void repl_cleanup_pending_entries(void);
static int repl_process_pending_entries(void);

// Initialize the replication subsystem
int repl_init(repl_config_t *config, repl_callbacks_t *callbacks) {
    if (config == NULL || callbacks == NULL) {
        return -1;
    }
    
    memset(&rs, 0, sizeof(rs));
    
    // Copy config and callbacks
    rs.config = *config;
    rs.callbacks = *callbacks;
    
    // Initialize WAL
    if (wal_init(&rs.wal, config->data_dir) < 0) {
        mfs_log(MFSLOG_SYSLOG,LOG_ERR, "Failed to initialize WAL");
        return -1;
    }
    
    // Initialize mutex
    pthread_mutex_init(&rs.mutex, NULL);
    
    // Initialize node list
    rs.nodes = NULL;
    rs.node_count = 0;
    
    // Initialize pending entries
    rs.pending_head = NULL;
    rs.pending_tail = NULL;
    rs.pending_count = 0;
    
    // Initialize networking
    rs.server_socket = -1;
    rs.poll_fds = NULL;
    rs.poll_count = 0;
    
    return 0;
}

// Add a node to the replication topology
int repl_add_node(uint32_t node_id, const char *hostname, uint16_t port) {
    repl_node_t *node, *curr;
    
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
    node = malloc(sizeof(repl_node_t));
    if (node == NULL) {
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    node->node_id = node_id;
    node->hostname = strdup(hostname);
    node->port = port;
    node->sock = -1;
    node->last_seq_num = 0;
    node->last_ack_seq_num = 0;
    node->status = NODE_STATUS_INACTIVE;
    node->next = rs.nodes;
    rs.nodes = node;
    rs.node_count++;
    
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Remove a node from the replication topology
int repl_remove_node(uint32_t node_id) {
    repl_node_t *prev = NULL, *curr;
    
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
            
            // Close socket if open
            if (curr->sock >= 0) {
                close(curr->sock);
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

// Start the replication subsystem
int repl_start(void) {
    pthread_mutex_lock(&rs.mutex);
    
    if (rs.running) {
        pthread_mutex_unlock(&rs.mutex);
        return 0;  // Already running
    }
    
    // Start main thread
    rs.running = 1;
    if (pthread_create(&rs.main_thread, NULL, repl_main_loop, NULL) != 0) {
        rs.running = 0;
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Stop the replication subsystem
int repl_stop(void) {
    pthread_mutex_lock(&rs.mutex);
    
    if (!rs.running) {
        pthread_mutex_unlock(&rs.mutex);
        return 0;  // Not running
    }
    
    rs.running = 0;
    pthread_mutex_unlock(&rs.mutex);
    
    // Wait for main thread to exit
    pthread_join(rs.main_thread, NULL);
    
    return 0;
}

// Submit an entry to the WAL
int repl_submit_entry(uint8_t *data, uint32_t data_size, consistency_level_t consistency) {
    wal_entry_t entry;
    pending_entry_t *pending;
    
    // Create WAL entry
    entry.seq_num = rs.wal.next_seq_num++;
    entry.timestamp = rs.callbacks.get_timestamp ? rs.callbacks.get_timestamp() : time(NULL);
    entry.node_id = rs.node_id;
    entry.data_size = data_size;
    entry.data = malloc(data_size);
    if (!entry.data) {
        return -1;
    }
    memcpy(entry.data, data, data_size);
    
    pthread_mutex_lock(&rs.mutex);
    
    // Append to WAL
    if (wal_append_entry(&rs.wal, &entry) < 0) {
        free(entry.data);
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    // Create pending entry
    pending = malloc(sizeof(pending_entry_t));
    if (!pending) {
        // Entry is already in WAL, so we don't free entry.data
        pthread_mutex_unlock(&rs.mutex);
        return -1;
    }
    
    pending->entry = entry;
    pending->submit_time = monotonic_useconds();
    pending->consistency = consistency;
    pending->next = NULL;
    
    // Add to pending list
    if (rs.pending_tail) {
        rs.pending_tail->next = pending;
    } else {
        rs.pending_head = pending;
    }
    rs.pending_tail = pending;
    rs.pending_count++;
    
    // Send to all active nodes
    for (repl_node_t *node = rs.nodes; node != NULL; node = node->next) {
        if (node->status == NODE_STATUS_ACTIVE && node->sock >= 0) {
            repl_send_wal_entry(node, &entry);
        }
    }
    
    pthread_mutex_unlock(&rs.mutex);
    
    // If synchronous replication is required, wait for acknowledgments
    if (consistency == CONSISTENCY_STRONG && rs.config.mode == REPL_MODE_SYNC) {
        return repl_sync(rs.config.sync_timeout_ms);
    }
    
    return 0;
}

// Wait for all entries to be replicated (synchronous mode only)
int repl_sync(uint32_t timeout_ms) {
    uint64_t start_time, current_time;
    int result = 0;
    
    // Only applicable in synchronous mode
    if (rs.config.mode != REPL_MODE_SYNC) {
        return 0;
    }
    
    start_time = monotonic_useconds();
    timeout_ms *= 1000;  // Convert to microseconds
    
    while (1) {
        pthread_mutex_lock(&rs.mutex);
        
        // Check if all entries have been acknowledged
        if (rs.pending_head == NULL) {
            pthread_mutex_unlock(&rs.mutex);
            break;
        }
        
        // Check for timeout
        current_time = monotonic_useconds();
        if (timeout_ms > 0 && current_time - start_time >= timeout_ms) {
            result = -1;  // Timeout
            pthread_mutex_unlock(&rs.mutex);
            break;
        }
        
        pthread_mutex_unlock(&rs.mutex);
        
        // Sleep briefly to avoid busy waiting
        usleep(10000);  // 10ms
    }
    
    return result;
}

// Get the current replication lag for a node (in entries)
uint64_t repl_get_lag(uint32_t node_id) {
    uint64_t lag = 0;
    repl_node_t *node;
    
    pthread_mutex_lock(&rs.mutex);
    
    // Find the node
    for (node = rs.nodes; node != NULL; node = node->next) {
        if (node->node_id == node_id) {
            // Calculate lag
            lag = rs.wal.next_seq_num - node->last_ack_seq_num - 1;
            break;
        }
    }
    
    pthread_mutex_unlock(&rs.mutex);
    
    return lag;
}

// Set the replication mode
int repl_set_mode(repl_mode_t mode) {
    pthread_mutex_lock(&rs.mutex);
    rs.config.mode = mode;
    pthread_mutex_unlock(&rs.mutex);
    return 0;
}

// Register a new callback for node state changes
void repl_set_node_state_callback(void (*on_active)(uint32_t), void (*on_inactive)(uint32_t)) {
    pthread_mutex_lock(&rs.mutex);
    rs.callbacks.on_node_active = on_active;
    rs.callbacks.on_node_inactive = on_inactive;
    pthread_mutex_unlock(&rs.mutex);
}

// Clean up resources used by replication
void repl_term(void) {
    repl_node_t *node, *next_node;
    pending_entry_t *pending, *next_pending;
    
    // Stop if running
    repl_stop();
    
    // Clean up nodes
    node = rs.nodes;
    while (node) {
        next_node = node->next;
        if (node->sock >= 0) {
            close(node->sock);
        }
        free(node->hostname);
        free(node);
        node = next_node;
    }
    rs.nodes = NULL;
    
    // Clean up pending entries
    pending = rs.pending_head;
    while (pending) {
        next_pending = pending->next;
        free(pending->entry.data);
        free(pending);
        pending = next_pending;
    }
    rs.pending_head = NULL;
    rs.pending_tail = NULL;
    
    // Close WAL file
    if (rs.wal.wal_fd >= 0) {
        close(rs.wal.wal_fd);
        rs.wal.wal_fd = -1;
    }
    
    // Free paths
    free(rs.wal.wal_path);
    
    // Destroy mutex
    pthread_mutex_destroy(&rs.mutex);
}

// Initialize WAL
static int wal_init(wal_t *wal, const char *dir) {
    // Set WAL path
    if (dir) {
        size_t len = strlen(dir);
        wal->wal_path = malloc(len + 10);
        if (!wal->wal_path) {
            return -1;
        }
        strcpy(wal->wal_path, dir);
        if (dir[len - 1] != '/') {
            strcat(wal->wal_path, "/");
        }
        strcat(wal->wal_path, "repl.wal");
    } else {
        wal->wal_path = strdup("repl.wal");
        if (!wal->wal_path) {
            return -1;
        }
    }
    
    // Open WAL file
    wal->wal_fd = open(wal->wal_path, O_RDWR | O_CREAT, 0644);
    if (wal->wal_fd < 0) {
        free(wal->wal_path);
        wal->wal_path = NULL;
        return -1;
    }
    
    // Initialize sequence numbers
    wal->next_seq_num = 1;
    wal->first_seq_num = 1;
    
    // Load existing WAL
    if (wal_load(wal) < 0) {
        // Failed to load, but we can continue with an empty WAL
        mfs_log(MFSLOG_SYSLOG,LOG_WARNING, "Failed to load WAL, starting fresh");
        
        // Write header for new WAL
        wal_header_t header;
        header.magic = WAL_MAGIC;
        header.version = 1;
        header.first_seq_num = wal->first_seq_num;
        header.entry_count = 0;
        
        if (pwrite(wal->wal_fd, &header, sizeof(header), 0) != sizeof(header)) {
            close(wal->wal_fd);
            wal->wal_fd = -1;
            free(wal->wal_path);
            wal->wal_path = NULL;
            return -1;
        }
    }
    
    return 0;
}

// Load WAL from disk
static int wal_load(wal_t *wal) {
    wal_header_t header;
    ssize_t bytes_read;
    
    // Read header
    bytes_read = pread(wal->wal_fd, &header, sizeof(header), 0);
    if (bytes_read != sizeof(header)) {
        return -1;
    }
    
    // Check magic
    if (header.magic != WAL_MAGIC) {
        return -1;
    }
    
    // Set sequence numbers
    wal->first_seq_num = header.first_seq_num;
    wal->next_seq_num = wal->first_seq_num + header.entry_count;
    
    return 0;
}

// Append an entry to the WAL
static int wal_append_entry(wal_t *wal, wal_entry_t *entry) {
    wal_header_t header;
    ssize_t bytes_read, bytes_written;
    off_t offset;
    uint8_t *buffer;
    uint32_t buffer_size;
    
    // Read header
    bytes_read = pread(wal->wal_fd, &header, sizeof(header), 0);
    if (bytes_read != sizeof(header)) {
        return -1;
    }
    
    // Calculate offset for the new entry
    offset = sizeof(header) + header.entry_count * (sizeof(uint64_t) * 4 + sizeof(uint32_t));
    header.entry_count++;
    
    // Update header
    bytes_written = pwrite(wal->wal_fd, &header, sizeof(header), 0);
    if (bytes_written != sizeof(header)) {
        return -1;
    }
    
    // Write entry metadata
    buffer_size = sizeof(uint64_t) * 3 + sizeof(uint32_t) + entry->data_size;
    buffer = malloc(buffer_size);
    if (!buffer) {
        return -1;
    }
    
    // Pack entry
    uint8_t *ptr = buffer;
    put64bit(&ptr, entry->seq_num);
    put64bit(&ptr, entry->timestamp);
    put32bit(&ptr, entry->node_id);
    put32bit(&ptr, entry->data_size);
    memcpy(ptr, entry->data, entry->data_size);
    
    // Write to WAL
    bytes_written = pwrite(wal->wal_fd, buffer, buffer_size, offset);
    free(buffer);
    
    if (bytes_written != buffer_size) {
        return -1;
    }
    
    // Sync to disk
    fsync(wal->wal_fd);
    
    return 0;
}

// Send a WAL entry to a node
static int repl_send_wal_entry(repl_node_t *node, wal_entry_t *entry) {
    uint8_t *buffer, *ptr;
    uint32_t buffer_size;
    ssize_t bytes_sent;
    
    // Calculate buffer size
    buffer_size = 4 + 1 + 8 + 8 + 4 + 4 + entry->data_size;
    buffer = malloc(buffer_size);
    if (!buffer) {
        return -1;
    }
    
    // Pack message
    ptr = buffer;
    put32bit(&ptr, buffer_size - 4);  // Message size
    put8bit(&ptr, MSG_WAL_ENTRY);     // Message type
    put64bit(&ptr, entry->seq_num);
    put64bit(&ptr, entry->timestamp);
    put32bit(&ptr, entry->node_id);
    put32bit(&ptr, entry->data_size);
    memcpy(ptr, entry->data, entry->data_size);
    
    // Send message
    bytes_sent = write(node->sock, buffer, buffer_size);
    free(buffer);
    
    if (bytes_sent != buffer_size) {
        return -1;
    }
    
    return 0;
}

// Send a WAL acknowledgment to a node
static int repl_send_wal_ack(repl_node_t *node, uint64_t seq_num) {
    uint8_t buffer[4 + 1 + 8], *ptr;
    ssize_t bytes_sent;
    
    // Pack message
    ptr = buffer;
    put32bit(&ptr, sizeof(buffer) - 4);  // Message size
    put8bit(&ptr, MSG_WAL_ACK);          // Message type
    put64bit(&ptr, seq_num);
    
    // Send message
    bytes_sent = write(node->sock, buffer, sizeof(buffer));
    
    if (bytes_sent != sizeof(buffer)) {
        return -1;
    }
    
    return 0;
}

// Handle a received WAL entry
static int repl_handle_wal_entry(repl_node_t *node, const uint8_t *data, uint32_t data_size) {
    wal_entry_t entry;
    const uint8_t *ptr = data;
    int result;
    
    // Unpack entry
    entry.seq_num = get64bit(&ptr);
    entry.timestamp = get64bit(&ptr);
    entry.node_id = get32bit(&ptr);
    entry.data_size = get32bit(&ptr);
    
    // Check data size
    if (entry.data_size > data_size - 16 - 4 - 4) {
        return -1;
    }
    
    // Copy data
    entry.data = malloc(entry.data_size);
    if (!entry.data) {
        return -1;
    }
    memcpy(entry.data, ptr, entry.data_size);
    
    // Update node's last sequence number
    if (entry.seq_num > node->last_seq_num) {
        node->last_seq_num = entry.seq_num;
    }
    
    // Apply the entry
    if (rs.callbacks.apply_entry) {
        result = rs.callbacks.apply_entry(&entry);
    } else {
        result = 0;
    }
    
    // Send acknowledgment
    repl_send_wal_ack(node, entry.seq_num);
    
    // Free data
    free(entry.data);
    
    return result;
}

// Handle a received WAL acknowledgment
static int repl_handle_wal_ack(repl_node_t *node, const uint8_t *data, uint32_t data_size) {
    uint64_t seq_num;
    
    // Check data size
    if (data_size < 8) {
        return -1;
    }
    
    // Unpack sequence number
    seq_num = get64bit(&data);
    
    // Update node's last acknowledged sequence number
    if (seq_num > node->last_ack_seq_num) {
        node->last_ack_seq_num = seq_num;
    }
    
    // Clean up acknowledged entries
    repl_cleanup_pending_entries();
    
    return 0;
}

// Handle a received node status message
static int repl_handle_node_status(repl_node_t *node, const uint8_t *data, uint32_t data_size) {
    node_status_t status;
    
    // Check data size
    if (data_size < 1) {
        return -1;
    }
    
    // Unpack status
    status = get8bit(&data);
    
    // Update node status
    if (status != node->status) {
        node->status = status;
        
        // Notify about status change
        if (status == NODE_STATUS_ACTIVE && rs.callbacks.on_node_active) {
            rs.callbacks.on_node_active(node->node_id);
        } else if (status == NODE_STATUS_INACTIVE && rs.callbacks.on_node_inactive) {
            rs.callbacks.on_node_inactive(node->node_id);
        }
    }
    
    return 0;
}

// Handle a received sync request
static int repl_handle_sync_request(repl_node_t *node, const uint8_t *data, uint32_t data_size) {
    uint64_t from_seq_num;
    
    // Check data size
    if (data_size < 8) {
        return -1;
    }
    
    // Unpack from sequence number
    from_seq_num = get64bit(&data);
    
    // TODO: Send entries from from_seq_num to current
    
    return 0;
}

// Handle a received sync response
static int repl_handle_sync_response(repl_node_t *node, const uint8_t *data, uint32_t data_size) {
    // TODO: Process sync response
    return 0;
}

// Find a node by socket
static repl_node_t *repl_find_node_by_sock(int sock) {
    repl_node_t *node;
    
    for (node = rs.nodes; node != NULL; node = node->next) {
        if (node->sock == sock) {
            return node;
        }
    }
    
    return NULL;
}

// Handle an incoming message
static int repl_handle_message(int sock) {
    repl_node_t *node;
    uint8_t header[5];
    uint8_t *data = NULL;
    uint32_t data_size;
    uint8_t msg_type;
    ssize_t bytes_read;
    int result = -1;
    
    // Find the node
    node = repl_find_node_by_sock(sock);
    if (!node) {
        return -1;
    }
    
    // Read header
    bytes_read = read(sock, header, sizeof(header));
    if (bytes_read != sizeof(header)) {
        return -1;
    }
    
    // Parse header
    const uint8_t *ptr = header;
    data_size = get32bit(&ptr);
    msg_type = get8bit(&ptr);
    
    // Check size
    if (data_size < 1 || data_size > 1024 * 1024) {
        return -1;
    }
    
    // Allocate buffer for data
    data = malloc(data_size);
    if (!data) {
        return -1;
    }
    
    // Read data
    bytes_read = read(sock, data, data_size);
    if (bytes_read != data_size) {
        free(data);
        return -1;
    }
    
    // Handle message based on type
    switch (msg_type) {
        case MSG_WAL_ENTRY:
            result = repl_handle_wal_entry(node, data, data_size);
            break;
        case MSG_WAL_ACK:
            result = repl_handle_wal_ack(node, data, data_size);
            break;
        case MSG_NODE_STATUS:
            result = repl_handle_node_status(node, data, data_size);
            break;
        case MSG_SYNC_REQUEST:
            result = repl_handle_sync_request(node, data, data_size);
            break;
        case MSG_SYNC_RESPONSE:
            result = repl_handle_sync_response(node, data, data_size);
            break;
        default:
            result = -1;
            break;
    }
    
    free(data);
    return result;
}

// Clean up pending entries that have been acknowledged by all nodes
static void repl_cleanup_pending_entries(void) {
    pending_entry_t *curr, *next;
    int all_acked;
    
    curr = rs.pending_head;
    while (curr) {
        // Check if all nodes have acknowledged this entry
        all_acked = 1;
        for (repl_node_t *node = rs.nodes; node != NULL; node = node->next) {
            if (node->status == NODE_STATUS_ACTIVE && node->last_ack_seq_num < curr->entry.seq_num) {
                all_acked = 0;
                break;
            }
        }
        
        if (!all_acked) {
            break;
        }
        
        // Remove this entry
        next = curr->next;
        if (curr == rs.pending_head) {
            rs.pending_head = next;
        }
        if (curr == rs.pending_tail) {
            rs.pending_tail = NULL;
        }
        
        free(curr->entry.data);
        free(curr);
        rs.pending_count--;
        
        curr = next;
    }
}

// Process pending entries
static int repl_process_pending_entries(void) {
    uint64_t now = monotonic_useconds();
    pending_entry_t *curr;
    
    // Retry sending entries to nodes
    for (repl_node_t *node = rs.nodes; node != NULL; node = node->next) {
        if (node->status != NODE_STATUS_ACTIVE || node->sock < 0) {
            continue;
        }
        
        // Send all pending entries not yet acknowledged by this node
        for (curr = rs.pending_head; curr != NULL; curr = curr->next) {
            if (curr->entry.seq_num > node->last_ack_seq_num) {
                repl_send_wal_entry(node, &curr->entry);
            }
        }
    }
    
    return 0;
}

// Main replication thread loop
static void *repl_main_loop(void *arg) {
    uint64_t last_process_time;
    
    last_process_time = monotonic_useconds();
    
    while (1) {
        pthread_mutex_lock(&rs.mutex);
        
        if (!rs.running) {
            pthread_mutex_unlock(&rs.mutex);
            break;
        }
        
        // Process pending entries
        uint64_t now = monotonic_useconds();
        if (now - last_process_time >= rs.config.sync_interval_ms * 1000) {
            repl_process_pending_entries();
            last_process_time = now;
        }
        
        // TODO: Implement networking
        
        pthread_mutex_unlock(&rs.mutex);
        
        // Sleep a bit to avoid busy waiting
        usleep(10000);  // 10ms
    }
    
    return NULL;
} 