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

#ifndef _MFS_HA_H_
#define _MFS_HA_H_

#include <inttypes.h>
#include "ha_manager.h"

// Initialize MFS High Availability
int mfs_ha_init(void);

// Start MFS High Availability
int mfs_ha_start(void);

// Stop MFS High Availability
int mfs_ha_stop(void);

// Check if this node is the leader and can service write operations
int mfs_ha_is_leader(void);

// Get current HA status
// Returns:
//   0 - Not in HA mode or inactive
//   1 - Active leader (can accept reads and writes)
//   2 - Active follower (can accept reads only in active-active mode)
int mfs_ha_status(void);

// Set HA configuration
int mfs_ha_set_config(uint32_t node_id, const char *data_dir, uint16_t raft_port,
                      uint16_t repl_port, ha_mode_t mode);

// Intercept metadata changes for replication
int mfs_ha_changelog_interceptor(const char *logstring);

// Handle a MooseFS metadata operation
// op_type: 0 = read, 1 = write
// Returns:
//  0 - Operation allowed
//  1 - Operation redirected to leader
// -1 - Operation not allowed
int mfs_ha_handle_operation(uint8_t op_type);

// Clean up MFS HA resources
void mfs_ha_term(void);

// Initialize mfscgi integration with HA
int mfs_ha_init_cgi(void);

// CGI callback for HA status page
void mfs_ha_status_cgi(void);

// CGI callback for HA admin operations
void mfs_ha_admin_cgi(void);

#endif /* _MFS_HA_H_ */ 