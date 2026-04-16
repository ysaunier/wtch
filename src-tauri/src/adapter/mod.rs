pub mod google_status;
pub mod html;
pub mod json_api;
pub mod statuspage;

use crate::config::Watch;
use crate::state::WatchStatus;
use std::collections::HashMap;

/// The result returned by any adapter after a successful fetch and parse.
#[derive(Debug, Clone)]
pub struct AdapterResult {
    pub status: WatchStatus,
    pub description: String,
    pub data: serde_json::Value,
    pub components: Vec<ComponentStatus>,
}

/// Status of a single component reported by an adapter.
#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub name: String,
    pub status: WatchStatus,
    pub description: String,
}

/// Errors that an adapter can produce.
#[derive(Debug)]
pub enum AdapterError {
    HttpError(String),
    ParseError(String),
    UnsupportedAdapter(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpError(e) => write!(f, "HTTP error: {}", e),
            Self::ParseError(e) => write!(f, "parse error: {}", e),
            Self::UnsupportedAdapter(e) => write!(f, "unsupported adapter: {}", e),
        }
    }
}

/// Resolves `$VAR` and `${VAR}` placeholders in a string from environment variables.
///
/// Braced form (`${VAR}`) is substituted first. Unresolved variables are
/// replaced with an empty string, matching shell behaviour.
pub fn resolve_env_vars(value: &str) -> String {
    let re_braced = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    let result = re_braced
        .replace_all(value, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_default()
        })
        .to_string();

    let re_simple = regex::Regex::new(r"\$([A-Z_][A-Z0-9_]*)").unwrap();
    re_simple
        .replace_all(&result, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_default()
        })
        .to_string()
}

/// Builds a `reqwest::Client` with the headers from the watch configuration.
///
/// Header values are resolved for environment variable placeholders before
/// being inserted. Returns an [`AdapterError::HttpError`] if a header name or
/// value is invalid.
pub fn build_client_with_headers(
    headers: &HashMap<String, String>,
) -> Result<reqwest::Client, AdapterError> {
    let mut header_map = reqwest::header::HeaderMap::new();
    for (key, value) in headers {
        let resolved = resolve_env_vars(value);
        let name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| AdapterError::HttpError(e.to_string()))?;
        let val = reqwest::header::HeaderValue::from_str(&resolved)
            .map_err(|e| AdapterError::HttpError(e.to_string()))?;
        header_map.insert(name, val);
    }
    reqwest::Client::builder()
        .default_headers(header_map)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AdapterError::HttpError(e.to_string()))
}

/// Dispatches a watch to the correct adapter based on its `adapter` URN field.
///
/// Returns [`AdapterError::UnsupportedAdapter`] when the URN is not recognised.
pub async fn run_adapter(watch: &Watch) -> Result<AdapterResult, AdapterError> {
    let urn = watch.adapter.as_deref().unwrap_or("");
    match urn {
        "urn:wtch:adapter:google" => google_status::run(watch).await,
        "urn:wtch:adapter:html" => html::run(watch).await,
        "urn:wtch:adapter:json" => json_api::run(watch).await,
        "urn:wtch:adapter:statuspage" => statuspage::run(watch).await,
        other => Err(AdapterError::UnsupportedAdapter(other.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_resolve_env_vars_no_placeholders() {
        let result = resolve_env_vars("plain-value");
        assert_eq!(result, "plain-value");
    }

    #[test]
    fn test_resolve_env_vars_braced_present() {
        std::env::set_var("WTCH_TEST_TOKEN", "secret123");
        let result = resolve_env_vars("Bearer ${WTCH_TEST_TOKEN}");
        assert_eq!(result, "Bearer secret123");
        std::env::remove_var("WTCH_TEST_TOKEN");
    }

    #[test]
    fn test_resolve_env_vars_simple_present() {
        std::env::set_var("WTCH_TEST_KEY", "mykey");
        let result = resolve_env_vars("$WTCH_TEST_KEY");
        assert_eq!(result, "mykey");
        std::env::remove_var("WTCH_TEST_KEY");
    }

    #[test]
    fn test_resolve_env_vars_braced_missing_becomes_empty() {
        std::env::remove_var("WTCH_MISSING_VAR_XYZ");
        let result = resolve_env_vars("Bearer ${WTCH_MISSING_VAR_XYZ}");
        assert_eq!(result, "Bearer ");
    }

    #[test]
    fn test_resolve_env_vars_simple_missing_becomes_empty() {
        std::env::remove_var("WTCH_MISSING_SIMPLE_XYZ");
        let result = resolve_env_vars("$WTCH_MISSING_SIMPLE_XYZ");
        assert_eq!(result, "");
    }

    #[test]
    fn test_resolve_env_vars_braced_takes_priority_over_simple() {
        std::env::set_var("WTCH_PRIO_VAR", "braced");
        let result = resolve_env_vars("${WTCH_PRIO_VAR}");
        assert_eq!(result, "braced");
        std::env::remove_var("WTCH_PRIO_VAR");
    }

    #[test]
    fn test_build_client_with_headers_empty_map() {
        let headers: HashMap<String, String> = HashMap::new();
        let result = build_client_with_headers(&headers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_client_with_headers_valid_header() {
        let mut headers = HashMap::new();
        headers.insert("x-api-key".to_string(), "test-value".to_string());
        let result = build_client_with_headers(&headers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_client_with_headers_resolves_env_var() {
        std::env::set_var("WTCH_HDR_TEST", "resolved-token");
        let mut headers = HashMap::new();
        headers.insert(
            "authorization".to_string(),
            "Bearer ${WTCH_HDR_TEST}".to_string(),
        );
        let result = build_client_with_headers(&headers);
        assert!(result.is_ok());
        std::env::remove_var("WTCH_HDR_TEST");
    }

    #[test]
    fn test_run_adapter_unsupported_urn() {
        // We can't call async from a sync test without a runtime, so we test
        // the dispatch logic indirectly by verifying the error variant.
        // For a full async test, use tokio::test.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let watch = crate::config::Watch {
            name: Some("test".to_string()),
            watch_type: None,
            command: None,
            shell: None,
            format: None,
            interval: None,
            url: None,
            link: None,
            expand: false,
            notify: None,
            preset: None,
            conditions: crate::config::Conditions::default(),
            display: None,
            details: vec![],
            headers: HashMap::new(),
            adapter: Some("urn:wtch:adapter:unknown".to_string()),
            adapter_config: None,
        };
        let result = rt.block_on(run_adapter(&watch));
        assert!(matches!(result, Err(AdapterError::UnsupportedAdapter(_))));
    }
}
