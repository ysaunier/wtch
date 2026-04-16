use std::collections::HashMap;

use crate::config::{Conditions, Watch};

// Auto-generated preset registry (built by build.rs from presets/*.yml)
include!(concat!(env!("OUT_DIR"), "/preset_registry.rs"));

/// Load a built-in preset by name.
///
/// Returns `None` if the name is not recognized.
/// The preset YAML is embedded in the binary at compile time.
pub fn load_preset(name: &str) -> Option<Watch> {
    let yaml_str = load_preset_yaml(name)?;
    serde_yaml::from_str(yaml_str).ok()
}

/// Resolve a watch by merging it with its preset (if any).
///
/// User-defined fields always override preset fields.
/// If no `preset` field is set, the watch is returned as-is.
/// If the preset name is unknown, the watch is returned as-is.
pub fn resolve_preset(watch: &Watch) -> Watch {
    let preset_name = match &watch.preset {
        Some(name) => name.clone(),
        None => return watch.clone(),
    };

    let preset = match load_preset(&preset_name) {
        Some(p) => p,
        None => return watch.clone(),
    };

    merge_watch(&preset, watch)
}

/// Merge two `Watch` values: `base` provides defaults, `override_with` wins on conflicts.
///
/// Rules per field:
/// - `Option` fields: use override's value when `Some`, else fall back to base.
/// - `expand`: always use override's value (bool with a `default = false` serde default).
/// - `conditions`: if override has any condition set, use the entire override block;
///   otherwise use base's conditions.
/// - `display`: use override's block if `Some`, else base's.
/// - `details`: use override's vec if non-empty, else base's.
/// - `headers`: merge both maps; override wins on key conflicts.
fn merge_watch(base: &Watch, override_with: &Watch) -> Watch {
    Watch {
        name: override_with.name.clone().or_else(|| base.name.clone()),
        watch_type: override_with
            .watch_type
            .clone()
            .or_else(|| base.watch_type.clone()),
        command: override_with
            .command
            .clone()
            .or_else(|| base.command.clone()),
        shell: override_with.shell.clone().or_else(|| base.shell.clone()),
        format: override_with
            .format
            .clone()
            .or_else(|| base.format.clone()),
        interval: override_with
            .interval
            .clone()
            .or_else(|| base.interval.clone()),
        url: override_with.url.clone().or_else(|| base.url.clone()),
        link: override_with.link.clone().or_else(|| base.link.clone()),
        expand: override_with.expand,
        notify: override_with.notify.or(base.notify),
        preset: override_with.preset.clone().or_else(|| base.preset.clone()),
        conditions: merge_conditions(&base.conditions, &override_with.conditions),
        display: override_with
            .display
            .clone()
            .or_else(|| base.display.clone()),
        details: if override_with.details.is_empty() {
            base.details.clone()
        } else {
            override_with.details.clone()
        },
        headers: merge_headers(&base.headers, &override_with.headers),
        adapter: override_with
            .adapter
            .clone()
            .or_else(|| base.adapter.clone()),
        adapter_config: override_with
            .adapter_config
            .clone()
            .or_else(|| base.adapter_config.clone()),
    }
}

/// Merge conditions: if the override has any field set, use the override block entirely.
/// Otherwise use the base block.
fn merge_conditions(base: &Conditions, override_with: &Conditions) -> Conditions {
    let override_has_any = override_with.error.is_some()
        || override_with.warning.is_some()
        || override_with.maintenance.is_some()
        || override_with.success.is_some();

    if override_has_any {
        override_with.clone()
    } else {
        base.clone()
    }
}

/// Merge two header maps. Override wins on key conflicts.
fn merge_headers(
    base: &HashMap<String, String>,
    override_with: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = base.clone();
    for (k, v) in override_with {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DisplayStyle, WatchType};

    fn empty_watch() -> Watch {
        Watch {
            name: None,
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
            conditions: Conditions::default(),
            display: None,
            details: vec![],
            headers: HashMap::new(),
            adapter: None,
            adapter_config: None,
        }
    }

    /// Load the github preset and verify its key fields are populated.
    #[test]
    fn test_load_github_preset() {
        let preset = load_preset("github").expect("github preset should load");

        assert_eq!(preset.name.as_deref(), Some("GitHub"));
        assert_eq!(preset.watch_type, Some(WatchType::Adapter));
        assert_eq!(
            preset.url.as_deref(),
            Some("https://www.githubstatus.com")
        );
        assert_eq!(preset.interval.as_deref(), Some("60s"));

        assert!(preset.conditions.error.is_some());
        assert!(preset.conditions.warning.is_some());
        assert!(preset.conditions.success.is_some());

        let display = preset.display.as_ref().expect("display should be set");
        assert_eq!(display.style, Some(DisplayStyle::Status));
        assert_eq!(display.title.as_deref(), Some("GitHub"));
    }

    /// An unknown preset name returns None.
    #[test]
    fn test_unknown_preset_returns_none() {
        assert!(load_preset("does_not_exist").is_none());
        assert!(load_preset("").is_none());
    }

    /// When a user sets a field, it overrides the matching preset field.
    #[test]
    fn test_user_override_takes_priority() {
        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());
        user_watch.name = Some("My GitHub".to_string());
        user_watch.interval = Some("2m".to_string());

        let resolved = resolve_preset(&user_watch);

        assert_eq!(resolved.name.as_deref(), Some("My GitHub"));
        assert_eq!(resolved.interval.as_deref(), Some("2m"));
        // Other fields should come from the preset
        assert_eq!(resolved.watch_type, Some(WatchType::Adapter));
        assert_eq!(
            resolved.url.as_deref(),
            Some("https://www.githubstatus.com")
        );
    }

    /// When a user does not set a field, the preset value fills it in.
    #[test]
    fn test_preset_fills_missing_fields() {
        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());

        let resolved = resolve_preset(&user_watch);

        assert_eq!(resolved.name.as_deref(), Some("GitHub"));
        assert_eq!(resolved.watch_type, Some(WatchType::Adapter));
        assert_eq!(
            resolved.url.as_deref(),
            Some("https://www.githubstatus.com")
        );
        assert_eq!(resolved.interval.as_deref(), Some("60s"));
        assert!(resolved.conditions.error.is_some());
        assert!(resolved.display.is_some());
    }

    /// Conditions override is all-or-nothing: if the user sets any condition,
    /// the entire user conditions block is used and preset conditions are discarded.
    #[test]
    fn test_conditions_override_is_all_or_nothing() {
        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());
        user_watch.conditions = Conditions {
            error: Some(".status == \"critical\"".to_string()),
            warning: None,
            success: None,
            maintenance: None,
        };

        let resolved = resolve_preset(&user_watch);

        // User set error only, so the entire user conditions block wins
        assert_eq!(
            resolved.conditions.error.as_deref(),
            Some(".status == \"critical\"")
        );
        assert!(resolved.conditions.warning.is_none());
        assert!(resolved.conditions.success.is_none());
    }

    /// When the user sets no conditions, the preset conditions are used.
    #[test]
    fn test_conditions_fallback_to_preset_when_user_has_none() {
        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());

        let resolved = resolve_preset(&user_watch);

        // Preset conditions should be present
        assert!(resolved.conditions.error.is_some());
        assert!(resolved.conditions.warning.is_some());
        assert!(resolved.conditions.success.is_some());
    }

    /// Headers from base and override are merged; override wins on conflicts.
    #[test]
    fn test_headers_are_merged() {
        let mut base = empty_watch();
        base.headers.insert("X-Base".to_string(), "base-value".to_string());
        base.headers
            .insert("X-Shared".to_string(), "base-shared".to_string());

        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());
        user_watch
            .headers
            .insert("X-User".to_string(), "user-value".to_string());
        user_watch
            .headers
            .insert("X-Shared".to_string(), "user-shared".to_string());

        // Merge directly using merge_watch to control both sides
        let preset = load_preset("github").expect("github preset must load");
        let mut preset_with_base_headers = preset;
        preset_with_base_headers
            .headers
            .insert("X-Base".to_string(), "base-value".to_string());
        preset_with_base_headers
            .headers
            .insert("X-Shared".to_string(), "base-shared".to_string());

        let resolved = merge_watch(&preset_with_base_headers, &user_watch);

        assert_eq!(
            resolved.headers.get("X-Base").map(String::as_str),
            Some("base-value")
        );
        assert_eq!(
            resolved.headers.get("X-User").map(String::as_str),
            Some("user-value")
        );
        // Override wins on the conflicting key
        assert_eq!(
            resolved.headers.get("X-Shared").map(String::as_str),
            Some("user-shared")
        );
    }

    /// A watch without a preset field is returned unchanged.
    #[test]
    fn test_no_preset_returns_watch_as_is() {
        let mut watch = empty_watch();
        watch.name = Some("My Watch".to_string());
        watch.interval = Some("10s".to_string());

        let resolved = resolve_preset(&watch);

        assert_eq!(resolved.name.as_deref(), Some("My Watch"));
        assert_eq!(resolved.interval.as_deref(), Some("10s"));
        assert!(resolved.watch_type.is_none());
    }

    /// An unknown preset name causes the watch to be returned as-is.
    #[test]
    fn test_unknown_preset_returns_watch_as_is() {
        let mut watch = empty_watch();
        watch.preset = Some("totally-unknown".to_string());
        watch.name = Some("Fallback".to_string());

        let resolved = resolve_preset(&watch);

        assert_eq!(resolved.name.as_deref(), Some("Fallback"));
        assert!(resolved.watch_type.is_none());
    }

    /// expand flag uses the override's value (not the preset's).
    #[test]
    fn test_expand_uses_override_value() {
        let mut user_watch = empty_watch();
        user_watch.preset = Some("github".to_string());
        user_watch.expand = true;

        let resolved = resolve_preset(&user_watch);

        assert!(resolved.expand);
    }
}
