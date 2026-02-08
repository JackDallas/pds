use serde_json::Value;

/// Assert the response status is 200 and return the JSON body.
pub fn assert_xrpc_ok(status: u16, body: &Value) -> &Value {
    assert_eq!(status, 200, "Expected 200 OK, got {status}: {body}");
    body
}

/// Assert the response matches the expected error status and error name.
pub fn assert_xrpc_error(status: u16, body: &Value, expected_status: u16, expected_error: &str) {
    assert_eq!(
        status, expected_status,
        "Expected status {expected_status}, got {status}: {body}"
    );
    if let Some(error) = body.get("error").and_then(|e| e.as_str()) {
        assert_eq!(
            error, expected_error,
            "Expected error '{expected_error}', got '{error}'"
        );
    }
}
