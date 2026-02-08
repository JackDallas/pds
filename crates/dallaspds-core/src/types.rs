use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Active,
    Deactivated,
    Takendown,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct ActorAccount {
    pub did: String,
    pub handle: Option<String>,
    pub email: Option<String>,
    pub email_confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub password_hash: String,
    pub signing_key: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: AccountStatus,
    pub deactivated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub takedown_ref: Option<String>,
    pub delete_after: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateAccountInput {
    pub did: String,
    pub handle: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub signing_key: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RepoRoot {
    pub did: String,
    pub cid: Vec<u8>,
    pub rev: String,
    pub indexed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub id: String,
    pub did: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub next_id: Option<String>,
    pub app_password_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlobMeta {
    pub cid: String,
    pub mime_type: String,
    pub size: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCode {
    pub code: String,
    pub available_uses: i32,
    pub disabled: bool,
    pub for_account: String,
    pub created_by: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub uses: Vec<InviteCodeUse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCodeUse {
    pub code: String,
    pub used_by: String,
    pub used_at: chrono::DateTime<chrono::Utc>,
}
