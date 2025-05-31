use anyhow::{anyhow, Result};
use dashmap::DashMap;
use mooseng_common::types::{InodeId, SessionId, SessionInfo, now_micros};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};

/// Authentication token
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token: String,
    pub session_id: SessionId,
    pub expires_at: u64,
}

/// User information
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub uid: u32,
    pub gid: u32,
    pub groups: Vec<u32>,
    pub username: String,
}

/// Session state
#[derive(Debug, Clone)]
pub struct Session {
    pub info: SessionInfo,
    pub user: UserInfo,
    pub auth_token: AuthToken,
    pub created_at: u64,
}

/// File handle information
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub file_handle: u64,
    pub inode: InodeId,
    pub session_id: SessionId,
    pub flags: u32,
    pub created_at: u64,
}

/// Manages client sessions and authentication
pub struct SessionManager {
    sessions: Arc<DashMap<SessionId, Session>>,
    tokens: Arc<DashMap<String, SessionId>>,
    file_handles: Arc<DashMap<u64, FileHandle>>,
    next_session_id: AtomicU64,
    next_file_handle: AtomicU64,
    session_timeout: Duration,
}

impl SessionManager {
    pub fn new(session_timeout_ms: u64, _max_clients: usize) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            tokens: Arc::new(DashMap::new()),
            file_handles: Arc::new(DashMap::new()),
            next_session_id: AtomicU64::new(1),
            next_file_handle: AtomicU64::new(1),
            session_timeout: Duration::from_millis(session_timeout_ms),
        }
    }
    
    /// Start session cleanup task
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let sessions = self.sessions.clone();
        let tokens = self.tokens.clone();
        let timeout = self.session_timeout;
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));
            
            loop {
                cleanup_interval.tick().await;
                
                let now = now_micros();
                let timeout_micros = timeout.as_micros() as u64;
                let mut expired = Vec::new();
                
                // Find expired sessions
                for entry in sessions.iter() {
                    let session = entry.value();
                    if now - session.info.last_activity > timeout_micros {
                        expired.push(session.info.session_id);
                    }
                }
                
                // Remove expired sessions
                for session_id in expired {
                    if let Some((_, session)) = sessions.remove(&session_id) {
                        tokens.remove(&session.auth_token.token);
                        debug!("Expired session {} for user {}", session_id, session.user.username);
                    }
                }
            }
        })
    }
    
    /// Create a new session with full parameters
    pub async fn create_session(
        &self,
        client_ip: IpAddr,
        mount_point: String,
        user: UserInfo,
        version: u32,
    ) -> Result<(SessionId, String)> {
        let session_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let now = now_micros();
        
        // Generate auth token
        let token = self.generate_token();
        let auth_token = AuthToken {
            token: token.clone(),
            session_id,
            expires_at: now + self.session_timeout.as_micros() as u64,
        };
        
        // Create session info
        let session_info = SessionInfo {
            session_id,
            client_ip,
            mount_point,
            version,
            open_files: vec![],
            locked_chunks: vec![],
            last_activity: now,
            metadata_cache_size: 0,
        };
        
        // Create session
        let session = Session {
            info: session_info,
            user,
            auth_token: auth_token.clone(),
            created_at: now,
        };
        
        // Store session
        self.sessions.insert(session_id, session);
        self.tokens.insert(token.clone(), session_id);
        
        info!("Created session {} for client {}", session_id, client_ip);
        Ok((session_id, token))
    }
    
    /// Create a simple session (for testing/placeholder use)
    pub async fn create_simple_session(&self) -> Result<SessionId> {
        use std::net::Ipv4Addr;
        
        let user = UserInfo {
            uid: 1000,
            gid: 1000,
            groups: vec![1000],
            username: "default".to_string(),
        };
        
        let (session_id, _) = self.create_session(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "/mnt/mooseng".to_string(),
            user,
            1,
        ).await?;
        
        Ok(session_id)
    }
    
    /// Validate session token
    pub async fn validate_token(&self, token: &str) -> Result<SessionId> {
        let session_id = self.tokens.get(token)
            .ok_or_else(|| anyhow!("Invalid token"))?
            .value()
            .clone();
        
        // Check if session exists and not expired
        let mut session = self.sessions.get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        
        let now = now_micros();
        if session.auth_token.expires_at < now {
            // Remove expired session
            drop(session);
            self.sessions.remove(&session_id);
            self.tokens.remove(token);
            return Err(anyhow!("Session expired"));
        }
        
        // Update last activity
        session.info.last_activity = now;
        
        Ok(session_id)
    }
    
    /// Get session info
    pub async fn get_session(&self, session_id: SessionId) -> Result<Session> {
        self.sessions.get(&session_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| anyhow!("Session not found"))
    }
    
    /// Get user info for session
    pub async fn get_user(&self, session_id: SessionId) -> Result<UserInfo> {
        self.sessions.get(&session_id)
            .map(|entry| entry.user.clone())
            .ok_or_else(|| anyhow!("Session not found"))
    }
    
    /// Update session activity
    pub async fn touch_session(&self, session_id: SessionId) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.info.last_activity = now_micros();
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// Add open file to session
    pub async fn add_open_file(&self, session_id: SessionId, inode: InodeId) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            if !session.info.open_files.contains(&inode) {
                session.info.open_files.push(inode);
            }
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// Remove open file from session
    pub async fn remove_open_file(&self, session_id: SessionId, inode: InodeId) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.info.open_files.retain(|&id| id != inode);
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// Add locked chunk to session
    pub async fn add_locked_chunk(&self, session_id: SessionId, chunk_id: u64) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            if !session.info.locked_chunks.contains(&chunk_id) {
                session.info.locked_chunks.push(chunk_id);
            }
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// Remove locked chunk from session
    pub async fn remove_locked_chunk(&self, session_id: SessionId, chunk_id: u64) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.info.locked_chunks.retain(|&id| id != chunk_id);
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// Renew session token
    pub async fn renew_token(&self, session_id: SessionId) -> Result<String> {
        let mut session = self.sessions.get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        
        // Remove old token
        self.tokens.remove(&session.auth_token.token);
        
        // Generate new token
        let new_token = self.generate_token();
        let now = now_micros();
        
        session.auth_token = AuthToken {
            token: new_token.clone(),
            session_id,
            expires_at: now + self.session_timeout.as_micros() as u64,
        };
        session.info.last_activity = now;
        
        // Store new token
        self.tokens.insert(new_token.clone(), session_id);
        
        debug!("Renewed token for session {}", session_id);
        Ok(new_token)
    }
    
    /// Destroy session
    pub async fn destroy_session(&self, session_id: SessionId) -> Result<()> {
        if let Some((_, session)) = self.sessions.remove(&session_id) {
            self.tokens.remove(&session.auth_token.token);
            info!("Destroyed session {} for user {}", session_id, session.user.username);
            Ok(())
        } else {
            Err(anyhow!("Session not found"))
        }
    }
    
    /// List active sessions
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.iter()
            .map(|entry| entry.info.clone())
            .collect()
    }
    
    /// Check permission for operation
    pub async fn check_permission(
        &self,
        session_id: SessionId,
        uid: u32,
        gid: u32,
        required_uid: Option<u32>,
        required_gid: Option<u32>,
    ) -> Result<bool> {
        let session = self.get_session(session_id).await?;
        
        // Root always has permission
        if session.user.uid == 0 {
            return Ok(true);
        }
        
        // Check UID requirement
        if let Some(req_uid) = required_uid {
            if session.user.uid != req_uid {
                return Ok(false);
            }
        }
        
        // Check GID requirement
        if let Some(req_gid) = required_gid {
            if session.user.gid != req_gid && !session.user.groups.contains(&req_gid) {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Cleanup expired sessions
    pub async fn cleanup_expired(&self) {
        let now = now_micros();
        let timeout_micros = self.session_timeout.as_micros() as u64;
        let mut expired = Vec::new();
        
        // Find expired sessions
        for entry in self.sessions.iter() {
            let session = entry.value();
            if now - session.info.last_activity > timeout_micros {
                expired.push(session.info.session_id);
            }
        }
        
        // Remove expired sessions
        for session_id in expired {
            if let Some((_, session)) = self.sessions.remove(&session_id) {
                self.tokens.remove(&session.auth_token.token);
                debug!("Expired session {} for user {}", session_id, session.user.username);
            }
        }
    }
    
    /// Close all active sessions
    pub async fn close_all_sessions(&self) {
        let sessions: Vec<_> = self.sessions.iter().map(|entry| entry.key().clone()).collect();
        for session_id in sessions {
            let _ = self.destroy_session(session_id).await;
        }
        info!("Closed all active sessions");
    }

    /// Get the number of active sessions
    pub async fn get_active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Generate a secure random token
    fn generate_token(&self) -> String {
        use rand::Rng;
        use rand::distributions::Alphanumeric;
        
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }
    
    /// Open a file and create a file handle
    pub async fn open_file(&self, inode: InodeId, flags: u32, session_id: SessionId) -> Result<u64> {
        // Verify session exists
        if !self.sessions.contains_key(&session_id) {
            return Err(anyhow!("Session not found"));
        }
        
        // Generate new file handle
        let file_handle_id = self.next_file_handle.fetch_add(1, Ordering::SeqCst);
        
        let file_handle = FileHandle {
            file_handle: file_handle_id,
            inode,
            session_id,
            flags,
            created_at: now_micros(),
        };
        
        // Store file handle
        self.file_handles.insert(file_handle_id, file_handle);
        
        // Add to session's open files
        self.add_open_file(session_id, inode).await?;
        
        debug!("Opened file handle {} for inode {} in session {}", file_handle_id, inode, session_id);
        Ok(file_handle_id)
    }
    
    /// Close a file handle
    pub async fn close_file(&self, file_handle_id: u64) -> Result<()> {
        if let Some((_, file_handle)) = self.file_handles.remove(&file_handle_id) {
            // Remove from session's open files
            self.remove_open_file(file_handle.session_id, file_handle.inode).await?;
            
            debug!("Closed file handle {} for inode {} in session {}", 
                   file_handle_id, file_handle.inode, file_handle.session_id);
            Ok(())
        } else {
            Err(anyhow!("File handle not found"))
        }
    }
    
    /// Get file handle information
    pub async fn get_file_handle(&self, file_handle_id: u64) -> Result<FileHandle> {
        self.file_handles.get(&file_handle_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| anyhow!("File handle not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    
    #[tokio::test]
    async fn test_session_creation() {
        let manager = SessionManager::new(60000, 1000); // 1 minute timeout, 1000 max clients
        
        let user = UserInfo {
            uid: 1000,
            gid: 1000,
            groups: vec![1000, 100],
            username: "testuser".to_string(),
        };
        
        let (session_id, token) = manager.create_session(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "/mnt/mooseng".to_string(),
            user,
            1,
        ).await.unwrap();
        
        // Validate token
        let validated_id = manager.validate_token(&token).await.unwrap();
        assert_eq!(session_id, validated_id);
        
        // Get session
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.user.username, "testuser");
    }
    
    #[tokio::test]
    async fn test_session_files() {
        let manager = SessionManager::new(60000, 1000);
        
        let user = UserInfo {
            uid: 1000,
            gid: 1000,
            groups: vec![],
            username: "testuser".to_string(),
        };
        
        let (session_id, _) = manager.create_session(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "/mnt".to_string(),
            user,
            1,
        ).await.unwrap();
        
        // Add open files
        manager.add_open_file(session_id, 100).await.unwrap();
        manager.add_open_file(session_id, 200).await.unwrap();
        
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.info.open_files.len(), 2);
        
        // Remove file
        manager.remove_open_file(session_id, 100).await.unwrap();
        
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.info.open_files.len(), 1);
        assert_eq!(session.info.open_files[0], 200);
    }
    
    #[tokio::test]
    async fn test_permission_check() {
        let manager = SessionManager::new(60000, 1000);
        
        // Regular user
        let user = UserInfo {
            uid: 1000,
            gid: 1000,
            groups: vec![1000, 100],
            username: "user".to_string(),
        };
        
        let (session_id, _) = manager.create_session(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "/mnt".to_string(),
            user,
            1,
        ).await.unwrap();
        
        // Should have permission for own UID
        assert!(manager.check_permission(session_id, 1000, 1000, Some(1000), None).await.unwrap());
        
        // Should not have permission for other UID
        assert!(!manager.check_permission(session_id, 2000, 2000, Some(2000), None).await.unwrap());
        
        // Should have permission for group member
        assert!(manager.check_permission(session_id, 1000, 100, None, Some(100)).await.unwrap());
        
        // Root user
        let root = UserInfo {
            uid: 0,
            gid: 0,
            groups: vec![0],
            username: "root".to_string(),
        };
        
        let (root_session, _) = manager.create_session(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "/mnt".to_string(),
            root,
            1,
        ).await.unwrap();
        
        // Root should always have permission
        assert!(manager.check_permission(root_session, 2000, 2000, Some(2000), None).await.unwrap());
    }
}