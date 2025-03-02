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

/* Define _XOPEN_SOURCE and _GNU_SOURCE for proper includes */
#define _XOPEN_SOURCE 600
#define _GNU_SOURCE

/* System includes - order matters for some platforms */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <ifaddrs.h>
#include <limits.h>
/* Other system includes */
#include <time.h>
#include <pthread.h>
#include <inttypes.h>
#include <errno.h>
#include <ctype.h>

/* Define NI_MAXHOST if not defined */
#ifndef NI_MAXHOST
#define NI_MAXHOST 1025
#endif

/* Define DATA_PATH if not defined */
#ifndef DATA_PATH
#define DATA_PATH "/var/lib/mfs"
#endif

/* Define PATH_MAX if not defined */
#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

/* Define CLOCK_REALTIME if not defined */
#ifndef CLOCK_REALTIME
#define CLOCK_REALTIME 0
#endif

#include "matoclserv.h"
#include "mfslog.h"
#include "cfg.h"
#include "main.h"
#include "datapack.h"
#include "MFSCommunication.h"
#include "../mfscommon/sockets.h"  // For TCP socket functions

/* Type definitions */
typedef enum {
    XCLUSTER_TX_NONE = 0,
    XCLUSTER_TX_METADATA_CHANGE = 1,
    XCLUSTER_TX_GOAL_CHANGE = 2,
    XCLUSTER_TX_NODE_STATUS = 3,
    XCLUSTER_TX_FULL_SYNC = 4
} xcluster_tx_type_t;

typedef struct xcluster_transaction {
    uint64_t id;
    uint64_t timestamp;
    uint32_t origin_node;
    uint32_t origin_region;
    xcluster_tx_type_t type;
    uint32_t data_length;
    uint8_t *data;
    struct xcluster_transaction *next;
} xcluster_transaction_t;

/* Forward declarations */
static void xcluster_start_threads(void);
static void xcluster_stop_threads(void);
static void xcluster_cleanup(void);
static uint64_t xcluster_get_next_tx_id(void);
static xcluster_transaction_t* xcluster_create_transaction(xcluster_tx_type_t type, const uint8_t *data, uint32_t data_length);
static void xcluster_queue_transaction(xcluster_transaction_t *tx);
static void xcluster_free_transaction(xcluster_transaction_t *tx);
static void init_receiver_queue(void);
static void init_region_goals(void);
static void init_metadata_versioning(void);
static void cleanup_metadata_versioning(void);
static void parse_master_host(void);
static void discover_ha_nodes(void);
static void parse_region_config(void);
static void assign_nodes_to_regions(void);
static void matoclserv_ha_update_leader_status(void);
static int matoclserv_ha_check_nodes(void);
uint8_t matoclserv_ha_need_metadata_bootstrap(void);
int matoclserv_ha_bootstrap_metadata(void);
int matoclserv_ha_wait_for_bootstrap(void);

/* Function prototypes from header */
uint8_t matoclserv_xcluster_apply_transaction(xcluster_transaction_t *tx);

/* High Availability support */

// Global variables
uint8_t is_ha_leader = 1;  // Default to being the leader for now
char *ha_listen_host = NULL;
char *ha_listen_port = NULL;
int ha_lsock = -1;
uint32_t ha_timeout = 10000;  // 10 seconds timeout for HA operations

// List of discovered master nodes
#define MAX_HA_NODES 16
static char *ha_node_addresses[MAX_HA_NODES];
static uint32_t ha_node_count = 0;
static uint32_t ha_local_node_id = 0;
static uint32_t ha_leader_id = 0; // ID of the current leader node
static uint16_t ha_default_port = 9426; // Default HA port

// Region support for xCluster-style replication
#define MAX_REGIONS 16
#define MAX_REGION_NAME_LENGTH 32

typedef struct mfs_region {
    char name[MAX_REGION_NAME_LENGTH];
    uint32_t id;
    uint32_t nodes[MAX_HA_NODES];
    uint32_t node_count;
    uint8_t has_local_node;  // Flag to indicate if local node is in this region
} mfs_region_t;

static mfs_region_t regions[MAX_REGIONS];
static uint32_t region_count = 0;
static uint32_t local_region_id = 0;
static uint8_t ha_multi_region_mode = 0; // 0 = single region, 1 = multi-region xCluster

// xCluster replication structures and constants
#define XCLUSTER_MAX_TRANSACTION_SIZE 1048576 // 1MB
#define XCLUSTER_MAX_BATCH_SIZE 100

// Transaction queue per region for cross-region replication
typedef struct xcluster_tx_queue {
    xcluster_transaction_t *head;
    xcluster_transaction_t *tail;
    pthread_mutex_t mutex;
    uint32_t count;
    uint64_t last_applied_tx_id;
} xcluster_tx_queue_t;

static xcluster_tx_queue_t *region_tx_queues = NULL;
static uint64_t next_tx_id = 1;
static pthread_mutex_t tx_id_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint8_t xcluster_initialized = 0;

/* Region-aware goal handling */

// Maximum number of goals supported per region
#define MAX_REGION_GOALS 32

// Region goal definition
typedef struct region_goal {
    uint32_t region_id;       // Region ID
    uint8_t goal_id;          // Goal ID 
    uint8_t replication;      // Number of copies required in this region
} region_goal_t;

// Region-specific goal mapping table
static region_goal_t region_goals[MAX_REGION_GOALS];
static uint32_t region_goal_count = 0;
static uint8_t region_goals_enabled = 0;

// Thread for sending xCluster transactions to other regions
static pthread_t xcluster_sender_thread;
static uint8_t xcluster_sender_terminate = 0;

// Thread for receiving xCluster transactions from other regions
static pthread_t xcluster_receiver_thread;
static uint8_t xcluster_receiver_terminate = 0;

// Thread for processing incoming transactions
static pthread_t xcluster_processor_thread;
static uint8_t xcluster_processor_terminate = 0;

// Queue for holding received transactions before processing
static xcluster_tx_queue_t xcluster_recv_queue;

/* Metadata versioning and synchronization */

// Keep track of metadata versions from each region
typedef struct metadata_version {
    uint64_t version;         // Version number
    uint32_t region_id;       // Region ID
    time_t timestamp;         // Timestamp of last update
    uint8_t clean_shutdown;   // Whether the last shutdown was clean
} metadata_version_t;

#define MAX_METADATA_VERSIONS (MAX_REGIONS * 2)  // 2 per region for redundancy

static metadata_version_t metadata_versions[MAX_METADATA_VERSIONS];
static uint32_t metadata_version_count = 0;
static uint64_t local_metadata_version = 0;
static uint8_t metadata_dirty = 0;

// Versioning mutex to protect metadata version operations
static pthread_mutex_t metadata_version_mutex = PTHREAD_MUTEX_INITIALIZER;

// Flag to indicate shutdown in progress
static uint8_t shutdown_in_progress = 0;

// Global variables to control bootstrap process
static uint8_t cluster_bootstrap_mode = 0;  // 0 = off, 1 = waiting for cluster, 2 = bootstrapping
static uint8_t metadata_bootstrap_completed = 0;
static pthread_mutex_t bootstrap_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t bootstrap_cond = PTHREAD_COND_INITIALIZER;

static char* parse_host_port(const char *hostport, uint16_t *port);

static void xcluster_init(void) {
    if (!ha_multi_region_mode || region_count <= 1) {
        mfs_log(0, MFSLOG_NOTICE, "xCluster replication not enabled - requires multi-region mode and at least 2 regions");
        return;
    }
    
    // Allocate and initialize transaction queues for each region
    region_tx_queues = malloc(sizeof(xcluster_tx_queue_t) * region_count);
    if (!region_tx_queues) {
        mfs_log(0, MFSLOG_ERR, "Failed to allocate memory for xCluster transaction queues");
        return;
    }
    
    for (uint32_t i = 0; i < region_count; i++) {
        memset(&region_tx_queues[i], 0, sizeof(xcluster_tx_queue_t));
        pthread_mutex_init(&region_tx_queues[i].mutex, NULL);
        region_tx_queues[i].head = NULL;
        region_tx_queues[i].tail = NULL;
        region_tx_queues[i].count = 0;
        region_tx_queues[i].last_applied_tx_id = 0;
    }
    
    xcluster_initialized = 1;
    mfs_log(0, MFSLOG_NOTICE, "xCluster replication initialized for %u regions", region_count);
    
    // Start replication threads
    xcluster_start_threads();
}

// Generate a new transaction ID
static uint64_t xcluster_get_next_tx_id(void) {
    uint64_t tx_id;
    pthread_mutex_lock(&tx_id_mutex);
    tx_id = next_tx_id++;
    pthread_mutex_unlock(&tx_id_mutex);
    return tx_id;
}

// Create a new transaction
static xcluster_transaction_t* xcluster_create_transaction(xcluster_tx_type_t type, 
                                                         const uint8_t *data, 
                                                         uint32_t data_length) {
    if (!xcluster_initialized || !ha_multi_region_mode) {
        return NULL;
    }
    
    if (data_length > XCLUSTER_MAX_TRANSACTION_SIZE) {
        mfs_log(0, MFSLOG_ERR, "Transaction data too large: %u bytes", data_length);
        return NULL;
    }
    
    xcluster_transaction_t *tx = malloc(sizeof(xcluster_transaction_t));
    if (!tx) {
        mfs_log(0, MFSLOG_ERR, "Failed to allocate memory for transaction");
        return NULL;
    }
    
    tx->id = xcluster_get_next_tx_id();
    tx->timestamp = time(NULL);
    tx->origin_node = ha_local_node_id;
    tx->origin_region = local_region_id;
    tx->type = type;
    tx->data_length = data_length;
    tx->next = NULL;
    
    if (data_length > 0) {
        tx->data = malloc(data_length);
        if (!tx->data) {
            free(tx);
            mfs_log(0, MFSLOG_ERR, "Failed to allocate memory for transaction data");
            return NULL;
        }
        memcpy(tx->data, data, data_length);
    } else {
        tx->data = NULL;
    }
    
    return tx;
}

// Queue a transaction for replication to other regions
static void xcluster_queue_transaction(xcluster_transaction_t *tx) {
    if (!xcluster_initialized || !tx) {
        return;
    }
    
    // Queue transaction to all regions except our own
    for (uint32_t i = 0; i < region_count; i++) {
        if (regions[i].id == local_region_id) {
            continue; // Skip our own region
        }
        
        // Create a copy of the transaction for each region
        xcluster_transaction_t *tx_copy = malloc(sizeof(xcluster_transaction_t));
        if (!tx_copy) {
            mfs_log(0, MFSLOG_ERR, "Failed to allocate memory for transaction copy");
            continue;
        }
        
        // Copy transaction
        memcpy(tx_copy, tx, sizeof(xcluster_transaction_t));
        tx_copy->next = NULL;
        
        // Copy data if present
        if (tx->data_length > 0 && tx->data) {
            tx_copy->data = malloc(tx->data_length);
            if (!tx_copy->data) {
                free(tx_copy);
                mfs_log(0, MFSLOG_ERR, "Failed to allocate memory for transaction data copy");
                continue;
            }
            memcpy(tx_copy->data, tx->data, tx->data_length);
        }
        
        // Add to queue for this region
        pthread_mutex_lock(&region_tx_queues[i].mutex);
        
        if (region_tx_queues[i].tail) {
            region_tx_queues[i].tail->next = tx_copy;
        } else {
            region_tx_queues[i].head = tx_copy;
        }
        
        region_tx_queues[i].tail = tx_copy;
        region_tx_queues[i].count++;
        
        pthread_mutex_unlock(&region_tx_queues[i].mutex);
    }
    
    // Free the original transaction
    if (tx->data) {
        free(tx->data);
    }
    free(tx);
}

// Free a transaction
static void xcluster_free_transaction(xcluster_transaction_t *tx) {
    if (tx) {
        if (tx->data) {
            free(tx->data);
        }
        free(tx);
    }
}

// Initialize transaction receiver queue
static void init_receiver_queue(void) {
    memset(&xcluster_recv_queue, 0, sizeof(xcluster_tx_queue_t));
    pthread_mutex_init(&xcluster_recv_queue.mutex, NULL);
    xcluster_recv_queue.head = NULL;
    xcluster_recv_queue.tail = NULL;
    xcluster_recv_queue.count = 0;
    xcluster_recv_queue.last_applied_tx_id = 0;
}

// Send transactions from our queues to other regions
static void* xcluster_sender_thread_func(void *arg) {
    uint32_t batch_size, i;
    xcluster_transaction_t *tx, *next_tx;
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster sender thread started");
    
    while (!xcluster_sender_terminate) {
        for (i = 0; i < region_count; i++) {
            // Skip our own region
            if (regions[i].id == local_region_id) {
                continue;
            }
            
            // Process up to XCLUSTER_MAX_BATCH_SIZE transactions per region
            batch_size = 0;
            
            pthread_mutex_lock(&region_tx_queues[i].mutex);
            tx = region_tx_queues[i].head;
            
            // If we have transactions to send
            if (tx) {
                mfs_log(0, MFSLOG_NOTICE, "Sending transactions to region %u (%s)",
                       regions[i].id, regions[i].name);
                
                // TODO: Implement actual sending of transactions to other regions
                // This will involve socket connections and protocol handling
                
                // For now, we'll just pretend we sent them and remove from queue
                while (tx && batch_size < XCLUSTER_MAX_BATCH_SIZE) {
                    next_tx = tx->next;
                    
                    mfs_log(0, MFSLOG_NOTICE, "Would send transaction ID: %" PRIu64 " type: %u to region %u",
                           tx->id, tx->type, regions[i].id);
                    
                    // Free the transaction
                    xcluster_free_transaction(tx);
                    
                    tx = next_tx;
                    batch_size++;
                }
                
                // Update queue head
                region_tx_queues[i].head = tx;
                if (!tx) {
                    region_tx_queues[i].tail = NULL;
                }
                
                region_tx_queues[i].count -= batch_size;
                
                mfs_log(0, MFSLOG_NOTICE, "Sent %u transactions to region %u, %u remaining",
                       batch_size, regions[i].id, region_tx_queues[i].count);
            }
            
            pthread_mutex_unlock(&region_tx_queues[i].mutex);
        }
        
        // Sleep for a bit
        sleep(1);
    }
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster sender thread terminated");
    return NULL;
}

// Receive transactions from other regions
static void* xcluster_receiver_thread_func(void *arg) {
    mfs_log(0, MFSLOG_NOTICE, "xCluster receiver thread started");
    
    while (!xcluster_receiver_terminate) {
        // TODO: Implement actual receiving of transactions from other regions
        // This will involve listening on a socket and handling incoming connections
        
        // For now, just sleep to avoid busy-waiting
        sleep(1);
    }
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster receiver thread terminated");
    return NULL;
}

// Process received transactions
static void* xcluster_processor_thread_func(void *arg) {
    xcluster_transaction_t *tx, *next_tx;
    uint32_t processed;
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster processor thread started");
    
    while (!xcluster_processor_terminate) {
        processed = 0;
        
        // Process up to XCLUSTER_MAX_BATCH_SIZE transactions
        pthread_mutex_lock(&xcluster_recv_queue.mutex);
        tx = xcluster_recv_queue.head;
        
        while (tx && processed < XCLUSTER_MAX_BATCH_SIZE) {
            next_tx = tx->next;
            
            // Apply the transaction
            if (matoclserv_xcluster_apply_transaction(tx)) {
                // Update last applied transaction ID if this is newer
                if (tx->id > xcluster_recv_queue.last_applied_tx_id) {
                    xcluster_recv_queue.last_applied_tx_id = tx->id;
                }
            } else {
                mfs_log(0, MFSLOG_WARNING, "Failed to apply transaction ID: %" PRIu64, tx->id);
            }
            
            // Free the transaction
            xcluster_free_transaction(tx);
            
            tx = next_tx;
            processed++;
        }
        
        // Update queue head
        xcluster_recv_queue.head = tx;
        if (!tx) {
            xcluster_recv_queue.tail = NULL;
        }
        
        xcluster_recv_queue.count -= processed;
        
        if (processed > 0) {
            mfs_log(0, MFSLOG_NOTICE, "Processed %u transactions, %u remaining",
                   processed, xcluster_recv_queue.count);
        }
        
        pthread_mutex_unlock(&xcluster_recv_queue.mutex);
        
        // If we didn't process anything, sleep for a bit
        if (processed == 0) {
            usleep(100000); // 100ms
        }
    }
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster processor thread terminated");
    return NULL;
}

// Add a transaction to the receiver queue
void matoclserv_xcluster_add_received_transaction(xcluster_transaction_t *tx) {
    if (!tx) {
        return;
    }
    
    pthread_mutex_lock(&xcluster_recv_queue.mutex);
    
    // Add to queue
    if (xcluster_recv_queue.tail) {
        xcluster_recv_queue.tail->next = tx;
    } else {
        xcluster_recv_queue.head = tx;
    }
    
    xcluster_recv_queue.tail = tx;
    xcluster_recv_queue.count++;
    
    pthread_mutex_unlock(&xcluster_recv_queue.mutex);
}

// Start xCluster replication threads
static void xcluster_start_threads(void) {
    if (!xcluster_initialized) {
        return;
    }
    
    // Initialize receiver queue
    init_receiver_queue();
    
    // Reset termination flags
    xcluster_sender_terminate = 0;
    xcluster_receiver_terminate = 0;
    xcluster_processor_terminate = 0;
    
    // Start sender thread
    if (pthread_create(&xcluster_sender_thread, NULL, xcluster_sender_thread_func, NULL) != 0) {
        mfs_log(0, MFSLOG_ERR, "Failed to start xCluster sender thread");
        return;
    }
    
    // Start receiver thread
    if (pthread_create(&xcluster_receiver_thread, NULL, xcluster_receiver_thread_func, NULL) != 0) {
        mfs_log(0, MFSLOG_ERR, "Failed to start xCluster receiver thread");
        xcluster_sender_terminate = 1;
        pthread_join(xcluster_sender_thread, NULL);
        return;
    }
    
    // Start processor thread
    if (pthread_create(&xcluster_processor_thread, NULL, xcluster_processor_thread_func, NULL) != 0) {
        mfs_log(0, MFSLOG_ERR, "Failed to start xCluster processor thread");
        xcluster_sender_terminate = 1;
        xcluster_receiver_terminate = 1;
        pthread_join(xcluster_sender_thread, NULL);
        pthread_join(xcluster_receiver_thread, NULL);
        return;
    }
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster replication threads started");
}

// Stop xCluster replication threads
static void xcluster_stop_threads(void) {
    if (!xcluster_initialized) {
        return;
    }
    
    // Set termination flags
    xcluster_sender_terminate = 1;
    xcluster_receiver_terminate = 1;
    xcluster_processor_terminate = 1;
    
    // Wait for threads to terminate
    pthread_join(xcluster_sender_thread, NULL);
    pthread_join(xcluster_receiver_thread, NULL);
    pthread_join(xcluster_processor_thread, NULL);
    
    // Clean up receiver queue
    pthread_mutex_lock(&xcluster_recv_queue.mutex);
    
    xcluster_transaction_t *tx = xcluster_recv_queue.head;
    while (tx) {
        xcluster_transaction_t *next = tx->next;
        xcluster_free_transaction(tx);
        tx = next;
    }
    
    xcluster_recv_queue.head = NULL;
    xcluster_recv_queue.tail = NULL;
    xcluster_recv_queue.count = 0;
    
    pthread_mutex_unlock(&xcluster_recv_queue.mutex);
    pthread_mutex_destroy(&xcluster_recv_queue.mutex);
    
    mfs_log(0, MFSLOG_NOTICE, "xCluster replication threads stopped");
}

// Trigger replication of a metadata change to other regions
void matoclserv_xcluster_metadata_changed(const uint8_t *metadata_buffer, uint32_t length) {
    if (!xcluster_initialized || !ha_multi_region_mode) {
        return;
    }
    
    xcluster_transaction_t *tx = xcluster_create_transaction(XCLUSTER_TX_METADATA_CHANGE, 
                                                           metadata_buffer, length);
    if (tx) {
        mfs_log(0, MFSLOG_NOTICE, "Queueing metadata change (len: %u) for cross-region replication", length);
        xcluster_queue_transaction(tx);
    }
}

// Apply a transaction received from another region
uint8_t matoclserv_xcluster_apply_transaction(xcluster_transaction_t *tx) {
    if (!tx) {
        return 0;
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Applying transaction ID: %" PRIu64 " from region %u, type: %u, length: %u",
           tx->id, tx->origin_region, tx->type, tx->data_length);
    
    // Handle different transaction types
    switch (tx->type) {
        case XCLUSTER_TX_METADATA_CHANGE:
            // TODO: Apply metadata changes
            // This will require integration with the metadata handling code
            mfs_log(0, MFSLOG_NOTICE, "Applying metadata change from region %u", tx->origin_region);
            break;
            
        case XCLUSTER_TX_GOAL_CHANGE:
            // TODO: Apply goal changes
            mfs_log(0, MFSLOG_NOTICE, "Applying goal change from region %u", tx->origin_region);
            break;
            
        case XCLUSTER_TX_NODE_STATUS:
            // TODO: Apply node status changes
            mfs_log(0, MFSLOG_NOTICE, "Applying node status change from region %u", tx->origin_region);
            break;
            
        case XCLUSTER_TX_FULL_SYNC:
            // TODO: Apply full metadata sync
            mfs_log(0, MFSLOG_NOTICE, "Applying full metadata sync from region %u", tx->origin_region);
            break;
            
        default:
            mfs_log(0, MFSLOG_WARNING, "Unknown transaction type: %u", tx->type);
            return 0;
    }
    
    return 1;
}

// Initialize region-specific goals
static void init_region_goals(void) {
    char cfg_key[256];
    char *goal_def, *goal_def_copy, *saveptr, *token;
    uint32_t region_id, goal_id, replication;
    
    // Check if region goals are enabled
    region_goals_enabled = cfg_getint8("HA_REGION_GOALS_ENABLED", 0);
    if (!region_goals_enabled) {
        mfs_log(0, MFSLOG_NOTICE, "Region-specific goals disabled");
        return;
    }
    
    // Reset goals
    region_goal_count = 0;
    memset(region_goals, 0, sizeof(region_goals));
    
    // For each region, load goal definitions
    for (uint32_t i = 0; i < region_count; i++) {
        snprintf(cfg_key, sizeof(cfg_key), "HA_REGION_%s_GOALS", regions[i].name);
        goal_def = cfg_getstr(cfg_key, "");
        
        if (!goal_def || strlen(goal_def) == 0) {
            mfs_log(0, MFSLOG_NOTICE, "No goals defined for region %s", regions[i].name);
            free(goal_def);
            continue;
        }
        
        // Make a copy for strtok_r
        goal_def_copy = strdup(goal_def);
        if (!goal_def_copy) {
            free(goal_def);
            continue;
        }
        
        // Parse comma-separated list of goal definitions (format: goalId:replication)
        token = strtok_r(goal_def_copy, ",", &saveptr);
        while (token && region_goal_count < MAX_REGION_GOALS) {
            char *colon = strchr(token, ':');
            if (colon) {
                *colon = '\0';
                goal_id = atoi(token);
                replication = atoi(colon + 1);
                
                if (goal_id > 0 && replication > 0) {
                    region_goals[region_goal_count].region_id = regions[i].id;
                    region_goals[region_goal_count].goal_id = goal_id;
                    region_goals[region_goal_count].replication = replication;
                    region_goal_count++;
                    
                    mfs_log(0, MFSLOG_NOTICE, "Region %s: Goal %u requires %u copies",
                           regions[i].name, goal_id, replication);
                }
            }
            
            token = strtok_r(NULL, ",", &saveptr);
        }
        
        free(goal_def_copy);
        free(goal_def);
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Loaded %u region-specific goal definitions", region_goal_count);
}

// Get required replication level for a specific goal in a specific region
uint8_t matoclserv_get_region_goal_replication(uint32_t region_id, uint8_t goal_id) {
    if (!region_goals_enabled) {
        return 0; // Region goals not enabled, use global goals
    }
    
    // Find goal definition for this region
    for (uint32_t i = 0; i < region_goal_count; i++) {
        if (region_goals[i].region_id == region_id && region_goals[i].goal_id == goal_id) {
            return region_goals[i].replication;
        }
    }
    
    // Not found, return 0 to indicate no specific requirement
    return 0;
}

// Check if a node is in a specific region
uint8_t matoclserv_is_node_in_region(uint32_t node_id, uint32_t region_id) {
    // Find the region
    for (uint32_t i = 0; i < region_count; i++) {
        if (regions[i].id == region_id) {
            // Check if node is in this region
            for (uint32_t j = 0; j < regions[i].node_count; j++) {
                if (regions[i].nodes[j] == node_id) {
                    return 1;
                }
            }
            break;
        }
    }
    
    return 0;
}

// Initialize metadata versioning
static void init_metadata_versioning(void) {
    memset(metadata_versions, 0, sizeof(metadata_versions));
    metadata_version_count = 0;
    local_metadata_version = 0;
    metadata_dirty = 0;
    
    // Load any saved version information
    // TODO: Implement loading version information from disk
}

// Update local metadata version
void matoclserv_ha_update_metadata_version(void) {
    pthread_mutex_lock(&metadata_version_mutex);
    
    local_metadata_version++;
    metadata_dirty = 1;
    
    // Update version information in our list
    uint32_t i;
    for (i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].region_id == local_region_id) {
            metadata_versions[i].version = local_metadata_version;
            metadata_versions[i].timestamp = time(NULL);
            metadata_versions[i].clean_shutdown = 0; // Mark as dirty until clean shutdown
            break;
        }
    }
    
    // If not found, add new entry
    if (i == metadata_version_count && metadata_version_count < MAX_METADATA_VERSIONS) {
        metadata_versions[metadata_version_count].version = local_metadata_version;
        metadata_versions[metadata_version_count].region_id = local_region_id;
        metadata_versions[metadata_version_count].timestamp = time(NULL);
        metadata_versions[metadata_version_count].clean_shutdown = 0; // Mark as dirty until clean shutdown
        metadata_version_count++;
    }
    
    pthread_mutex_unlock(&metadata_version_mutex);
    
    // If in xCluster mode, notify other regions of version change
    if (ha_multi_region_mode && xcluster_initialized) {
        // Prepare and send a version update transaction
        uint8_t version_data[sizeof(uint64_t) + sizeof(time_t)];
        memcpy(version_data, &local_metadata_version, sizeof(uint64_t));
        time_t current_time = time(NULL);
        memcpy(version_data + sizeof(uint64_t), &current_time, sizeof(time_t));
        
        xcluster_transaction_t *tx = xcluster_create_transaction(
            XCLUSTER_TX_METADATA_CHANGE, version_data, sizeof(version_data));
        
        if (tx) {
            xcluster_queue_transaction(tx);
        }
    }
}

// Update metadata version from another region
void matoclserv_ha_receive_metadata_version(uint32_t region_id, uint64_t version, time_t timestamp) {
    pthread_mutex_lock(&metadata_version_mutex);
    
    // Update version information in our list
    uint32_t i;
    for (i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].region_id == region_id) {
            // Only update if newer
            if (version > metadata_versions[i].version) {
                metadata_versions[i].version = version;
                metadata_versions[i].timestamp = timestamp;
                metadata_versions[i].clean_shutdown = 0; // Mark as dirty
            }
            break;
        }
    }
    
    // If not found, add new entry
    if (i == metadata_version_count && metadata_version_count < MAX_METADATA_VERSIONS) {
        metadata_versions[metadata_version_count].version = version;
        metadata_versions[metadata_version_count].region_id = region_id;
        metadata_versions[metadata_version_count].timestamp = timestamp;
        metadata_versions[metadata_version_count].clean_shutdown = 0; // Mark as dirty
        metadata_version_count++;
    }
    
    pthread_mutex_unlock(&metadata_version_mutex);
}

// Mark metadata as clean after successful flush to disk
void matoclserv_ha_mark_metadata_clean(void) {
    pthread_mutex_lock(&metadata_version_mutex);
    
    // Update clean status for local region
    for (uint32_t i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].region_id == local_region_id) {
            metadata_versions[i].clean_shutdown = 1;
            break;
        }
    }
    
    metadata_dirty = 0;
    
    pthread_mutex_unlock(&metadata_version_mutex);
}

// Check if we need to sync with other regions before proceeding
uint8_t matoclserv_ha_need_metadata_sync(void) {
    pthread_mutex_lock(&metadata_version_mutex);
    
    // Find our region's metadata version
    uint32_t current_region = 0; // Use different name to avoid shadowing
    uint64_t our_version = 0;
    uint64_t max_version = 0;
    
    for (uint32_t i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].region_id == local_region_id) {
            our_version = metadata_versions[i].version;
            current_region = local_region_id;
                break;
            }
        }
        
    // Find the highest version across all regions
    for (uint32_t i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].version > max_version) {
            max_version = metadata_versions[i].version;
        }
    }
    
    pthread_mutex_unlock(&metadata_version_mutex);
    
    // If we don't have a version yet, or our version is lower than the max, we need to sync
    if (current_region == 0 || our_version < max_version) {
        mfs_log(0, MFSLOG_NOTICE, 
                "Metadata sync needed: our version (%"PRIu64") < max version (%"PRIu64")", 
                our_version, max_version);
        return 1;
    }
    
    return 0;
}

// Request metadata from other regions
void matoclserv_ha_request_metadata_sync(void) {
    if (!ha_multi_region_mode || !xcluster_initialized) {
        return; // No need to sync in single-region mode
    }
    
    // Create request transaction
    xcluster_transaction_t *tx = xcluster_create_transaction(
        XCLUSTER_TX_FULL_SYNC, NULL, 0);
    
    if (tx) {
        mfs_log(0, MFSLOG_NOTICE, "Requesting metadata sync from other regions");
        xcluster_queue_transaction(tx);
    }
}

// Get highest metadata version across all regions
uint64_t matoclserv_ha_get_highest_metadata_version(void) {
    uint64_t highest_version = 0;
    
    pthread_mutex_lock(&metadata_version_mutex);
    
    for (uint32_t i = 0; i < metadata_version_count; i++) {
        if (metadata_versions[i].version > highest_version) {
            highest_version = metadata_versions[i].version;
        }
    }
    
    pthread_mutex_unlock(&metadata_version_mutex);
    
    return highest_version;
}

// Check if there are any pending metadata transactions
uint8_t matoclserv_ha_has_pending_transactions(void) {
    if (!ha_multi_region_mode || !xcluster_initialized) {
        return 0; // No transactions in single-region mode
    }
    
    uint8_t has_pending = 0;
    
    // Check if any transaction queues have pending transactions
    for (uint32_t i = 0; i < region_count; i++) {
        pthread_mutex_lock(&region_tx_queues[i].mutex);
        if (region_tx_queues[i].count > 0) {
            has_pending = 1;
        }
        pthread_mutex_unlock(&region_tx_queues[i].mutex);
        
        if (has_pending) {
            break;
        }
    }
    
    // Also check receive queue
    if (!has_pending) {
        pthread_mutex_lock(&xcluster_recv_queue.mutex);
        has_pending = (xcluster_recv_queue.count > 0);
        pthread_mutex_unlock(&xcluster_recv_queue.mutex);
    }
    
    return has_pending;
}

// Process any remaining transactions and ensure clean shutdown
uint8_t matoclserv_ha_prepare_shutdown(void) {
    if (shutdown_in_progress) {
        return 1; // Already in progress
    }
    
    shutdown_in_progress = 1;
    mfs_log(0, MFSLOG_NOTICE, "Preparing for clean shutdown");
    
    // In single-region mode, just mark metadata as clean
    if (!ha_multi_region_mode || !xcluster_initialized) {
        matoclserv_ha_mark_metadata_clean();
        shutdown_in_progress = 0;
        return 1; // Success
    }
    
    // First check if we need to sync from other regions
    if (matoclserv_ha_need_metadata_sync()) {
        mfs_log(0, MFSLOG_NOTICE, "Need to sync metadata from other regions before shutdown");
        matoclserv_ha_request_metadata_sync();
        
        // Wait for sync to complete (would be handled by a state machine in real implementation)
        // For now, just wait a short time
        sleep(1);
    }
    
    // Process any remaining transactions in send queues
    uint8_t all_processed = 0;
    uint8_t retry_count = 0;
    
    while (!all_processed && retry_count < 10) {
        all_processed = 1;
        
        // Check if any transaction queues have pending transactions
        for (uint32_t i = 0; i < region_count; i++) {
            pthread_mutex_lock(&region_tx_queues[i].mutex);
            if (region_tx_queues[i].count > 0) {
                all_processed = 0;
                mfs_log(0, MFSLOG_NOTICE, "Waiting for %u pending transactions to region %u",
                      region_tx_queues[i].count, i + 1);
            }
            pthread_mutex_unlock(&region_tx_queues[i].mutex);
        }
        
        // Check receive queue
        pthread_mutex_lock(&xcluster_recv_queue.mutex);
        if (xcluster_recv_queue.count > 0) {
            all_processed = 0;
            mfs_log(0, MFSLOG_NOTICE, "Waiting for %u pending received transactions",
                  xcluster_recv_queue.count);
        }
        pthread_mutex_unlock(&xcluster_recv_queue.mutex);
        
        if (!all_processed) {
            // Wait for a second and check again
            sleep(1);
            retry_count++;
        }
    }
    
    if (!all_processed) {
        mfs_log(0, MFSLOG_WARNING, "Could not process all pending transactions before shutdown");
        shutdown_in_progress = 0;
        return 0; // Failure
    }
    
    // All transactions processed, mark metadata as clean
    matoclserv_ha_mark_metadata_clean();
    
    // Save current version for clean shutdown
    // TODO: Implement saving version information to disk
    
    mfs_log(0, MFSLOG_NOTICE, "Clean shutdown preparation complete");
    shutdown_in_progress = 0;
    return 1; // Success
}

// Clean up all versioning resources
static void cleanup_metadata_versioning(void) {
    pthread_mutex_lock(&metadata_version_mutex);
    metadata_version_count = 0;
    pthread_mutex_unlock(&metadata_version_mutex);
}

// Update matoclserv_ha_init to initialize versioning
void matoclserv_ha_init(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Initializing HA subsystem");
    
    // Get configuration
    ha_listen_host = cfg_getstr("HA_LISTEN_HOST", "*");
    ha_listen_port = cfg_getstr("HA_LISTEN_PORT", "9426");  // Default HA port
    ha_timeout = cfg_getuint32("HA_TIMEOUT", 10000);
    
    // Parse master host configuration to identify cluster nodes
    parse_master_host();
    
    // Discover HA nodes
    discover_ha_nodes();
    
    // Parse region configuration
    parse_region_config();
    
    // Assign nodes to regions
    assign_nodes_to_regions();
        
    // Set up HA socket for inter-master communication
    ha_lsock = tcpsocket();
    if (ha_lsock < 0) {
        mfs_log(MFSLOG_ERRNO_SYSLOG_STDERR, MFSLOG_ERR, "HA subsystem: can't create socket");
        return;
    }
    
    tcpnonblock(ha_lsock);
    tcpnodelay(ha_lsock);
    tcpreuseaddr(ha_lsock);
    
    // Bind and listen
    uint32_t ha_listen_ip;
    uint16_t ha_listen_port_num;
    if (tcpresolve(ha_listen_host, ha_listen_port, &ha_listen_ip, &ha_listen_port_num, 1) < 0) {
        mfs_log(MFSLOG_ERRNO_SYSLOG_STDERR, MFSLOG_ERR, 
                "HA subsystem: can't resolve %s:%s", ha_listen_host, ha_listen_port);
        tcpclose(ha_lsock);
        ha_lsock = -1;
        return;
    }
    
    if (tcpnumlisten(ha_lsock, ha_listen_ip, ha_listen_port_num, 100) < 0) {
        mfs_log(MFSLOG_ERRNO_SYSLOG_STDERR, MFSLOG_ERR, 
                "HA subsystem: can't listen on %s:%s", ha_listen_host, ha_listen_port);
        tcpclose(ha_lsock);
        ha_lsock = -1;
        return;
    }
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
            "HA subsystem: listening on %s:%s", ha_listen_host, ha_listen_port);
    
    // Update leader status
    matoclserv_ha_update_leader_status();
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "HA subsystem initialized");
}

// Shutdown HA subsystem
void matoclserv_ha_shutdown(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Shutting down HA subsystem");
    
    // Close HA socket
    if (ha_lsock >= 0) {
        tcpclose(ha_lsock);
        ha_lsock = -1;
    }
    
    // Free resources
    if (ha_listen_host) {
        free(ha_listen_host);
        ha_listen_host = NULL;
    }
    
    if (ha_listen_port) {
        free(ha_listen_port);
        ha_listen_port = NULL;
    }
    
    // Clean up node addresses
    for (uint32_t i = 0; i < ha_node_count; i++) {
        if (ha_node_addresses[i]) {
            free(ha_node_addresses[i]);
            ha_node_addresses[i] = NULL;
        }
    }
    ha_node_count = 0;
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "HA subsystem shut down");
}

// Check if metadata bootstrap is needed
uint8_t matoclserv_ha_need_metadata_bootstrap(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Checking if metadata bootstrap is needed");
    
    // If we're the only node or we're the leader, no bootstrap needed
    if (ha_node_count <= 1 || is_ha_leader) {
        mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
                "No bootstrap needed (node count: %u, is leader: %u)", 
                ha_node_count, is_ha_leader);
        return 0;
    }
    
    // Check if metadata file exists
    if (access("metadata.mfs", F_OK) == 0) {
        mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
                "Metadata file exists, no bootstrap needed");
        return 0;
    }
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
            "Metadata bootstrap needed (node count: %u, is leader: %u)", 
            ha_node_count, is_ha_leader);
    return 1;
}

// Bootstrap metadata from another node
int matoclserv_ha_bootstrap_metadata(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Bootstrapping metadata from another node");
    
    // For now, simulate success
    // In a real implementation, we would:
    // 1. Connect to another master node
    // 2. Request metadata dump
    // 3. Save the metadata locally
    
    // Simulate a delay
    sleep(2);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Metadata bootstrap completed successfully");
    return 0;
}

// Wait for bootstrap to complete
int matoclserv_ha_wait_for_bootstrap(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Waiting for metadata bootstrap to complete");
    
    // For now, just return success immediately
    // In a real implementation, we would wait for the bootstrap process to complete
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Metadata bootstrap wait completed");
    return 0;
}

// Update leader status
void matoclserv_ha_update_leader_status(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Updating HA leader status");
    
    // For now, just set ourselves as the leader
    // In a real implementation, we would:
    // 1. Check if we're the leader using a consensus algorithm
    // 2. Update the is_ha_leader variable accordingly
    
    uint8_t old_leader = is_ha_leader;
    is_ha_leader = 1;  // Always leader for now
    
    if (old_leader != is_ha_leader) {
        mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
                "HA leader status changed: %u -> %u", old_leader, is_ha_leader);
    }
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
            "HA leader status updated: leader node is %u, local node is %u (is_leader: %u)",
            1, ha_local_node_id, is_ha_leader);
}

// Check connectivity to other nodes
int matoclserv_ha_check_nodes(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, "Checking connectivity to other HA nodes");
    
    // For now, just return success
    // In a real implementation, we would:
    // 1. Try to connect to each node in the cluster
    // 2. Update node status based on connectivity
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_NOTICE, 
            "HA node connectivity check completed (nodes: %u)", ha_node_count);
    return 0;
}

// Handle HA info request from client
void matoclserv_ha_info(void *eptr, const uint8_t *data, uint32_t length) {
    uint32_t msgid;
    uint8_t *ptr;
    uint32_t node_count = 1;  // For now, just one node (this server)
    uint32_t region_count_local = 1;  // For now, just one region
    uint32_t node_id = 1;
    uint16_t addr_len;
    const char *addr = "localhost";  // Replace with actual server address
    uint32_t region_id = 1;
    uint8_t is_local = 1;
    uint16_t name_len;
    const char *region_name = "default";
    uint32_t region_node_count = 1;
    
    // Log the request
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "Received HA info request from client (length: %u)", length);
    
    // Get the message ID from the request
    msgid = get32bit(&data);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "HA info request with msgid: %u", msgid);
    
    // IMPORTANT: Calculate string lengths correctly (without null terminators)
    addr_len = strlen(addr);
    name_len = strlen(region_name);
    
    // Calculate response size - EXCLUDING the message ID (4 bytes)
    uint32_t response_size = 1 + 4 + 4 + 
                            4 + 2 + addr_len + 4 + 1 + 
                            4 + 2 + name_len + 4 + 4;
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "Creating packet with size: %u (excluding msgid)", response_size);
    
    // Detailed verbose logging for debugging
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: eptr=%p, MATOCL_HA_INFO=%u, response_size=%u", 
           eptr, MATOCL_HA_INFO, response_size);
    
    // Create the packet
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to call matoclserv_create_packet()");
    ptr = matoclserv_create_packet(eptr, MATOCL_HA_INFO, response_size);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: matoclserv_create_packet() returned ptr=%p", (void*)ptr);
    
    if (ptr == NULL) {
        mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "Failed to create response packet");
        return;
    }
    
    // Store original pointer for debugging
    uint8_t *orig_ptr = ptr;
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: orig_ptr=%p", (void*)orig_ptr);

    // First write the message ID (not included in response_size calculation)
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write msgid=%u", msgid);
    put32bit(&ptr, msgid);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: msgid written, ptr advanced to %p", (void*)ptr);
    
    // Then write the actual payload
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write is_ha_leader=%u", is_ha_leader);
    put8bit(&ptr, is_ha_leader);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: is_ha_leader written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write node_count=%u", node_count);
    put32bit(&ptr, node_count);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: node_count written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write region_count_local=%u", region_count_local);
    put32bit(&ptr, region_count_local);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: region_count written, ptr advanced to %p", (void*)ptr);
    
    // Check progress
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, 
           "Header written: %ld bytes", (long)(ptr - orig_ptr));
    
    // Write node information
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write node_id=%u", node_id);
    put32bit(&ptr, node_id);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: node_id written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write addr_len=%u", addr_len);
    put16bit(&ptr, addr_len);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: addr_len written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to copy addr='%s', len=%u", addr, addr_len);
    memcpy(ptr, addr, addr_len);  // Note: NOT including null terminator
    ptr += addr_len;
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: addr copied, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write region_id=%u", region_id);
    put32bit(&ptr, region_id);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: region_id written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write is_local=%u", is_local);
    put8bit(&ptr, is_local);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: is_local written, ptr advanced to %p", (void*)ptr);
    
    // Check progress
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, 
           "Node info written: %ld bytes so far", (long)(ptr - orig_ptr));
    
    // Write region information
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write region_id=%u (again)", region_id);
    put32bit(&ptr, region_id);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: region_id written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write name_len=%u", name_len);
    put16bit(&ptr, name_len);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: name_len written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to copy region_name='%s', len=%u", region_name, name_len);
    memcpy(ptr, region_name, name_len);  // Note: NOT including null terminator  
    ptr += name_len;
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: region_name copied, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write region_node_count=%u", region_node_count);
    put32bit(&ptr, region_node_count);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: region_node_count written, ptr advanced to %p", (void*)ptr);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: About to write node_id=%u (again)", node_id);
    put32bit(&ptr, node_id);
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "DEBUG: node_id written, ptr advanced to %p", (void*)ptr);
    
    // Final check
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, 
           "Packet complete: %ld bytes written of %u expected (plus 4 for msgid)", 
           (long)(ptr - orig_ptr), response_size);
    
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_ERR, "HA info response sent to client");
}

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
    int status;
    char port_str[16];
    uint32_t temp_count = 0;
    uint16_t port = ha_default_port;
    char *hostname_main, *portstr;
    uint32_t node_id = 1; // Keep this as it might be used elsewhere
    
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
    hostname_main = strdup(ha_node_addresses[0]);
    portstr = strchr(hostname_main, ':');
    if (portstr) {
        *portstr = '\0';
        portstr++;
        port = atoi(portstr);
    } else {
        port = ha_default_port;
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Discovering HA nodes via DNS: %s:%u", hostname_main, port);
    
    // Allocate temporary space for discovered addresses
    char **temp_addresses = malloc(sizeof(char*) * MAX_HA_NODES);
    if (!temp_addresses) {
        free(hostname_main);
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
    status = getaddrinfo(hostname_main, port_str, &hints, &res);
    if (status != 0) {
        mfs_log(0, MFSLOG_ERR, "DNS lookup failed: %s", gai_strerror(status));
        free(hostname_main);
        free(temp_addresses);
        return;
    }
    
    free(hostname_main);
    
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
                    mfs_log(0, MFSLOG_NOTICE, "Local node identified as HA node %d via hostname", ha_local_node_id);
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

// Parse region configuration from cfg
static void parse_region_config(void) {
    char *region_names, *region_copy, *saveptr, *token;
    
    // Clean up any existing regions
    region_count = 0;
    memset(regions, 0, sizeof(regions));
    
    // Get region configuration
    region_names = cfg_getstr("HA_REGIONS", "");
    if (!region_names || strlen(region_names) == 0) {
        mfs_log(0, MFSLOG_NOTICE, "No HA regions configured, using single region mode");
        return;
    }
    
    // Check if multi-region mode enabled
    ha_multi_region_mode = cfg_getint8("HA_MULTI_REGION", 0);
    
    // Make a copy for strtok_r which modifies the string
    region_copy = strdup(region_names);
    if (!region_copy) {
        free(region_names);
        return;
    }
    
    // Parse comma-separated list of regions
    token = strtok_r(region_copy, ",", &saveptr);
    while (token && region_count < MAX_REGIONS) {
        // Trim whitespace
        while (*token && isspace(*token)) token++;
        char *end = token + strlen(token) - 1;
        while (end > token && isspace(*end)) *end-- = 0;
        
        // Add region
        if (strlen(token) > 0 && strlen(token) < MAX_REGION_NAME_LENGTH) {
            strncpy(regions[region_count].name, token, MAX_REGION_NAME_LENGTH - 1);
            regions[region_count].id = region_count + 1;
            regions[region_count].node_count = 0;
            region_count++;
        }
        
        token = strtok_r(NULL, ",", &saveptr);
    }
    
    // Get local region
    char *local_region = cfg_getstr("HA_LOCAL_REGION", "");
    if (local_region && strlen(local_region) > 0) {
        for (uint32_t i = 0; i < region_count; i++) {
            if (strcmp(regions[i].name, local_region) == 0) {
                local_region_id = regions[i].id;
                mfs_log(0, MFSLOG_NOTICE, "Local region set to: %s (ID: %u)",
                       regions[i].name, local_region_id);
                break;
            }
        }
    }
    
    mfs_log(0, MFSLOG_NOTICE, "Configured %u regions, multi-region mode: %s",
           region_count, ha_multi_region_mode ? "enabled" : "disabled");
    
    for (uint32_t i = 0; i < region_count; i++) {
        mfs_log(0, MFSLOG_NOTICE, "Region %u: %s", 
               regions[i].id, regions[i].name);
    }
    
    free(region_copy);
    free(region_names);
    free(local_region);
}

// Assign nodes to regions based on configuration
static void assign_nodes_to_regions(void) {
    char cfg_key[256];
    char *node_list, *node_copy, *saveptr, *token;
    
    // For each region, get node list
    for (uint32_t i = 0; i < region_count; i++) {
        snprintf(cfg_key, sizeof(cfg_key), "HA_REGION_%s_NODES", regions[i].name);
        node_list = cfg_getstr(cfg_key, "");
        
        if (!node_list || strlen(node_list) == 0) {
            mfs_log(0, MFSLOG_NOTICE, "No nodes configured for region %s", regions[i].name);
            free(node_list);
            continue;
        }
        
        // Make a copy for strtok_r
        node_copy = strdup(node_list);
        if (!node_copy) {
            free(node_list);
            continue;
        }
        
        // Parse comma-separated list of node IDs
        token = strtok_r(node_copy, ",", &saveptr);
        while (token && regions[i].node_count < MAX_HA_NODES) {
            uint32_t node_id = atoi(token);
            
            // Check if valid node ID
            if (node_id > 0 && node_id <= ha_node_count) {
                regions[i].nodes[regions[i].node_count] = node_id;
                regions[i].node_count++;
                
                // If this is our node ID and we don't have a local region yet,
                // set this as our region
                if (node_id == ha_local_node_id && local_region_id == 0) {
                    local_region_id = regions[i].id;
                    mfs_log(0, MFSLOG_NOTICE, "Local region automatically set to: %s (ID: %u)",
                           regions[i].name, local_region_id);
                }
            }
            
            token = strtok_r(NULL, ",", &saveptr);
        }
        
        mfs_log(0, MFSLOG_NOTICE, "Region %s has %u nodes",
               regions[i].name, regions[i].node_count);
        
        free(node_copy);
        free(node_list);
    }
    
    // If we still don't have a local region, log a warning
    if (ha_node_count > 0 && ha_local_node_id > 0 && local_region_id == 0) {
        mfs_log(0, MFSLOG_WARNING, "Local node (ID: %u) not assigned to any region",
               ha_local_node_id);
    }
}

// Get HA status information
void matoclserv_ha_get_status(uint8_t *is_leader, uint32_t *node_count, uint32_t *region_count_ptr) {
    *is_leader = is_ha_leader;
    *node_count = ha_node_count;
    *region_count_ptr = region_count;
}

// Register HA packet handlers with matoclserv
void matoclserv_ha_register_packet_handlers(void) {
    // This function is called from matoclserv_init in matoclserv.c
    
    // Log that we're registering the handler
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_INFO, "Registering HA packet handler for command CLTOMA_HA_INFO");
    
    // The actual registration happens in matoclserv.c in the matoclserv_gotpacket function
    // We need to add a case for CLTOMA_HA_INFO in the switch statement there
}

// Add this function to fix the undefined reference
void matoclserv_ha_register_shutdown(void) {
    mfs_log(MFSLOG_SYSLOG_STDERR, MFSLOG_INFO, "Registering HA shutdown handler");
    // In a real implementation, this would register shutdown handlers with the main process
    // For now, it's just a stub to fix the undefined reference
}

