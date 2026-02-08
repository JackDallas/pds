use crate::state::AppState;
use dallaspds_core::traits::*;

use super::events::FirehoseEvent;
use super::wire;

/// Persist a firehose event to the event store (if configured), then broadcast
/// it via the sequencer. The event must already have its `seq` assigned.
pub async fn emit_and_persist<A, R, B>(state: &AppState<A, R, B>, event: FirehoseEvent)
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let (event_type, did) = match &event {
        FirehoseEvent::Commit(e) => ("commit", e.repo.as_str()),
        FirehoseEvent::Identity(e) => ("identity", e.did.as_str()),
        FirehoseEvent::Account(e) => ("account", e.did.as_str()),
    };

    // Persist the wire-encoded event payload.
    if let Some(ref event_store) = state.event_store {
        match wire::encode_event_frame(&event) {
            Ok(payload) => {
                if let Err(e) = event_store.append_event(event_type, did, &payload).await {
                    tracing::warn!("Failed to persist firehose event: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to encode firehose event for persistence: {e}");
            }
        }
    }

    // Broadcast to live subscribers.
    if let Some(ref sequencer) = state.sequencer {
        sequencer.emit(event);
    }
}
