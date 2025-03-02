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

#ifndef _MATOCLSERV_HA_H_
#define _MATOCLSERV_HA_H_

#include <inttypes.h>
#include <time.h>

/* High Availability Functions */

// Initialize HA subsystem
void matoclserv_ha_init(void);

// Shutdown HA subsystem
void matoclserv_ha_shutdown(void);

// Register shutdown handler with main process
void matoclserv_ha_register_shutdown(void);

// Called when HA role changes
void matoclserv_ha_state_changed(uint8_t is_leader);

// Redirect clients to the new leader
void matoclserv_ha_redirect(uint32_t leader_id);

// Get IP and port for a node by ID
uint8_t matoclserv_get_node_ip_port(uint32_t node_id, char **ip_ptr, uint16_t *port_ptr);

// Get HA status information
void matoclserv_ha_get_status(uint8_t *is_leader, uint32_t *node_count, uint32_t *region_count);

// Handle HA info request from client
void matoclserv_ha_info(void *eptr, const uint8_t *data, uint32_t length);

// Get information about a specific node
uint8_t matoclserv_ha_get_node_info(uint32_t node_id, char **node_addr, uint32_t *region_id, char **region_name, uint8_t *is_local);

/* Metadata Versioning */

// Update local metadata version after changes
void matoclserv_ha_update_metadata_version(void);

// Update metadata version from another region
void matoclserv_ha_receive_metadata_version(uint32_t region_id, uint64_t version, time_t timestamp);

// Mark metadata as clean after successful flush to disk
void matoclserv_ha_mark_metadata_clean(void);

// Check if we need to sync with other regions
uint8_t matoclserv_ha_need_metadata_sync(void);

// Request metadata from other regions
void matoclserv_ha_request_metadata_sync(void);

// Get highest metadata version across all regions
uint64_t matoclserv_ha_get_highest_metadata_version(void);

// Check if there are any pending metadata transactions
uint8_t matoclserv_ha_has_pending_transactions(void);

// Process remaining transactions and ensure clean shutdown
uint8_t matoclserv_ha_prepare_shutdown(void);

/* xCluster Functions */

// Transaction type enumeration
typedef enum {
    XCLUSTER_TX_NONE = 0,
    XCLUSTER_TX_METADATA_CHANGE = 1,
    XCLUSTER_TX_GOAL_CHANGE = 2,
    XCLUSTER_TX_NODE_STATUS = 3,
    XCLUSTER_TX_FULL_SYNC = 4
} xcluster_tx_type_t;

// Transaction structure
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

// Trigger replication of a metadata change to other regions
void matoclserv_xcluster_metadata_changed(const uint8_t *metadata_buffer, uint32_t length);

// Apply a transaction received from another region
uint8_t matoclserv_xcluster_apply_transaction(xcluster_transaction_t *tx);

// Add a transaction to the receiver queue
void matoclserv_xcluster_add_received_transaction(xcluster_transaction_t *tx);

/* Region-aware goal handling */

// Get required replication level for a specific goal in a specific region
uint8_t matoclserv_get_region_goal_replication(uint32_t region_id, uint8_t goal_id);

// Check if a node is in a specific region
uint8_t matoclserv_is_node_in_region(uint32_t node_id, uint32_t region_id);

/* Metalogger support */

// Check if node is allowed to act as a metalogger
uint8_t matoclserv_ha_is_metalogger_allowed(void);

// Get list of nodes that may have current metadata
uint32_t matoclserv_ha_get_active_masters(uint32_t *node_ids, uint32_t max_nodes);

// Notify master that a metalogger is connecting
void matoclserv_ha_metalogger_connected(uint32_t metalogger_id);

// Check if this node should serve the metalogger
uint8_t matoclserv_ha_should_serve_metalogger(uint32_t metalogger_id);

// Trigger sending metadata to connected metaloggers
void matoclserv_ha_send_metadata_to_metaloggers(const uint8_t *metadata_buffer, uint32_t length);

// Metadata bootstrap functions
uint8_t matoclserv_ha_need_metadata_bootstrap(void);
int matoclserv_ha_bootstrap_metadata(void);
int matoclserv_ha_wait_for_bootstrap(void);

// Leader detection functions
void matoclserv_ha_update_leader_status(void);
int matoclserv_ha_check_nodes(void);

#endif /* _MATOCLSERV_HA_H_ */ 