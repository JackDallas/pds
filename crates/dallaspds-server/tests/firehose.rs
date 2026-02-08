use dallaspds_test_utils::*;
use serde_json::json;

#[tokio::test]
async fn emit_persists_to_event_store() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "firehose.test.pds.local").await;

    // Create a record (which should emit a firehose event)
    send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": {
                "$type": "app.bsky.feed.post",
                "text": "firehose test",
                "createdAt": "2025-01-01T00:00:00Z"
            }
        })),
    )
    .await;

    // Check event store
    use dallaspds_core::EventStore;
    let events = stores.event_store.get_events_after(0, 100).await.unwrap();
    // At least 1 commit event from the createRecord
    let commit_events: Vec<_> = events.iter().filter(|e| e.event_type == "commit").collect();
    assert!(
        !commit_events.is_empty(),
        "should have persisted at least one commit event"
    );
    assert_eq!(commit_events[0].did, did);
}

#[tokio::test]
async fn emit_broadcasts_to_sequencer() {
    let stores = create_test_stores().await;
    let state = create_test_app_state(&stores);
    let sequencer = state.sequencer.as_ref().unwrap();
    let mut rx = sequencer.subscribe();

    // Emit an event via sequencer directly
    use dallaspds_server::firehose::events::{FirehoseEvent, IdentityEvent};
    let event = FirehoseEvent::Identity(IdentityEvent {
        seq: sequencer.next_seq(),
        did: "did:plc:broadcast".to_string(),
        time: "2025-01-01T00:00:00Z".to_string(),
        handle: Some("broadcast.test".to_string()),
    });
    sequencer.emit(event);

    let received = rx.try_recv().unwrap();
    assert_eq!(received.seq(), 1);
}

#[tokio::test]
async fn emit_without_event_store_still_broadcasts() {
    let stores = create_test_stores().await;
    let mut state = create_test_app_state(&stores);
    // Disable event store
    state.event_store = None;

    let sequencer = state.sequencer.as_ref().unwrap();
    let mut rx = sequencer.subscribe();

    use dallaspds_server::firehose::events::{FirehoseEvent, IdentityEvent};
    let event = FirehoseEvent::Identity(IdentityEvent {
        seq: sequencer.next_seq(),
        did: "did:plc:nostore".to_string(),
        time: "2025-01-01T00:00:00Z".to_string(),
        handle: None,
    });

    dallaspds_server::firehose::emit::emit_and_persist(&state, event).await;

    // Should still receive via broadcast even without persistence
    let received = rx.try_recv().unwrap();
    assert_eq!(received.seq(), 1);
}
