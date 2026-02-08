use serde::Serialize;

use super::events::{ErrorFrame, FirehoseEvent, InfoFrame};

/// Frame header sent before each message body on the wire.
/// The AT Protocol firehose uses a two-part framing:
///   1. A small DAG-CBOR header: { "op": 1, "t": "#commit" } for messages,
///      or { "op": -1 } for errors.
///   2. A DAG-CBOR body (the event payload).
///
/// Both are concatenated into a single WebSocket binary frame.
#[derive(Debug, Serialize)]
struct FrameHeader {
    /// 1 = message frame, -1 = error frame
    op: i32,
    /// Event type tag (e.g. "#commit", "#identity", "#account", "#info")
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<String>,
}

/// Encode a firehose event into a binary WebSocket frame (header + body, both DAG-CBOR).
pub fn encode_event_frame(event: &FirehoseEvent) -> Result<Vec<u8>, String> {
    let (tag, body_bytes) = match event {
        FirehoseEvent::Commit(e) => (
            "#commit",
            dagcbor_encode(e)?,
        ),
        FirehoseEvent::Identity(e) => (
            "#identity",
            dagcbor_encode(e)?,
        ),
        FirehoseEvent::Account(e) => (
            "#account",
            dagcbor_encode(e)?,
        ),
    };

    let header = FrameHeader {
        op: 1,
        t: Some(tag.to_string()),
    };
    let header_bytes = dagcbor_encode(&header)?;

    let mut frame = Vec::with_capacity(header_bytes.len() + body_bytes.len());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&body_bytes);
    Ok(frame)
}

/// Encode an info frame for the firehose.
pub fn encode_info_frame(info: &InfoFrame) -> Result<Vec<u8>, String> {
    let header = FrameHeader {
        op: 1,
        t: Some("#info".to_string()),
    };
    let header_bytes = dagcbor_encode(&header)?;
    let body_bytes = dagcbor_encode(info)?;

    let mut frame = Vec::with_capacity(header_bytes.len() + body_bytes.len());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&body_bytes);
    Ok(frame)
}

/// Encode an error frame for the firehose.
pub fn encode_error_frame(error: &ErrorFrame) -> Result<Vec<u8>, String> {
    let header = FrameHeader {
        op: -1,
        t: None,
    };
    let header_bytes = dagcbor_encode(&header)?;
    let body_bytes = dagcbor_encode(error)?;

    let mut frame = Vec::with_capacity(header_bytes.len() + body_bytes.len());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&body_bytes);
    Ok(frame)
}

/// Helper: encode a value as DAG-CBOR bytes.
fn dagcbor_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_ipld_dagcbor::to_vec(value).map_err(|e| format!("DAG-CBOR encode error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firehose::events::*;

    fn make_commit_event() -> FirehoseEvent {
        FirehoseEvent::Commit(CommitEvent {
            seq: 1,
            too_big: false,
            repo: "did:plc:test".to_string(),
            commit: CidLink {
                link: "bafyreiabc123".to_string(),
            },
            prev: None,
            rev: "2222222222222".to_string(),
            time: "2025-01-01T00:00:00Z".to_string(),
            ops: vec![RepoOp {
                action: "create".to_string(),
                path: "app.bsky.feed.post/abc".to_string(),
                cid: Some(CidLink {
                    link: "bafyrecidef456".to_string(),
                }),
            }],
            blocks: vec![1, 2, 3],
        })
    }

    #[test]
    fn encode_commit_event_nonempty() {
        let event = make_commit_event();
        let frame = encode_event_frame(&event).unwrap();
        assert!(!frame.is_empty(), "encoded frame should not be empty");
        // Should be at least header + body
        assert!(frame.len() > 10);
    }

    #[test]
    fn encode_identity_event() {
        let event = FirehoseEvent::Identity(IdentityEvent {
            seq: 2,
            did: "did:plc:test".to_string(),
            time: "2025-01-01T00:00:00Z".to_string(),
            handle: Some("alice.test".to_string()),
        });
        let frame = encode_event_frame(&event).unwrap();
        assert!(!frame.is_empty());
    }

    #[test]
    fn encode_error_frame_negative_op() {
        let error = ErrorFrame {
            error: "FutureCursor".to_string(),
            message: Some("cursor is in the future".to_string()),
        };
        let frame = encode_error_frame(&error).unwrap();
        assert!(!frame.is_empty());

        // The error header has op: -1, encoded separately.
        // We know the header from our implementation.
        let header_bytes = dagcbor_encode(&FrameHeader { op: -1, t: None }).unwrap();
        let header: serde_json::Value =
            serde_ipld_dagcbor::from_slice(&header_bytes).unwrap();
        assert_eq!(header["op"], -1);

        // Verify the frame starts with the header
        assert!(frame.starts_with(&header_bytes));

        // Decode the body part (after header)
        let body: ErrorFrame =
            serde_ipld_dagcbor::from_slice(&frame[header_bytes.len()..]).unwrap();
        assert_eq!(body.error, "FutureCursor");
    }

    #[test]
    fn frame_is_valid_dagcbor() {
        let event = make_commit_event();
        let frame = encode_event_frame(&event).unwrap();

        // We know the header for a commit event
        let expected_header = FrameHeader {
            op: 1,
            t: Some("#commit".to_string()),
        };
        let header_bytes = dagcbor_encode(&expected_header).unwrap();

        // Frame should start with the header
        assert!(frame.starts_with(&header_bytes));

        // Decode the header independently
        let header: serde_json::Value =
            serde_ipld_dagcbor::from_slice(&header_bytes).unwrap();
        assert_eq!(header["op"], 1);
        assert_eq!(header["t"], "#commit");

        // Decode the body as CommitEvent (serde_bytes fields don't work with Value)
        let body: CommitEvent =
            serde_ipld_dagcbor::from_slice(&frame[header_bytes.len()..]).unwrap();
        assert_eq!(body.repo, "did:plc:test");
        assert_eq!(body.seq, 1);
    }

    #[test]
    fn roundtrip_commit_decode() {
        let event = make_commit_event();
        let frame = encode_event_frame(&event).unwrap();

        let header_bytes = dagcbor_encode(&FrameHeader {
            op: 1,
            t: Some("#commit".to_string()),
        })
        .unwrap();

        let decoded: CommitEvent =
            serde_ipld_dagcbor::from_slice(&frame[header_bytes.len()..])
                .expect("should decode commit event");

        assert_eq!(decoded.seq, 1);
        assert_eq!(decoded.repo, "did:plc:test");
        assert_eq!(decoded.ops.len(), 1);
        assert_eq!(decoded.ops[0].action, "create");
    }
}
