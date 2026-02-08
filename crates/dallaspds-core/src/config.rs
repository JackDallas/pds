use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum PdsMode {
    Single,
    Multi,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdsConfig {
    pub hostname: String,
    pub port: u16,
    pub public_url: String,
    pub plc_url: String,
    pub available_user_domains: Vec<String>,
    pub invite_required: bool,
    pub jwt: JwtConfig,
    pub database: DatabaseConfig,
    pub blobs: BlobsConfig,
    #[serde(default = "default_mode")]
    pub mode: PdsMode,
    /// URL of the AppView service for proxying unknown XRPC methods.
    #[serde(default)]
    pub appview_url: Option<String>,
    /// DID of the AppView service (used as JWT audience in service auth).
    #[serde(default)]
    pub appview_did: Option<String>,
    /// URL of the relay/BGS to notify via requestCrawl after writes.
    #[serde(default)]
    pub relay_url: Option<String>,
    /// Optional TLS configuration for automatic Let's Encrypt certificates.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub access_secret: String,
    pub refresh_secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlobsConfig {
    pub path: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Domains to obtain certificates for, e.g. ["pds.example.com"]
    pub domains: Vec<String>,
    /// ACME contact email, e.g. "admin@example.com"
    pub contact_email: String,
    /// Directory to cache certificates (default: "data/certs")
    #[serde(default = "default_cert_cache")]
    pub cert_cache: String,
    /// Use Let's Encrypt production directory (default: false = staging)
    #[serde(default)]
    pub production: bool,
}

fn default_cert_cache() -> String {
    "data/certs".to_string()
}

fn default_mode() -> PdsMode {
    PdsMode::Single
}

impl PdsConfig {
    pub fn load(path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("DALLAS_PDS_").split("__"))
            .extract()
    }
}
