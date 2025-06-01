use tonic::transport::{Channel, Endpoint};
use std::time::Duration;
use tracing::info;
use prost_types;

use mooseng_protocol::{
    MasterServiceClient,
    GetFileInfoRequest,
    ListDirectoryRequest,
    CreateFileRequest,
    DeleteFileRequest,
    CreateDirectoryRequest,
    DeleteDirectoryRequest,
    RenameFileRequest,
    SetFileAttributesRequest,
    FileMetadata,
    FileType as ProtoFileType,
};
use mooseng_common::types::{
    InodeId, FileAttr, SessionId, FileType,
};
use std::net::SocketAddr;
use crate::{ClientError, ClientResult};

/// Client for communicating with MooseNG master server
#[derive(Clone)]
pub struct MasterClient {
    /// gRPC client for master service
    client: MasterServiceClient<Channel>,
    
    /// Session ID for this client
    session_id: SessionId,
}

impl MasterClient {
    /// Connect to master server
    pub async fn connect(master_addr: SocketAddr, session_id: SessionId) -> ClientResult<Self> {
        info!("Connecting to master server at {}", master_addr);
        
        let endpoint = Endpoint::from_shared(format!("http://{}", master_addr))
            .map_err(|e| ClientError::InvalidArgument(format!("Invalid master address: {}", e)))?
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10));
            
        let client = MasterServiceClient::connect(endpoint).await
            .map_err(|e| ClientError::ConnectionError(e))?;
            
        Ok(Self {
            client,
            session_id,
        })
    }
    
    /// Convert protocol FileType to our FileType
    fn convert_file_type(proto_type: i32) -> FileType {
        match ProtoFileType::try_from(proto_type) {
            Ok(ProtoFileType::File) => FileType::File,
            Ok(ProtoFileType::Directory) => FileType::Directory,
            Ok(ProtoFileType::Symlink) => FileType::Symlink,
            Ok(ProtoFileType::BlockDevice) => FileType::BlockDev,
            Ok(ProtoFileType::CharDevice) => FileType::CharDev,
            Ok(ProtoFileType::Fifo) => FileType::Fifo,
            Ok(ProtoFileType::Socket) => FileType::Socket,
            _ => FileType::File,
        }
    }
    
    /// Convert FileMetadata to FileAttr
    fn metadata_to_attr(metadata: &FileMetadata) -> ClientResult<FileAttr> {
        let file_type = Self::convert_file_type(metadata.file_type);
        
        // Convert timestamps
        let atime = metadata.atime.as_ref()
            .map(|t| (t.seconds as u64) * 1_000_000 + (t.nanos as u64) / 1_000)
            .unwrap_or(0);
        let mtime = metadata.mtime.as_ref()
            .map(|t| (t.seconds as u64) * 1_000_000 + (t.nanos as u64) / 1_000)
            .unwrap_or(0);
        let ctime = metadata.ctime.as_ref()
            .map(|t| (t.seconds as u64) * 1_000_000 + (t.nanos as u64) / 1_000)
            .unwrap_or(0);
            
        Ok(FileAttr {
            inode: metadata.inode,
            file_type,
            mode: metadata.mode as u16,
            uid: metadata.uid,
            gid: metadata.gid,
            atime,
            mtime,
            ctime,
            nlink: metadata.nlink,
            length: metadata.size,
            storage_class: Default::default(), // TODO: Convert storage class
        })
    }
    
    /// Get file attributes
    pub async fn getattr(&mut self, inode: InodeId) -> ClientResult<FileAttr> {
        let request = tonic::Request::new(GetFileInfoRequest {
            session_id: self.session_id,
            inode,
        });
        
        let response = self.client.get_file_info(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let info_response = response.into_inner();
        
        if let Some(metadata) = info_response.metadata {
            Self::metadata_to_attr(&metadata)
        } else {
            Err(ClientError::FileNotFound(inode))
        }
    }
    
    /// Lookup a file in a directory
    pub async fn lookup(&mut self, parent: InodeId, name: &str) -> ClientResult<(InodeId, FileAttr)> {
        // List directory and find the entry
        let request = tonic::Request::new(ListDirectoryRequest {
            session_id: self.session_id,
            parent,
            offset: 0,
            limit: 1000, // TODO: Handle pagination properly
        });
        
        let response = self.client.list_directory(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let list_response = response.into_inner();
        
        // Find the entry with matching name
        for entry in list_response.entries {
            if entry.name == name {
                if let Some(metadata) = entry.metadata {
                    let attr = Self::metadata_to_attr(&metadata)?;
                    return Ok((metadata.inode, attr));
                }
            }
        }
        
        Err(ClientError::FileNotFound(parent))
    }
    
    /// Read directory contents
    pub async fn readdir(&mut self, inode: InodeId, offset: u64) -> ClientResult<Vec<(String, InodeId, FileType)>> {
        let request = tonic::Request::new(ListDirectoryRequest {
            session_id: self.session_id,
            parent: inode,
            offset,
            limit: 1000, // TODO: Handle pagination properly
        });
        
        let response = self.client.list_directory(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let list_response = response.into_inner();
        
        let mut entries = Vec::new();
        for entry in list_response.entries {
            if let Some(metadata) = entry.metadata {
                let file_type = Self::convert_file_type(metadata.file_type);
                entries.push((entry.name, metadata.inode, file_type));
            }
        }
        
        Ok(entries)
    }
    
    /// Create a new file
    pub async fn create(&mut self, parent: InodeId, name: &str, mode: u32, _flags: u32) -> ClientResult<(InodeId, FileAttr, u64)> {
        let request = tonic::Request::new(CreateFileRequest {
            session_id: self.session_id,
            parent,
            name: name.to_string(),
            mode,
            storage_class_id: 0, // TODO: Handle storage class properly
            xattrs: Default::default(), // Empty extended attributes
        });
        
        let response = self.client.create_file(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let create_response = response.into_inner();
        
        if let Some(metadata) = create_response.metadata {
            let attr = Self::metadata_to_attr(&metadata)?;
            let fh = metadata.inode; // Use inode as file handle for now
            Ok((metadata.inode, attr, fh))
        } else {
            Err(ClientError::MasterError("Failed to create file".to_string()))
        }
    }
    
    /// Read data from a file
    pub async fn read(&mut self, inode: InodeId, offset: u64, size: u32) -> ClientResult<Vec<u8>> {
        // First get file attributes to check size and validate offset
        let attr = self.getattr(inode).await?;
        
        if offset >= attr.length {
            return Ok(Vec::new()); // EOF
        }
        
        let actual_size = std::cmp::min(size as u64, attr.length - offset) as u32;
        
        // For now, simulate reading data until chunk server integration is complete
        // TODO: Implement proper chunk server communication for reads
        match attr.file_type {
            FileType::File => {
                // Return dummy data pattern for testing
                let mut data = vec![0u8; actual_size as usize];
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte = ((offset + i as u64) % 256) as u8;
                }
                Ok(data)
            }
            _ => Err(ClientError::InvalidArgument("Cannot read from non-file".to_string())),
        }
    }
    
    /// Write data to a file
    pub async fn write(&mut self, inode: InodeId, offset: u64, data: &[u8]) -> ClientResult<u32> {
        // First get file attributes to check permissions and type
        let attr = self.getattr(inode).await?;
        
        match attr.file_type {
            FileType::File => {
                // For now, simulate writing until chunk server integration is complete
                // TODO: Implement proper chunk server communication for writes
                
                // Validate write parameters
                if data.is_empty() {
                    return Ok(0);
                }
                
                // Check for potential overflow
                if offset.saturating_add(data.len() as u64) < offset {
                    return Err(ClientError::InvalidArgument("Write would overflow file size".to_string()));
                }
                
                // Simulate successful write
                Ok(data.len() as u32)
            }
            _ => Err(ClientError::InvalidArgument("Cannot write to non-file".to_string())),
        }
    }
    
    /// Remove a file
    pub async fn unlink(&mut self, parent: InodeId, name: &str) -> ClientResult<()> {
        // First lookup the file to get its inode
        let (inode, _) = self.lookup(parent, name).await?;
        
        let request = tonic::Request::new(DeleteFileRequest {
            session_id: self.session_id,
            inode,
        });
        
        self.client.delete_file(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
            
        Ok(())
    }
    
    /// Create a directory
    pub async fn mkdir(&mut self, parent: InodeId, name: &str, mode: u32) -> ClientResult<(InodeId, FileAttr)> {
        let request = tonic::Request::new(CreateDirectoryRequest {
            session_id: self.session_id,
            parent,
            name: name.to_string(),
            mode,
            storage_class_id: 0, // TODO: Handle storage class properly
            xattrs: Default::default(), // Empty extended attributes
        });
        
        let response = self.client.create_directory(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let create_response = response.into_inner();
        
        if let Some(metadata) = create_response.metadata {
            let attr = Self::metadata_to_attr(&metadata)?;
            Ok((metadata.inode, attr))
        } else {
            Err(ClientError::MasterError("Failed to create directory".to_string()))
        }
    }
    
    /// Remove a directory
    pub async fn rmdir(&mut self, parent: InodeId, name: &str) -> ClientResult<()> {
        // First lookup the directory to get its inode
        let (inode, _) = self.lookup(parent, name).await?;
        
        let request = tonic::Request::new(DeleteDirectoryRequest {
            session_id: self.session_id,
            inode,
            recursive: false,
        });
        
        self.client.delete_directory(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
            
        Ok(())
    }
    
    /// Rename a file or directory
    pub async fn rename(&mut self, parent: InodeId, name: &str, newparent: InodeId, newname: &str) -> ClientResult<()> {
        // First lookup the file to get its inode
        let (inode, _) = self.lookup(parent, name).await?;
        
        let request = tonic::Request::new(RenameFileRequest {
            session_id: self.session_id,
            inode,
            new_parent: newparent,
            new_name: newname.to_string(),
        });
        
        self.client.rename_file(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
            
        Ok(())
    }
    
    /// Set file attributes
    pub async fn setattr(
        &mut self,
        inode: InodeId,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        _size: Option<u64>,
        atime: Option<u64>,
        mtime: Option<u64>,
    ) -> ClientResult<FileAttr> {
        // SetFileAttributesRequest uses path, so we need to construct a path from inode
        // For now, we'll use inode as path - this should be handled properly with a path cache
        let path = format!("/{}", inode); // TODO: Implement proper inode-to-path mapping
        
        // Convert microseconds to google::protobuf::Timestamp
        let atime_timestamp = atime.map(|us| {
            let secs = us / 1_000_000;
            let nanos = ((us % 1_000_000) * 1_000) as i32;
            prost_types::Timestamp {
                seconds: secs as i64,
                nanos,
            }
        });
        
        let mtime_timestamp = mtime.map(|us| {
            let secs = us / 1_000_000;
            let nanos = ((us % 1_000_000) * 1_000) as i32;
            prost_types::Timestamp {
                seconds: secs as i64,
                nanos,
            }
        });
        
        let request = tonic::Request::new(SetFileAttributesRequest {
            path,
            mode,
            uid,
            gid,
            atime: atime_timestamp,
            mtime: mtime_timestamp,
        });
        
        let response = self.client.set_file_attributes(request).await
            .map_err(|e| ClientError::GrpcError(e))?;
        let attr_response = response.into_inner();
        
        if let Some(metadata) = attr_response.metadata {
            Self::metadata_to_attr(&metadata)
        } else {
            Err(ClientError::MasterError("Failed to set attributes".to_string()))
        }
    }
}