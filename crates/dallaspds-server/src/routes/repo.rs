use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;
use dallaspds_core::PdsError;
use dallaspds_crypto::TidGenerator;
use dallaspds_repo::cid_from_bytes;

/// Helper: convert raw CID bytes to a display string (base32lower CIDv1).
fn cid_bytes_to_string(cid_bytes: &[u8]) -> Result<String, XrpcError> {
    let cid = cid_from_bytes(cid_bytes)
        .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e))?;
    Ok(cid.to_string())
}

/// Helper: reconstruct SigningKey from stored private key bytes.
fn signing_key_from_account(
    account: &dallaspds_core::types::ActorAccount,
) -> Result<dallaspds_crypto::SigningKey, XrpcError> {
    dallaspds_crypto::SigningKey::from_bytes("p256", &account.signing_key).map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            format!("failed to load signing key: {e}"),
        )
    })
}

/// Helper: get repo root CID bytes for a DID, returning error if not initialized.
async fn get_repo_root_bytes<A: AccountStore>(
    account_store: &A,
    did: &str,
) -> Result<Vec<u8>, XrpcError> {
    let repo_root = account_store
        .get_repo_root(did)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "RepoNotFound",
                format!("repository not initialized for {did}"),
            )
        })?;
    Ok(repo_root.cid)
}

// ---------------------------------------------------------------------------
// 1. createRecord
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateRecordRequest {
    pub repo: String,
    pub collection: String,
    pub rkey: Option<String>,
    pub record: Value,
}

pub async fn create_record<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<CreateRecordRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Verify repo DID matches authenticated user.
    if body.repo != user.did {
        return Err(XrpcError::new(
            StatusCode::FORBIDDEN,
            "AuthorizationError",
            "Token did not match repo DID",
        ));
    }

    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let signing_key = signing_key_from_account(&account)?;
    let current_root = get_repo_root_bytes(&*state.account_store, &user.did).await?;
    let tid_gen = TidGenerator::new();

    let output = dallaspds_repo::create_record(
        state.repo_store.clone(),
        &user.did,
        &signing_key,
        &body.collection,
        body.rkey.as_deref(),
        &body.record,
        &tid_gen,
        &current_root,
    )
    .await?;

    // Update repo root after successful write.
    let prev_root = current_root.clone();
    state
        .account_store
        .update_repo_root(&user.did, &output.new_root, &output.new_rev)
        .await?;

    // Emit firehose event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::*;
        let seq = sequencer.next_seq();
        let commit_cid_str = cid_bytes_to_string(&output.new_root).unwrap_or_default();
        let record_cid_str = cid_bytes_to_string(&output.cid).unwrap_or_default();
        let diff_car = dallaspds_repo::generate_diff_car(
            state.repo_store.clone(),
            &user.did,
            &output.new_root,
            Some(&prev_root),
        )
        .await
        .unwrap_or_default();

        let event = FirehoseEvent::Commit(CommitEvent {
            seq,
            too_big: false,
            repo: user.did.clone(),
            commit: CidLink { link: commit_cid_str },
            prev: Some(CidLink {
                link: cid_bytes_to_string(&prev_root).unwrap_or_default(),
            }),
            rev: output.new_rev.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            ops: vec![RepoOp {
                action: "create".to_string(),
                path: format!("{}/{}", body.collection, body.rkey.as_deref().unwrap_or("")),
                cid: Some(CidLink { link: record_cid_str }),
            }],
            blocks: diff_car,
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;

        if let Some(ref notifier) = state.relay_notifier {
            notifier.notify(&user.did);
        }
    }

    let cid_string = cid_bytes_to_string(&output.cid)?;

    Ok(Json(json!({
        "uri": output.uri,
        "cid": cid_string,
    })))
}

// ---------------------------------------------------------------------------
// 2. getRecord
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetRecordQuery {
    pub repo: String,
    pub collection: String,
    pub rkey: String,
}

pub async fn get_record<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<GetRecordQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let current_root = get_repo_root_bytes(&*state.account_store, &params.repo).await?;

    let record = dallaspds_repo::get_record(
        state.repo_store.clone(),
        &params.repo,
        &params.collection,
        &params.rkey,
        &current_root,
    )
    .await?
    .ok_or_else(|| {
        XrpcError::new(
            StatusCode::BAD_REQUEST,
            "RecordNotFound",
            format!(
                "record not found: at://{}/{}/{}",
                params.repo, params.collection, params.rkey
            ),
        )
    })?;

    let cid_string = cid_bytes_to_string(&record.cid)?;

    Ok(Json(json!({
        "uri": record.uri,
        "cid": cid_string,
        "value": record.value,
    })))
}

// ---------------------------------------------------------------------------
// 3. listRecords
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListRecordsQuery {
    pub repo: String,
    pub collection: String,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

pub async fn list_records<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<ListRecordsQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let limit = params.limit.unwrap_or(50).min(100);
    let current_root = get_repo_root_bytes(&*state.account_store, &params.repo).await?;

    let records = dallaspds_repo::list_records(
        state.repo_store.clone(),
        &params.repo,
        &params.collection,
        limit,
        params.cursor.as_deref(),
        &current_root,
    )
    .await?;

    // Build the cursor: last record's rkey if we got a full page.
    let cursor = if records.len() >= limit {
        records.last().and_then(|r| {
            // URI format: at://did/collection/rkey — extract the rkey.
            r.uri.rsplit('/').next().map(|s| s.to_string())
        })
    } else {
        None
    };

    let record_values: Vec<Value> = records
        .iter()
        .map(|r| {
            let cid_str = cid_bytes_to_string(&r.cid).unwrap_or_default();
            json!({
                "uri": r.uri,
                "cid": cid_str,
                "value": r.value,
            })
        })
        .collect();

    let mut response = json!({ "records": record_values });
    if let Some(c) = cursor {
        response["cursor"] = json!(c);
    }

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// 4. deleteRecord
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeleteRecordRequest {
    pub repo: String,
    pub collection: String,
    pub rkey: String,
}

pub async fn delete_record<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<DeleteRecordRequest>,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Verify repo DID matches authenticated user.
    if body.repo != user.did {
        return Err(XrpcError::new(
            StatusCode::FORBIDDEN,
            "AuthorizationError",
            "Token did not match repo DID",
        ));
    }

    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let signing_key = signing_key_from_account(&account)?;
    let current_root = get_repo_root_bytes(&*state.account_store, &user.did).await?;
    let tid_gen = TidGenerator::new();

    let prev_root = current_root.clone();
    let (new_root, new_rev) = dallaspds_repo::delete_record(
        state.repo_store.clone(),
        &user.did,
        &signing_key,
        &body.collection,
        &body.rkey,
        &tid_gen,
        &current_root,
    )
    .await?;

    // Update repo root after successful write.
    state
        .account_store
        .update_repo_root(&user.did, &new_root, &new_rev)
        .await?;

    // Emit firehose event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::*;
        let seq = sequencer.next_seq();
        let commit_cid_str = cid_bytes_to_string(&new_root).unwrap_or_default();
        let diff_car = dallaspds_repo::generate_diff_car(
            state.repo_store.clone(),
            &user.did,
            &new_root,
            Some(&prev_root),
        )
        .await
        .unwrap_or_default();

        let event = FirehoseEvent::Commit(CommitEvent {
            seq,
            too_big: false,
            repo: user.did.clone(),
            commit: CidLink { link: commit_cid_str },
            prev: Some(CidLink {
                link: cid_bytes_to_string(&prev_root).unwrap_or_default(),
            }),
            rev: new_rev.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            ops: vec![RepoOp {
                action: "delete".to_string(),
                path: format!("{}/{}", body.collection, body.rkey),
                cid: None,
            }],
            blocks: diff_car,
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;

        if let Some(ref notifier) = state.relay_notifier {
            notifier.notify(&user.did);
        }
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// 5. putRecord
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PutRecordRequest {
    pub repo: String,
    pub collection: String,
    pub rkey: String,
    pub record: Value,
}

pub async fn put_record<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<PutRecordRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Verify repo DID matches authenticated user.
    if body.repo != user.did {
        return Err(XrpcError::new(
            StatusCode::FORBIDDEN,
            "AuthorizationError",
            "Token did not match repo DID",
        ));
    }

    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let signing_key = signing_key_from_account(&account)?;
    let current_root = get_repo_root_bytes(&*state.account_store, &user.did).await?;
    let tid_gen = TidGenerator::new();

    let prev_root = current_root.clone();
    let output = dallaspds_repo::put_record(
        state.repo_store.clone(),
        &user.did,
        &signing_key,
        &body.collection,
        &body.rkey,
        &body.record,
        &tid_gen,
        &current_root,
    )
    .await?;

    // Update repo root after successful write.
    state
        .account_store
        .update_repo_root(&user.did, &output.new_root, &output.new_rev)
        .await?;

    // Emit firehose event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::*;
        let seq = sequencer.next_seq();
        let commit_cid_str = cid_bytes_to_string(&output.new_root).unwrap_or_default();
        let record_cid_str = cid_bytes_to_string(&output.cid).unwrap_or_default();
        let diff_car = dallaspds_repo::generate_diff_car(
            state.repo_store.clone(),
            &user.did,
            &output.new_root,
            Some(&prev_root),
        )
        .await
        .unwrap_or_default();

        let event = FirehoseEvent::Commit(CommitEvent {
            seq,
            too_big: false,
            repo: user.did.clone(),
            commit: CidLink { link: commit_cid_str },
            prev: Some(CidLink {
                link: cid_bytes_to_string(&prev_root).unwrap_or_default(),
            }),
            rev: output.new_rev.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            ops: vec![RepoOp {
                action: "update".to_string(),
                path: format!("{}/{}", body.collection, body.rkey),
                cid: Some(CidLink { link: record_cid_str }),
            }],
            blocks: diff_car,
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;

        if let Some(ref notifier) = state.relay_notifier {
            notifier.notify(&user.did);
        }
    }

    let cid_string = cid_bytes_to_string(&output.cid)?;

    Ok(Json(json!({
        "uri": output.uri,
        "cid": cid_string,
    })))
}

// ---------------------------------------------------------------------------
// 6. describeRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DescribeRepoQuery {
    pub repo: String,
}

pub async fn describe_repo<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<DescribeRepoQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Resolve by DID or handle.
    let account = if params.repo.starts_with("did:") {
        state
            .account_store
            .get_account_by_did(&params.repo)
            .await?
    } else {
        state
            .account_store
            .get_account_by_handle(&params.repo)
            .await?
    };

    let account = account.ok_or(PdsError::AccountNotFound)?;

    let handle = account.handle.clone().unwrap_or_default();
    let did = account.did.clone();

    // Build a minimal DID document.
    let did_doc = json!({
        "@context": [
            "https://www.w3.org/ns/did/v1",
            "https://w3id.org/security/multikey/v1",
            "https://w3id.org/security/suites/secp256k1-2019/v1"
        ],
        "id": did,
        "alsoKnownAs": [format!("at://{handle}")],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": state.config.public_url,
        }]
    });

    Ok(Json(json!({
        "handle": handle,
        "did": did,
        "didDoc": did_doc,
        "collections": [],
        "handleIsCorrect": true,
    })))
}

// ---------------------------------------------------------------------------
// 7. uploadBlob
// ---------------------------------------------------------------------------

pub async fn upload_blob<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Compute CID: SHA-256 hash, raw codec (0x55), CIDv1.
    let digest = <sha2::Sha256 as sha2::Digest>::digest(&body);
    let mh =
        ipld_core::cid::multihash::Multihash::wrap(0x12, digest.as_slice()).map_err(|e| {
            XrpcError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                format!("failed to create multihash: {e}"),
            )
        })?;
    let cid = ipld_core::cid::Cid::new_v1(0x55, mh);
    let cid_string = cid.to_string();

    let size = body.len();

    // Store the blob.
    state
        .blob_store
        .put_blob(&user.did, &cid_string, body, &content_type)
        .await?;

    Ok(Json(json!({
        "blob": {
            "$type": "blob",
            "ref": {
                "$link": cid_string,
            },
            "mimeType": content_type,
            "size": size,
        }
    })))
}

// ---------------------------------------------------------------------------
// 8. applyWrites
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyWritesRequest {
    pub repo: String,
    pub writes: Vec<ApplyWriteOp>,
    pub swap_commit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
pub enum ApplyWriteOp {
    #[serde(rename = "com.atproto.repo.applyWrites#create")]
    Create {
        collection: String,
        rkey: Option<String>,
        value: Value,
    },
    #[serde(rename = "com.atproto.repo.applyWrites#update")]
    Update {
        collection: String,
        rkey: String,
        value: Value,
    },
    #[serde(rename = "com.atproto.repo.applyWrites#delete")]
    Delete {
        collection: String,
        rkey: String,
    },
}

pub async fn apply_writes<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<ApplyWritesRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Verify repo DID matches authenticated user.
    if body.repo != user.did {
        return Err(XrpcError::new(
            StatusCode::FORBIDDEN,
            "AuthorizationError",
            "Token did not match repo DID",
        ));
    }

    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let signing_key = signing_key_from_account(&account)?;
    let current_root = get_repo_root_bytes(&*state.account_store, &user.did).await?;

    // Validate swap_commit if provided.
    if let Some(ref swap_cid) = body.swap_commit {
        let current_cid_str = cid_bytes_to_string(&current_root)?;
        if *swap_cid != current_cid_str {
            return Err(XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidSwap",
                format!(
                    "swap_commit mismatch: expected {swap_cid}, got {current_cid_str}"
                ),
            ));
        }
    }

    let tid_gen = TidGenerator::new();
    let prev_root = current_root.clone();
    let mut running_root = current_root;
    let mut ops = Vec::new();
    let mut results = Vec::new();

    for write_op in &body.writes {
        match write_op {
            ApplyWriteOp::Create {
                collection,
                rkey,
                value,
            } => {
                let output = dallaspds_repo::create_record(
                    state.repo_store.clone(),
                    &user.did,
                    &signing_key,
                    collection,
                    rkey.as_deref(),
                    value,
                    &tid_gen,
                    &running_root,
                )
                .await?;

                let record_cid_str = cid_bytes_to_string(&output.cid)?;
                let rkey_actual = output
                    .uri
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string();

                ops.push(crate::firehose::events::RepoOp {
                    action: "create".to_string(),
                    path: format!("{collection}/{rkey_actual}"),
                    cid: Some(crate::firehose::events::CidLink {
                        link: record_cid_str,
                    }),
                });
                results.push(json!({
                    "uri": output.uri,
                    "cid": cid_bytes_to_string(&output.cid)?,
                }));
                running_root = output.new_root;
            }
            ApplyWriteOp::Update {
                collection,
                rkey,
                value,
            } => {
                let output = dallaspds_repo::put_record(
                    state.repo_store.clone(),
                    &user.did,
                    &signing_key,
                    collection,
                    rkey,
                    value,
                    &tid_gen,
                    &running_root,
                )
                .await?;

                let record_cid_str = cid_bytes_to_string(&output.cid)?;

                ops.push(crate::firehose::events::RepoOp {
                    action: "update".to_string(),
                    path: format!("{collection}/{rkey}"),
                    cid: Some(crate::firehose::events::CidLink {
                        link: record_cid_str,
                    }),
                });
                results.push(json!({
                    "uri": output.uri,
                    "cid": cid_bytes_to_string(&output.cid)?,
                }));
                running_root = output.new_root;
            }
            ApplyWriteOp::Delete { collection, rkey } => {
                let (new_root, _new_rev) = dallaspds_repo::delete_record(
                    state.repo_store.clone(),
                    &user.did,
                    &signing_key,
                    collection,
                    rkey,
                    &tid_gen,
                    &running_root,
                )
                .await?;

                ops.push(crate::firehose::events::RepoOp {
                    action: "delete".to_string(),
                    path: format!("{collection}/{rkey}"),
                    cid: None,
                });
                running_root = new_root;
            }
        }
    }

    // Get the final rev from the last commit.
    // We need to extract it from the repo root. The TID gen was used for each operation,
    // so the rev from the last op is the final rev. We'll regenerate one for the root update.
    let final_rev = tid_gen.next_tid();

    // Update repo root once with the final state.
    state
        .account_store
        .update_repo_root(&user.did, &running_root, &final_rev)
        .await?;

    // Emit a single firehose commit event with all operations.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::*;
        let seq = sequencer.next_seq();
        let commit_cid_str = cid_bytes_to_string(&running_root).unwrap_or_default();
        let diff_car = dallaspds_repo::generate_diff_car(
            state.repo_store.clone(),
            &user.did,
            &running_root,
            Some(&prev_root),
        )
        .await
        .unwrap_or_default();

        let event = FirehoseEvent::Commit(CommitEvent {
            seq,
            too_big: false,
            repo: user.did.clone(),
            commit: CidLink {
                link: commit_cid_str,
            },
            prev: Some(CidLink {
                link: cid_bytes_to_string(&prev_root).unwrap_or_default(),
            }),
            rev: final_rev,
            time: chrono::Utc::now().to_rfc3339(),
            ops,
            blocks: diff_car,
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;

        if let Some(ref notifier) = state.relay_notifier {
            notifier.notify(&user.did);
        }
    }

    Ok(Json(json!({
        "results": results,
    })))
}
