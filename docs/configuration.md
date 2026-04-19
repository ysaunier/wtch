# Configuration

wtch reads its config from `~/.config/wtch/wtch.yml`.

See [examples/](examples/) for ready-to-use configs:
- [minimal.yml](examples/minimal.yml) - just presets
- [homelab.yml](examples/homelab.yml) - local services + cloud
- [devops.yml](examples/devops.yml) - CI/CD, regions, scripts, groups
- [full.yml](examples/full.yml) - every feature

Custom watch guides:
- [claude-usage.md](claude-usage.md) - Monitor Claude API usage with progress bars

## Full example

```yaml
general:
  poll_interval: "60s"
  default_shell: wsl
  notify: true
  notify_cooldown: "5m"

shells:
  ubuntu:
    command: wsl.exe
    args: ["-d", "Ubuntu", "--"]

groups:
  Infrastructure:
    - GitHub
    - CloudFlare
  Google:
    - Gmail
    - Google Drive

watch:
  - preset: github
  - name: My API
    type: http
    url: https://api.example.com/health
```

## General

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `poll_interval` | string | `"60s"` | How often watches are re-evaluated. Supports `s`, `m`, `h` suffixes. |
| `default_shell` | string | none | Default shell used for script watches. |
| `notify` | bool | `true` | Enable desktop notifications on status changes. |
| `notify_cooldown` | string | `"5m"` | Minimum time between repeated notifications for the same watch. |
| `allow_scripts` | bool | `false` | Allow script execution (type: script watches). When disabled, only HTTP, Statuspage, and adapter watches run. |
| `debug` | bool | `false` | Enable debug logging to `~/.config/wtch/<date>.log`. |

## Shells

Built-in shells are always available:

| Name | Command | Args | Platform |
|------|---------|------|----------|
| `bash` | `bash` | `-c` | All |
| `sh` | `sh` | `-c` | All |
| `cmd` | `cmd.exe` | `/C` | Windows |
| `powershell` | `powershell.exe` | `-NoProfile -Command` | Windows |
| `wsl` | `wsl.exe` | `-e bash -ic` | Windows (WSL) |

You can define custom shells:

```yaml
shells:
  ubuntu:
    command: wsl.exe
    args: ["-d", "Ubuntu", "--"]
  bash:
    command: bash
    args: ["-c"]
```

## Groups

Groups organize watches visually in the UI.

```yaml
groups:
  My tools:
    - GitHub
    - OpenAI
  Cloud:
    - AWS
    - GCP
```

## Presets

Presets are built-in service configurations. Browse the full list in the [`presets/`](../presets/) directory, or search them in the app settings panel.

```yaml
watch:
  - preset: github
  - preset: openai
  - preset: gmail
  - preset: snowflake-aws-canada-central
  - preset: gcp-northamerica-northeast1
  - preset: 1password-canada
```

You can override any preset field:

```yaml
watch:
  - preset: github
    interval: "2m"
    expand: true
```

## Watch entry

Every item in the `watch` array supports the following keys:

### Core

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | string | none | Display name shown in the UI. |
| `type` | string | none | Watch type: `http`, `script`, `adapter`. |
| `url` | string | none | URL to fetch data from. |
| `link` | string | same as `url` | URL opened when clicking the watch name. Useful when the data URL differs from the status page URL. |
| `interval` | string | from `general.poll_interval` | Per-watch polling interval (`"30s"`, `"5m"`, `"1h"`). |
| `preset` | string | none | Name of a built-in preset to use as base config. |
| `expand` | bool | `false` | Whether the detail panel is expanded by default. |
| `notify` | bool | from `general.notify` | Enable/disable notifications for this watch. |

### Script-specific

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `command` | string | none | Shell command to execute. |
| `shell` | string | from `general.default_shell` | Shell to use (`cmd`, `powershell`, `wsl`, or any custom shell). |
| `format` | string | none | How to interpret stdout: `json` (parse as JSON), `value` (raw string as `.value`), `exit_code` (only use exit code). |

### HTTP headers

```yaml
watch:
  - name: Private API
    type: http
    url: https://api.example.com/status
    headers:
      Authorization: "Bearer ${API_TOKEN}"
      X-Custom: "my-value"
```

Header values support environment variable expansion with `${VAR}` or `$VAR` syntax.

### Adapter-specific

| Key | Type | Description |
|-----|------|-------------|
| `adapter` | string | Adapter URN: `urn:wtch:adapter:statuspage`, `urn:wtch:adapter:json`, `urn:wtch:adapter:html`, `urn:wtch:adapter:google`. |
| `adapter_config` | map | Key-value pairs specific to each adapter (see below). |

#### StatusPage adapter config

| Key | Description |
|-----|-------------|
| `group_id` | Filter components by StatusPage group ID. |
| `group_filter` | Filter groups by name prefix (e.g. `"AWS"`). Shows one component per matching group. |

#### JSON adapter config

| Key | Default | Description |
|-----|---------|-------------|
| `status_field` | `.status` | jq-style path to the status value. |
| `description_field` | none | jq-style path to a description string. |
| `default_status` | `unknown` | Status when `status_field` resolves to empty: `success`, `warning`, `error`, `maintenance`, `unknown`. |
| `status_map` | none | Mapping from raw values to status: `success`, `warning`, `error`, `maintenance`. |
| `components_field` | none | jq-style path to a components array. |
| `component_name` | `.name` | jq-style path to component name within each item. |
| `component_status` | `.status` | jq-style path to component status within each item. |

#### HTML adapter config

| Key | Default | Description |
|-----|---------|-------------|
| `mode` | none | `css` (CSS selector) or `script_json` (extract JSON from script tags). |
| `selector` | none | CSS selector to match the status element. |
| `extract` | `text` | What to extract: `text`, `class`, or an attribute name. |
| `status_map` | none | Mapping from extracted values to status. Uses partial/contains matching. |

#### Google adapter config

| Key | Description |
|-----|-------------|
| `product_id` | Filter to a single product by ID (from `products.json`). |
| `location_id` | Filter incidents by GCP region (e.g. `us-central1`). |

### Conditions

jq-style boolean expressions evaluated against the watch data. If any condition is set, all four should be defined.

```yaml
conditions:
  error: ".cost > .limit"
  warning: ".cost > .limit * 0.8"
  maintenance: ".maintenance == true"
  success: ".cost <= .limit * 0.8"
```

When no conditions are set, the status is derived automatically:
- **HTTP/Adapter**: response received = success, otherwise error.
- **Script**: exit code 0 = success, otherwise error.

### Display

Controls how the watch is rendered in the UI.

| Key | Type | Description |
|-----|------|-------------|
| `style` | string | `status`, `progress`, `value`, `text`, `list`. |
| `title` | string | Title text (supports `{{ .field }}` templates). |
| `description` | string | Inline text shown next to the name (supports templates). |
| `content` | string | Content for `text` style (supports templates). |
| `value` | string | jq path for the current value (`progress`/`value` styles). |
| `min` | string | jq path or literal for minimum value (`progress` style). |
| `max` | string | jq path or literal for maximum value (`progress` style). |
| `details_from` | string | jq path to a JSON array for generating expandable details (e.g. `.components`). |
| `detail_title` | string | jq path within each array item for the detail title (default: `.name`). |
| `detail_status` | string | jq path within each array item for the detail status (default: `.status`). |

Template syntax: `{{ .field }}` or `{{ .nested.field }}` resolves values from the watch data.

### Details

Static detail panels attached to a watch. Each entry uses the same keys as `display`.

```yaml
details:
  - title: Budget
    style: progress
    value: .cost
    max: .limit
  - title: Request count
    style: value
    value: "{{ .requests_today }}"
```

## Project structure

```
src/              Vue frontend
src-tauri/        Rust backend (Tauri)
  src/
    adapter/      Status page adapters (statuspage, json, html, google)
    provider/     Data providers (http, script)
    config.rs     YAML config parser
    watcher.rs    Watch evaluation engine
    scheduler.rs  Background polling
    tray.rs       System tray management
presets/          Built-in service presets (375+ YAML files)
```
