use fuser::{
    FileAttr as FuseFileAttr, FileType as FuseFileType, Filesystem, Request, ReplyAttr,
    ReplyData, ReplyDirectory, ReplyEntry, ReplyOpen, ReplyWrite, ReplyEmpty,
    FUSE_ROOT_ID,
};
use libc::{ENOENT, ENOSYS, EIO, EACCES, EINVAL};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, error, info, warn};

use mooseng_common::types::{
    FileAttr, FileType, InodeId, MFS_ROOT_ID, now_micros,
};
use crate::{
    cache::{ClientCache, DirEntry, DirListing},
    master_client::MasterClient,
    config::ClientConfig,
    error::{ClientError, ClientResult},
};

/// Convert MooseNG FileType to FUSE FileType
fn convert_file_type(file_type: FileType) -> FuseFileType {
    match file_type {
        FileType::File => FuseFileType::RegularFile,
        FileType::Directory => FuseFileType::Directory,
        FileType::Symlink => FuseFileType::Symlink,
        FileType::Fifo => FuseFileType::NamedPipe,
        FileType::BlockDev => FuseFileType::BlockDevice,
        FileType::CharDev => FuseFileType::CharDevice,
        FileType::Socket => FuseFileType::Socket,
        FileType::Trash | FileType::Sustained => FuseFileType::RegularFile,
    }
}

/// Convert MooseNG FileAttr to FUSE FileAttr
fn convert_file_attr(attr: &FileAttr) -> FuseFileAttr {
    let file_type = convert_file_type(attr.file_type);
    
    FuseFileAttr {
        ino: attr.inode,
        size: attr.length,
        blocks: (attr.length + 511) / 512, // 512-byte blocks
        atime: UNIX_EPOCH + Duration::from_micros(attr.atime),
        mtime: UNIX_EPOCH + Duration::from_micros(attr.mtime),
        ctime: UNIX_EPOCH + Duration::from_micros(attr.ctime),
        crtime: UNIX_EPOCH + Duration::from_micros(attr.ctime),
        kind: file_type,
        perm: attr.mode,
        nlink: attr.nlink,
        uid: attr.uid,
        gid: attr.gid,
        rdev: 0, // TODO: Handle device files
        blksize: 4096,
        flags: 0,
    }
}

/// Request types for async operations
#[derive(Debug)]
enum FuseRequest {
    Lookup {
        parent: u64,
        name: String,
        reply: oneshot::Sender<ClientResult<(InodeId, FileAttr)>>,
    },
    GetAttr {
        inode: u64,
        reply: oneshot::Sender<ClientResult<FileAttr>>,
    },
    ReadDir {
        inode: u64,
        reply: oneshot::Sender<ClientResult<Vec<DirEntry>>>,
    },
    Read {
        inode: u64,
        offset: u64,
        size: u32,
        reply: oneshot::Sender<ClientResult<Vec<u8>>>,
    },
    Write {
        inode: u64,
        offset: u64,
        data: Vec<u8>,
        reply: oneshot::Sender<ClientResult<u32>>,
    },
    Create {
        parent: u64,
        name: String,
        mode: u32,
        flags: u32,
        reply: oneshot::Sender<ClientResult<(InodeId, FileAttr, u64)>>,
    },
    Unlink {
        parent: u64,
        name: String,
        reply: oneshot::Sender<ClientResult<()>>,
    },
}

/// MooseNG FUSE filesystem implementation
pub struct MooseFuse {
    /// Master client for communication
    master_client: Arc<RwLock<MasterClient>>,
    
    /// Client-side cache
    cache: Arc<ClientCache>,
    
    /// Configuration
    config: Arc<ClientConfig>,
    
    /// Channel for sending requests to async worker
    request_tx: mpsc::UnboundedSender<FuseRequest>,
}

impl MooseFuse {
    /// Create a new MooseFuse instance
    pub async fn new(config: ClientConfig) -> ClientResult<Self> {
        // Connect to master server
        let session_id = 1; // TODO: Implement proper session management
        let master_client = MasterClient::connect(config.master_addr, session_id).await?;
        
        // Initialize cache
        let cache = Arc::new(ClientCache::new(
            config.cache.metadata_cache_size,
            config.cache.metadata_cache_ttl,
            config.cache.dir_cache_size,
            config.cache.dir_cache_ttl,
            config.cache.data_cache_size,
            config.cache.data_cache_ttl,
            config.cache.enable_negative_cache,
        ));
        
        // Create channel for async requests
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<FuseRequest>();
        
        // Start async worker
        let master_client = Arc::new(RwLock::new(master_client));
        let worker_master = master_client.clone();
        let worker_cache = cache.clone();
        let worker_config = Arc::new(config.clone());
        
        tokio::spawn(async move {
            while let Some(request) = request_rx.recv().await {
                match request {
                    FuseRequest::Lookup { parent, name, reply } => {
                        let result = Self::lookup_cached_async(
                            &worker_master,
                            &worker_cache,
                            parent,
                            &name,
                        ).await;
                        let _ = reply.send(result);
                    }
                    FuseRequest::GetAttr { inode, reply } => {
                        let result = Self::get_attr_cached_async(
                            &worker_master,
                            &worker_cache,
                            inode,
                        ).await;
                        let _ = reply.send(result);
                    }
                    FuseRequest::ReadDir { inode, reply } => {
                        let result = Self::readdir_cached_async(
                            &worker_master,
                            &worker_cache,
                            inode,
                        ).await;
                        let _ = reply.send(result);
                    }
                    FuseRequest::Read { inode, offset, size, reply } => {
                        let mut master = worker_master.write().await;
                        let result = master.read(inode, offset, size).await;
                        let _ = reply.send(result);
                    }
                    FuseRequest::Write { inode, offset, data, reply } => {
                        let mut master = worker_master.write().await;
                        let result = master.write(inode, offset, &data).await;
                        drop(master);
                        if result.is_ok() {
                            worker_cache.invalidate_data(inode).await;
                            worker_cache.invalidate_attr(inode).await;
                        }
                        let _ = reply.send(result);
                    }
                    FuseRequest::Create { parent, name, mode, flags, reply } => {
                        let mut master = worker_master.write().await;
                        let result = master.create(parent, &name, mode, flags).await;
                        drop(master);
                        if let Ok((inode, attr, _)) = &result {
                            worker_cache.put_attr(*inode, attr.clone()).await;
                            worker_cache.invalidate_dir(parent).await;
                            worker_cache.remove_negative(parent, &name).await;
                        }
                        let _ = reply.send(result);
                    }
                    FuseRequest::Unlink { parent, name, reply } => {
                        let mut master = worker_master.write().await;
                        let result = master.unlink(parent, &name).await;
                        drop(master);
                        if result.is_ok() {
                            worker_cache.invalidate_dir(parent).await;
                        }
                        let _ = reply.send(result);
                    }
                }
            }
        });
        
        info!("MooseFuse initialized with master at {}", config.master_addr);
        
        Ok(Self {
            master_client,
            cache,
            config: Arc::new(config),
            request_tx,
        })
    }
    
    /// Get file attributes with caching
    async fn get_attr_cached_async(
        master_client: &Arc<RwLock<MasterClient>>,
        cache: &Arc<ClientCache>,
        inode: InodeId,
    ) -> ClientResult<FileAttr> {
        // Check cache first
        if let Some(attr) = cache.get_attr(inode).await {
            debug!("Cache hit for getattr inode {}", inode);
            return Ok(attr);
        }
        
        // Fetch from master
        let mut master = master_client.write().await;
        let attr = master.getattr(inode).await?;
        
        // Cache the result
        cache.put_attr(inode, attr.clone()).await;
        debug!("Cached attr for inode {}", inode);
        
        Ok(attr)
    }
    
    /// Lookup file in directory with caching
    async fn lookup_cached_async(
        master_client: &Arc<RwLock<MasterClient>>,
        cache: &Arc<ClientCache>,
        parent: InodeId,
        name: &str,
    ) -> ClientResult<(InodeId, FileAttr)> {
        // Check negative cache
        if cache.is_negative_cached(parent, name).await {
            debug!("Negative cache hit for lookup {}/{}", parent, name);
            return Err(ClientError::FileNotFound(parent));
        }
        
        // Check if we have a cached directory listing
        if let Some(dir_listing) = cache.get_dir(parent).await {
            if let Some(entry) = dir_listing.entries.iter().find(|e| e.name == name) {
                // Get attributes for the found inode
                let attr = Self::get_attr_cached_async(master_client, cache, entry.inode).await?;
                debug!("Directory cache hit for lookup {}/{}", parent, name);
                return Ok((entry.inode, attr));
            }
        }
        
        // Fetch from master
        let mut master = master_client.write().await;
        match master.lookup(parent, name).await {
            Ok((inode, attr)) => {
                // Cache the attributes
                cache.put_attr(inode, attr.clone()).await;
                debug!("Lookup successful for {}/{} -> {}", parent, name, inode);
                Ok((inode, attr))
            }
            Err(ClientError::FileNotFound(_)) => {
                // Add to negative cache
                cache.add_negative(parent, name.to_string()).await;
                debug!("Added to negative cache: {}/{}", parent, name);
                Err(ClientError::FileNotFound(parent))
            }
            Err(e) => Err(e),
        }
    }
    
    /// Read directory with caching
    async fn readdir_cached_async(
        master_client: &Arc<RwLock<MasterClient>>,
        cache: &Arc<ClientCache>,
        inode: InodeId,
    ) -> ClientResult<Vec<DirEntry>> {
        // Check cache first
        if let Some(dir_listing) = cache.get_dir(inode).await {
            debug!("Directory cache hit for readdir {}", inode);
            return Ok(dir_listing.entries);
        }
        
        // Fetch from master
        let mut master = master_client.write().await;
        let entries = master.readdir(inode, 0).await?;
        
        // Convert to DirEntry format
        let dir_entries: Vec<DirEntry> = entries
            .into_iter()
            .map(|(name, inode, file_type)| DirEntry {
                name,
                inode,
                file_type,
            })
            .collect();
        
        // Cache the directory listing
        let dir_listing = DirListing {
            entries: dir_entries.clone(),
            cached_at: std::time::Instant::now(),
        };
        cache.put_dir(inode, dir_listing).await;
        debug!("Cached directory listing for inode {}", inode);
        
        Ok(dir_entries)
    }
}

impl Filesystem for MooseFuse {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        info!("FUSE filesystem initialized");
        Ok(())
    }
    
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = match name.to_str() {
            Some(name) => name,
            None => {
                reply.error(EINVAL);
                return;
            }
        };
        
        debug!("lookup: parent={}, name={}", parent, name);
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::Lookup {
            parent,
            name: name.to_string(),
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send lookup request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok((inode, attr))) => {
                let fuse_attr = convert_file_attr(&attr);
                let ttl = Duration::from_secs(1);
                reply.entry(&ttl, &fuse_attr, 0);
            }
            Ok(Err(ClientError::FileNotFound(_))) => {
                reply.error(ENOENT);
            }
            Ok(Err(e)) => {
                error!("lookup error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Lookup request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr: ino={}", ino);
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::GetAttr {
            inode: ino,
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send getattr request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(attr)) => {
                let fuse_attr = convert_file_attr(&attr);
                let ttl = Duration::from_secs(1);
                reply.attr(&ttl, &fuse_attr);
            }
            Ok(Err(ClientError::FileNotFound(_))) => {
                reply.error(ENOENT);
            }
            Ok(Err(e)) => {
                error!("getattr error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("GetAttr request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        debug!("readdir: ino={}, offset={}", ino, offset);
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::ReadDir {
            inode: ino,
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send readdir request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(entries)) => {
                let mut i = offset;
                
                // Add "." and ".." entries
                if i == 0 {
                    if reply.add(ino, 1, FuseFileType::Directory, ".") {
                        reply.ok();
                        return;
                    }
                    i = 1;
                }
                
                if i == 1 {
                    let parent_ino = if ino == MFS_ROOT_ID { MFS_ROOT_ID } else { ino }; // TODO: Get actual parent
                    if reply.add(parent_ino, 2, FuseFileType::Directory, "..") {
                        reply.ok();
                        return;
                    }
                    i = 2;
                }
                
                // Add actual directory entries
                for (idx, entry) in entries.iter().enumerate().skip((i - 2) as usize) {
                    let file_type = convert_file_type(entry.file_type);
                    if reply.add(entry.inode, (idx as i64) + 3, file_type, &entry.name) {
                        break;
                    }
                }
                
                reply.ok();
            }
            Ok(Err(e)) => {
                error!("readdir error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("ReadDir request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        debug!("open: ino={}, flags={}", ino, flags);
        
        // For now, just check if the file exists
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::GetAttr {
            inode: ino,
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send open request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(_attr)) => {
                // TODO: Implement proper file handle management
                let fh = ino; // Use inode as file handle for now
                reply.opened(fh, 0);
            }
            Ok(Err(ClientError::FileNotFound(_))) => {
                reply.error(ENOENT);
            }
            Ok(Err(e)) => {
                error!("open error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Open request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        debug!("read: ino={}, offset={}, size={}", ino, offset, size);
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::Read {
            inode: ino,
            offset: offset as u64,
            size,
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send read request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(data)) => {
                reply.data(&data);
            }
            Ok(Err(e)) => {
                error!("read error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Read request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let name = match name.to_str() {
            Some(name) => name,
            None => {
                reply.error(EINVAL);
                return;
            }
        };
        
        debug!("create: parent={}, name={}, mode={}, flags={}", parent, name, mode, flags);
        
        if self.config.read_only {
            reply.error(EACCES);
            return;
        }
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::Create {
            parent,
            name: name.to_string(),
            mode,
            flags: flags as u32,
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send create request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok((_, _, fh))) => {
                reply.opened(fh, 0);
            }
            Ok(Err(e)) => {
                error!("create error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Create request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        debug!("write: ino={}, offset={}, size={}", ino, offset, data.len());
        
        if self.config.read_only {
            reply.error(EACCES);
            return;
        }
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::Write {
            inode: ino,
            offset: offset as u64,
            data: data.to_vec(),
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send write request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(written)) => {
                reply.written(written);
            }
            Ok(Err(e)) => {
                error!("write error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Write request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name = match name.to_str() {
            Some(name) => name,
            None => {
                reply.error(EINVAL);
                return;
            }
        };
        
        debug!("unlink: parent={}, name={}", parent, name);
        
        if self.config.read_only {
            reply.error(EACCES);
            return;
        }
        
        let (tx, rx) = oneshot::channel();
        let request = FuseRequest::Unlink {
            parent,
            name: name.to_string(),
            reply: tx,
        };
        
        if self.request_tx.send(request).is_err() {
            error!("Failed to send unlink request");
            reply.error(EIO);
            return;
        }
        
        match rx.blocking_recv() {
            Ok(Ok(_)) => {
                reply.ok();
            }
            Ok(Err(e)) => {
                error!("unlink error: {}", e);
                reply.error(e.to_errno());
            }
            Err(_) => {
                error!("Unlink request failed: channel closed");
                reply.error(EIO);
            }
        }
    }
    
    // Placeholder implementations for other operations
    fn mkdir(&mut self, _req: &Request, _parent: u64, _name: &OsStr, _mode: u32, _umask: u32, reply: ReplyEntry) {
        warn!("mkdir not implemented yet");
        reply.error(ENOSYS);
    }
    
    fn rmdir(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        warn!("rmdir not implemented yet");
        reply.error(ENOSYS);
    }
    
    fn rename(&mut self, _req: &Request, _parent: u64, _name: &OsStr, _newparent: u64, _newname: &OsStr, _flags: u32, reply: ReplyEmpty) {
        warn!("rename not implemented yet");
        reply.error(ENOSYS);
    }
    
    fn setattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        warn!("setattr not implemented yet");
        reply.error(ENOSYS);
    }
}