use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tauri::async_runtime::JoinHandle;
use tokio::sync::RwLock;
use tokio::task::JoinSet;

use crate::config::Config;
use crate::state::SharedState;
use crate::watcher::evaluate_watch;

/// Thread-safe shared handle to [`Config`] that supports hot-reloading.
pub type SharedConfig = Arc<RwLock<Config>>;

// ---------------------------------------------------------------------------
// Interval parsing
// ---------------------------------------------------------------------------

/// Parses a human-readable interval string into a [`Duration`].
///
/// Supported suffixes:
/// - `s` — seconds (e.g. `"30s"`)
/// - `m` — minutes (e.g. `"5m"`)
/// - `h` — hours (e.g. `"1h"`)
///
/// Returns `None` for unrecognized formats.
pub fn parse_interval(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (digits, suffix) = if s.ends_with('s') {
        (&s[..s.len() - 1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], "h")
    } else {
        return None;
    };

    let n: u64 = digits.trim().parse().ok()?;

    let secs = match suffix {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        _ => return None,
    };

    Some(Duration::from_secs(secs))
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// Spawns the background polling task.
///
/// On startup all watches are evaluated immediately, then the loop repeats
/// every `config.general.poll_interval`. Each tick reads the current shared
/// config so that hot-reloaded presets are picked up automatically.
pub fn start_scheduler(shared_config: SharedConfig, state: SharedState) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let config = shared_config.read().await.clone();
            let interval = parse_interval(&config.general.poll_interval)
                .unwrap_or(Duration::from_secs(60));

            crate::logging::info(&format!("[scheduler] tick: {} watches, interval={:?}", config.watches.len(), interval));
            run_all_watches(&config, &state).await;
            crate::logging::info("[scheduler] tick complete");
            tokio::time::sleep(interval).await;
        }
    })
}

/// Evaluates all watches in parallel and writes the results to `state`.
async fn run_all_watches(config: &Config, state: &SharedState) {
    if config.watches.is_empty() {
        let mut guard = state.write().await;
        guard.last_check = Some(Utc::now().to_rfc3339());
        return;
    }

    let mut set = JoinSet::new();

    for watch in &config.watches {
        let watch = watch.clone();
        let config = config.clone();
        set.spawn(async move { evaluate_watch(&watch, &config).await });
    }

    let mut results = Vec::with_capacity(config.watches.len());
    while let Some(outcome) = set.join_next().await {
        match outcome {
            Ok(watch_state) => results.push(watch_state),
            Err(_) => {
                // A task panicked; skip it rather than poisoning the whole update.
            }
        }
    }

    // Sort results by the original watch order so the UI stays stable.
    let names: Vec<String> = config
        .watches
        .iter()
        .map(|w| {
            w.name
                .clone()
                .unwrap_or_else(|| w.preset.clone().unwrap_or_else(|| "unnamed".to_string()))
        })
        .collect();

    results.sort_by_key(|r| names.iter().position(|n| n == &r.name).unwrap_or(usize::MAX));

    let mut guard = state.write().await;
    guard.watches = results;
    guard.last_check = Some(Utc::now().to_rfc3339());
    guard.groups = config.groups.clone();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Interval parsing ---------------------------------------------------

    #[test]
    fn test_parse_interval_seconds() {
        assert_eq!(parse_interval("30s"), Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_parse_interval_minutes() {
        assert_eq!(parse_interval("5m"), Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_parse_interval_hours() {
        assert_eq!(parse_interval("1h"), Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_parse_interval_60s() {
        assert_eq!(parse_interval("60s"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_parse_interval_2h() {
        assert_eq!(parse_interval("2h"), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn test_parse_interval_whitespace_trimmed() {
        assert_eq!(parse_interval("  10s  "), Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_parse_interval_empty_string_returns_none() {
        assert_eq!(parse_interval(""), None);
    }

    #[test]
    fn test_parse_interval_no_suffix_returns_none() {
        assert_eq!(parse_interval("60"), None);
    }

    #[test]
    fn test_parse_interval_invalid_number_returns_none() {
        assert_eq!(parse_interval("abcs"), None);
    }

    #[test]
    fn test_parse_interval_unknown_suffix_returns_none() {
        assert_eq!(parse_interval("30d"), None);
    }

    #[test]
    fn test_parse_interval_zero_seconds() {
        assert_eq!(parse_interval("0s"), Some(Duration::from_secs(0)));
    }

    // -- run_all_watches integration test ------------------------------------

    #[tokio::test]
    async fn test_run_all_watches_empty_config_updates_last_check() {
        let config = crate::config::Config::default();
        let state = crate::state::new_shared_state();

        run_all_watches(&config, &state).await;

        let guard = state.read().await;
        assert!(guard.last_check.is_some(), "last_check should be set after tick");
        assert!(guard.watches.is_empty());
    }
}
