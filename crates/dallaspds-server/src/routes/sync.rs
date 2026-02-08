use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;
use dallaspds_repo::cid_from_bytes;

/// Helper: convert raw CID bytes to a display string.
fn cid_bytes_to_string(cid_bytes: &[u8]) -> Result<String, XrpcError> {
    let cid = cid_from_bytes(cid_bytes)
        .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e))?;
    Ok(cid.to_string())
}

// ---------------------------------------------------------------------------
// 1. getRepo — returns the full repo as a CAR file
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetRepoQuery {
    pub did: String,
    /// Optional: only return blocks since this CID.
    pub since: Option<String>,
}

pub async fn get_repo<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<GetRepoQuery>,
) -> Result<Response, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let repo_root = state
        .account_store
        .get_repo_root(&params.did)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "RepoNotFound",
                format!("repository not found for {}", params.did),
            )
        })?;

    let car_bytes = if let Some(since) = &params.since {
        // Parse the `since` CID string back to bytes.
        let since_cid = ipld_core::cid::Cid::try_from(since.as_str())
            .map_err(|e| XrpcError::new(StatusCode::BAD_REQUEST, "InvalidRequest", format!("invalid since CID: {e}")))?;
        let since_bytes = since_cid.to_bytes();
        dallaspds_repo::generate_diff_car(
            state.repo_store.clone(),
            &params.did,
            &repo_root.cid,
            Some(&since_bytes),
        )
        .await?
    } else {
        dallaspds_repo::export_full_car(
            state.repo_store.clone(),
            &params.did,
            &repo_root.cid,
        )
        .await?
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.ipld.car")
        .body(Body::from(car_bytes))
        .unwrap())
}

// ---------------------------------------------------------------------------
// 2. getLatestCommit
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetLatestCommitQuery {
    pub did: String,
}

pub async fn get_latest_commit<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<GetLatestCommitQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let repo_root = state
        .account_store
        .get_repo_root(&params.did)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "RepoNotFound",
                format!("repository not found for {}", params.did),
            )
        })?;

    let cid_string = cid_bytes_to_string(&repo_root.cid)?;

    Ok(Json(json!({
        "cid": cid_string,
        "rev": repo_root.rev,
    })))
}

// ---------------------------------------------------------------------------
// 3. getBlob
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetBlobQuery {
    pub did: String,
    pub cid: String,
}

pub async fn get_blob<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<GetBlobQuery>,
) -> Result<Response, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let blob = state
        .blob_store
        .get_blob(&params.did, &params.cid)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::NOT_FOUND,
                "BlobNotFound",
                format!("blob not found: {}", params.cid),
            )
        })?;

    let (data, mime_type) = blob;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .body(Body::from(data))
        .unwrap())
}

// ---------------------------------------------------------------------------
// 4. listBlobs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListBlobsQuery {
    pub did: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

pub async fn list_blobs<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<ListBlobsQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let limit = params.limit.unwrap_or(500).min(1000);
    let cids = state
        .blob_store
        .list_blobs(&params.did, params.cursor.as_deref(), limit)
        .await?;

    let cursor = if cids.len() >= limit {
        cids.last().cloned()
    } else {
        None
    };

    let mut response = json!({ "cids": cids });
    if let Some(c) = cursor {
        response["cursor"] = json!(c);
    }

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// 5. listRepos
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListReposQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

pub async fn list_repos<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<ListReposQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let limit = params.limit.unwrap_or(500).min(1000);
    let accounts = state
        .account_store
        .list_accounts(params.cursor.as_deref(), limit)
        .await?;

    let mut repos = Vec::new();
    for account in &accounts {
        if let Some(root) = state.account_store.get_repo_root(&account.did).await? {
            // Skip accounts with empty repo roots (newly created, no commits yet).
            if root.cid.is_empty() {
                continue;
            }
            let head = cid_bytes_to_string(&root.cid)?;
            let active = account.status == dallaspds_core::AccountStatus::Active;
            let mut repo = json!({
                "did": account.did,
                "head": head,
                "rev": root.rev,
                "active": active,
            });
            if !active {
                let status = match account.status {
                    dallaspds_core::AccountStatus::Deactivated => "deactivated",
                    dallaspds_core::AccountStatus::Takendown => "takendown",
                    dallaspds_core::AccountStatus::Suspended => "suspended",
                    dallaspds_core::AccountStatus::Deleted => "deleted",
                    dallaspds_core::AccountStatus::Active => unreachable!(),
                };
                repo["status"] = json!(status);
            }
            repos.push(repo);
        }
    }

    let cursor = if accounts.len() >= limit {
        accounts.last().map(|a| json!(a.did))
    } else {
        None
    };

    let mut response = json!({ "repos": repos });
    if let Some(c) = cursor {
        response["cursor"] = c;
    }

    Ok(Json(response))
}
