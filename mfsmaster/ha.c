#include "ha.h"
#include "mfsmaster.h"

static ha_master_info_t leader_info = {0};

void ha_init() {
    // Initialize HA components
    leader_info.port = 9420;
}

void ha_shutdown() {
    // Clean up HA resources
}

int ha_get_state() {
    // Return current HA state
    return MASTER_STATE_FOLLOWER;
}

int ha_is_leader() {
    return ha_get_state() == MASTER_STATE_LEADER;
}

ha_master_info_t* ha_get_leader() {
    return &leader_info;
}