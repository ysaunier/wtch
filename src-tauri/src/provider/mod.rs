pub mod http;
pub mod script;

/// The result of a successful provider execution.
#[derive(Debug, Clone)]
pub struct ProviderResult {
    /// Parsed output data. Shape depends on the provider type.
    pub data: serde_json::Value,
    /// Exit code reported by the subprocess (script provider only).
    pub exit_code: Option<i32>,
    /// Round-trip latency in milliseconds (HTTP provider only).
    pub latency_ms: Option<u64>,
}

/// Errors that can occur during provider execution.
#[derive(Debug)]
pub enum ProviderError {
    /// The operation did not complete within the allowed time window.
    Timeout,
    /// The subprocess could not be spawned or returned an OS-level error.
    ExecutionFailed(String),
    /// The output could not be parsed into the expected format.
    ParseError(String),
    /// An HTTP-level error occurred (network failure, DNS, TLS, …).
    HttpError(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Timeout => write!(f, "provider timed out"),
            ProviderError::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            ProviderError::ParseError(msg) => write!(f, "parse error: {msg}"),
            ProviderError::HttpError(msg) => write!(f, "HTTP error: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}
