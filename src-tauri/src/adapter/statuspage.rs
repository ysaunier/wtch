use super::{build_client_with_headers, AdapterError, AdapterResult, ComponentStatus};
use super::json_api::get_config_str;
use crate::config::Watch;
use crate::state::WatchStatus;

/// Maps a StatusPage top-level indicator string to a [`WatchStatus`].
fn map_indicator(indicator: &str) -> WatchStatus {
    match indicator {
        "none" => WatchStatus::Success,
        "minor" => WatchStatus::Warning,
        "major" => WatchStatus::Error,
        "critical" => WatchStatus::Error,
        "maintenance" => WatchStatus::Maintenance,
        _ => WatchStatus::Unknown,
    }
}

/// Maps a StatusPage component status string to a [`WatchStatus`].
fn map_component_status(status: &str) -> WatchStatus {
    match status {
        "operational" => WatchStatus::Success,
        "degraded_performance" => WatchStatus::Warning,
        "partial_outage" => WatchStatus::Warning,
        "major_outage" => WatchStatus::Error,
        "under_maintenance" => WatchStatus::Maintenance,
        _ => WatchStatus::Unknown,
    }
}

/// Returns the more severe of two statuses.
fn pick_worse(a: WatchStatus, b: WatchStatus) -> WatchStatus {
    let rank = |s: &WatchStatus| match s {
        WatchStatus::Success => 0,
        WatchStatus::Maintenance => 1,
        WatchStatus::Unknown => 2,
        WatchStatus::Warning => 3,
        WatchStatus::Error => 4,
    };
    if rank(&b) > rank(&a) { b } else { a }
}

/// Fetches `{url}/api/v2/summary.json` and parses the Atlassian StatusPage response.
///
/// Requires `watch.url` to be set. Returns an [`AdapterError::ParseError`] if
/// the URL is absent or the response body cannot be decoded as JSON.
pub async fn run(watch: &Watch) -> Result<AdapterResult, AdapterError> {
    let base_url = watch.url.as_deref().ok_or_else(|| {
        AdapterError::ParseError("statuspage adapter requires a url".to_string())
    })?;

    let url = format!("{}/api/v2/summary.json", base_url.trim_end_matches('/'));

    let client = build_client_with_headers(&watch.headers)?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AdapterError::HttpError(e.to_string()))?;

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let indicator = data
        .pointer("/status/indicator")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let description = data
        .pointer("/status/description")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let status = map_indicator(indicator);

    let adapter_config = watch.adapter_config.as_ref();
    let group_id = adapter_config.and_then(|c| get_config_str(c, "group_id"));
    let group_filter = adapter_config.and_then(|c| get_config_str(c, "group_filter"));

    // When group_filter is set, collect all group IDs whose name starts with the prefix.
    // Then show each matching group as a component (with aggregated status from its children).
    let matched_group_ids: Option<Vec<String>> = group_filter.as_ref().map(|prefix| {
        data.pointer("/components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let is_group = c.get("group").and_then(|v| v.as_bool()).unwrap_or(false);
                        if !is_group { return None; }
                        let name = c.get("name")?.as_str()?;
                        if name.starts_with(prefix.as_str()) {
                            Some(c.get("id")?.as_str()?.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    });

    let components: Vec<ComponentStatus> = if let Some(ref gids) = matched_group_ids {
        // Show one component per matching group, with worst status from its children
        let all_components = data.pointer("/components").and_then(|v| v.as_array());
        gids.iter()
            .filter_map(|gid| {
                let all = all_components?;
                let group_name = all.iter()
                    .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(gid))
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())?
                    .to_string();
                let children: Vec<_> = all.iter()
                    .filter(|c| c.get("group_id").and_then(|v| v.as_str()) == Some(gid))
                    .collect();
                let worst = children.iter().fold(WatchStatus::Success, |acc, c| {
                    let s = c.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    pick_worse(acc, map_component_status(s))
                });
                let desc = if worst == WatchStatus::Success {
                    "operational".to_string()
                } else {
                    children.iter()
                        .find(|c| {
                            let s = c.get("status").and_then(|v| v.as_str()).unwrap_or("operational");
                            s != "operational"
                        })
                        .and_then(|c| {
                            let n = c.get("name")?.as_str()?;
                            let s = c.get("status")?.as_str()?;
                            Some(format!("{}: {}", n, s.replace('_', " ")))
                        })
                        .unwrap_or_else(|| "affected".to_string())
                };
                Some(ComponentStatus { name: group_name, status: worst, description: desc })
            })
            .collect()
    } else {
        data.pointer("/components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let name = c.get("name")?.as_str()?.to_string();
                        if name.starts_with("Visit ") {
                            return None;
                        }
                        // Skip group headers
                        if c.get("group").and_then(|v| v.as_bool()).unwrap_or(false) {
                            return None;
                        }
                        if let Some(gid) = &group_id {
                            let cg = c.get("group_id").and_then(|v| v.as_str());
                            if cg != Some(gid.as_str()) {
                                return None;
                            }
                        }
                        let comp_status = c
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let ws = map_component_status(comp_status);
                        let desc = comp_status.replace('_', " ");
                        Some(ComponentStatus { name, status: ws, description: desc })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let has_filter = group_id.is_some() || matched_group_ids.is_some();
    let (status, description) = if has_filter {
        let worst = components
            .iter()
            .map(|c| &c.status)
            .fold(WatchStatus::Success, |acc, s| pick_worse(acc, s.clone()));
        let desc = if worst == WatchStatus::Success {
            "All services operational".to_string()
        } else {
            components
                .iter()
                .find(|c| c.status != WatchStatus::Success)
                .map(|c| format!("{}: {}", c.name, c.description))
                .unwrap_or(description)
        };
        (worst, desc)
    } else {
        (status, description)
    };

    Ok(AdapterResult {
        status,
        description,
        data,
        components,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- indicator mapping ---

    #[test]
    fn test_map_indicator_none_is_success() {
        assert_eq!(map_indicator("none"), WatchStatus::Success);
    }

    #[test]
    fn test_map_indicator_minor_is_warning() {
        assert_eq!(map_indicator("minor"), WatchStatus::Warning);
    }

    #[test]
    fn test_map_indicator_major_is_error() {
        assert_eq!(map_indicator("major"), WatchStatus::Error);
    }

    #[test]
    fn test_map_indicator_critical_is_error() {
        assert_eq!(map_indicator("critical"), WatchStatus::Error);
    }

    #[test]
    fn test_map_indicator_maintenance() {
        assert_eq!(map_indicator("maintenance"), WatchStatus::Maintenance);
    }

    #[test]
    fn test_map_indicator_unknown_string_is_unknown() {
        assert_eq!(map_indicator("some_future_value"), WatchStatus::Unknown);
    }

    // --- component status mapping ---

    #[test]
    fn test_map_component_operational() {
        assert_eq!(map_component_status("operational"), WatchStatus::Success);
    }

    #[test]
    fn test_map_component_degraded() {
        assert_eq!(
            map_component_status("degraded_performance"),
            WatchStatus::Warning
        );
    }

    #[test]
    fn test_map_component_partial_outage() {
        assert_eq!(
            map_component_status("partial_outage"),
            WatchStatus::Warning
        );
    }

    #[test]
    fn test_map_component_major_outage() {
        assert_eq!(map_component_status("major_outage"), WatchStatus::Error);
    }

    #[test]
    fn test_map_component_maintenance() {
        assert_eq!(
            map_component_status("under_maintenance"),
            WatchStatus::Maintenance
        );
    }

    #[test]
    fn test_map_component_unknown_string_is_unknown() {
        assert_eq!(map_component_status("pending"), WatchStatus::Unknown);
    }

    // --- component parsing from JSON ---

    #[test]
    fn test_components_parsed_from_json() {
        let data = json!({
            "status": { "indicator": "minor", "description": "Partial system outage" },
            "components": [
                { "name": "API", "status": "operational" },
                { "name": "Dashboard", "status": "degraded_performance" },
                { "name": "Visit https://example.com", "status": "operational" }
            ]
        });

        let indicator = data
            .pointer("/status/indicator")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        assert_eq!(map_indicator(indicator), WatchStatus::Warning);

        let components: Vec<ComponentStatus> = data
            .pointer("/components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let name = c.get("name")?.as_str()?.to_string();
                        if name.starts_with("Visit ") {
                            return None;
                        }
                        let comp_status = c
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let ws = map_component_status(comp_status);
                        let desc = comp_status.replace('_', " ");
                        Some(ComponentStatus {
                            name,
                            status: ws,
                            description: desc,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "API");
        assert_eq!(components[0].status, WatchStatus::Success);
        assert_eq!(components[1].name, "Dashboard");
        assert_eq!(components[1].status, WatchStatus::Warning);
        assert_eq!(components[1].description, "degraded performance");
    }

    #[test]
    fn test_visit_component_is_filtered_out() {
        let data = json!({
            "components": [
                { "name": "Visit https://status.example.com", "status": "operational" },
                { "name": "Real Service", "status": "major_outage" }
            ]
        });

        let components: Vec<_> = data
            .pointer("/components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let name = c.get("name")?.as_str()?.to_string();
                        if name.starts_with("Visit ") {
                            return None;
                        }
                        Some(name)
                    })
                    .collect()
            })
            .unwrap_or_default();

        assert_eq!(components, vec!["Real Service"]);
    }

    #[test]
    fn test_missing_components_array_returns_empty_vec() {
        let data = json!({
            "status": { "indicator": "none", "description": "All good" }
        });

        let components: Vec<ComponentStatus> = data
            .pointer("/components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let name = c.get("name")?.as_str()?.to_string();
                        let comp_status = c
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        Some(ComponentStatus {
                            name,
                            status: map_component_status(comp_status),
                            description: comp_status.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        assert!(components.is_empty());
    }

    #[test]
    fn test_description_underscores_replaced_with_spaces() {
        let desc = "degraded_performance".replace('_', " ");
        assert_eq!(desc, "degraded performance");
    }
}
