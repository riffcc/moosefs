# MooseNG Instance Task Delegation

## COORDINATION PROTOCOL
- Each instance should focus ONLY on their assigned files to avoid conflicts
- Update task progress using TaskMaster tools  
- Check this file for coordination updates
- Main coordinator monitors overall integration

## INSTANCE 1: Master Compilation Specialist 🔥 CRITICAL PATH
**Working Directory**: `/home/wings/projects/moosefs/mooseng/mooseng-master/`
**Priority**: URGENT - 126 compilation errors blocking all development

### Immediate Tasks:
1. **Fix CRDT compilation errors** in `src/multiregion/crdt.rs`
   - Type mismatches: `&String` vs `&str` 
   - Serde/Deserialize trait conflicts
   - Generic type constraints

2. **Resolve gRPC service issues** in `src/grpc_services.rs`
   - Unused variable warnings
   - Service implementation conflicts

3. **Fix cache configuration** Serde conflicts
   - DeserializeOwned trait issues
   - Generic type parameter constraints

### Files to Focus On:
- `src/multiregion/crdt.rs` (Priority 1)
- `src/grpc_services.rs` (Priority 2)  
- `src/cache_config.rs` (Priority 3)
- `src/raft/**` (Priority 4)

---

## INSTANCE 2: Infrastructure & Health Specialist
**Working Directory**: `/home/wings/projects/moosefs/mooseng/`
**Focus**: Complete core infrastructure components

### Primary Tasks:
1. **Complete Chunk Server** (`mooseng-chunkserver/src/`)
   - Finish core functionality (Task 3)
   - gRPC server implementation
   - Background verification processes

2. **Health Monitoring Integration** (Task 21)
   - `mooseng-client/src/health_*`
   - `mooseng-metalogger/src/health_*`
   - Health endpoint implementations

### Files to Focus On:
- `mooseng-chunkserver/src/**`
- `mooseng-client/src/health_*.rs`
- `mooseng-metalogger/src/health_*.rs`

---

## INSTANCE 3: CLI & Benchmarking Specialist  
**Working Directory**: `/home/wings/projects/moosefs/mooseng/`
**Focus**: Developer tools and performance measurement

### Primary Tasks:
1. **CLI Tool Completion** (Task 18)
   - `mooseng-cli/src/`
   - gRPC client integration
   - Admin command implementations

2. **Unified Benchmark Suite** (Task 32)
   - `mooseng-benchmarks/src/`
   - Dashboard integration
   - Performance measurement framework

### Files to Focus On:
- `mooseng-cli/src/**`
- `mooseng-benchmarks/src/**`
- `unified_dashboard/**`

---

## CRITICAL SUCCESS CRITERIA

### 🚨 COMPILATION GATE (Instance 1)
**BLOCKER**: Until Instance 1 resolves the 126 master compilation errors, no integration work can proceed.

**Success Metric**: `cargo check --workspace` passes without errors

### 🔧 INFRASTRUCTURE GATE (Instance 2)  
**Target**: Complete core chunk server functionality and health monitoring

**Success Metric**: Chunk server can start, accept connections, and report health

### 🛠️ TOOLING GATE (Instance 3)
**Target**: CLI tools functional and benchmarking framework operational

**Success Metric**: CLI can connect to services and benchmarks can run

---

## COORDINATION CHECKPOINTS

1. **Every 2 hours**: Update task progress in TaskMaster
2. **Daily**: Check INSTANCE_DELEGATION.md for updates
3. **Critical issues**: Update COORDINATION_STATUS.md
4. **Integration ready**: Notify main coordinator

---
*Created: 2025-05-31 by Main Coordinator*
*Status: Active - 3 instances spawned and assigned*