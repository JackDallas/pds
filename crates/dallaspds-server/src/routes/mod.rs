pub mod admin;
pub mod health;
pub mod identity;
pub mod oauth;
pub mod repo;
pub mod server;
pub mod sync;
pub mod well_known;

use axum::Extension;

use crate::auth::{JwtRefreshSecret, JwtSecret};
use crate::state::AppState;
use dallaspds_core::traits::*;

pub fn build_router<A, R, B>(state: AppState<A, R, B>) -> axum::Router
where
    A: AccountStore + Clone,
    R: RepoStore + Clone,
    B: BlobStore + Clone,
{
    let jwt_secret = JwtSecret(state.config.jwt.access_secret.clone());
    let jwt_refresh_secret = JwtRefreshSecret(state.config.jwt.refresh_secret.clone());

    axum::Router::new()
        // Health
        .route("/xrpc/_health", axum::routing::get(health::health_check))
        // Server endpoints
        .route(
            "/xrpc/com.atproto.server.describeServer",
            axum::routing::get(server::describe_server::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.createAccount",
            axum::routing::post(server::create_account::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.createSession",
            axum::routing::post(server::create_session::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.getSession",
            axum::routing::get(server::get_session::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.refreshSession",
            axum::routing::post(server::refresh_session::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.deleteSession",
            axum::routing::post(server::delete_session::<A, R, B>),
        )
        // Account lifecycle
        .route(
            "/xrpc/com.atproto.server.deleteAccount",
            axum::routing::post(admin::delete_account::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.deactivateAccount",
            axum::routing::post(admin::deactivate_account::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.server.activateAccount",
            axum::routing::post(admin::activate_account::<A, R, B>),
        )
        // Repo endpoints
        .route(
            "/xrpc/com.atproto.repo.createRecord",
            axum::routing::post(repo::create_record::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.getRecord",
            axum::routing::get(repo::get_record::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.listRecords",
            axum::routing::get(repo::list_records::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.deleteRecord",
            axum::routing::post(repo::delete_record::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.putRecord",
            axum::routing::post(repo::put_record::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.describeRepo",
            axum::routing::get(repo::describe_repo::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.uploadBlob",
            axum::routing::post(repo::upload_blob::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.repo.applyWrites",
            axum::routing::post(repo::apply_writes::<A, R, B>),
        )
        // Sync endpoints
        .route(
            "/xrpc/com.atproto.sync.getRepo",
            axum::routing::get(sync::get_repo::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.sync.getLatestCommit",
            axum::routing::get(sync::get_latest_commit::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.sync.getBlob",
            axum::routing::get(sync::get_blob::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.sync.listBlobs",
            axum::routing::get(sync::list_blobs::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.sync.listRepos",
            axum::routing::get(sync::list_repos::<A, R, B>),
        )
        // Firehose WebSocket
        .route(
            "/xrpc/com.atproto.sync.subscribeRepos",
            axum::routing::get(crate::firehose::stream::subscribe_repos::<A, R, B>),
        )
        // Identity endpoints
        .route(
            "/xrpc/com.atproto.identity.resolveHandle",
            axum::routing::get(identity::resolve_handle::<A, R, B>),
        )
        .route(
            "/xrpc/com.atproto.identity.updateHandle",
            axum::routing::post(identity::update_handle::<A, R, B>),
        )
        // OAuth metadata endpoints
        .route(
            "/.well-known/oauth-authorization-server",
            axum::routing::get(oauth::authorization_server_metadata::<A, R, B>),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(oauth::protected_resource_metadata::<A, R, B>),
        )
        // OAuth operational endpoints (stubs)
        .route(
            "/oauth/par",
            axum::routing::post(oauth::oauth_par::<A, R, B>),
        )
        .route(
            "/oauth/authorize",
            axum::routing::get(oauth::oauth_authorize::<A, R, B>),
        )
        .route(
            "/oauth/token",
            axum::routing::post(oauth::oauth_token::<A, R, B>),
        )
        .route(
            "/oauth/revoke",
            axum::routing::post(oauth::oauth_revoke::<A, R, B>),
        )
        .route(
            "/oauth/jwks",
            axum::routing::get(oauth::oauth_jwks::<A, R, B>),
        )
        // Well-known
        .route(
            "/.well-known/atproto-did",
            axum::routing::get(well_known::atproto_did::<A, R, B>),
        )
        // Fallback: proxy unknown XRPC methods to the configured AppView.
        .fallback(crate::proxy::pipethrough::pipethrough_fallback::<A, R, B>)
        .layer(Extension(jwt_secret))
        .layer(Extension(jwt_refresh_secret))
        // CORS: allow any origin for XRPC (AT Protocol expects this).
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .expose_headers(tower_http::cors::Any),
        )
        // Request body size limit: 10 MiB default.
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            10 * 1024 * 1024,
        ))
        .with_state(state)
}
