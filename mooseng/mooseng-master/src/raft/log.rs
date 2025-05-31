use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write, Seek, SeekFrom};
use std::path::PathBuf;
use tracing::{debug, warn};

use crate::raft::state::{Term, LogIndex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogCommand {
    Noop,
    
    SetMetadata { key: String, value: Vec<u8> },
    
    DeleteMetadata { key: String },
    
    CreateFile { path: String, metadata: Vec<u8> },
    
    DeleteFile { path: String },
    
    UpdateChunkLocation { chunk_id: u64, locations: Vec<String> },
    
    Configuration { members: Vec<String> },
    
    /// Configuration change with joint consensus support
    ConfigChange {
        change_type: crate::raft::membership::ConfigChangeType,
        joint_config: crate::raft::membership::JointConfiguration,
    },
    
    /// Custom command for extensibility
    Custom {
        command_type: String,
        key: String,
        value: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: LogIndex,
    pub term: Term,
    pub command: LogCommand,
    pub timestamp: u64,
}

impl LogEntry {
    pub fn new(index: LogIndex, term: Term, command: LogCommand) -> Self {
        Self {
            index,
            term,
            command,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn new_noop(term: Term) -> Self {
        Self::new(0, term, LogCommand::Noop)
    }
}

pub struct RaftLog {
    log_dir: PathBuf,
    entries: Vec<LogEntry>,
    snapshot_index: LogIndex,
    snapshot_term: Term,
    log_file: File,
}

impl std::fmt::Debug for RaftLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaftLog")
            .field("log_dir", &self.log_dir)
            .field("entries_count", &self.entries.len())
            .field("snapshot_index", &self.snapshot_index)
            .field("snapshot_term", &self.snapshot_term)
            .field("log_file", &"<file>")
            .finish()
    }
}

impl RaftLog {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&log_dir)?;
        
        let log_path = log_dir.join("raft.log");
        let mut entries = Vec::new();
        let mut snapshot_index = 0;
        let mut snapshot_term = 0;
        
        if log_path.exists() {
            let file = File::open(&log_path)?;
            let reader = BufReader::new(file);
            
            if let Ok(loaded) = Self::load_from_reader(reader) {
                entries = loaded.0;
                snapshot_index = loaded.1;
                snapshot_term = loaded.2;
            }
        }
        
        let log_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .append(true)
            .open(&log_path)?;
        
        Ok(Self {
            log_dir,
            entries,
            snapshot_index,
            snapshot_term,
            log_file,
        })
    }
    
    fn load_from_reader(mut reader: BufReader<File>) -> Result<(Vec<LogEntry>, LogIndex, Term)> {
        let mut entries = Vec::new();
        let mut snapshot_index = 0;
        let mut snapshot_term = 0;
        
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        
        if buffer.len() >= 16 {
            snapshot_index = u64::from_le_bytes(buffer[0..8].try_into()?);
            snapshot_term = u64::from_le_bytes(buffer[8..16].try_into()?);
            
            let entries_bytes = &buffer[16..];
            if !entries_bytes.is_empty() {
                entries = bincode::deserialize(entries_bytes)?;
            }
        }
        
        Ok((entries, snapshot_index, snapshot_term))
    }
    
    pub fn append(&mut self, mut entry: LogEntry) -> Result<LogIndex> {
        let index = self.last_index() + 1;
        entry.index = index;
        
        self.entries.push(entry.clone());
        self.persist_entry(&entry)?;
        
        Ok(index)
    }
    
    pub fn append_entries(&mut self, prev_index: LogIndex, new_entries: Vec<LogEntry>) -> Result<LogIndex> {
        let insert_point = if prev_index < self.snapshot_index {
            return Err(anyhow!("Cannot append entries before snapshot"));
        } else if prev_index == self.snapshot_index {
            0
        } else {
            let logical_idx = (prev_index - self.snapshot_index) as usize;
            if logical_idx > self.entries.len() {
                return Err(anyhow!("Gap in log at index {}", prev_index));
            }
            logical_idx
        };
        
        self.entries.truncate(insert_point);
        
        let mut last_index = prev_index;
        for mut entry in new_entries {
            last_index += 1;
            entry.index = last_index;
            self.entries.push(entry.clone());
            self.persist_entry(&entry)?;
        }
        
        Ok(last_index)
    }
    
    pub fn get(&self, index: LogIndex) -> Result<Option<LogEntry>> {
        if index <= self.snapshot_index {
            return Ok(None);
        }
        
        let logical_idx = (index - self.snapshot_index - 1) as usize;
        Ok(self.entries.get(logical_idx).cloned())
    }
    
    pub fn last_index(&self) -> LogIndex {
        if self.entries.is_empty() {
            self.snapshot_index
        } else {
            self.entries.last().unwrap().index
        }
    }
    
    pub fn last_entry_info(&self) -> (LogIndex, Term) {
        if let Some(last) = self.entries.last() {
            (last.index, last.term)
        } else {
            (self.snapshot_index, self.snapshot_term)
        }
    }
    
    pub fn term_at(&self, index: LogIndex) -> Result<Term> {
        if index == self.snapshot_index {
            Ok(self.snapshot_term)
        } else if let Some(entry) = self.get(index)? {
            Ok(entry.term)
        } else {
            Err(anyhow!("No entry at index {}", index))
        }
    }
    
    pub fn matches_at(&self, index: LogIndex, term: Term) -> bool {
        if index == 0 {
            return true;
        }
        
        match self.term_at(index) {
            Ok(t) => t == term,
            Err(_) => false,
        }
    }
    
    pub fn truncate_from(&mut self, index: LogIndex) -> Result<()> {
        if index <= self.snapshot_index {
            return Err(anyhow!("Cannot truncate before snapshot"));
        }
        
        let logical_idx = (index - self.snapshot_index - 1) as usize;
        self.entries.truncate(logical_idx);
        self.persist_all()?;
        
        Ok(())
    }
    
    pub fn create_snapshot(&mut self, index: LogIndex, data: Vec<u8>) -> Result<()> {
        if index <= self.snapshot_index {
            return Err(anyhow!("Snapshot index must be greater than current snapshot"));
        }
        
        let term = self.term_at(index)?;
        
        let snapshot_path = self.log_dir.join(format!("snapshot-{}-{}", term, index));
        std::fs::write(&snapshot_path, data)?;
        
        let old_snapshot_index = self.snapshot_index;
        self.snapshot_index = index;
        self.snapshot_term = term;
        
        self.entries.retain(|e| e.index > index);
        
        self.persist_all()?;
        
        if old_snapshot_index > 0 {
            let old_snapshot = self.log_dir.join(format!("snapshot-*-{}", old_snapshot_index));
            if let Ok(paths) = glob::glob(&old_snapshot.to_string_lossy()) {
                for path in paths.flatten() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        
        Ok(())
    }
    
    pub fn load_snapshot(&self) -> Result<Option<(LogIndex, Term, Vec<u8>)>> {
        let pattern = self.log_dir.join("snapshot-*");
        let mut snapshots = Vec::new();
        
        if let Ok(paths) = glob::glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    if let Some((term_str, index_str)) = filename
                        .strip_prefix("snapshot-")
                        .and_then(|s| s.split_once('-'))
                    {
                        if let (Ok(term), Ok(index)) = (term_str.parse::<Term>(), index_str.parse::<LogIndex>()) {
                            snapshots.push((index, term, path));
                        }
                    }
                }
            }
        }
        
        snapshots.sort_by_key(|&(idx, _, _)| idx);
        
        if let Some((index, term, path)) = snapshots.last() {
            let data = std::fs::read(path)?;
            Ok(Some((*index, *term, data)))
        } else {
            Ok(None)
        }
    }
    
    fn persist_entry(&mut self, entry: &LogEntry) -> Result<()> {
        let serialized = bincode::serialize(entry)?;
        self.log_file.write_all(&serialized)?;
        self.log_file.sync_all()?;
        Ok(())
    }
    
    fn persist_all(&mut self) -> Result<()> {
        self.log_file.seek(SeekFrom::Start(0))?;
        self.log_file.set_len(0)?;
        
        self.log_file.write_all(&self.snapshot_index.to_le_bytes())?;
        self.log_file.write_all(&self.snapshot_term.to_le_bytes())?;
        
        let entries_bytes = bincode::serialize(&self.entries)?;
        self.log_file.write_all(&entries_bytes)?;
        self.log_file.sync_all()?;
        
        Ok(())
    }
}