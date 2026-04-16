/// JSON API adapter for wtch.
///
/// Fetches a JSON endpoint and uses jq-like expressions from the `expr` module
/// to extract status, description, and optional component list.
///
/// # Required adapter_config keys
/// - `status_field`  – jq path to the overall status value (default: `.status`)
/// - `status_map`    – mapping from raw string values to `WatchStatus` variants
///
/// # Optional adapter_config keys
/// - `description_field` – jq path to the description string
/// - `components_field`  – jq path to an array of component objects
/// - `component_name`    – jq path within each component for its name
/// - `component_status`  – jq path within each component for its status value
use std::collections::HashMap;

use crate::config::Watch;
use crate::state::WatchStatus;

use super::{AdapterError, AdapterResult, ComponentStatus, build_client_with_headers};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Runs the JSON API adapter for the given watch configuration.
///
/// Fetches the URL, parses the response as JSON, and extracts status,
/// description, and components using the jq expressions in `adapter_config`.
pub async fn run(watch: &Watch) -> Result<AdapterResult, AdapterError> {
    let url = watch
        .url
        .as_deref()
        .ok_or_else(|| AdapterError::ParseError("json adapter requires a url".to_string()))?;

    let config = watch.adapter_config.as_ref().ok_or_else(|| {
        AdapterError::ParseError("json adapter requires adapter_config".to_string())
    })?;

    let client = build_client_with_headers(&watch.headers)?;
    let data: serde_json::Value = client
        .get(url)
        .send()
        .await
        .map_err(|e| AdapterError::HttpError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    Ok(build_result(config, data))
}

// ---------------------------------------------------------------------------
// Result construction (pure, testable without HTTP)
// ---------------------------------------------------------------------------

/// Builds an [`AdapterResult`] from a parsed JSON payload and adapter config.
///
/// Extracted so unit tests can call this directly without an HTTP round-trip.
pub(crate) fn build_result(
    config: &HashMap<String, serde_yaml::Value>,
    data: serde_json::Value,
) -> AdapterResult {
    let status_field = get_config_str(config, "status_field").unwrap_or_else(|| ".status".to_string());
    let status_map = get_config_map(config, "status_map");

    let raw_status = crate::expr::evaluate(&status_field, &data)
        .ok()
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.to_string()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let default_status = get_config_str(config, "default_status")
        .and_then(|s| parse_watch_status(&s))
        .unwrap_or(WatchStatus::Unknown);

    let status = if raw_status.is_empty() {
        default_status
    } else {
        map_status(&raw_status, &status_map)
    };

    let description = get_config_str(config, "description_field")
        .and_then(|f| crate::expr::evaluate(&f, &data).ok())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| raw_status.clone());

    let components = extract_components(config, &data, &status_map);

    AdapterResult {
        status,
        description,
        data,
        components,
    }
}

// ---------------------------------------------------------------------------
// Helper: extract a string value from adapter_config
// ---------------------------------------------------------------------------

/// Returns the string value for `key` in `config`, or `None` if absent or
/// not a string.
pub(crate) fn get_config_str(
    config: &HashMap<String, serde_yaml::Value>,
    key: &str,
) -> Option<String> {
    config.get(key)?.as_str().map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Helper: extract a WatchStatus map from adapter_config
// ---------------------------------------------------------------------------

/// Returns a `HashMap<String, WatchStatus>` from a nested mapping in
/// `config[key]`.  Any entry whose value is not a recognised status string is
/// silently skipped.
pub(crate) fn get_config_map(
    config: &HashMap<String, serde_yaml::Value>,
    key: &str,
) -> HashMap<String, WatchStatus> {
    let mut result = HashMap::new();
    if let Some(serde_yaml::Value::Mapping(map)) = config.get(key) {
        for (k, v) in map {
            if let (Some(raw_key), Some(raw_val)) = (k.as_str(), v.as_str()) {
                if let Some(status) = parse_watch_status(raw_val) {
                    result.insert(raw_key.to_string(), status);
                }
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Helper: map a raw string to a WatchStatus
// ---------------------------------------------------------------------------

/// Looks up `value` in `map` and returns the mapped status, or
/// [`WatchStatus::Unknown`] when not found.
pub(crate) fn map_status(value: &str, map: &HashMap<String, WatchStatus>) -> WatchStatus {
    map.get(value).cloned().unwrap_or(WatchStatus::Unknown)
}

// ---------------------------------------------------------------------------
// Helper: extract component list
// ---------------------------------------------------------------------------

/// Extracts a list of [`ComponentStatus`] entries from `data` using the
/// `components_field`, `component_name`, and `component_status` keys in
/// `config`.
///
/// Returns an empty vec when those keys are absent or when the path does not
/// resolve to an array.
pub(crate) fn extract_components(
    config: &HashMap<String, serde_yaml::Value>,
    data: &serde_json::Value,
    status_map: &HashMap<String, WatchStatus>,
) -> Vec<ComponentStatus> {
    let components_field = match get_config_str(config, "components_field") {
        Some(f) => f,
        None => return vec![],
    };

    let array = match crate::expr::evaluate(&components_field, data).ok() {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return vec![],
    };

    let name_field = get_config_str(config, "component_name").unwrap_or_else(|| ".name".to_string());
    let status_field =
        get_config_str(config, "component_status").unwrap_or_else(|| ".status".to_string());

    array
        .iter()
        .filter_map(|item| {
            let name = crate::expr::evaluate(&name_field, item)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();

            let raw_status = crate::expr::evaluate(&status_field, item)
                .ok()
                .and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.to_string()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    serde_json::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            let status = map_status(&raw_status, status_map);

            Some(ComponentStatus {
                name,
                status,
                description: raw_status,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Internal: parse a status string from YAML config values
// ---------------------------------------------------------------------------

fn parse_watch_status(s: &str) -> Option<WatchStatus> {
    match s {
        "success" => Some(WatchStatus::Success),
        "warning" => Some(WatchStatus::Warning),
        "error" => Some(WatchStatus::Error),
        "maintenance" => Some(WatchStatus::Maintenance),
        "unknown" => Some(WatchStatus::Unknown),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Build a minimal adapter_config from YAML text.
    fn config_from_yaml(yaml: &str) -> HashMap<String, serde_yaml::Value> {
        serde_yaml::from_str(yaml).expect("test YAML should parse")
    }

    // Build the status_map used in most tests.
    fn standard_status_map() -> HashMap<String, WatchStatus> {
        let mut m = HashMap::new();
        m.insert("operational".to_string(), WatchStatus::Success);
        m.insert("degraded".to_string(), WatchStatus::Error);
        m.insert("partially-degraded".to_string(), WatchStatus::Warning);
        m.insert("under-maintenance".to_string(), WatchStatus::Maintenance);
        m
    }

    // -----------------------------------------------------------------------
    // get_config_str
    // -----------------------------------------------------------------------

    #[test]
    fn get_config_str_returns_value_for_existing_string_key() {
        let config = config_from_yaml("status_field: \".state\"");
        assert_eq!(
            get_config_str(&config, "status_field"),
            Some(".state".to_string())
        );
    }

    #[test]
    fn get_config_str_returns_none_for_missing_key() {
        let config = config_from_yaml("other_key: value");
        assert_eq!(get_config_str(&config, "status_field"), None);
    }

    #[test]
    fn get_config_str_returns_none_for_non_string_value() {
        let config = config_from_yaml("status_field: 42");
        assert_eq!(get_config_str(&config, "status_field"), None);
    }

    // -----------------------------------------------------------------------
    // get_config_map
    // -----------------------------------------------------------------------

    #[test]
    fn get_config_map_parses_known_status_values() {
        let config = config_from_yaml(
            "status_map:\n  operational: success\n  degraded: error\n  partial: warning",
        );
        let map = get_config_map(&config, "status_map");

        assert_eq!(map.get("operational"), Some(&WatchStatus::Success));
        assert_eq!(map.get("degraded"), Some(&WatchStatus::Error));
        assert_eq!(map.get("partial"), Some(&WatchStatus::Warning));
    }

    #[test]
    fn get_config_map_skips_unknown_status_values() {
        let config = config_from_yaml("status_map:\n  foo: not-a-status");
        let map = get_config_map(&config, "status_map");
        assert!(map.is_empty());
    }

    #[test]
    fn get_config_map_returns_empty_when_key_missing() {
        let config: HashMap<String, serde_yaml::Value> = HashMap::new();
        let map = get_config_map(&config, "status_map");
        assert!(map.is_empty());
    }

    #[test]
    fn get_config_map_returns_empty_when_value_is_not_mapping() {
        let config = config_from_yaml("status_map: just-a-string");
        let map = get_config_map(&config, "status_map");
        assert!(map.is_empty());
    }

    // -----------------------------------------------------------------------
    // map_status
    // -----------------------------------------------------------------------

    #[test]
    fn map_status_returns_mapped_value() {
        let map = standard_status_map();
        assert_eq!(map_status("operational", &map), WatchStatus::Success);
        assert_eq!(map_status("degraded", &map), WatchStatus::Error);
        assert_eq!(map_status("partially-degraded", &map), WatchStatus::Warning);
        assert_eq!(
            map_status("under-maintenance", &map),
            WatchStatus::Maintenance
        );
    }

    #[test]
    fn map_status_returns_unknown_for_unmapped_value() {
        let map = standard_status_map();
        assert_eq!(map_status("some-unknown-value", &map), WatchStatus::Unknown);
    }

    #[test]
    fn map_status_returns_unknown_for_empty_string() {
        let map = standard_status_map();
        assert_eq!(map_status("", &map), WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // extract_components
    // -----------------------------------------------------------------------

    #[test]
    fn extract_components_returns_empty_when_components_field_absent() {
        let config: HashMap<String, serde_yaml::Value> = HashMap::new();
        let data = json!({"components": []});
        let map = standard_status_map();
        let result = extract_components(&config, &data, &map);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_components_returns_empty_when_path_not_array() {
        let config = config_from_yaml("components_field: \".components\"");
        let data = json!({"components": "not-an-array"});
        let map = standard_status_map();
        let result = extract_components(&config, &data, &map);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_components_parses_name_and_status_with_defaults() {
        let config = config_from_yaml("components_field: \".components\"");
        let data = json!({
            "components": [
                {"name": "API", "status": "operational"},
                {"name": "Database", "status": "degraded"}
            ]
        });
        let map = standard_status_map();
        let components = extract_components(&config, &data, &map);

        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "API");
        assert_eq!(components[0].status, WatchStatus::Success);
        assert_eq!(components[0].description, "operational");

        assert_eq!(components[1].name, "Database");
        assert_eq!(components[1].status, WatchStatus::Error);
    }

    #[test]
    fn extract_components_respects_custom_name_and_status_fields() {
        let config = config_from_yaml(
            "components_field: \".services\"\ncomponent_name: \".label\"\ncomponent_status: \".state\"",
        );
        let data = json!({
            "services": [
                {"label": "Auth", "state": "operational"},
                {"label": "Storage", "state": "partially-degraded"}
            ]
        });
        let map = standard_status_map();
        let components = extract_components(&config, &data, &map);

        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "Auth");
        assert_eq!(components[0].status, WatchStatus::Success);
        assert_eq!(components[1].name, "Storage");
        assert_eq!(components[1].status, WatchStatus::Warning);
    }

    #[test]
    fn extract_components_handles_missing_name_field_gracefully() {
        let config = config_from_yaml("components_field: \".components\"");
        let data = json!({
            "components": [
                {"status": "operational"}
            ]
        });
        let map = standard_status_map();
        let components = extract_components(&config, &data, &map);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name, "");
        assert_eq!(components[0].status, WatchStatus::Success);
    }

    #[test]
    fn extract_components_handles_missing_status_field_gracefully() {
        let config = config_from_yaml("components_field: \".components\"");
        let data = json!({
            "components": [
                {"name": "API"}
            ]
        });
        let map = standard_status_map();
        let components = extract_components(&config, &data, &map);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name, "API");
        assert_eq!(components[0].status, WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // build_result (integration-style, no HTTP)
    // -----------------------------------------------------------------------

    #[test]
    fn build_result_extracts_status_and_description() {
        let config = config_from_yaml(
            "status_field: \".components[0].state\"\ndescription_field: \".components[0].state\"\nstatus_map:\n  operational: success\n  degraded: error",
        );
        let data = json!({
            "components": [{"state": "operational", "name": "API"}]
        });

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Success);
        assert_eq!(result.description, "operational");
    }

    #[test]
    fn build_result_falls_back_to_raw_status_as_description_when_no_description_field() {
        let config = config_from_yaml(
            "status_field: \".status\"\nstatus_map:\n  ok: success",
        );
        let data = json!({"status": "ok"});

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Success);
        assert_eq!(result.description, "ok");
    }

    #[test]
    fn build_result_uses_default_status_field_when_absent_from_config() {
        let config = config_from_yaml("status_map:\n  operational: success");
        let data = json!({"status": "operational"});

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Success);
    }

    #[test]
    fn build_result_returns_unknown_when_status_not_in_map() {
        let config = config_from_yaml(
            "status_field: \".status\"\nstatus_map:\n  ok: success",
        );
        let data = json!({"status": "some-new-value"});

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Unknown);
    }

    #[test]
    fn build_result_returns_unknown_when_status_field_missing_in_data() {
        let config = config_from_yaml(
            "status_field: \".does_not_exist\"\nstatus_map:\n  ok: success",
        );
        let data = json!({"status": "ok"});

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Unknown);
    }

    #[test]
    fn build_result_includes_full_component_list() {
        let config = config_from_yaml(
            "status_field: \".status\"\nstatus_map:\n  ok: success\n  fail: error\ncomponents_field: \".components\"\ncomponent_name: \".name\"\ncomponent_status: \".status\"",
        );
        let data = json!({
            "status": "ok",
            "components": [
                {"name": "API", "status": "ok"},
                {"name": "DB",  "status": "fail"}
            ]
        });

        let result = build_result(&config, data);
        assert_eq!(result.components.len(), 2);
        assert_eq!(result.components[0].name, "API");
        assert_eq!(result.components[0].status, WatchStatus::Success);
        assert_eq!(result.components[1].name, "DB");
        assert_eq!(result.components[1].status, WatchStatus::Error);
    }

    #[test]
    fn build_result_uses_default_status_when_field_is_empty() {
        let config = config_from_yaml(
            "status_field: \".[0].status\"\ndefault_status: success\nstatus_map:\n  bad: error",
        );
        let data = json!([]);

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Success);
    }

    #[test]
    fn build_result_ignores_default_status_when_field_resolves() {
        let config = config_from_yaml(
            "status_field: \".status\"\ndefault_status: success\nstatus_map:\n  bad: error",
        );
        let data = json!({"status": "bad"});

        let result = build_result(&config, data);
        assert_eq!(result.status, WatchStatus::Error);
    }

    #[test]
    fn build_result_data_field_preserves_original_json() {
        let config = config_from_yaml("status_map:\n  ok: success");
        let data = json!({"status": "ok", "extra": 42});

        let result = build_result(&config, data.clone());
        assert_eq!(result.data, data);
    }
}
