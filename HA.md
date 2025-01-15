# MooseFS High Availability Master Configuration

This guide explains how to set up MooseFS with multiple master servers in a high-availability configuration using DNS-based discovery and Raft leader leases.

## Overview

The HA setup uses:
- DNS-based master discovery (multiple A records for `mfsmaster`)
- Raft leader leases for safe leader election
- Chunkservers to vote for the leader
- Automatic failover between masters
- Web interface showing master states

## Requirements

- Multiple servers for master nodes (minimum 3 recommended for proper quorum)
- DNS server that supports multiple A records
- Network connectivity between all master nodes

## Configuration Steps

### 1. DNS Configuration

Configure your DNS server to have multiple A records for the hostname `mfsmaster`. For example:

```
mfsmaster.    IN    A    192.168.1.10    ; Master 1
mfsmaster.    IN    A    192.168.1.11    ; Master 2
mfsmaster.    IN    A    192.168.1.12    ; Master 3
```

### 2. Master Configuration

On each master node, edit `/etc/mfs/mfsmaster.cfg`:

```ini
# Enable HA mode
HA_ENABLED = 1

# Lease duration in seconds (default: 2)
LEADER_LEASE_DURATION = 2

# Master-to-master communication port
MASTER_PORT = 9420

# Time in seconds between DNS resolution attempts
DNS_REFRESH_TIME = 5
```

### 3. Starting the Masters

1. First master:
```bash
mfsmaster start
```

2. Second master:
```bash
mfsmaster start
```

3. Third master:
```bash
mfsmaster start
```

The masters will:
1. Discover each other through DNS
2. Elect a leader using the lease system
3. Maintain synchronization

### 4. Monitoring

Access the web interface to monitor master states:
```
http://any-master:9425/
```

The interface shows:
- All discovered masters
- Current state (LEADER/FOLLOWER/CANDIDATE)
- Lease information
- Connection status

### 5. Failover Behavior

#### Automatic Failover
If the leader fails:
1. Leader lease expires
2. Remaining masters detect missing leader
3. New leader election occurs
4. New leader acquires lease
5. Service continues with new leader

#### Manual Failover
To manually failover:
1. Access the current leader's web interface
2. Click "Step Down" in the masters view
3. Current leader will release lease
4. New leader election occurs

### 6. Client Configuration

Clients (mfsmount) will automatically:
1. Resolve mfsmaster hostname
2. Try each master until finding the leader
3. Follow redirects to current leader
4. Reconnect if leader changes

### 7. Best Practices

1. Use odd number of chunkservers (3, 5, etc.) for proper quorum
2. Place masters in different failure domains (racks/datacenters)
3. Ensure reliable network between masters
4. Monitor lease durations and failover events
5. Test failover scenarios periodically

### 8. Troubleshooting

#### No Leader Election
- Check DNS resolution
- Verify network connectivity
- Ensure minimum masters available
- Check logs for lease conflicts

#### Split Brain Prevention
The lease system prevents split-brain by:
- Only allowing one active lease at a time
- Requiring lease expiry before new leader
- Using monotonic clocks for timing
- Maintaining lease history in lease.log

#### Monitoring Lease Health
Watch for:
- Frequent leader changes
- Lease expiration without election
- Network partitions
- Clock synchronization issues

## Technical Details

### Leader Lease Mechanism

The leader lease system:
1. Leader acquires time-based lease
2. Lease renewed periodically
3. Followers track lease expiration
4. New election after lease expiry
5. Prevents multiple leaders

### Master States

Masters can be in one of three states:
- LEADER: Holds active lease, serves requests
- FOLLOWER: Syncs from leader, ready for election
- CANDIDATE: Temporarily during election

### Lease Timing

- Default lease: 2 seconds
- Renewal: Every 1 second
- Expiry: Automatic if no renewal
- Election: After lease expiry

### Data Synchronization

- Leader maintains authoritative metadata
- Followers sync continuously
- Metadata journaled for recovery
- Version vectors track changes