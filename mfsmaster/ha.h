#ifndef HA_H
#define HA_H

typedef struct {
    char hostname[256];
    unsigned short port;
} ha_master_info_t;

int ha_init(void);
void ha_shutdown(void);
int ha_get_state(void);
int ha_is_leader(void);
ha_master_info_t* ha_get_leader(void);

#endif // HA_H
