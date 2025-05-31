use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    /// Strong consistency - all reads see the most recent write
    Strong,
    
    /// Bounded staleness - reads may be stale by at most the specified duration
    BoundedStaleness(Duration),
    
    /// Session consistency - reads within a session see writes from that session
    Session,
    
    /// Eventual consistency - reads may see stale data but will eventually converge
    Eventual,
}

impl Default for ConsistencyLevel {
    fn default() -> Self {
        Self::BoundedStaleness(Duration::from_secs(5))
    }
}