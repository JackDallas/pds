
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use futures::SinkExt;
use futures::stream::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use super::events::{ErrorFrame, InfoFrame};
use super::wire;
use crate::state::AppState;
use dallaspds_core::traits::*;

#[derive(Debug, Deserialize)]
pub struct SubscribeReposQuery {
    pub cursor: Option<i64>,
}

/// Handler for `com.atproto.sync.subscribeRepos` WebSocket endpoint.
pub async fn subscribe_repos<A, R, B>(
    ws: WebSocketUpgrade,
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<SubscribeReposQuery>,
) -> impl IntoResponse
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    ws.on_upgrade(move |socket| handle_subscribe(socket, state, params.cursor))
}

async fn handle_subscribe<A, R, B>(
    socket: WebSocket,
    state: AppState<A, R, B>,
    cursor: Option<i64>,
) where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let (mut sender, mut receiver) = socket.split();

    let sequencer = match &state.sequencer {
        Some(seq) => seq.clone(),
        None => {
            // No sequencer configured — send error and close
            let err = wire::encode_error_frame(&ErrorFrame {
                error: "FutureCursor".to_string(),
                message: Some("Firehose not available".to_string()),
            });
            if let Ok(frame) = err {
                let _ = sender.send(Message::Binary(frame.into())).await;
            }
            return;
        }
    };

    // Validate cursor: if provided and in the future, reject.
    if let Some(cursor_val) = cursor {
        let current = sequencer.current_seq();
        if cursor_val > current {
            let err = wire::encode_error_frame(&ErrorFrame {
                error: "FutureCursor".to_string(),
                message: Some(format!(
                    "Cursor {cursor_val} is ahead of current seq {current}"
                )),
            });
            if let Ok(frame) = err {
                let _ = sender.send(Message::Binary(frame.into())).await;
            }
            return;
        }
    }

    // Subscribe to live events FIRST (before backfill) to avoid gaps.
    let mut rx = sequencer.subscribe();

    // Backfill from event store if cursor is provided and behind current seq.
    let mut last_sent_seq: i64 = cursor.unwrap_or(0);

    if let (Some(cursor_val), Some(event_store)) = (cursor, &state.event_store) {
        let current_seq = sequencer.current_seq();
        if cursor_val < current_seq {
            // Send an info frame indicating backfill.
            if let Ok(info_frame) = wire::encode_info_frame(&InfoFrame {
                name: "OutdatedCursor".to_string(),
                message: Some("Replaying historical events".to_string()),
            }) {
                if sender
                    .send(Message::Binary(info_frame.into()))
                    .await
                    .is_err()
                {
                    return;
                }
            }

            // Replay persisted events in batches.
            let mut replay_cursor = cursor_val;
            loop {
                let events = match event_store.get_events_after(replay_cursor, 100).await {
                    Ok(events) => events,
                    Err(e) => {
                        tracing::warn!("Failed to read events from store: {e}");
                        break;
                    }
                };

                if events.is_empty() {
                    break;
                }

                for event in &events {
                    // The payload is already a wire-encoded frame.
                    if sender
                        .send(Message::Binary(event.payload.clone().into()))
                        .await
                        .is_err()
                    {
                        return; // Client disconnected
                    }
                    last_sent_seq = event.seq;
                }

                replay_cursor = last_sent_seq;
            }
        }
    }

    // Spawn a task to drain incoming messages (pings/pongs/close).
    let drain_handle = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {}
    });

    // Stream live events to the client, skipping any already sent during backfill.
    loop {
        match rx.recv().await {
            Ok(event) => {
                // Skip events already sent during backfill.
                if event.seq() <= last_sent_seq {
                    continue;
                }

                match wire::encode_event_frame(&event) {
                    Ok(frame) => {
                        if sender.send(Message::Binary(frame.into())).await.is_err() {
                            break; // Client disconnected
                        }
                        last_sent_seq = event.seq();
                    }
                    Err(e) => {
                        tracing::warn!("Failed to encode firehose event: {e}");
                        continue;
                    }
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!("Firehose subscriber lagged by {n} events");
                // Send an info frame and continue — the subscriber missed some events.
                if let Ok(info_frame) = wire::encode_info_frame(&InfoFrame {
                    name: "OutdatedCursor".to_string(),
                    message: Some(format!("Skipped {n} events due to slow consumption")),
                }) {
                    if sender
                        .send(Message::Binary(info_frame.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            Err(RecvError::Closed) => {
                // Sequencer was dropped — server shutting down.
                break;
            }
        }
    }

    drain_handle.abort();
}
