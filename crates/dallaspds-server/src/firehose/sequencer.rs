use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tokio::sync::broadcast;

use super::events::FirehoseEvent;

/// The sequencer assigns monotonically increasing sequence numbers to firehose
/// events and broadcasts them to connected subscribers.
///
/// Sequence numbers are atomic and in-memory. For persistence across restarts,
/// the caller should persist the last-used seq and pass it when constructing.
#[derive(Clone)]
pub struct Sequencer {
    inner: Arc<SequencerInner>,
}

struct SequencerInner {
    next_seq: AtomicI64,
    /// Broadcast channel for live event streaming.
    /// Subscribers receive cloned events.
    sender: broadcast::Sender<Arc<FirehoseEvent>>,
}

impl Sequencer {
    /// Create a new sequencer.
    ///
    /// `start_seq` is the first sequence number to assign (typically last_persisted + 1).
    /// `channel_capacity` controls the broadcast buffer size (events before slow subscribers lag).
    pub fn new(start_seq: i64, channel_capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(channel_capacity);
        Sequencer {
            inner: Arc::new(SequencerInner {
                next_seq: AtomicI64::new(start_seq),
                sender,
            }),
        }
    }

    /// Allocate the next sequence number.
    pub fn next_seq(&self) -> i64 {
        self.inner.next_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Emit (broadcast) a firehose event to all connected subscribers.
    pub fn emit(&self, event: FirehoseEvent) {
        // Ignore send errors — they just mean no subscribers are connected.
        let _ = self.inner.sender.send(Arc::new(event));
    }

    /// Subscribe to the live event stream.
    ///
    /// Returns a receiver that yields events as they are emitted.
    /// If the subscriber falls behind by more than `channel_capacity` events,
    /// it will receive a `Lagged` error.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<FirehoseEvent>> {
        self.inner.sender.subscribe()
    }

    /// Returns the current (next-to-be-assigned) sequence number.
    /// Useful for knowing the "head" of the stream.
    pub fn current_seq(&self) -> i64 {
        self.inner.next_seq.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firehose::events::{IdentityEvent, FirehoseEvent};

    fn make_identity_event(seq: i64) -> FirehoseEvent {
        FirehoseEvent::Identity(IdentityEvent {
            seq,
            did: "did:plc:test".to_string(),
            time: "2025-01-01T00:00:00Z".to_string(),
            handle: Some("test.handle".to_string()),
        })
    }

    #[test]
    fn assigns_sequential_numbers() {
        let seq = Sequencer::new(1, 16);
        assert_eq!(seq.next_seq(), 1);
        assert_eq!(seq.next_seq(), 2);
        assert_eq!(seq.next_seq(), 3);
    }

    #[test]
    fn starts_at_given_seq() {
        let seq = Sequencer::new(100, 16);
        assert_eq!(seq.current_seq(), 100);
        assert_eq!(seq.next_seq(), 100);
        assert_eq!(seq.current_seq(), 101);
    }

    #[test]
    fn subscribe_receives_events() {
        let seq = Sequencer::new(1, 16);
        let mut rx = seq.subscribe();

        let event = make_identity_event(1);
        seq.emit(event);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.seq(), 1);
    }

    #[test]
    fn current_seq_reflects_allocations() {
        let seq = Sequencer::new(1, 16);
        assert_eq!(seq.current_seq(), 1);
        seq.next_seq();
        assert_eq!(seq.current_seq(), 2);
        seq.next_seq();
        seq.next_seq();
        assert_eq!(seq.current_seq(), 4);
    }
}
