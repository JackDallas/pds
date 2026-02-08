use dallaspds_core::{PdsError, PdsResult};

/// Resolve a handle to a DID using DNS TXT and HTTPS fallback.
///
/// 1. Try DNS TXT record at `_atproto.{handle}` looking for `did=did:...`
/// 2. Fallback to HTTPS: `https://{handle}/.well-known/atproto-did`
pub async fn resolve_handle(handle: &str) -> PdsResult<Option<String>> {
    // Try DNS first.
    match resolve_handle_dns(handle).await {
        Ok(Some(did)) => return Ok(Some(did)),
        Ok(None) => {}
        Err(e) => {
            tracing::debug!("DNS handle resolution failed for {handle}: {e}");
        }
    }

    // Fallback to HTTPS.
    match resolve_handle_https(handle).await {
        Ok(Some(did)) => Ok(Some(did)),
        Ok(None) => Ok(None),
        Err(e) => {
            tracing::debug!("HTTPS handle resolution failed for {handle}: {e}");
            Ok(None)
        }
    }
}

/// Resolve a DID document.
///
/// - `did:plc:*` -> fetch from PLC directory (`https://plc.directory/{did}`)
/// - `did:web:*` -> fetch `https://{domain}/.well-known/did.json`
pub async fn resolve_did(did: &str) -> PdsResult<Option<serde_json::Value>> {
    if let Some(plc_id) = did.strip_prefix("did:plc:") {
        if plc_id.is_empty() {
            return Ok(None);
        }
        let url = format!("https://plc.directory/{did}");
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| PdsError::Upstream(e.to_string()))?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let doc: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PdsError::Upstream(e.to_string()))?;
        Ok(Some(doc))
    } else if let Some(domain) = did.strip_prefix("did:web:") {
        if domain.is_empty() {
            return Ok(None);
        }
        let url = format!("https://{}/.well-known/did.json", domain);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| PdsError::Upstream(e.to_string()))?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let doc: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PdsError::Upstream(e.to_string()))?;
        Ok(Some(doc))
    } else {
        Ok(None)
    }
}

/// Try resolving a handle via DNS TXT record at `_atproto.{handle}`.
async fn resolve_handle_dns(handle: &str) -> PdsResult<Option<String>> {
    use hickory_resolver::Resolver;

    let resolver = Resolver::builder_tokio()
        .map_err(|e| PdsError::InternalError(format!("DNS resolver init failed: {e}")))?
        .build();

    let lookup_name = format!("_atproto.{handle}.");
    let txt_lookup = resolver
        .txt_lookup(&lookup_name)
        .await
        .map_err(|e| PdsError::Upstream(format!("DNS TXT lookup failed: {e}")))?;

    for record in txt_lookup {
        let txt = record.to_string();
        if let Some(did) = txt.strip_prefix("did=") {
            let did = did.trim();
            if did.starts_with("did:") {
                return Ok(Some(did.to_string()));
            }
        }
    }

    Ok(None)
}

/// Try resolving a handle via HTTPS well-known endpoint.
async fn resolve_handle_https(handle: &str) -> PdsResult<Option<String>> {
    let url = format!("https://{handle}/.well-known/atproto-did");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| PdsError::InternalError(e.to_string()))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PdsError::Upstream(e.to_string()))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let body = resp
        .text()
        .await
        .map_err(|e| PdsError::Upstream(e.to_string()))?;

    let did = body.trim();
    if did.starts_with("did:") {
        Ok(Some(did.to_string()))
    } else {
        Ok(None)
    }
}
