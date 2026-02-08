use std::sync::Arc;

use dallaspds_core::config::PdsConfig;
use dallaspds_core::traits::*;

use crate::firehose::relay::RelayNotifier;
use crate::firehose::sequencer::Sequencer;

#[derive(Clone)]
pub struct AppState<A, R, B>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    pub account_store: Arc<A>,
    pub repo_store: Arc<R>,
    pub blob_store: Arc<B>,
    pub config: Arc<PdsConfig>,
    /// Firehose event sequencer (None if firehose is disabled).
    pub sequencer: Option<Sequencer>,
    /// Relay notifier (None if no relay is configured).
    pub relay_notifier: Option<RelayNotifier>,
    /// Event store for firehose persistence (None if not configured).
    pub event_store: Option<Arc<dyn EventStore>>,
}
