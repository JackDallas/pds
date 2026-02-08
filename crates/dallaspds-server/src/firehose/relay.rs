use std::sync::Arc;

use tokio::sync::mpsc;

/// Notifies a configured relay (e.g., the Bluesky BGS) to crawl this PDS
/// by sending `com.atproto.sync.requestCrawl` after each repo write.
#[derive(Clone)]
pub struct RelayNotifier {
    sender: mpsc::UnboundedSender<String>,
}

impl RelayNotifier {
    /// Create a new relay notifier that will POST requestCrawl to the given URL.
    ///
    /// `relay_url` is the base URL of the relay (e.g. `https://bsky.network`).
    /// `pds_hostname` is the hostname of this PDS (used in the requestCrawl body).
    ///
    /// Returns the notifier handle and a future that should be spawned to run
    /// the background notification loop.
    pub fn new(relay_url: String, pds_hostname: String) -> (Self, RelayNotifierWorker) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let notifier = RelayNotifier { sender };
        let worker = RelayNotifierWorker {
            relay_url,
            pds_hostname,
            receiver,
            client: Arc::new(reqwest::Client::new()),
        };
        (notifier, worker)
    }

    /// Notify the relay that a repo has been updated.
    /// This is a non-blocking fire-and-forget call.
    pub fn notify(&self, _did: &str) {
        // We just need to poke the relay; the DID doesn't matter for requestCrawl.
        let _ = self.sender.send("crawl".to_string());
    }
}

pub struct RelayNotifierWorker {
    relay_url: String,
    pds_hostname: String,
    receiver: mpsc::UnboundedReceiver<String>,
    client: Arc<reqwest::Client>,
}

impl RelayNotifierWorker {
    /// Run the notification worker loop. Should be spawned as a tokio task.
    pub async fn run(mut self) {
        while let Some(_) = self.receiver.recv().await {
            let url = format!(
                "{}/xrpc/com.atproto.sync.requestCrawl",
                self.relay_url.trim_end_matches('/')
            );
            let body = serde_json::json!({
                "hostname": self.pds_hostname,
            });

            match self.client.post(&url).json(&body).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        tracing::warn!(
                            "Relay requestCrawl returned {}: {}",
                            status,
                            text
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to notify relay at {}: {e}", url);
                }
            }
        }
    }
}
