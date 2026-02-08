use async_trait::async_trait;

use crate::error::PdsResult;

/// A persisted firehose event row.
#[derive(Debug, Clone)]
pub struct PersistedEvent {
    pub seq: i64,
    pub event_type: String,
    pub did: String,
    pub payload: Vec<u8>,
}

#[async_trait]
pub trait EventStore: Send + Sync + 'static {
    /// Append a firehose event and return the assigned sequence number.
    async fn append_event(&self, event_type: &str, did: &str, payload: &[u8]) -> PdsResult<i64>;

    /// Get events with seq > after_seq, up to `limit`.
    async fn get_events_after(&self, after_seq: i64, limit: usize)
        -> PdsResult<Vec<PersistedEvent>>;

    /// Get the maximum sequence number in the store (0 if empty).
    async fn get_max_seq(&self) -> PdsResult<i64>;
}
