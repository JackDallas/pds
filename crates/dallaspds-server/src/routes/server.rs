use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::{AuthenticatedUser, JwtRefreshSecret};
use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;
use dallaspds_core::types::{CreateAccountInput, RefreshTokenRecord};
use dallaspds_core::PdsError;

// ---------------------------------------------------------------------------
// 1. describeServer
// ---------------------------------------------------------------------------

pub async fn describe_server<A, R, B>(
    State(state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let did = format!("did:web:{}", state.config.hostname);
    Ok(Json(json!({
        "availableUserDomains": state.config.available_user_domains,
        "inviteCodeRequired": state.config.invite_required,
        "did": did,
    })))
}

// ---------------------------------------------------------------------------
// 2. createAccount
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountRequest {
    pub handle: String,
    pub email: Option<String>,
    pub password: String,
    pub invite_code: Option<String>,
}

pub async fn create_account<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Json(body): Json<CreateAccountRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Check single-user mode: reject if an account already exists.
    if matches!(state.config.mode, dallaspds_core::config::PdsMode::Single) {
        let existing = state.account_store.list_accounts(None, 1).await?;
        if !existing.is_empty() {
            return Err(XrpcError::new(
                StatusCode::BAD_REQUEST,
                "AccountLimitReached",
                "This server is running in single-user mode and already has an account",
            ));
        }
    }

    // (a) Validate handle — must end with one of the available user domains.
    let handle_valid = state
        .config
        .available_user_domains
        .iter()
        .any(|domain| body.handle.ends_with(domain));
    if !handle_valid {
        return Err(XrpcError::new(
            StatusCode::BAD_REQUEST,
            "InvalidHandle",
            format!(
                "Handle must end with one of: {}",
                state.config.available_user_domains.join(", ")
            ),
        ));
    }

    // Enforce invite code requirement
    if state.config.invite_required {
        let code_str = body.invite_code.as_deref().ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidInviteCode",
                "An invite code is required to create an account on this server",
            )
        })?;

        let invite = state
            .account_store
            .get_invite_code(code_str)
            .await?
            .ok_or_else(|| {
                XrpcError::new(StatusCode::BAD_REQUEST, "InvalidInviteCode", "Invite code not found")
            })?;

        if invite.disabled {
            return Err(XrpcError::new(StatusCode::BAD_REQUEST, "InvalidInviteCode", "Invite code has been disabled"));
        }

        if invite.uses.len() as i32 >= invite.available_uses {
            return Err(XrpcError::new(StatusCode::BAD_REQUEST, "InvalidInviteCode", "Invite code has no remaining uses"));
        }
    }

    // (b) Generate P-256 signing keypair.
    let signing_key = dallaspds_crypto::SigningKey::generate_p256().map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            e.to_string(),
        )
    })?;

    // (c) Create did:plc genesis operation.
    let rotation_keys = vec![signing_key.did_key()];
    let pds_endpoint = state.config.public_url.clone();
    let (did, signed_genesis_op) = dallaspds_crypto::create_did_plc_operation(
        &signing_key,
        rotation_keys,
        &body.handle,
        &pds_endpoint,
    )
    .map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            e.to_string(),
        )
    })?;

    // (d) POST genesis op to PLC directory.
    //     Wrap in a try — in dev mode the PLC directory may not be reachable.
    let plc_url = format!("{}/{}", state.config.plc_url.trim_end_matches('/'), did);
    let client = reqwest::Client::new();
    match client.post(&plc_url).json(&signed_genesis_op).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "PLC directory returned non-success status {}: {}",
                    status,
                    text
                );
            }
        }
        Err(e) => {
            tracing::warn!("Failed to reach PLC directory at {}: {}", plc_url, e);
        }
    }

    // (e) Hash password.
    let password_hash = dallaspds_crypto::hash_password(&body.password).map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            e.to_string(),
        )
    })?;

    // (f) Insert account via AccountStore.
    let input = CreateAccountInput {
        did: did.clone(),
        handle: body.handle.clone(),
        email: body.email.clone(),
        password_hash,
        signing_key: signing_key.to_bytes(),
    };
    state.account_store.create_account(&input).await?;

    // Record invite code usage
    if state.config.invite_required {
        if let Some(ref code_str) = body.invite_code {
            state.account_store.use_invite_code(code_str, &did).await?;
        }
    }

    // Send verification email if SMTP is configured
    if let (Some(email_sender), Some(email)) = (&state.email_sender, &body.email) {
        let token = hex::encode(rand::random::<[u8; 16]>());
        let _ = state.account_store.create_email_token("confirm_email", &did, &token).await;
        if let Err(e) = email_sender.send_verification_email(email, &token, &state.config.public_url).await {
            tracing::warn!("Failed to send verification email: {e}");
        }
    }

    // (f2) Initialize the repository (empty MST + signed commit).
    let (repo_root_cid, repo_rev) = dallaspds_repo::create_repo(
        state.repo_store.clone(),
        &did,
        &signing_key,
    )
    .await
    .map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            format!("failed to initialize repository: {e}"),
        )
    })?;
    state
        .account_store
        .update_repo_root(&did, &repo_root_cid, &repo_rev)
        .await?;

    // (g) Create access + refresh JWTs.
    let access_jwt =
        dallaspds_crypto::create_access_token(&did, &state.config.jwt.access_secret)?;
    let refresh_jti = uuid::Uuid::new_v4().to_string();
    let refresh_jwt =
        dallaspds_crypto::create_refresh_token(&did, &refresh_jti, &state.config.jwt.refresh_secret)?;

    // (h) Store refresh token.
    let refresh_record = RefreshTokenRecord {
        id: refresh_jti,
        did: did.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(90),
        next_id: None,
        app_password_name: None,
    };
    state
        .account_store
        .create_refresh_token(&refresh_record)
        .await?;

    // (i) Return response.
    Ok(Json(json!({
        "did": did,
        "handle": body.handle,
        "accessJwt": access_jwt,
        "refreshJwt": refresh_jwt,
    })))
}

// ---------------------------------------------------------------------------
// 3. createSession
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub identifier: String,
    pub password: String,
}

pub async fn create_session<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // (a) Lookup account by handle or email (try both).
    let account = state
        .account_store
        .get_account_by_handle(&body.identifier)
        .await?;
    let account = match account {
        Some(a) => a,
        None => state
            .account_store
            .get_account_by_email(&body.identifier)
            .await?
            .ok_or(PdsError::AccountNotFound)?,
    };

    // (b) Verify password.
    let valid =
        dallaspds_crypto::verify_password(&body.password, &account.password_hash).map_err(
            |e| {
                XrpcError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalServerError",
                    e.to_string(),
                )
            },
        )?;
    if !valid {
        return Err(PdsError::InvalidPassword.into());
    }

    // (c) Create access + refresh JWTs.
    let access_jwt =
        dallaspds_crypto::create_access_token(&account.did, &state.config.jwt.access_secret)?;
    let refresh_jti = uuid::Uuid::new_v4().to_string();
    let refresh_jwt = dallaspds_crypto::create_refresh_token(
        &account.did,
        &refresh_jti,
        &state.config.jwt.refresh_secret,
    )?;

    // (d) Store refresh token.
    let refresh_record = RefreshTokenRecord {
        id: refresh_jti,
        did: account.did.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(90),
        next_id: None,
        app_password_name: None,
    };
    state
        .account_store
        .create_refresh_token(&refresh_record)
        .await?;

    // (e) Return response.
    Ok(Json(json!({
        "did": account.did,
        "handle": account.handle,
        "email": account.email,
        "accessJwt": access_jwt,
        "refreshJwt": refresh_jwt,
    })))
}

// ---------------------------------------------------------------------------
// 4. getSession
// ---------------------------------------------------------------------------

pub async fn get_session<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    Ok(Json(json!({
        "did": account.did,
        "handle": account.handle,
        "email": account.email,
        "emailConfirmed": account.email_confirmed_at.is_some(),
    })))
}

// ---------------------------------------------------------------------------
// 5. refreshSession
// ---------------------------------------------------------------------------

pub async fn refresh_session<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    headers: HeaderMap,
    axum::Extension(refresh_secret): axum::Extension<JwtRefreshSecret>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Read Authorization header manually (refresh token, not access token).
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::UNAUTHORIZED,
                "AuthenticationRequired",
                "Missing authorization header",
            )
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        XrpcError::new(
            StatusCode::UNAUTHORIZED,
            "AuthenticationRequired",
            "Invalid authorization format",
        )
    })?;

    // Validate refresh token using the REFRESH secret.
    let claims =
        dallaspds_crypto::validate_refresh_token(token, &refresh_secret.0).map_err(|e| {
            let err_msg = e.to_string();
            if err_msg.contains("ExpiredSignature") {
                XrpcError::new(
                    StatusCode::UNAUTHORIZED,
                    "ExpiredToken",
                    "Refresh token has expired",
                )
            } else {
                XrpcError::new(
                    StatusCode::UNAUTHORIZED,
                    "InvalidToken",
                    "Invalid refresh token",
                )
            }
        })?;

    // Lookup the stored refresh token record.
    let _old_record = state
        .account_store
        .get_refresh_token(&claims.jti)
        .await?
        .ok_or_else(|| PdsError::Auth("Refresh token not found".to_string()))?;

    // Lookup account.
    let account = state
        .account_store
        .get_account_by_did(&claims.sub)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    // Delete old refresh token.
    state
        .account_store
        .delete_refresh_token(&claims.jti)
        .await?;

    // Create new tokens.
    let access_jwt =
        dallaspds_crypto::create_access_token(&account.did, &state.config.jwt.access_secret)?;
    let new_refresh_jti = uuid::Uuid::new_v4().to_string();
    let refresh_jwt = dallaspds_crypto::create_refresh_token(
        &account.did,
        &new_refresh_jti,
        &state.config.jwt.refresh_secret,
    )?;

    // Store new refresh token.
    let refresh_record = RefreshTokenRecord {
        id: new_refresh_jti,
        did: account.did.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(90),
        next_id: None,
        app_password_name: None,
    };
    state
        .account_store
        .create_refresh_token(&refresh_record)
        .await?;

    Ok(Json(json!({
        "did": account.did,
        "handle": account.handle,
        "accessJwt": access_jwt,
        "refreshJwt": refresh_jwt,
    })))
}

// ---------------------------------------------------------------------------
// 6. deleteSession
// ---------------------------------------------------------------------------

pub async fn delete_session<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Delete all refresh tokens for this user.
    state
        .account_store
        .delete_refresh_tokens_for_did(&user.did)
        .await?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// 7. requestEmailConfirmation
// ---------------------------------------------------------------------------

pub async fn request_email_confirmation<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let token = hex::encode(rand::random::<[u8; 16]>());
    state
        .account_store
        .create_email_token("confirm_email", &user.did, &token)
        .await?;

    if let (Some(email_sender), Some(email)) = (&state.email_sender, &account.email) {
        if let Err(e) = email_sender
            .send_verification_email(email, &token, &state.config.public_url)
            .await
        {
            tracing::warn!("Failed to send verification email: {e}");
        }
    }

    Ok(Json(json!({})))
}

// ---------------------------------------------------------------------------
// 8. confirmEmail
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmEmailRequest {
    pub email: String,
    pub token: String,
}

pub async fn confirm_email<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<ConfirmEmailRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let stored = state
        .account_store
        .get_email_token("confirm_email", &user.did)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidToken",
                "No confirmation token found",
            )
        })?;

    let (stored_token, requested_at) = stored;

    if stored_token != body.token {
        return Err(XrpcError::new(
            StatusCode::BAD_REQUEST,
            "InvalidToken",
            "Invalid confirmation token",
        ));
    }

    if requested_at + chrono::Duration::hours(1) < chrono::Utc::now() {
        return Err(XrpcError::new(
            StatusCode::BAD_REQUEST,
            "ExpiredToken",
            "Confirmation token has expired",
        ));
    }

    state.account_store.confirm_email(&user.did).await?;
    state
        .account_store
        .delete_email_token("confirm_email", &user.did)
        .await?;

    Ok(Json(json!({})))
}

// ---------------------------------------------------------------------------
// 9. requestPasswordReset
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPasswordResetRequest {
    pub email: String,
}

pub async fn request_password_reset<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Json(body): Json<RequestPasswordResetRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Always return 200 OK regardless of whether the email exists.
    // This prevents email enumeration attacks.
    if let Ok(Some(account)) = state.account_store.get_account_by_email(&body.email).await {
        let token = hex::encode(rand::random::<[u8; 16]>());
        if let Err(e) = state
            .account_store
            .create_email_token("reset_password", &account.did, &token)
            .await
        {
            tracing::warn!("Failed to store password reset token: {e}");
            return Ok(Json(json!({})));
        }

        if let Some(ref email_sender) = state.email_sender {
            if let Err(e) = email_sender
                .send_password_reset_email(&body.email, &token, &state.config.public_url)
                .await
            {
                tracing::warn!("Failed to send password reset email: {e}");
            }
        }
    }

    Ok(Json(json!({})))
}

// ---------------------------------------------------------------------------
// 10. resetPassword
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

pub async fn reset_password<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let (did, requested_at) = state
        .account_store
        .get_email_token_by_token("reset_password", &body.token)
        .await?
        .ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidToken",
                "Invalid password reset token",
            )
        })?;

    if requested_at + chrono::Duration::hours(1) < chrono::Utc::now() {
        return Err(XrpcError::new(
            StatusCode::BAD_REQUEST,
            "ExpiredToken",
            "Password reset token has expired",
        ));
    }

    let password_hash = dallaspds_crypto::hash_password(&body.password).map_err(|e| {
        XrpcError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            e.to_string(),
        )
    })?;

    state
        .account_store
        .update_password(&did, &password_hash)
        .await?;
    state
        .account_store
        .delete_email_token("reset_password", &did)
        .await?;
    state
        .account_store
        .delete_refresh_tokens_for_did(&did)
        .await?;

    Ok(Json(json!({})))
}

// ---------------------------------------------------------------------------
// 11. requestEmailUpdate
// ---------------------------------------------------------------------------

pub async fn request_email_update<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let token = hex::encode(rand::random::<[u8; 16]>());
    state
        .account_store
        .create_email_token("update_email", &user.did, &token)
        .await?;

    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    if let (Some(email_sender), Some(email)) = (&state.email_sender, &account.email) {
        if let Err(e) = email_sender
            .send_email_update_email(email, &token, &state.config.public_url)
            .await
        {
            tracing::warn!("Failed to send email update email: {e}");
        }
    }

    Ok(Json(json!({ "tokenRequired": state.email_sender.is_some() })))
}

// ---------------------------------------------------------------------------
// 12. updateEmail
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEmailRequest {
    pub email: String,
    pub token: Option<String>,
}

pub async fn update_email<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<UpdateEmailRequest>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    if state.email_sender.is_some() {
        let provided_token = body.token.as_deref().ok_or_else(|| {
            XrpcError::new(
                StatusCode::BAD_REQUEST,
                "TokenRequired",
                "Email verification token is required",
            )
        })?;

        let (stored_token, requested_at) = state
            .account_store
            .get_email_token("update_email", &user.did)
            .await?
            .ok_or_else(|| {
                XrpcError::new(
                    StatusCode::BAD_REQUEST,
                    "InvalidToken",
                    "No email update token found",
                )
            })?;

        if stored_token != provided_token {
            return Err(XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidToken",
                "Invalid email update token",
            ));
        }

        if requested_at + chrono::Duration::hours(1) < chrono::Utc::now() {
            return Err(XrpcError::new(
                StatusCode::BAD_REQUEST,
                "ExpiredToken",
                "Email update token has expired",
            ));
        }

        state
            .account_store
            .delete_email_token("update_email", &user.did)
            .await?;
    }

    state
        .account_store
        .update_email(&user.did, &body.email)
        .await?;

    Ok(Json(json!({})))
}
