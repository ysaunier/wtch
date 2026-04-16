/// HTML adapter for wtch.
///
/// Fetches an HTML page and extracts a status value using one of two modes:
///
/// - `css`: applies a CSS selector to the parsed DOM, then extracts text,
///   class, or a named attribute from the matched element.
/// - `script_json`: locates a `<script>` tag whose content contains a known
///   string, extracts embedded JSON via a regex, and evaluates a jq-style
///   expression against it.
///
/// Both modes resolve the extracted string against a `status_map` (partial
/// contains match) to produce a [`WatchStatus`].
use std::collections::HashMap;

use regex::Regex;
use scraper::{Html, Selector};

use crate::config::Watch;
use crate::expr::evaluate;
use crate::state::WatchStatus;

use super::{AdapterError, AdapterResult, build_client_with_headers};

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

/// Reads a string value from an adapter_config map.
///
/// Returns `None` when the key is absent or the value is not a string.
fn get_config_str<'a>(
    config: &'a HashMap<String, serde_yaml::Value>,
    key: &str,
) -> Option<String> {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Reads a string map from an adapter_config key whose value is a YAML mapping.
///
/// Returns an empty map when the key is absent or the value is not a mapping.
fn get_config_map(
    config: &HashMap<String, serde_yaml::Value>,
    key: &str,
) -> HashMap<String, String> {
    config
        .get(key)
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let key = k.as_str()?.to_string();
                    let val = v.as_str()?.to_string();
                    Some((key, val))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Status mapping
// ---------------------------------------------------------------------------

/// Maps an extracted string to a [`WatchStatus`] by checking whether the
/// string *contains* any key in `status_map`.
///
/// Returns [`WatchStatus::Unknown`] when no key matches.
fn map_status(extracted: &str, status_map: &HashMap<String, String>) -> WatchStatus {
    for (pattern, status_str) in status_map {
        if extracted.contains(pattern.as_str()) {
            return parse_status(status_str);
        }
    }
    WatchStatus::Unknown
}

/// Converts a status string from the config to a [`WatchStatus`] variant.
fn parse_status(s: &str) -> WatchStatus {
    match s {
        "success" => WatchStatus::Success,
        "warning" => WatchStatus::Warning,
        "error" => WatchStatus::Error,
        "maintenance" => WatchStatus::Maintenance,
        _ => WatchStatus::Unknown,
    }
}

// ---------------------------------------------------------------------------
// CSS mode
// ---------------------------------------------------------------------------

/// Runs the `css` extraction mode.
///
/// Selects one element in the document using `selector`, extracts either its
/// text content, its `class` attribute, or a custom attribute specified as
/// `attr:NAME` in the `extract` config key, then maps the result against
/// `status_map`.
fn run_css_mode(
    html: &str,
    config: &HashMap<String, serde_yaml::Value>,
) -> Result<AdapterResult, AdapterError> {
    let selector_str = get_config_str(config, "selector")
        .ok_or_else(|| AdapterError::ParseError("html css mode: missing 'selector'".to_string()))?;

    let selector = Selector::parse(&selector_str)
        .map_err(|e| AdapterError::ParseError(format!("invalid CSS selector: {e:?}")))?;

    let document = Html::parse_document(html);
    let status_map = get_config_map(config, "status_map");
    let extract = get_config_str(config, "extract").unwrap_or_else(|| "text".to_string());

    let element = document.select(&selector).next();

    let extracted = match element {
        None => {
            return Ok(AdapterResult {
                status: WatchStatus::Unknown,
                description: format!("no element matched selector: {selector_str}"),
                data: serde_json::Value::Null,
                components: vec![],
            });
        }
        Some(el) => {
            if extract == "text" {
                el.text().collect::<Vec<_>>().join("").trim().to_string()
            } else if extract == "class" {
                el.value()
                    .attr("class")
                    .unwrap_or("")
                    .to_string()
            } else if let Some(attr_name) = extract.strip_prefix("attr:") {
                el.value().attr(attr_name).unwrap_or("").to_string()
            } else {
                return Err(AdapterError::ParseError(format!(
                    "unknown extract mode: {extract}"
                )));
            }
        }
    };

    let status = map_status(&extracted, &status_map);

    Ok(AdapterResult {
        status,
        description: extracted.clone(),
        data: serde_json::json!({ "extracted": extracted }),
        components: vec![],
    })
}

// ---------------------------------------------------------------------------
// Script JSON mode
// ---------------------------------------------------------------------------

/// Runs the `script_json` extraction mode.
///
/// Finds a `<script>` tag (matched by `script_selector`, defaulting to
/// `"script"`) whose text content contains `script_match`, then extracts a
/// JSON object from it using `json_regex` (first capture group). A jq
/// expression in `status_field` is evaluated against the parsed JSON, and the
/// resulting string is resolved via `status_map`.
fn run_script_json_mode(
    html: &str,
    config: &HashMap<String, serde_yaml::Value>,
) -> Result<AdapterResult, AdapterError> {
    let script_selector_str =
        get_config_str(config, "script_selector").unwrap_or_else(|| "script".to_string());

    let script_match = get_config_str(config, "script_match").ok_or_else(|| {
        AdapterError::ParseError("html script_json mode: missing 'script_match'".to_string())
    })?;

    let json_regex_str = get_config_str(config, "json_regex").ok_or_else(|| {
        AdapterError::ParseError("html script_json mode: missing 'json_regex'".to_string())
    })?;

    let status_field = get_config_str(config, "status_field").ok_or_else(|| {
        AdapterError::ParseError("html script_json mode: missing 'status_field'".to_string())
    })?;

    let status_map = get_config_map(config, "status_map");

    let selector = Selector::parse(&script_selector_str).map_err(|e| {
        AdapterError::ParseError(format!("invalid script CSS selector: {e:?}"))
    })?;

    let document = Html::parse_document(html);
    let json_regex = Regex::new(&json_regex_str)
        .map_err(|e| AdapterError::ParseError(format!("invalid json_regex: {e}")))?;

    // Find the script tag whose text contains script_match.
    let script_content = document
        .select(&selector)
        .map(|el| el.text().collect::<Vec<_>>().join(""))
        .find(|text| text.contains(&script_match))
        .ok_or_else(|| {
            AdapterError::ParseError(format!(
                "no script tag found containing '{script_match}'"
            ))
        })?;

    // Extract JSON using the regex (first capture group).
    let json_str = json_regex
        .captures(&script_content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            AdapterError::ParseError("json_regex did not match script content".to_string())
        })?;

    let json_value: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| AdapterError::ParseError(format!("failed to parse extracted JSON: {e}")))?;

    // Evaluate the jq-style expression against the parsed JSON.
    let field_value = evaluate(&status_field, &json_value).map_err(|e| {
        AdapterError::ParseError(format!("failed to evaluate status_field expression: {e}"))
    })?;

    let extracted = match &field_value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let status = map_status(&extracted, &status_map);

    Ok(AdapterResult {
        status,
        description: extracted.clone(),
        data: json_value,
        components: vec![],
    })
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Runs the HTML adapter for the given watch configuration.
///
/// Fetches the URL, parses the HTML, then dispatches to the mode specified
/// by `adapter_config.mode` (`css` or `script_json`).
pub async fn run(watch: &Watch) -> Result<AdapterResult, AdapterError> {
    let url = watch
        .url
        .as_deref()
        .ok_or_else(|| AdapterError::ParseError("html adapter: missing 'url'".to_string()))?;

    let config = watch.adapter_config.as_ref().ok_or_else(|| {
        AdapterError::ParseError("html adapter: missing 'adapter_config'".to_string())
    })?;

    let mode = get_config_str(config, "mode").unwrap_or_else(|| "css".to_string());

    let client = build_client_with_headers(&watch.headers)?;
    let html_text = client
        .get(url)
        .send()
        .await
        .map_err(|e| AdapterError::HttpError(e.to_string()))?
        .text()
        .await
        .map_err(|e| AdapterError::HttpError(e.to_string()))?;

    match mode.as_str() {
        "css" => run_css_mode(&html_text, config),
        "script_json" => run_script_json_mode(&html_text, config),
        other => Err(AdapterError::ParseError(format!(
            "unknown html mode: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_config(pairs: &[(&str, serde_yaml::Value)]) -> HashMap<String, serde_yaml::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn str_val(s: &str) -> serde_yaml::Value {
        serde_yaml::Value::String(s.to_string())
    }

    fn map_val(pairs: &[(&str, &str)]) -> serde_yaml::Value {
        let mut m = serde_yaml::Mapping::new();
        for (k, v) in pairs {
            m.insert(str_val(k), str_val(v));
        }
        serde_yaml::Value::Mapping(m)
    }

    // -----------------------------------------------------------------------
    // CSS mode: extract class
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_extract_class_maps_to_success() {
        let html = r#"
            <html><body>
              <svg data-testid="heads-up" class="text-icon-operational icon-large"></svg>
            </body></html>
        "#;

        let config = make_config(&[
            ("mode", str_val("css")),
            ("selector", str_val("[data-testid='heads-up']")),
            ("extract", str_val("class")),
            (
                "status_map",
                map_val(&[
                    ("text-icon-operational", "success"),
                    ("text-icon-degraded-performance", "warning"),
                    ("text-icon-full-outage", "error"),
                    ("text-icon-under-maintenance", "maintenance"),
                ]),
            ),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Success);
        assert!(result.description.contains("text-icon-operational"));
    }

    #[test]
    fn css_mode_extract_class_maps_to_warning() {
        let html = r#"
            <html><body>
              <svg data-testid="heads-up" class="text-icon-degraded-performance"></svg>
            </body></html>
        "#;

        let config = make_config(&[
            ("mode", str_val("css")),
            ("selector", str_val("[data-testid='heads-up']")),
            ("extract", str_val("class")),
            (
                "status_map",
                map_val(&[
                    ("text-icon-operational", "success"),
                    ("text-icon-degraded-performance", "warning"),
                    ("text-icon-full-outage", "error"),
                    ("text-icon-under-maintenance", "maintenance"),
                ]),
            ),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Warning);
    }

    #[test]
    fn css_mode_extract_class_maps_to_maintenance() {
        let html = r#"
            <html><body>
              <div id="status" class="text-icon-under-maintenance foo"></div>
            </body></html>
        "#;

        let config = make_config(&[
            ("selector", str_val("#status")),
            ("extract", str_val("class")),
            (
                "status_map",
                map_val(&[
                    ("text-icon-operational", "success"),
                    ("text-icon-under-maintenance", "maintenance"),
                ]),
            ),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Maintenance);
    }

    // -----------------------------------------------------------------------
    // CSS mode: extract text
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_extract_text_maps_to_success() {
        let html = r#"
            <html><body>
              <p id="status-label">All Systems Operational</p>
            </body></html>
        "#;

        let config = make_config(&[
            ("selector", str_val("#status-label")),
            ("extract", str_val("text")),
            (
                "status_map",
                map_val(&[
                    ("Operational", "success"),
                    ("Degraded", "warning"),
                    ("Outage", "error"),
                ]),
            ),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Success);
        assert_eq!(result.description, "All Systems Operational");
    }

    #[test]
    fn css_mode_extract_text_default_when_no_extract_key() {
        let html = r#"<html><body><span class="indicator">Operational</span></body></html>"#;

        // No "extract" key: defaults to "text"
        let config = make_config(&[
            ("selector", str_val(".indicator")),
            ("status_map", map_val(&[("Operational", "success")])),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Success);
    }

    // -----------------------------------------------------------------------
    // CSS mode: extract custom attribute
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_extract_custom_attr_maps_to_error() {
        let html = r#"
            <html><body>
              <div id="svc" data-status="full-outage"></div>
            </body></html>
        "#;

        let config = make_config(&[
            ("selector", str_val("#svc")),
            ("extract", str_val("attr:data-status")),
            (
                "status_map",
                map_val(&[
                    ("operational", "success"),
                    ("degraded", "warning"),
                    ("full-outage", "error"),
                ]),
            ),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Error);
        assert_eq!(result.description, "full-outage");
    }

    #[test]
    fn css_mode_extract_attr_empty_when_attribute_missing() {
        let html = r#"<html><body><div id="svc"></div></body></html>"#;

        let config = make_config(&[
            ("selector", str_val("#svc")),
            ("extract", str_val("attr:data-status")),
            ("status_map", map_val(&[("operational", "success")])),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        // Attribute absent: extracted is empty, no map key matches -> Unknown
        assert_eq!(result.status, WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // CSS mode: missing element returns Unknown
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_missing_element_returns_unknown() {
        let html = r#"<html><body><p>No matching element here</p></body></html>"#;

        let config = make_config(&[
            ("selector", str_val(".does-not-exist")),
            ("extract", str_val("class")),
            ("status_map", map_val(&[("operational", "success")])),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed without error");
        assert_eq!(result.status, WatchStatus::Unknown);
        assert!(result.description.contains("no element matched"));
    }

    // -----------------------------------------------------------------------
    // CSS mode: no status_map match returns Unknown
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_no_status_map_match_returns_unknown() {
        let html = r#"<html><body><div id="s" class="some-other-class"></div></body></html>"#;

        let config = make_config(&[
            ("selector", str_val("#s")),
            ("extract", str_val("class")),
            ("status_map", map_val(&[("text-icon-operational", "success")])),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // CSS mode: missing selector key returns an error
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_missing_selector_returns_error() {
        let config = make_config(&[
            ("extract", str_val("text")),
            ("status_map", map_val(&[("ok", "success")])),
        ]);

        let result = run_css_mode("<html/>", &config);
        assert!(matches!(result, Err(AdapterError::ParseError(_))));
    }

    // -----------------------------------------------------------------------
    // CSS mode: unknown extract mode returns error
    // -----------------------------------------------------------------------

    #[test]
    fn css_mode_unknown_extract_mode_returns_error() {
        let html = r#"<html><body><div id="s" class="x"></div></body></html>"#;

        let config = make_config(&[
            ("selector", str_val("#s")),
            ("extract", str_val("invalid_mode")),
            ("status_map", map_val(&[])),
        ]);

        let result = run_css_mode(html, &config);
        assert!(matches!(result, Err(AdapterError::ParseError(_))));
    }

    // -----------------------------------------------------------------------
    // Script JSON mode
    // -----------------------------------------------------------------------

    fn script_json_html(status: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
  <script>
    var unrelated = {{}};
  </script>
  <script>
    window.statusData = {{"activeIncidents": [], "site": {{"status": "{status}"}}}};
  </script>
</head>
<body><p>Status page</p></body>
</html>"#
        )
    }

    #[test]
    fn script_json_mode_extracts_up_status() {
        let html = script_json_html("UP");

        let config = make_config(&[
            ("mode", str_val("script_json")),
            ("script_selector", str_val("script")),
            ("script_match", str_val("activeIncidents")),
            (
                "json_regex",
                str_val(r"window\.statusData\s*=\s*(\{.*\})"),
            ),
            ("status_field", str_val(".site.status")),
            (
                "status_map",
                map_val(&[("UP", "success"), ("HASISSUES", "warning")]),
            ),
        ]);

        let result = run_script_json_mode(&html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Success);
        assert_eq!(result.description, "UP");
    }

    #[test]
    fn script_json_mode_extracts_hasissues_status() {
        let html = script_json_html("HASISSUES");

        let config = make_config(&[
            ("script_selector", str_val("script")),
            ("script_match", str_val("activeIncidents")),
            (
                "json_regex",
                str_val(r"window\.statusData\s*=\s*(\{.*\})"),
            ),
            ("status_field", str_val(".site.status")),
            (
                "status_map",
                map_val(&[("UP", "success"), ("HASISSUES", "warning")]),
            ),
        ]);

        let result = run_script_json_mode(&html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Warning);
        assert_eq!(result.description, "HASISSUES");
    }

    #[test]
    fn script_json_mode_no_matching_script_tag_returns_error() {
        let html =
            r#"<html><head><script>var x = 1;</script></head><body></body></html>"#;

        let config = make_config(&[
            ("script_selector", str_val("script")),
            ("script_match", str_val("activeIncidents")),
            (
                "json_regex",
                str_val(r"window\.statusData\s*=\s*(\{.*\})"),
            ),
            ("status_field", str_val(".site.status")),
            ("status_map", map_val(&[("UP", "success")])),
        ]);

        let result = run_script_json_mode(html, &config);
        assert!(matches!(result, Err(AdapterError::ParseError(_))));
    }

    #[test]
    fn script_json_mode_regex_no_match_returns_error() {
        let html = r#"
            <html><head>
              <script>var activeIncidents = true; var other = "data";</script>
            </head></html>
        "#;

        let config = make_config(&[
            ("script_selector", str_val("script")),
            ("script_match", str_val("activeIncidents")),
            // Regex that will not match the script content.
            ("json_regex", str_val(r"window\.statusData\s*=\s*(\{.*\})")),
            ("status_field", str_val(".site.status")),
            ("status_map", map_val(&[("UP", "success")])),
        ]);

        let result = run_script_json_mode(html, &config);
        assert!(matches!(result, Err(AdapterError::ParseError(_))));
    }

    #[test]
    fn script_json_mode_missing_script_match_returns_error() {
        let config = make_config(&[
            ("script_selector", str_val("script")),
            ("json_regex", str_val(r"(\{.*\})")),
            ("status_field", str_val(".status")),
            ("status_map", map_val(&[("UP", "success")])),
        ]);

        let result = run_script_json_mode("<html/>", &config);
        assert!(matches!(result, Err(AdapterError::ParseError(_))));
    }

    #[test]
    fn script_json_mode_unknown_status_returns_unknown() {
        let html = script_json_html("UNKNOWN_VALUE");

        let config = make_config(&[
            ("script_selector", str_val("script")),
            ("script_match", str_val("activeIncidents")),
            (
                "json_regex",
                str_val(r"window\.statusData\s*=\s*(\{.*\})"),
            ),
            ("status_field", str_val(".site.status")),
            ("status_map", map_val(&[("UP", "success"), ("HASISSUES", "warning")])),
        ]);

        let result = run_script_json_mode(&html, &config).expect("should succeed");
        assert_eq!(result.status, WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // map_status: partial contains match
    // -----------------------------------------------------------------------

    #[test]
    fn map_status_partial_contains_match() {
        let mut status_map = HashMap::new();
        status_map.insert("operational".to_string(), "success".to_string());
        status_map.insert("degraded".to_string(), "warning".to_string());

        // Full class string containing the key
        assert_eq!(
            map_status("text-icon-operational some-other-class", &status_map),
            WatchStatus::Success
        );
        assert_eq!(
            map_status("text-icon-degraded-performance", &status_map),
            WatchStatus::Warning
        );
        assert_eq!(
            map_status("text-icon-unknown", &status_map),
            WatchStatus::Unknown
        );
    }

    #[test]
    fn map_status_empty_map_returns_unknown() {
        let status_map = HashMap::new();
        assert_eq!(map_status("operational", &status_map), WatchStatus::Unknown);
    }

    // -----------------------------------------------------------------------
    // Unknown html mode
    // -----------------------------------------------------------------------

    #[test]
    fn run_css_mode_with_data_field_in_result() {
        let html = r#"<html><body><span id="s">All Systems Operational</span></body></html>"#;

        let config = make_config(&[
            ("selector", str_val("#s")),
            ("extract", str_val("text")),
            ("status_map", map_val(&[("Operational", "success")])),
        ]);

        let result = run_css_mode(html, &config).expect("should succeed");
        assert_eq!(result.data["extracted"], "All Systems Operational");
    }
}
