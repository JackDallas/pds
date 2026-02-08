use std::sync::Arc;

use dallaspds_blob_fs::FsBlobStore;
use dallaspds_core::EventStore;
use dallaspds_core::config::PdsConfig;
use dallaspds_server::{AppState, build_router};
use dallaspds_storage_sqlite::{SqliteAccountStore, SqliteEventStore, SqliteRepoStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().init();

    let config_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config/single.toml".to_string());
    let config = PdsConfig::load(&config_path)?;

    // Ensure the data directory exists
    std::fs::create_dir_all("data")?;

    // Connect real storage backends
    let account_store = SqliteAccountStore::connect(&config.database.url).await?;
    let repo_store = SqliteRepoStore::connect(&config.database.url).await?;
    let event_store = SqliteEventStore::connect(&config.database.url).await?;

    let blobs_path = config.blobs.path.as_deref().unwrap_or("data/blobs");
    let blob_store = FsBlobStore::new(blobs_path)?;

    let addr = format!("0.0.0.0:{}", config.port);

    // Extract TLS config before moving config into Arc
    let tls_config = config.tls.clone();
    let public_url = config.public_url.clone();

    // Resume sequencer from the last persisted event sequence number.
    let max_seq = event_store.get_max_seq().await?;
    let sequencer = dallaspds_server::Sequencer::new(max_seq + 1, 1024);
    let relay_notifier = None; // No relay configured by default

    let event_store: Arc<dyn EventStore> = Arc::new(event_store);

    let email_sender = config.smtp.as_ref().map(|smtp_config| {
        Arc::new(
            dallaspds_server::email::EmailSender::new(smtp_config)
                .expect("Failed to init SMTP"),
        )
    });

    let state = AppState {
        account_store: Arc::new(account_store),
        repo_store: Arc::new(repo_store),
        blob_store: Arc::new(blob_store),
        config: Arc::new(config),
        sequencer: Some(sequencer),
        relay_notifier,
        event_store: Some(event_store),
        email_sender,
    };

    let router = build_router(state);

    if let Some(tls_config) = tls_config {
        use futures::StreamExt;
        use rustls_acme::{AcmeConfig, caches::DirCache};

        std::fs::create_dir_all(&tls_config.cert_cache)?;

        let mut acme_state = AcmeConfig::new(tls_config.domains)
            .contact([format!("mailto:{}", tls_config.contact_email)])
            .cache(DirCache::new(tls_config.cert_cache))
            .directory_lets_encrypt(tls_config.production)
            .state();
        let acceptor = acme_state.axum_acceptor(acme_state.default_rustls_config());
        tokio::spawn(async move {
            loop {
                acme_state.next().await;
            }
        });

        // HTTP -> HTTPS redirect on port 80
        tokio::spawn(http_redirect_server(public_url));

        tracing::info!("dallaspds-single starting HTTPS on {}", addr);
        let sock_addr: std::net::SocketAddr = addr.parse()?;
        axum_server::bind(sock_addr)
            .acceptor(acceptor)
            .serve(router.into_make_service())
            .await?;
    } else {
        tracing::info!("dallaspds-single starting on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, router).await?;
    }

    Ok(())
}

async fn http_redirect_server(public_url: String) {
    let app = axum::Router::new().fallback(move |req: axum::extract::Request| {
        let base = public_url.clone();
        async move {
            let target = format!("{}{}", base, req.uri());
            axum::response::Redirect::permanent(&target)
        }
    });
    let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:80").await else {
        tracing::warn!("Could not bind port 80 for HTTP redirect");
        return;
    };
    tracing::info!("HTTP redirect listening on 0.0.0.0:80");
    let _ = axum::serve(listener, app).await;
}
