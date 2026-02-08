pub async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"version": "0.1.0"}))
}
