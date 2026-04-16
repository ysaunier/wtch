use std::time::Instant;

use reqwest::Client;
use tokio::time::{timeout, Duration};

use crate::config::Watch;
use crate::provider::{ProviderError, ProviderResult};

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Builds a [`ProviderResult`] from a raw HTTP status code and measured latency.
///
/// Extracted as a pure function so tests can exercise result construction
/// without performing real network requests.
pub fn build_result(status: u16, latency_ms: u64) -> ProviderResult {
    let up = (200..300).contains(&status);
    ProviderResult {
        data: serde_json::json!({
            "status": status,
            "latency_ms": latency_ms,
            "up": up,
        }),
        exit_code: None,
        latency_ms: Some(latency_ms),
    }
}

/// Sends a GET request to `watch.url`, measures round-trip latency, and
/// returns a [`ProviderResult`] containing status, latency, and an `up` flag.
///
/// Optional request headers defined in `watch.headers` are forwarded verbatim.
pub async fn run(watch: &Watch) -> Result<ProviderResult, ProviderError> {
    let url = watch.url.as_deref().ok_or_else(|| {
        ProviderError::ExecutionFailed("watch has no url field".to_string())
    })?;

    let client = Client::builder()
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| ProviderError::HttpError(e.to_string()))?;

    let mut request = client.get(url);
    for (key, value) in &watch.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    let start = Instant::now();

    let fetch = async {
        request
            .send()
            .await
            .map_err(|e| ProviderError::HttpError(e.to_string()))
    };

    let response = timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS), fetch)
        .await
        .map_err(|_| ProviderError::Timeout)??;

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    Ok(build_result(status, latency_ms))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- build_result unit tests (no network required) ---

    #[test]
    fn test_build_result_200_is_up() {
        let r = build_result(200, 42);
        assert_eq!(r.data["status"], 200);
        assert_eq!(r.data["latency_ms"], 42);
        assert_eq!(r.data["up"], true);
        assert_eq!(r.latency_ms, Some(42));
        assert!(r.exit_code.is_none());
    }

    #[test]
    fn test_build_result_204_is_up() {
        let r = build_result(204, 10);
        assert_eq!(r.data["up"], true);
    }

    #[test]
    fn test_build_result_299_is_up() {
        let r = build_result(299, 10);
        assert_eq!(r.data["up"], true);
    }

    #[test]
    fn test_build_result_300_is_not_up() {
        let r = build_result(300, 10);
        assert_eq!(r.data["up"], false);
    }

    #[test]
    fn test_build_result_404_is_not_up() {
        let r = build_result(404, 55);
        assert_eq!(r.data["status"], 404);
        assert_eq!(r.data["up"], false);
    }

    #[test]
    fn test_build_result_500_is_not_up() {
        let r = build_result(500, 200);
        assert_eq!(r.data["status"], 500);
        assert_eq!(r.data["up"], false);
    }

    #[test]
    fn test_build_result_latency_is_propagated() {
        let r = build_result(200, 123);
        assert_eq!(r.latency_ms, Some(123));
        assert_eq!(r.data["latency_ms"], 123);
    }

    #[test]
    fn test_build_result_199_is_not_up() {
        // 1xx informational — not considered "up" since we expect a 2xx success.
        let r = build_result(199, 5);
        assert_eq!(r.data["up"], false);
    }
}
