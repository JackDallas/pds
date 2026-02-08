use async_trait::async_trait;
use sqlx::{PgPool, Row};

use dallaspds_core::{EventStore, PdsError, PdsResult, PersistedEvent};

#[derive(Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    pub async fn connect(url: &str) -> PdsResult<Self> {
        let pool = PgPool::connect(url)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn append_event(&self, event_type: &str, did: &str, payload: &[u8]) -> PdsResult<i64> {
        let row = sqlx::query(
            "INSERT INTO firehose_event (event_type, did, payload) VALUES ($1, $2, $3) RETURNING seq",
        )
        .bind(event_type)
        .bind(did)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;

        row.try_get("seq")
            .map_err(|e| PdsError::Storage(e.to_string()))
    }

    async fn get_events_after(
        &self,
        after_seq: i64,
        limit: usize,
    ) -> PdsResult<Vec<PersistedEvent>> {
        let rows = sqlx::query(
            "SELECT seq, event_type, did, payload FROM firehose_event WHERE seq > $1 ORDER BY seq ASC LIMIT $2",
        )
        .bind(after_seq)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;

        rows.iter()
            .map(|r| {
                Ok(PersistedEvent {
                    seq: r.try_get("seq").map_err(|e| PdsError::Storage(e.to_string()))?,
                    event_type: r
                        .try_get("event_type")
                        .map_err(|e| PdsError::Storage(e.to_string()))?,
                    did: r.try_get("did").map_err(|e| PdsError::Storage(e.to_string()))?,
                    payload: r
                        .try_get("payload")
                        .map_err(|e| PdsError::Storage(e.to_string()))?,
                })
            })
            .collect()
    }

    async fn get_max_seq(&self) -> PdsResult<i64> {
        let row = sqlx::query("SELECT COALESCE(MAX(seq), 0) as max_seq FROM firehose_event")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        row.try_get("max_seq")
            .map_err(|e| PdsError::Storage(e.to_string()))
    }
}
