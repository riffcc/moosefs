# MooseNG Error Analysis - Session 7

## Quick Error Fix Guide

### Instance 2 - Raft & Multiregion Fixes

#### 1. Missing Imports in raft/mod.rs
```rust
// Add to mooseng-master/src/raft/mod.rs
pub use self::state::{LogCommand, NodeId, LogIndex};
```

#### 2. Lifetime Shadowing
- Remove duplicate `'de` lifetime parameters in serde implementations
- Use the existing lifetime from the impl block

#### 3. Missing Multiregion Modules
Need to create:
- multiregion/crdt.rs
- multiregion/consistency.rs  
- multiregion/placement.rs
- multiregion/replication.rs

### Instance 3 - Client/FUSE Fixes

#### 1. Protocol Mismatches
Update mooseng-protocol/proto/client.proto:
- Add xattrs field to CreateFileRequest and CreateDirectoryRequest
- Fix SetFileAttributesRequest to match implementation

#### 2. Missing Methods
- Use `tokio::io::AsyncSeekExt` for seek operations
- Implement missing FUSE callbacks

### Instance 4 - ChunkServer Fixes

#### 1. Add to Cargo.toml
```toml
rand = "0.8"
```

#### 2. Fix ChunkStorage Trait
Change storage fields from:
```rust
storage: dyn ChunkStorage
```
To:
```rust
storage: Box<dyn ChunkStorage>
```

#### 3. Implement Missing Metrics Methods
In metrics.rs, add:
- record_read()
- record_write()
- record_delete()
- record_checksum_failure()

## Priority Order:
1. Fix import/dependency issues first
2. Fix trait/type issues
3. Implement missing functionality
4. Run integration tests