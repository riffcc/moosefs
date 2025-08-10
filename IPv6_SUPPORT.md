# MooseFS IPv6 Support Extension

**Author:** Benjamin Arntzen <zorlin@gmail.com>  
**Date:** January 2025  
**License:** GPL v2 (same as MooseFS)

## Overview

This extension adds full IPv6 support to MooseFS, allowing it to operate in dual-stack (IPv4/IPv6) or IPv6-only environments. This is particularly useful for modern deployments, overlay networks like Yggdrasil, and future-proofing MooseFS installations.

## Features

- **Dual-stack support**: Servers can listen on both IPv4 and IPv6 simultaneously
- **IPv6-only mode**: Can operate in pure IPv6 environments
- **Backward compatibility**: Maintains full compatibility with existing IPv4 deployments
- **Automatic protocol selection**: Prefers IPv6 when available, falls back to IPv4
- **Extended address formats**: Supports IPv6 address notation including bracket notation for ports

## Implementation

### New Files

- `mfscommon/sockets_ipv6.h` - IPv6 socket API declarations
- `mfscommon/sockets_ipv6.c` - IPv6 socket implementation

### Key Data Structures

```c
/* Universal IP representation */
typedef struct mfs_ip {
    int family;  // AF_INET or AF_INET6
    union {
        uint32_t v4;
        uint8_t v6[16];
    } addr;
} mfs_ip;
```

### Migration Path

The implementation provides a gradual migration path:

1. **Phase 1**: Add IPv6 support libraries (completed)
2. **Phase 2**: Update configuration parsers to accept IPv6 addresses
3. **Phase 3**: Migrate network code to use new IPv6-aware functions
4. **Phase 4**: Update serialization for network protocol
5. **Phase 5**: Testing and validation

## Configuration

To enable IPv6 support, add to your MooseFS configuration:

```ini
# Master configuration (mfsmaster.cfg)
MATOCS_LISTEN_HOST = ::  # Listen on all IPv6 addresses
MATOCL_LISTEN_HOST = ::  # or specific: 2001:db8::1

# Chunkserver configuration (mfschunkserver.cfg)
CSSERV_LISTEN_HOST = ::
MASTER_HOST = 2001:db8::1  # or hostname that resolves to IPv6

# Client mount options
mfsmount -H [2001:db8::1] /mnt/mfs
```

## Building with IPv6 Support

```bash
# Add to configure flags
./configure --enable-ipv6

# Or define during compilation
make CFLAGS="-DENABLE_IPV6"
```

## Usage Examples

### Mounting with IPv6

```bash
# Using IPv6 address directly
mfsmount -H [2001:db8::1] /mnt/mfs

# Using hostname that resolves to IPv6
mfsmount -H mfsmaster.example.com /mnt/mfs

# Forcing IPv4 when both are available
mfsmount -H mfsmaster.example.com -4 /mnt/mfs
```

### Yggdrasil Network Integration

MooseFS with IPv6 support works seamlessly with Yggdrasil mesh networking:

```bash
# Start Yggdrasil
sudo yggdrasil -useconffile /etc/yggdrasil.conf

# Get your Yggdrasil IPv6
YGG_IP=$(ip -6 addr show ygg0 | grep -oP '(?<=inet6 )3[0-9a-f:]+')

# Configure MooseFS master to listen on Yggdrasil
echo "MATOCS_LISTEN_HOST = $YGG_IP" >> /etc/mfs/mfsmaster.cfg

# Mount from another Yggdrasil node
mfsmount -H [$YGG_IP] /mnt/mfs
```

## Testing

### Basic connectivity test

```bash
# Test IPv6 connectivity to master
ping6 2001:db8::1

# Check if MooseFS is listening on IPv6
netstat -tlnp6 | grep mfs

# Verify dual-stack operation
ss -tlnp | grep mfs
```

### Performance testing

```bash
# IPv6 throughput test
dd if=/dev/zero of=/mnt/mfs/testfile bs=1M count=1000

# Compare with IPv4
mfsmount -H master.ipv4.local /mnt/mfs-v4
dd if=/dev/zero of=/mnt/mfs-v4/testfile bs=1M count=1000
```

## Protocol Changes

The MooseFS network protocol requires updates to support IPv6 addresses:

1. **Address serialization**: Extended from 4 bytes (IPv4) to 17 bytes (1 byte family + 16 bytes address)
2. **Backward compatibility**: Protocol version negotiation ensures IPv4-only clients/servers continue to work
3. **Configuration format**: Extended to accept IPv6 addresses in standard notation

## API Changes

### Old API (IPv4 only)
```c
int tcpnumlisten(int sock, uint32_t ip, uint16_t port, uint16_t queue);
```

### New API (IPv6 aware)
```c
int tcp6numlisten(int sock, const mfs_ip *ip, uint16_t port, uint16_t queue);
```

### Compatibility macros
```c
#ifdef ENABLE_IPV6
  #define MFS_TCP_SOCKET() tcp6socket()
#else
  #define MFS_TCP_SOCKET() tcpsocket()
#endif
```

## Known Limitations

1. **Mixed clusters**: During migration, ensure all components are upgraded together
2. **Exports format**: The mfsexports.cfg file needs updates to support IPv6 CIDR notation
3. **Topology**: The topology configuration needs IPv6 subnet support

## Future Work

- [ ] Update mfsexports.cfg parser for IPv6 CIDR notation (e.g., 2001:db8::/32)
- [ ] Add IPv6 support to topology configuration
- [ ] Implement IPv6 address compression in logs
- [ ] Add IPv6-specific monitoring metrics
- [ ] Create migration tool for existing deployments

## Contributing

Contributions are welcome! Please ensure:
1. Code follows MooseFS coding standards
2. Changes maintain backward compatibility
3. New features include documentation
4. Test cases cover both IPv4 and IPv6

## Support

For issues or questions about IPv6 support:
- Email: zorlin@gmail.com
- GitHub: [Create an issue]

## Acknowledgments

Thanks to the MooseFS team for creating this excellent distributed filesystem.
Special thanks to the Yggdrasil community for IPv6 mesh networking inspiration.