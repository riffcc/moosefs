set(HA_SRCS
    raft.c
    replication.c
    ha_manager.c
    mfs_ha.c
    mfs_ha_cgi.c
    mfs_ha_integration.c
)

set(HA_HDRS
    raft.h
    replication.h
    ha_manager.h
    mfs_ha.h
)

add_library(mfsmaster-ha OBJECT ${HA_SRCS} ${HA_HDRS}) 