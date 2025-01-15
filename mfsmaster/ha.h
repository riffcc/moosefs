#ifndef HA_H
#define HA_H

typedef struct {
    char hostname[256];
    unsigned short port;
} ha_master_info_t;

void ha_init();
void ha_shutdown();
int ha_get_state();
int ha_is_leader();
ha_master_info_t* ha_get_leader();

#endif // HA_H