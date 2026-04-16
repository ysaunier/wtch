use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The evaluated status of a single watch entry.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Success,
    Warning,
    Error,
    Maintenance,
    Unknown,
}

/// Resolved state of a single list item inside a List-style display block.
#[derive(Debug, Clone, Serialize)]
pub struct ListItemState {
    pub title: String,
    pub status: WatchStatus,
}

/// Resolved display block (templates interpolated, expressions evaluated).
#[derive(Debug, Clone, Serialize)]
pub struct DisplayState {
    pub style: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub value: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub items: Option<Vec<ListItemState>>,
}

/// Resolved detail panel state.
#[derive(Debug, Clone, Serialize)]
pub struct DetailState {
    pub title: Option<String>,
    pub style: String,
    pub description: Option<String>,
    pub value: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// Full evaluated state for one watch entry.
#[derive(Debug, Clone, Serialize)]
pub struct WatchState {
    pub name: String,
    pub status: WatchStatus,
    /// Raw provider output data.
    pub data: serde_json::Value,
    /// Resolved display block (templates interpolated).
    pub display: DisplayState,
    /// Resolved detail panels.
    pub details: Vec<DetailState>,
    pub expand: bool,
    pub url: Option<String>,
    /// ISO 8601 timestamp of the last successful check.
    pub last_check: Option<String>,
    /// Human-readable error message when the provider failed.
    pub error_message: Option<String>,
}

/// Full application state exposed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AppState {
    pub watches: Vec<WatchState>,
    /// ISO 8601 timestamp of the last scheduler tick.
    pub last_check: Option<String>,
    /// Group definitions mapping group name to watch names.
    #[serde(default)]
    pub groups: std::collections::HashMap<String, Vec<String>>,
}

/// Thread-safe shared handle to [`AppState`].
pub type SharedState = Arc<RwLock<AppState>>;

/// Creates a new empty [`SharedState`].
pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState {
        watches: Vec::new(),
        last_check: None,
        groups: std::collections::HashMap::new(),
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_display_state() -> DisplayState {
        DisplayState {
            style: "status".to_string(),
            title: Some("My Watch".to_string()),
            description: Some("All good".to_string()),
            content: None,
            value: None,
            min: None,
            max: None,
            items: None,
        }
    }

    fn make_watch_state(status: WatchStatus) -> WatchState {
        WatchState {
            name: "test-watch".to_string(),
            status,
            data: json!({"cost": 1.5}),
            display: make_display_state(),
            details: vec![],
            expand: false,
            url: Some("https://example.com".to_string()),
            last_check: Some("2026-04-15T00:00:00Z".to_string()),
            error_message: None,
        }
    }

    #[test]
    fn test_watch_status_serializes_as_snake_case() {
        let cases = vec![
            (WatchStatus::Success, "\"success\""),
            (WatchStatus::Warning, "\"warning\""),
            (WatchStatus::Error, "\"error\""),
            (WatchStatus::Unknown, "\"unknown\""),
        ];
        for (status, expected) in cases {
            let serialized = serde_json::to_string(&status).unwrap();
            assert_eq!(serialized, expected);
        }
    }

    #[test]
    fn test_watch_state_serializes_fields() {
        let state = make_watch_state(WatchStatus::Success);
        let json = serde_json::to_value(&state).unwrap();

        assert_eq!(json["name"], "test-watch");
        assert_eq!(json["status"], "success");
        assert_eq!(json["data"]["cost"], 1.5);
        assert_eq!(json["expand"], false);
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["last_check"], "2026-04-15T00:00:00Z");
        assert!(json["error_message"].is_null());
    }

    #[test]
    fn test_watch_state_error_status_serialization() {
        let mut state = make_watch_state(WatchStatus::Error);
        state.error_message = Some("provider timed out".to_string());
        let json = serde_json::to_value(&state).unwrap();

        assert_eq!(json["status"], "error");
        assert_eq!(json["error_message"], "provider timed out");
    }

    #[test]
    fn test_display_state_with_items_serializes() {
        let items = vec![
            ListItemState {
                title: "Service A".to_string(),
                status: WatchStatus::Success,
            },
            ListItemState {
                title: "Service B".to_string(),
                status: WatchStatus::Error,
            },
        ];
        let display = DisplayState {
            style: "list".to_string(),
            title: None,
            description: None,
            content: None,
            value: None,
            min: None,
            max: None,
            items: Some(items),
        };

        let json = serde_json::to_value(&display).unwrap();
        assert_eq!(json["style"], "list");
        assert_eq!(json["items"][0]["title"], "Service A");
        assert_eq!(json["items"][0]["status"], "success");
        assert_eq!(json["items"][1]["status"], "error");
    }

    #[test]
    fn test_app_state_serializes_watches_array() {
        let app = AppState {
            watches: vec![
                make_watch_state(WatchStatus::Success),
                make_watch_state(WatchStatus::Warning),
            ],
            last_check: Some("2026-04-15T12:00:00Z".to_string()),
            groups: std::collections::HashMap::new(),
        };

        let json = serde_json::to_value(&app).unwrap();
        assert_eq!(json["watches"].as_array().unwrap().len(), 2);
        assert_eq!(json["last_check"], "2026-04-15T12:00:00Z");
    }

    #[test]
    fn test_detail_state_serializes() {
        let detail = DetailState {
            title: Some("Budget".to_string()),
            style: "progress".to_string(),
            description: None,
            value: Some(json!(0.75)),
            min: Some(0.0),
            max: Some(1.0),
        };

        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["title"], "Budget");
        assert_eq!(json["style"], "progress");
        assert_eq!(json["value"], 0.75);
        assert_eq!(json["min"], 0.0);
        assert_eq!(json["max"], 1.0);
    }

    #[test]
    fn test_new_shared_state_is_empty() {
        let state = new_shared_state();
        // This is a sync check on the initial value — read without async by
        // using try_read.
        let guard = state.try_read().unwrap();
        assert!(guard.watches.is_empty());
        assert!(guard.last_check.is_none());
    }
}
