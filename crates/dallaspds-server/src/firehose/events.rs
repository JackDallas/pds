use serde::{Deserialize, Serialize};

/// A repo operation within a commit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoOp {
    /// Operation action: "create", "update", or "delete".
    pub action: String,
    /// The MST path: "collection/rkey".
    pub path: String,
    /// CID of the record (None for deletes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<CidLink>,
}

/// A CID link for DAG-CBOR serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CidLink {
    #[serde(rename = "$link")]
    pub link: String,
}

/// A `#commit` firehose event body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEvent {
    /// Sequence number assigned by the sequencer.
    pub seq: i64,
    /// Whether this event should update the subscriber's cursor.
    #[serde(rename = "tooBig")]
    pub too_big: bool,
    /// The DID of the repo that was modified.
    pub repo: String,
    /// The new commit CID (as a string).
    pub commit: CidLink,
    /// The previous commit CID, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<CidLink>,
    /// The new rev (TID) of the repo.
    pub rev: String,
    /// Timestamp of the event.
    pub time: String,
    /// The operations performed in this commit.
    pub ops: Vec<RepoOp>,
    /// The blocks that changed (CAR file bytes, base64-encoded for JSON but raw for CBOR).
    /// In the actual wire format this is raw bytes, but we use Vec<u8> and handle encoding.
    #[serde(with = "serde_bytes")]
    pub blocks: Vec<u8>,
}

/// A `#identity` firehose event body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityEvent {
    pub seq: i64,
    pub did: String,
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
}

/// A `#account` firehose event body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEvent {
    pub seq: i64,
    pub did: String,
    pub time: String,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// A `#info` firehose frame (sent at connection start or on error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoFrame {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// An error frame sent over the firehose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorFrame {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Discriminated union of all firehose event types.
#[derive(Debug, Clone)]
pub enum FirehoseEvent {
    Commit(CommitEvent),
    Identity(IdentityEvent),
    Account(AccountEvent),
}

impl FirehoseEvent {
    pub fn seq(&self) -> i64 {
        match self {
            FirehoseEvent::Commit(e) => e.seq,
            FirehoseEvent::Identity(e) => e.seq,
            FirehoseEvent::Account(e) => e.seq,
        }
    }
}
