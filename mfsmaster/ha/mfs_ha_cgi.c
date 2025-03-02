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
#include <stdio.h>
#include <unistd.h>
#include <inttypes.h>

#include "mfs_ha.h"
#include "ha_manager.h"
#include "mfslog.h"

// Define LOG_* constants
#define LOG_ERR MFSLOG_ERROR
#define LOG_WARNING MFSLOG_WARNING

// CGI handlers for HA integration
// This is just a stub implementation until we have the proper CGI utilities

// Status callback
void mfs_ha_status_cgi(void) {
    // This function would normally output HTML with HA status
    // Since we don't have cgiutils.h, we're just implementing a stub
    printf("Content-Type: text/html\n\n");
    printf("<html><head><title>MooseFS HA Status</title></head>\n");
    printf("<body><h1>MooseFS High Availability Status</h1>\n");
    
    // Get HA status
    if (!mfs_ha_is_leader()) {
        printf("<p>This node is currently a <strong>FOLLOWER</strong></p>\n");
    } else {
        printf("<p>This node is currently the <strong>LEADER</strong></p>\n");
    }
    
    printf("<p>HA Status: %d</p>\n", mfs_ha_status());
    
    printf("</body></html>\n");
}

// Admin callback
void mfs_ha_admin_cgi(void) {
    // This function would normally process admin commands for HA
    // Since we don't have cgiutils.h, we're just implementing a stub
    printf("Content-Type: text/html\n\n");
    printf("<html><head><title>MooseFS HA Admin</title></head>\n");
    printf("<body><h1>MooseFS High Availability Administration</h1>\n");
    
    // Check for basic auth (would be handled by cgiutils in a real implementation)
    // Just pretend we're authenticated for now
    int authenticated = 1;
    
    if (!authenticated) {
        printf("<p>Authentication required</p>\n");
        return;
    }
    
    // Parse command (would be handled by cgiutils in a real implementation)
    const char *command = getenv("QUERY_STRING");
    if (command == NULL) {
        command = "";
    }
    
    // Process commands
    if (strcmp(command, "switchover") == 0) {
        // Switch leadership to another node
        printf("<p>Initiating leadership transfer...</p>\n");
    } else if (strcmp(command, "status") == 0) {
        // Show status (similar to status page)
        if (!mfs_ha_is_leader()) {
            printf("<p>This node is currently a <strong>FOLLOWER</strong></p>\n");
        } else {
            printf("<p>This node is currently the <strong>LEADER</strong></p>\n");
        }
    } else {
        // Show help
        printf("<p>Available commands:</p>\n");
        printf("<ul>\n");
        printf("<li><a href=\"?status\">status</a> - Show HA status</li>\n");
        printf("<li><a href=\"?switchover\">switchover</a> - Transfer leadership</li>\n");
        printf("</ul>\n");
    }
    
    printf("</body></html>\n");
}

// Initialize HA CGI integration
int mfs_ha_init_cgi(void) {
    // In a real implementation, this would register CGI handlers
    // Since we don't have cgiutils, this is just a stub
    return 0;
} 