use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// The type of provider for a watch entry.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WatchType {
    Script,
    Http,
    Statuspage,
    Adapter,
}

/// How a script's stdout should be interpreted.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFormat {
    Json,
    Value,
    ExitCode,
}

/// Visual style used for a display block.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayStyle {
    Status,
    Progress,
    Value,
    Text,
    List,
}

// ---------------------------------------------------------------------------
// Sub-structures
// ---------------------------------------------------------------------------

/// jq-like condition expressions evaluated against script output.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Conditions {
    pub error: Option<String>,
    pub warning: Option<String>,
    pub maintenance: Option<String>,
    pub success: Option<String>,
}

/// A display block describing how a watch (or a detail panel) is rendered.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Display {
    pub style: Option<DisplayStyle>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub value: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub items: Option<String>,
    pub item_title: Option<String>,
    pub item_status: Option<String>,
    /// JSON path to an array for dynamic detail generation (e.g. ".components")
    pub details_from: Option<String>,
    /// Field path within each array item for the detail title (e.g. ".name")
    pub detail_title: Option<String>,
    /// Field path within each array item for the status string (e.g. ".status")
    pub detail_status: Option<String>,
    /// Display style for dynamic details: "status" (default) or "progress"
    pub detail_style: Option<DisplayStyle>,
    /// Field path within each array item for the value (progress style)
    pub detail_value: Option<String>,
    /// Literal or field path for the max value (progress style)
    pub detail_max: Option<String>,
    /// Field path within each array item for the description
    pub detail_description: Option<String>,
}

/// A detail panel attached to a watch. Shares the same fields as Display.
/// The `title` field is typically set explicitly in each `details` entry.
pub type Details = Display;

/// Shell profile: the executable and its argument prefix.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ShellConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

// ---------------------------------------------------------------------------
// Watch entry
// ---------------------------------------------------------------------------

/// A single monitored item as declared in the `watches` array.
#[derive(Debug, Clone, Deserialize)]
pub struct Watch {
    pub name: Option<String>,

    #[serde(rename = "type")]
    pub watch_type: Option<WatchType>,

    pub command: Option<String>,
    pub shell: Option<String>,
    pub format: Option<ScriptFormat>,
    pub interval: Option<String>,
    pub url: Option<String>,
    pub link: Option<String>,

    #[serde(default)]
    pub expand: bool,

    pub notify: Option<bool>,
    pub preset: Option<String>,

    #[serde(default)]
    pub conditions: Conditions,

    pub display: Option<Display>,

    #[serde(default)]
    pub details: Vec<Details>,

    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Adapter URN, e.g. "urn:wtch:adapter:statuspage"
    pub adapter: Option<String>,

    /// Adapter-specific configuration key/value pairs.
    #[serde(default)]
    pub adapter_config: Option<HashMap<String, serde_yaml::Value>>,
}

// ---------------------------------------------------------------------------
// General section
// ---------------------------------------------------------------------------

/// Top-level `general` section.
#[derive(Debug, Clone, Deserialize)]
pub struct General {
    #[serde(default = "default_poll_interval")]
    pub poll_interval: String,

    pub default_shell: Option<String>,

    #[serde(default = "default_notify")]
    pub notify: bool,

    #[serde(default = "default_notify_cooldown")]
    pub notify_cooldown: String,

    #[serde(default)]
    pub debug: bool,
}

fn default_poll_interval() -> String {
    "60s".to_string()
}

fn default_notify() -> bool {
    true
}

fn default_notify_cooldown() -> String {
    "5m".to_string()
}

impl Default for General {
    fn default() -> Self {
        Self {
            poll_interval: default_poll_interval(),
            default_shell: None,
            notify: default_notify(),
            notify_cooldown: default_notify_cooldown(),
            debug: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

/// The full deserialized `wtch.yml` configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: General,

    #[serde(default)]
    pub shells: HashMap<String, ShellConfig>,

    #[serde(default)]
    pub groups: HashMap<String, Vec<String>>,

    #[serde(rename = "watch", default)]
    pub watches: Vec<Watch>,
}

impl Default for Config {
    fn default() -> Self {
        let mut cfg = Self {
            general: General::default(),
            shells: HashMap::new(),
            groups: HashMap::new(),
            watches: Vec::new(),
        };
        cfg.apply_builtin_shells();
        cfg
    }
}

/// Returns the path to the configuration file (`~/.config/wtch/wtch.yml`).
pub fn config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("wtch").join("wtch.yml"))
}

/// Appends a watch entry with the given preset name to the config file.
///
/// Reads the current file, adds the preset to the `watches` array, and
/// writes the file back using serde_yaml. Creates the file (and any parent
/// directories) if it does not exist. No-op when the preset is already present.
pub fn add_preset_to_file(preset_name: &str) -> Result<(), ConfigError> {
    let path = config_path().ok_or(ConfigError::HomeDirNotFound)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
    }

    let mut doc: serde_yaml::Value = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| ConfigError::Io(e.to_string()))?;
        if content.trim().is_empty() {
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
        } else {
            serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?
        }
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };

    // Ensure the top-level mapping has a "watch" sequence.
    let mapping = doc
        .as_mapping_mut()
        .ok_or_else(|| ConfigError::Parse("config root is not a mapping".to_string()))?;

    let watch_key = serde_yaml::Value::String("watch".to_string());
    let watches = mapping
        .entry(watch_key)
        .or_insert_with(|| serde_yaml::Value::Sequence(vec![]));

    let seq = watches
        .as_sequence_mut()
        .ok_or_else(|| ConfigError::Parse("watch is not a sequence".to_string()))?;

    // Guard against duplicates.
    let preset_val = serde_yaml::Value::String(preset_name.to_string());
    for entry in seq.iter() {
        if let Some(existing) = entry.get("preset") {
            if existing == &preset_val {
                return Ok(());
            }
        }
    }

    let mut new_entry = serde_yaml::Mapping::new();
    new_entry.insert(
        serde_yaml::Value::String("preset".to_string()),
        preset_val,
    );
    seq.push(serde_yaml::Value::Mapping(new_entry));

    let output =
        serde_yaml::to_string(&doc).map_err(|e| ConfigError::Parse(e.to_string()))?;
    std::fs::write(&path, output).map_err(|e| ConfigError::Io(e.to_string()))
}

/// Removes all watch entries that use the given preset name from the config file.
///
/// Reads, filters, and rewrites the file using serde_yaml. No-op when the file
/// does not exist.
pub fn remove_preset_from_file(preset_name: &str) -> Result<(), ConfigError> {
    let path = config_path().ok_or(ConfigError::HomeDirNotFound)?;
    if !path.exists() {
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| ConfigError::Io(e.to_string()))?;
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;

    let mapping = doc
        .as_mapping_mut()
        .ok_or_else(|| ConfigError::Parse("config root is not a mapping".to_string()))?;

    let watch_key = serde_yaml::Value::String("watch".to_string());
    if let Some(watches) = mapping.get_mut(&watch_key) {
        if let Some(seq) = watches.as_sequence_mut() {
            let preset_val = serde_yaml::Value::String(preset_name.to_string());
            seq.retain(|entry| {
                entry
                    .get("preset")
                    .map(|v| v != &preset_val)
                    .unwrap_or(true)
            });
        }
    }

    let output =
        serde_yaml::to_string(&doc).map_err(|e| ConfigError::Parse(e.to_string()))?;
    std::fs::write(&path, output).map_err(|e| ConfigError::Io(e.to_string()))
}

/// Reorders watch entries in the config file according to `order`.
///
/// Entries not present in `order` are appended at the end. Each entry is
/// identified by its `name` field first, then its `preset` field.
/// No-op when the file does not exist.
pub fn reorder_watches_in_file(order: &[String]) -> Result<(), ConfigError> {
    let path = config_path().ok_or(ConfigError::HomeDirNotFound)?;
    if !path.exists() {
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| ConfigError::Io(e.to_string()))?;
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;

    let mapping = doc
        .as_mapping_mut()
        .ok_or_else(|| ConfigError::Parse("config root is not a mapping".to_string()))?;

    let watch_key = serde_yaml::Value::String("watch".to_string());
    if let Some(watches) = mapping.get_mut(&watch_key) {
        if let Some(seq) = watches.as_sequence_mut() {
            let get_key = |entry: &serde_yaml::Value| -> String {
                if let Some(v) = entry.get("name").and_then(|v| v.as_str()) {
                    return v.to_string();
                }
                if let Some(v) = entry.get("preset").and_then(|v| v.as_str()) {
                    return v.to_string();
                }
                String::new()
            };

            let mut remaining: Vec<serde_yaml::Value> = seq.drain(..).collect();
            let mut sorted: Vec<serde_yaml::Value> = Vec::with_capacity(remaining.len());

            for name in order {
                let lower = name.to_lowercase();
                if let Some(idx) = remaining.iter().position(|e| {
                    let k = get_key(e);
                    k.to_lowercase() == lower || &k == name
                }) {
                    sorted.push(remaining.remove(idx));
                }
            }
            sorted.extend(remaining);
            *seq = sorted;
        }
    }

    let output =
        serde_yaml::to_string(&doc).map_err(|e| ConfigError::Parse(e.to_string()))?;
    std::fs::write(&path, output).map_err(|e| ConfigError::Io(e.to_string()))
}

impl Config {
    /// Returns the three shells that are always available unless overridden by the user.
    fn builtin_shells() -> HashMap<String, ShellConfig> {
        let mut map = HashMap::new();

        // Cross-platform: bash is available everywhere
        map.insert(
            "bash".to_string(),
            ShellConfig {
                command: "bash".to_string(),
                args: vec!["-c".to_string()],
            },
        );
        map.insert(
            "sh".to_string(),
            ShellConfig {
                command: "sh".to_string(),
                args: vec!["-c".to_string()],
            },
        );

        // Windows-specific
        map.insert(
            "cmd".to_string(),
            ShellConfig {
                command: "cmd.exe".to_string(),
                args: vec!["/C".to_string()],
            },
        );
        map.insert(
            "powershell".to_string(),
            ShellConfig {
                command: "powershell.exe".to_string(),
                args: vec!["-NoProfile".to_string(), "-Command".to_string()],
            },
        );
        map.insert(
            "wsl".to_string(),
            ShellConfig {
                command: "wsl.exe".to_string(),
                args: vec!["-e".to_string(), "bash".to_string(), "-ic".to_string()],
            },
        );

        map
    }

    /// Inserts built-in shells only where the user has not defined them.
    pub fn apply_builtin_shells(&mut self) {
        for (name, shell) in Self::builtin_shells() {
            self.shells.entry(name).or_insert(shell);
        }
    }

    /// Loads configuration from `~/.config/wtch/wtch.yml`.
    ///
    /// If the file does not exist, a default config is returned.
    /// Built-in shells are always merged as fallbacks.
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = dirs::home_dir()
            .ok_or(ConfigError::HomeDirNotFound)?
            .join(".config")
            .join("wtch")
            .join("wtch.yml");

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::Io(e.to_string()))?;

        let mut config: Config =
            serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;

        config.apply_builtin_shells();
        Ok(config)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while loading the configuration.
#[derive(Debug)]
pub enum ConfigError {
    HomeDirNotFound,
    Io(String),
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::HomeDirNotFound => write!(f, "could not determine home directory"),
            ConfigError::Io(msg) => write!(f, "IO error: {msg}"),
            ConfigError::Parse(msg) => write!(f, "YAML parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a complete YAML string covering every section of the schema.
    fn full_yaml() -> &'static str {
        r#"
general:
  poll_interval: "30s"
  default_shell: wsl
  notify: false
  notify_cooldown: "10m"

groups:
  My tools:
    - Claude Usage
    - uTorrent
  Services:
    - GitHub

shells:
  ubuntu:
    command: wsl.exe
    args: ["-d", "Ubuntu", "--"]

watch:
  - name: Claude Usage
    type: script
    command: claude-usage
    shell: wsl
    format: json
    interval: "5m"
    url: https://console.anthropic.com
    expand: true
    notify: true
    conditions:
      error: ".cost > .limit"
      warning: ".cost > .limit * 0.8"
      success: ".cost <= .limit * 0.8"
    display:
      style: progress
      title: Claude
      description: "${{ .cost }} / ${{ .limit }}"
      value: .cost
      max: .limit
    details:
      - title: Budget
        style: progress
        value: .cost
        max: .limit
      - title: Requests
        style: value
        value: "{{ .requests_today }}"

  - name: Public IP
    type: script
    command: curl -s ifconfig.me
    shell: wsl
    format: value
    display:
      style: text
      title: IP
      content: "{{ .value }}"

  - preset: github
"#
    }

    #[test]
    fn test_parse_complete_yaml() {
        let config: Config =
            serde_yaml::from_str(full_yaml()).expect("should parse complete YAML");

        // General section
        assert_eq!(config.general.poll_interval, "30s");
        assert_eq!(config.general.default_shell.as_deref(), Some("wsl"));
        assert!(!config.general.notify);
        assert_eq!(config.general.notify_cooldown, "10m");

        // Groups
        assert_eq!(config.groups.len(), 2);
        assert_eq!(
            config.groups.get("My tools").unwrap(),
            &vec!["Claude Usage".to_string(), "uTorrent".to_string()]
        );

        // Custom shell
        let ubuntu = config.shells.get("ubuntu").expect("ubuntu shell should exist");
        assert_eq!(ubuntu.command, "wsl.exe");
        assert_eq!(ubuntu.args, vec!["-d", "Ubuntu", "--"]);

        // Watches
        assert_eq!(config.watches.len(), 3);

        let claude = &config.watches[0];
        assert_eq!(claude.name.as_deref(), Some("Claude Usage"));
        assert_eq!(claude.watch_type, Some(WatchType::Script));
        assert_eq!(claude.format, Some(ScriptFormat::Json));
        assert_eq!(claude.interval.as_deref(), Some("5m"));
        assert!(claude.expand);
        assert_eq!(claude.notify, Some(true));
        assert_eq!(claude.conditions.error.as_deref(), Some(".cost > .limit"));

        let display = claude.display.as_ref().expect("display should exist");
        assert_eq!(display.style, Some(DisplayStyle::Progress));
        assert_eq!(display.title.as_deref(), Some("Claude"));

        assert_eq!(claude.details.len(), 2);
        assert_eq!(claude.details[0].title.as_deref(), Some("Budget"));
        assert_eq!(claude.details[1].title.as_deref(), Some("Requests"));

        // Preset-only watch
        let github = &config.watches[2];
        assert_eq!(github.preset.as_deref(), Some("github"));
        assert!(github.name.is_none());
    }

    #[test]
    fn test_defaults_are_applied() {
        let config: Config =
            serde_yaml::from_str("{}").expect("empty YAML should produce defaults");

        assert_eq!(config.general.poll_interval, "60s");
        assert!(config.general.default_shell.is_none());
        assert!(config.general.notify);
        assert_eq!(config.general.notify_cooldown, "5m");
        assert!(config.watches.is_empty());
        assert!(config.groups.is_empty());
    }

    #[test]
    fn test_builtin_shells_merged_with_custom() {
        let yaml_str = r#"
shells:
  ubuntu:
    command: wsl.exe
    args: ["-d", "Ubuntu", "--"]
"#;
        let mut config: Config = serde_yaml::from_str(yaml_str).expect("should parse");
        config.apply_builtin_shells();

        // Cross-platform shells must be present
        let bash = config.shells.get("bash").expect("bash shell must exist");
        assert_eq!(bash.command, "bash");
        assert_eq!(bash.args, vec!["-c"]);

        let sh = config.shells.get("sh").expect("sh shell must exist");
        assert_eq!(sh.command, "sh");
        assert_eq!(sh.args, vec!["-c"]);

        // Windows shells must be present
        let cmd = config.shells.get("cmd").expect("cmd shell must exist");
        assert_eq!(cmd.command, "cmd.exe");
        assert_eq!(cmd.args, vec!["/C"]);

        let ps = config
            .shells
            .get("powershell")
            .expect("powershell shell must exist");
        assert_eq!(ps.command, "powershell.exe");
        assert_eq!(ps.args, vec!["-NoProfile", "-Command"]);

        let wsl = config.shells.get("wsl").expect("wsl shell must exist");
        assert_eq!(wsl.command, "wsl.exe");
        assert_eq!(wsl.args, vec!["-e", "bash", "-ic"]);

        // Custom shell must also be present
        let ubuntu = config.shells.get("ubuntu").expect("ubuntu shell must exist");
        assert_eq!(ubuntu.args, vec!["-d", "Ubuntu", "--"]);
    }

    #[test]
    fn test_builtin_shells_not_overridden_when_user_redefines() {
        let yaml_str = r#"
shells:
  wsl:
    command: my-wsl.exe
    args: ["-x"]
"#;
        let mut config: Config = serde_yaml::from_str(yaml_str).expect("should parse");
        config.apply_builtin_shells();

        // User definition of wsl must win
        let wsl = config.shells.get("wsl").expect("wsl shell must exist");
        assert_eq!(wsl.command, "my-wsl.exe");
        assert_eq!(wsl.args, vec!["-x"]);
    }

    #[test]
    fn test_missing_file_returns_default_config() {
        // Config::default() is what load() returns when the file is absent.
        let config = Config::default();

        assert_eq!(config.general.poll_interval, "60s");
        assert!(config.general.notify);

        // All three built-in shells must be present even on a default config
        assert!(config.shells.contains_key("cmd"));
        assert!(config.shells.contains_key("powershell"));
        assert!(config.shells.contains_key("wsl"));

        assert!(config.watches.is_empty());
    }

    #[test]
    fn test_adapter_watch_type_parses() {
        let yaml_str = r#"
watch:
  - name: My Adapter
    type: adapter
    adapter: "urn:wtch:adapter:statuspage"
    url: https://example.com
"#;
        let config: Config = serde_yaml::from_str(yaml_str).expect("should parse adapter watch");
        let w = &config.watches[0];
        assert_eq!(w.watch_type, Some(WatchType::Adapter));
        assert_eq!(w.adapter.as_deref(), Some("urn:wtch:adapter:statuspage"));
    }

    #[test]
    fn test_add_preset_yaml_roundtrip() {
        let initial = r#"
general:
  poll_interval: "60s"
watch:
  - name: Test
    type: http
    url: https://example.com
"#;
        let mut doc: serde_yaml::Value = serde_yaml::from_str(initial).unwrap();
        let watch_key = serde_yaml::Value::String("watch".to_string());
        let watches = doc
            .as_mapping_mut()
            .unwrap()
            .entry(watch_key)
            .or_insert_with(|| serde_yaml::Value::Sequence(vec![]));
        let seq = watches.as_sequence_mut().unwrap();

        // Guard against duplicate
        let preset_val = serde_yaml::Value::String("github".to_string());
        let already_present = seq
            .iter()
            .any(|e| e.get("preset").map(|v| v == &preset_val).unwrap_or(false));
        assert!(!already_present);

        let mut entry = serde_yaml::Mapping::new();
        entry.insert(
            serde_yaml::Value::String("preset".to_string()),
            preset_val,
        );
        seq.push(serde_yaml::Value::Mapping(entry));

        let output = serde_yaml::to_string(&doc).unwrap();
        let config: Config = serde_yaml::from_str(&output).unwrap();
        assert_eq!(config.watches.len(), 2);
        assert_eq!(config.watches[1].preset.as_deref(), Some("github"));
    }

    #[test]
    fn test_remove_preset_roundtrip() {
        let initial = r#"
watch:
  - name: Keep Me
    type: http
    url: https://example.com
  - preset: github
  - preset: github
"#;
        let mut doc: serde_yaml::Value = serde_yaml::from_str(initial).unwrap();
        let watch_key = serde_yaml::Value::String("watch".to_string());
        let preset_val = serde_yaml::Value::String("github".to_string());
        if let Some(watches) = doc.as_mapping_mut().unwrap().get_mut(&watch_key) {
            if let Some(seq) = watches.as_sequence_mut() {
                seq.retain(|e| {
                    e.get("preset")
                        .map(|v| v != &preset_val)
                        .unwrap_or(true)
                });
            }
        }
        let output = serde_yaml::to_string(&doc).unwrap();
        let config: Config = serde_yaml::from_str(&output).unwrap();
        assert_eq!(config.watches.len(), 1);
        assert_eq!(config.watches[0].name.as_deref(), Some("Keep Me"));
    }

    #[test]
    fn test_reorder_watches_roundtrip() {
        let initial = r#"
watch:
  - name: Zebra
    type: http
    url: https://zebra.example.com
  - name: Alpha
    type: http
    url: https://alpha.example.com
  - preset: github
"#;
        let order = vec!["Alpha".to_string(), "github".to_string(), "Zebra".to_string()];

        let mut doc: serde_yaml::Value = serde_yaml::from_str(initial).unwrap();
        let watch_key = serde_yaml::Value::String("watch".to_string());
        if let Some(watches) = doc.as_mapping_mut().unwrap().get_mut(&watch_key) {
            if let Some(seq) = watches.as_sequence_mut() {
                let get_key = |entry: &serde_yaml::Value| -> String {
                    if let Some(v) = entry.get("name").and_then(|v| v.as_str()) {
                        return v.to_string();
                    }
                    if let Some(v) = entry.get("preset").and_then(|v| v.as_str()) {
                        return v.to_string();
                    }
                    String::new()
                };
                let mut remaining: Vec<serde_yaml::Value> = seq.drain(..).collect();
                let mut sorted: Vec<serde_yaml::Value> = Vec::new();
                for name in &order {
                    let lower = name.to_lowercase();
                    if let Some(idx) = remaining.iter().position(|e| {
                        let k = get_key(e);
                        k.to_lowercase() == lower || &k == name
                    }) {
                        sorted.push(remaining.remove(idx));
                    }
                }
                sorted.extend(remaining);
                *seq = sorted;
            }
        }
        let output = serde_yaml::to_string(&doc).unwrap();
        let config: Config = serde_yaml::from_str(&output).unwrap();
        assert_eq!(config.watches.len(), 3);
        assert_eq!(config.watches[0].name.as_deref(), Some("Alpha"));
        assert_eq!(config.watches[1].preset.as_deref(), Some("github"));
        assert_eq!(config.watches[2].name.as_deref(), Some("Zebra"));
    }
}
