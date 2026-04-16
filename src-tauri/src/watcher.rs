use chrono::Utc;
use regex::Regex;

use crate::config::{Config, Display, DisplayStyle, Watch, WatchType};
use crate::expr::evaluate_bool;
use crate::provider::{self, ProviderResult};
use crate::state::{DetailState, DisplayState, ListItemState, WatchState, WatchStatus};

// ---------------------------------------------------------------------------
// Template resolution
// ---------------------------------------------------------------------------

/// Replaces all `{{ .field }}` and `{{ .nested.field }}` placeholders in
/// `template` with values extracted from `data`.
///
/// If a field cannot be found or the value is not a primitive, the placeholder
/// is replaced with an empty string.
pub fn resolve_template(template: &str, data: &serde_json::Value) -> String {
    // Match {{ .anything.possibly.nested }} with optional array indices like [0].
    let re = Regex::new(r"\{\{\s*(\.[a-zA-Z0-9_.\[\]]*)\s*\}\}").unwrap();

    re.replace_all(template, |caps: &regex::Captures| {
        let path = &caps[1]; // e.g. ".cost", ".nested.field", ".bars[0].label"
        json_value_at_path(path, data)
            .map(primitive_to_string)
            .unwrap_or_default()
    })
    .into_owned()
}

/// Walks a dot-separated path like `.cost`, `.nested.field`, or `.bars[0].label`
/// inside `data` and returns the value at that path, or `None` if any segment
/// is missing. Supports array indexing with `[N]` syntax.
fn json_value_at_path<'a>(path: &str, data: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
    let stripped = path.strip_prefix('.').unwrap_or(path);
    if stripped.is_empty() {
        return Some(data);
    }
    let mut current = data;
    for segment in stripped.split('.') {
        if segment.is_empty() {
            continue;
        }
        if let Some(bracket_pos) = segment.find('[') {
            let key = &segment[..bracket_pos];
            if !key.is_empty() {
                current = current.get(key)?;
            }
            let mut rest = &segment[bracket_pos..];
            while let Some(start) = rest.find('[') {
                let end = rest.find(']')?;
                let idx: usize = rest[start + 1..end].parse().ok()?;
                current = current.get(idx)?;
                rest = &rest[end + 1..];
            }
        } else {
            current = current.get(segment)?;
        }
    }
    Some(current)
}

/// Converts a JSON primitive to a display string.
///
/// Numbers that are whole values are displayed without a decimal point
/// (e.g. `45.0` -> `"45"`). Arrays and objects fall back to an empty string.
fn primitive_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => {
            // Prefer the integer representation when the number is whole.
            if let Some(i) = n.as_i64() {
                return i.to_string();
            }
            if let Some(u) = n.as_u64() {
                return u.to_string();
            }
            if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    return (f as i64).to_string();
                }
                return f.to_string();
            }
            n.to_string()
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        _ => String::new(),
    }
}

/// Evaluates a numeric expression/field path against `data` and returns an
/// `f64`, or `None` if evaluation fails or the result is not a number.
fn resolve_f64(expr: &str, data: &serde_json::Value) -> Option<f64> {
    // If it looks like a plain number literal, parse directly.
    if let Ok(n) = expr.trim().parse::<f64>() {
        return Some(n);
    }
    // Otherwise evaluate as an expression.
    crate::expr::evaluate(expr, data)
        .ok()
        .and_then(|v| v.as_f64())
}

/// Evaluates a value expression against `data` and returns the JSON value.
/// Returns `None` if evaluation fails.
fn resolve_value(expr: &str, data: &serde_json::Value) -> Option<serde_json::Value> {
    // Plain number literal.
    if let Ok(n) = expr.trim().parse::<f64>() {
        return Some(serde_json::json!(n));
    }
    crate::expr::evaluate(expr, data).ok()
}

// ---------------------------------------------------------------------------
// Status determination
// ---------------------------------------------------------------------------

/// Determines the [`WatchStatus`] from evaluated conditions.
///
/// Conditions are checked in priority order: error > warning > success.
/// The first expression that evaluates to true is used. If none match,
/// `Unknown` is returned.
fn status_from_conditions(
    conditions: &crate::config::Conditions,
    data: &serde_json::Value,
) -> WatchStatus {
    if let Some(expr) = &conditions.error {
        if evaluate_bool(expr, data).unwrap_or(false) {
            return WatchStatus::Error;
        }
    }
    if let Some(expr) = &conditions.warning {
        if evaluate_bool(expr, data).unwrap_or(false) {
            return WatchStatus::Warning;
        }
    }
    if let Some(expr) = &conditions.maintenance {
        if evaluate_bool(expr, data).unwrap_or(false) {
            return WatchStatus::Maintenance;
        }
    }
    if let Some(expr) = &conditions.success {
        if evaluate_bool(expr, data).unwrap_or(false) {
            return WatchStatus::Success;
        }
    }
    WatchStatus::Unknown
}

/// Derives a default [`WatchStatus`] from a [`ProviderResult`] when no
/// conditions are configured.
///
/// - Script: exit code 0 -> Success, else Error.
/// - HTTP: `up == true` -> Success, else Error.
fn default_status(result: &ProviderResult, watch_type: &WatchType) -> WatchStatus {
    match watch_type {
        WatchType::Script => match result.exit_code {
            Some(0) => WatchStatus::Success,
            _ => WatchStatus::Error,
        },
        WatchType::Http | WatchType::Statuspage | WatchType::Adapter => {
            if result.data.get("up").and_then(|v| v.as_bool()).unwrap_or(false) {
                WatchStatus::Success
            } else {
                WatchStatus::Error
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Display resolution
// ---------------------------------------------------------------------------

/// Builds a [`DisplayState`] from a config [`Display`] block and resolved data.
fn resolve_display(display: &Display, data: &serde_json::Value) -> DisplayState {
    let style = style_name(display.style.as_ref());
    let title = display
        .title
        .as_deref()
        .map(|t| resolve_template(t, data));

    let description = display
        .description
        .as_deref()
        .map(|d| resolve_template(d, data));

    let content = display
        .content
        .as_deref()
        .map(|c| resolve_template(c, data));

    let value = display
        .value
        .as_deref()
        .and_then(|expr| resolve_value(expr, data));

    let min = display.min.as_deref().and_then(|e| resolve_f64(e, data));
    let max = display.max.as_deref().and_then(|e| resolve_f64(e, data));

    // Resolve list items if configured.
    let items = resolve_items(display, data);

    DisplayState {
        style,
        title,
        description,
        content,
        value,
        min,
        max,
        items,
    }
}

/// Returns the lowercase style name for serialization.
fn style_name(style: Option<&DisplayStyle>) -> String {
    match style {
        Some(DisplayStyle::Status) => "status".to_string(),
        Some(DisplayStyle::Progress) => "progress".to_string(),
        Some(DisplayStyle::Value) => "value".to_string(),
        Some(DisplayStyle::Text) => "text".to_string(),
        Some(DisplayStyle::List) => "list".to_string(),
        None => "status".to_string(),
    }
}

/// Resolves list items from a Display config.
///
/// `items` is an expression that should evaluate to a JSON array.
/// `item_title` and `item_status` are expressions evaluated per-item.
fn resolve_items(display: &Display, data: &serde_json::Value) -> Option<Vec<ListItemState>> {
    let items_expr = display.items.as_deref()?;

    let array = crate::expr::evaluate(items_expr, data)
        .ok()
        .and_then(|v| {
            if let serde_json::Value::Array(a) = v {
                Some(a)
            } else {
                None
            }
        })?;

    let item_title_expr = display.item_title.as_deref();
    let item_status_expr = display.item_status.as_deref();

    let items: Vec<ListItemState> = array
        .into_iter()
        .map(|item| {
            let title = item_title_expr
                .and_then(|expr| resolve_value(expr, &item))
                .and_then(|v| {
                    if let serde_json::Value::String(s) = v {
                        Some(s)
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let status = item_status_expr
                .and_then(|expr| {
                    let result = evaluate_bool(expr, &item);
                    result.ok()
                })
                .map(|is_ok| if is_ok { WatchStatus::Success } else { WatchStatus::Error })
                .unwrap_or(WatchStatus::Unknown);

            ListItemState { title, status }
        })
        .collect();

    Some(items)
}

/// Builds a fallback [`DisplayState`] when no display config is provided.
fn fallback_display(_status: &WatchStatus) -> DisplayState {
    DisplayState {
        style: "status".to_string(),
        title: None,
        description: None,
        content: None,
        value: None,
        min: None,
        max: None,
        items: None,
    }
}

// ---------------------------------------------------------------------------
// Detail resolution
// ---------------------------------------------------------------------------

fn resolve_details(details: &[crate::config::Details], data: &serde_json::Value) -> Vec<DetailState> {
    details
        .iter()
        .map(|d| {
            let style = style_name(d.style.as_ref());

            let title = d.title.as_deref().map(|t| resolve_template(t, data));
            let description = d.description.as_deref().map(|desc| resolve_template(desc, data));
            let value = d.value.as_deref().and_then(|expr| resolve_value(expr, data));
            let min = d.min.as_deref().and_then(|e| resolve_f64(e, data));
            let max = d.max.as_deref().and_then(|e| resolve_f64(e, data));

            DetailState {
                title,
                style,
                description,
                value,
                min,
                max,
            }
        })
        .collect()
}

/// Maps statuspage-style status strings to WatchStatus.
fn status_string_to_watch_status(s: &str) -> WatchStatus {
    match s {
        "operational" => WatchStatus::Success,
        "degraded_performance" => WatchStatus::Warning,
        "partial_outage" => WatchStatus::Warning,
        "major_outage" => WatchStatus::Error,
        "under_maintenance" => WatchStatus::Maintenance,
        _ => WatchStatus::Unknown,
    }
}

/// Resolves dynamic details from a JSON array path.
fn resolve_dynamic_details(
    display: &crate::config::Display,
    details_from: &str,
    data: &serde_json::Value,
) -> Vec<DetailState> {
    let title_field = display.detail_title.as_deref().unwrap_or(".name");
    let status_field = display.detail_status.as_deref().unwrap_or(".status");
    let detail_style = display.detail_style.as_ref();
    let value_field = display.detail_value.as_deref();
    let max_field = display.detail_max.as_deref();
    let desc_field = display.detail_description.as_deref();

    let array = match crate::expr::evaluate(details_from, data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let items = match array.as_array() {
        Some(arr) => arr,
        None => return vec![],
    };

    items
        .iter()
        .filter_map(|item| {
            let title = crate::expr::evaluate(title_field, item)
                .ok()
                .and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => v.as_str().map(|s| s.to_string()),
                });

            let is_progress = detail_style == Some(&crate::config::DisplayStyle::Progress);

            let status_str = if is_progress {
                String::new()
            } else {
                crate::expr::evaluate(status_field, item)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default()
            };

            let status = if is_progress {
                WatchStatus::Success
            } else {
                status_string_to_watch_status(&status_str)
            };
            if is_progress {
                let value = value_field
                    .and_then(|expr| resolve_value(expr, item));
                let max = max_field
                    .and_then(|e| resolve_f64(e, item));
                let description = desc_field
                    .map(|d| resolve_template(d, item));

                return title.map(|t| DetailState {
                    title: Some(t),
                    style: "progress".to_string(),
                    description,
                    value,
                    min: None,
                    max,
                });
            }

            let description = Some(status_str.replace('_', " "));

            title.map(|t| DetailState {
                title: Some(t),
                style: "status".to_string(),
                description,
                value: Some(serde_json::Value::String(
                    match &status {
                        WatchStatus::Success => "success",
                        WatchStatus::Warning => "warning",
                        WatchStatus::Error => "error",
                        WatchStatus::Maintenance => "maintenance",
                        WatchStatus::Unknown => "unknown",
                    }
                    .to_string(),
                )),
                min: None,
                max: None,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Watch evaluation
// ---------------------------------------------------------------------------

/// Evaluates a single watch configuration and returns its current [`WatchState`].
///
/// Provider errors result in a state with `status = Error` and a populated
/// `error_message` field.
pub async fn evaluate_watch(watch: &Watch, config: &Config) -> WatchState {
    let name = watch
        .name
        .clone()
        .unwrap_or_else(|| watch.preset.clone().unwrap_or_else(|| "unnamed".to_string()));

    let watch_type = watch
        .watch_type
        .clone()
        .unwrap_or(WatchType::Script);

    let provider_result = run_provider(watch, config, &watch_type).await;

    let last_check = Some(Utc::now().to_rfc3339());

    match provider_result {
        Err(ref err) => {
            crate::logging::warn(&format!("[watch] {name}: provider error: {err}"));
            let display = watch
                .display
                .as_ref()
                .map(|d| resolve_display(d, &serde_json::Value::Null))
                .unwrap_or_else(|| fallback_display(&WatchStatus::Error));

            WatchState {
                name,
                status: WatchStatus::Error,
                data: serde_json::Value::Null,
                display,
                details: vec![],
                expand: watch.expand,
                url: watch.link.clone().or_else(|| watch.url.clone()),
                last_check,
                error_message: Some(err.to_string()),
            }
        }
        Ok(result) => {
            let data = result.data.clone();
            crate::logging::debug(&format!("[watch] {name}: status={:?}", result.exit_code));

            let has_conditions = watch.conditions.error.is_some()
                || watch.conditions.warning.is_some()
                || watch.conditions.maintenance.is_some()
                || watch.conditions.success.is_some();

            let status = if has_conditions {
                status_from_conditions(&watch.conditions, &data)
            } else {
                default_status(&result, &watch_type)
            };

            let display = watch
                .display
                .as_ref()
                .map(|d| resolve_display(d, &data))
                .unwrap_or_else(|| fallback_display(&status));

            let mut details = resolve_details(&watch.details, &data);

            // Dynamic details from a JSON array (e.g. components from statuspage)
            if let Some(display_cfg) = &watch.display {
                if let Some(details_from) = &display_cfg.details_from {
                    let dynamic = resolve_dynamic_details(display_cfg, details_from, &data);
                    details.extend(dynamic);
                }
            }

            WatchState {
                name,
                status,
                data,
                display,
                details,
                expand: watch.expand,
                url: watch.link.clone().or_else(|| watch.url.clone()),
                last_check,
                error_message: None,
            }
        }
    }
}

/// Dispatches to the appropriate provider based on `watch_type`.
async fn run_provider(
    watch: &Watch,
    config: &Config,
    watch_type: &WatchType,
) -> Result<ProviderResult, provider::ProviderError> {
    match watch_type {
        WatchType::Script => provider::script::run(watch, &config.shells).await,
        WatchType::Http | WatchType::Statuspage => provider::http::run(watch).await,
        WatchType::Adapter => run_adapter_provider(watch).await,
    }
}

/// Runs an adapter and converts its result into a ProviderResult.
async fn run_adapter_provider(watch: &Watch) -> Result<ProviderResult, provider::ProviderError> {
    let result = crate::adapter::run_adapter(watch)
        .await
        .map_err(|e| provider::ProviderError::ExecutionFailed(e.to_string()))?;

    // Build JSON data with status, description, and components
    let components_json: Vec<serde_json::Value> = result
        .components
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "status": match &c.status {
                    WatchStatus::Success => "operational",
                    WatchStatus::Warning => "degraded_performance",
                    WatchStatus::Error => "major_outage",
                    WatchStatus::Maintenance => "under_maintenance",
                    WatchStatus::Unknown => "unknown",
                },
                "description": c.description,
            })
        })
        .collect();

    let data = serde_json::json!({
        "up": result.status == WatchStatus::Success
            || result.status == WatchStatus::Warning
            || result.status == WatchStatus::Maintenance,
        "status": {
            "indicator": match &result.status {
                WatchStatus::Success => "none",
                WatchStatus::Warning => "minor",
                WatchStatus::Error => "major",
                WatchStatus::Maintenance => "maintenance",
                WatchStatus::Unknown => "unknown",
            },
            "description": result.description,
        },
        "components": components_json,
    });

    Ok(ProviderResult {
        data,
        exit_code: Some(0),
        latency_ms: None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- Template resolution -------------------------------------------------

    #[test]
    fn test_resolve_template_simple_field() {
        let data = json!({"cost": 42});
        assert_eq!(resolve_template("Cost: {{ .cost }}", &data), "Cost: 42");
    }

    #[test]
    fn test_resolve_template_nested_field() {
        let data = json!({"billing": {"cost": 99.5}});
        assert_eq!(
            resolve_template("Amount: {{ .billing.cost }}", &data),
            "Amount: 99.5"
        );
    }

    #[test]
    fn test_resolve_template_multiple_placeholders() {
        let data = json!({"cost": 10, "limit": 100});
        assert_eq!(
            resolve_template("${{ .cost }} / ${{ .limit }}", &data),
            "$10 / $100"
        );
    }

    #[test]
    fn test_resolve_template_missing_field_becomes_empty() {
        let data = json!({"cost": 10});
        assert_eq!(
            resolve_template("Val: {{ .missing }}", &data),
            "Val: "
        );
    }

    #[test]
    fn test_resolve_template_string_value() {
        let data = json!({"status": "operational"});
        assert_eq!(
            resolve_template("Status: {{ .status }}", &data),
            "Status: operational"
        );
    }

    #[test]
    fn test_resolve_template_bool_value() {
        let data = json!({"up": true});
        assert_eq!(resolve_template("Up: {{ .up }}", &data), "Up: true");
    }

    #[test]
    fn test_resolve_template_null_value_becomes_empty() {
        let data = json!({"val": null});
        assert_eq!(resolve_template("{{ .val }}", &data), "");
    }

    #[test]
    fn test_resolve_template_no_placeholders() {
        let data = json!({});
        assert_eq!(resolve_template("Hello world", &data), "Hello world");
    }

    #[test]
    fn test_resolve_template_extra_whitespace_in_placeholder() {
        let data = json!({"x": 7});
        assert_eq!(resolve_template("{{  .x  }}", &data), "7");
    }

    // -- Path resolution -------------------------------------------------

    #[test]
    fn test_json_value_at_path_top_level() {
        let data = json!({"a": 1});
        assert_eq!(json_value_at_path(".a", &data), Some(&json!(1)));
    }

    #[test]
    fn test_json_value_at_path_nested() {
        let data = json!({"a": {"b": {"c": "deep"}}});
        assert_eq!(
            json_value_at_path(".a.b.c", &data),
            Some(&json!("deep"))
        );
    }

    #[test]
    fn test_json_value_at_path_missing() {
        let data = json!({"a": 1});
        assert_eq!(json_value_at_path(".b", &data), None);
    }

    // -- Status from conditions ----------------------------------------------

    #[test]
    fn test_status_error_condition_wins() {
        let conditions = crate::config::Conditions {
            error: Some(".cost > 100".to_string()),
            warning: Some(".cost > 80".to_string()),
            maintenance: None,
            success: Some(".cost <= 80".to_string()),
        };
        let data = json!({"cost": 110});
        assert_eq!(
            status_from_conditions(&conditions, &data),
            WatchStatus::Error
        );
    }

    #[test]
    fn test_status_warning_when_not_error() {
        let conditions = crate::config::Conditions {
            error: Some(".cost > 100".to_string()),
            warning: Some(".cost > 80".to_string()),
            maintenance: None,
            success: Some(".cost <= 80".to_string()),
        };
        let data = json!({"cost": 90});
        assert_eq!(
            status_from_conditions(&conditions, &data),
            WatchStatus::Warning
        );
    }

    #[test]
    fn test_status_success_when_all_clear() {
        let conditions = crate::config::Conditions {
            error: Some(".cost > 100".to_string()),
            warning: Some(".cost > 80".to_string()),
            maintenance: None,
            success: Some(".cost <= 80".to_string()),
        };
        let data = json!({"cost": 50});
        assert_eq!(
            status_from_conditions(&conditions, &data),
            WatchStatus::Success
        );
    }

    #[test]
    fn test_status_unknown_when_no_condition_matches() {
        let conditions = crate::config::Conditions {
            error: Some(".missing > 100".to_string()),
            warning: Some(".missing > 80".to_string()),
            maintenance: None,
            success: Some(".missing <= 80".to_string()),
        };
        // All expressions will fail (missing field) -> all evaluate to false -> Unknown.
        let data = json!({"cost": 50});
        assert_eq!(
            status_from_conditions(&conditions, &data),
            WatchStatus::Unknown
        );
    }

    // -- Default status (no conditions) ------------------------------------

    #[test]
    fn test_default_status_script_exit_0_is_success() {
        let result = provider::ProviderResult {
            data: json!({}),
            exit_code: Some(0),
            latency_ms: None,
        };
        assert_eq!(default_status(&result, &WatchType::Script), WatchStatus::Success);
    }

    #[test]
    fn test_default_status_script_nonzero_exit_is_error() {
        let result = provider::ProviderResult {
            data: json!({}),
            exit_code: Some(1),
            latency_ms: None,
        };
        assert_eq!(default_status(&result, &WatchType::Script), WatchStatus::Error);
    }

    #[test]
    fn test_default_status_http_up_is_success() {
        let result = provider::ProviderResult {
            data: json!({"up": true, "status": 200}),
            exit_code: None,
            latency_ms: Some(50),
        };
        assert_eq!(default_status(&result, &WatchType::Http), WatchStatus::Success);
    }

    #[test]
    fn test_default_status_http_down_is_error() {
        let result = provider::ProviderResult {
            data: json!({"up": false, "status": 503}),
            exit_code: None,
            latency_ms: Some(100),
        };
        assert_eq!(default_status(&result, &WatchType::Http), WatchStatus::Error);
    }

    // -- Display resolution --------------------------------------------------

    #[test]
    fn test_resolve_display_templates_interpolated() {
        let display = Display {
            style: Some(DisplayStyle::Progress),
            title: Some("Claude".to_string()),
            description: Some("${{ .cost }} / ${{ .limit }}".to_string()),
            value: Some(".cost".to_string()),
            max: Some(".limit".to_string()),
            ..Default::default()
        };
        let data = json!({"cost": 45.0, "limit": 100.0});
        let state = resolve_display(&display, &data);

        assert_eq!(state.style, "progress");
        assert_eq!(state.title.as_deref(), Some("Claude"));
        assert_eq!(state.description.as_deref(), Some("$45 / $100"));
        assert_eq!(state.value, Some(json!(45.0)));
        assert_eq!(state.max, Some(100.0));
    }

    #[test]
    fn test_resolve_display_missing_data_produces_empty_strings() {
        let display = Display {
            style: Some(DisplayStyle::Text),
            title: Some("{{ .missing }}".to_string()),
            ..Default::default()
        };
        let data = json!({});
        let state = resolve_display(&display, &data);
        assert_eq!(state.title.as_deref(), Some(""));
    }

    // -- resolve_f64 --------------------------------------------------------

    #[test]
    fn test_resolve_f64_literal() {
        assert_eq!(resolve_f64("100", &json!({})), Some(100.0));
    }

    #[test]
    fn test_resolve_f64_field_path() {
        assert_eq!(resolve_f64(".limit", &json!({"limit": 50.0})), Some(50.0));
    }

    #[test]
    fn test_resolve_f64_missing_returns_none() {
        assert_eq!(resolve_f64(".missing", &json!({})), None);
    }
}

#[cfg(test)]
mod template_array_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_template_with_array_index() {
        let data = json!({"bars": [{"percentage": 62, "reset": "in 30 min"}]});
        let result = resolve_template("{{ .bars[0].percentage }}% ({{ .bars[0].reset }})", &data);
        assert_eq!(result, "62% (in 30 min)");
    }

    #[test]
    fn test_json_value_at_path_array_index() {
        let data = json!({"bars": [{"percentage": 62}, {"percentage": 13}]});
        let val = json_value_at_path(".bars[0].percentage", &data);
        assert_eq!(val, Some(&json!(62)));
    }

    #[test]
    fn test_json_value_at_path_second_index() {
        let data = json!({"bars": [{"label": "a"}, {"label": "b"}]});
        let val = json_value_at_path(".bars[1].label", &data);
        assert_eq!(val, Some(&json!("b")));
    }
}
