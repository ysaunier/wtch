use std::collections::HashMap;

use crate::config::{ScriptFormat, ShellConfig, Watch};
use crate::provider::{ProviderError, ProviderResult};
use crate::shell::{ShellOutput, ShellRunner};

/// Default timeout in seconds applied to script execution.
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Parses raw [`ShellOutput`] into a [`ProviderResult`] according to `format`.
///
/// Extracted as a pure function so tests can exercise parsing without
/// spawning real processes.
pub fn parse_output(output: ShellOutput, format: &ScriptFormat) -> Result<ProviderResult, ProviderError> {
    let exit_code = output.exit_code;
    let trimmed = output.stdout.trim().to_string();

    let data = match format {
        ScriptFormat::Json => {
            serde_json::from_str::<serde_json::Value>(&trimmed).map_err(|e| {
                ProviderError::ParseError(format!("invalid JSON from script: {e}"))
            })?
        }
        ScriptFormat::Value => {
            // Try to interpret the raw string as a JSON number; fall back to string.
            if let Ok(num) = trimmed.parse::<f64>() {
                serde_json::json!({ "value": num })
            } else {
                serde_json::json!({ "value": trimmed })
            }
        }
        ScriptFormat::ExitCode => {
            serde_json::json!({ "exit_code": exit_code })
        }
    };

    Ok(ProviderResult {
        data,
        exit_code: Some(exit_code),
        latency_ms: None,
    })
}

/// Executes the command defined in `watch` through the resolved shell and
/// returns parsed output as a [`ProviderResult`].
pub async fn run(
    watch: &Watch,
    shells: &HashMap<String, ShellConfig>,
) -> Result<ProviderResult, ProviderError> {
    let command = watch.command.as_deref().ok_or_else(|| {
        ProviderError::ExecutionFailed("watch has no command field".to_string())
    })?;

    let shell_name = watch.shell.as_deref().ok_or_else(|| {
        ProviderError::ExecutionFailed("watch has no shell field".to_string())
    })?;

    let format = watch
        .format
        .as_ref()
        .unwrap_or(&ScriptFormat::Value);

    let output = ShellRunner::run(shell_name, command, shells, DEFAULT_TIMEOUT_SECS).await?;

    parse_output(output, format)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_output(stdout: &str, exit_code: i32) -> ShellOutput {
        ShellOutput {
            stdout: stdout.to_string(),
            stderr: String::new(),
            exit_code,
        }
    }

    // --- Json format ---

    #[test]
    fn test_parse_json_object() {
        let output = make_output(r#"{"cost": 1.5, "limit": 10}"#, 0);
        let result = parse_output(output, &ScriptFormat::Json).unwrap();
        assert_eq!(result.data["cost"], 1.5);
        assert_eq!(result.data["limit"], 10);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.latency_ms.is_none());
    }

    #[test]
    fn test_parse_json_array() {
        let output = make_output(r#"[1, 2, 3]"#, 0);
        let result = parse_output(output, &ScriptFormat::Json).unwrap();
        assert!(result.data.is_array());
    }

    #[test]
    fn test_parse_json_trims_whitespace() {
        let output = make_output("  {\"key\": \"val\"}  \n", 0);
        let result = parse_output(output, &ScriptFormat::Json).unwrap();
        assert_eq!(result.data["key"], "val");
    }

    #[test]
    fn test_parse_json_invalid_returns_parse_error() {
        let output = make_output("not json at all", 0);
        let err = parse_output(output, &ScriptFormat::Json).unwrap_err();
        assert!(matches!(err, ProviderError::ParseError(_)));
    }

    // --- Value format ---

    #[test]
    fn test_parse_value_numeric_string() {
        let output = make_output("42.5", 0);
        let result = parse_output(output, &ScriptFormat::Value).unwrap();
        assert_eq!(result.data["value"], 42.5_f64);
    }

    #[test]
    fn test_parse_value_integer_string() {
        let output = make_output("100", 0);
        let result = parse_output(output, &ScriptFormat::Value).unwrap();
        assert_eq!(result.data["value"], 100.0_f64);
    }

    #[test]
    fn test_parse_value_text_string() {
        let output = make_output("203.0.113.42", 0);
        let result = parse_output(output, &ScriptFormat::Value).unwrap();
        assert_eq!(result.data["value"], "203.0.113.42");
    }

    #[test]
    fn test_parse_value_trims_whitespace() {
        let output = make_output("  hello world  \n", 0);
        let result = parse_output(output, &ScriptFormat::Value).unwrap();
        assert_eq!(result.data["value"], "hello world");
    }

    // --- ExitCode format ---

    #[test]
    fn test_parse_exit_code_success() {
        let output = make_output("", 0);
        let result = parse_output(output, &ScriptFormat::ExitCode).unwrap();
        assert_eq!(result.data["exit_code"], 0);
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_parse_exit_code_failure() {
        let output = make_output("something failed", 1);
        let result = parse_output(output, &ScriptFormat::ExitCode).unwrap();
        assert_eq!(result.data["exit_code"], 1);
        assert_eq!(result.exit_code, Some(1));
    }

    #[test]
    fn test_parse_exit_code_ignores_stdout() {
        // Even with valid JSON on stdout, ExitCode format only cares about exit_code.
        let output = make_output(r#"{"irrelevant": true}"#, 2);
        let result = parse_output(output, &ScriptFormat::ExitCode).unwrap();
        assert_eq!(result.data["exit_code"], 2);
        // The JSON object must NOT appear in data.
        assert!(result.data.get("irrelevant").is_none());
    }
}
