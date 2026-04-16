# Claude Usage watch

Monitor your Claude subscription usage directly in wtch with progress bars per quota.

## Prerequisites

- WSL with bash
- [PowerShell 7](https://github.com/PowerShell/PowerShell) (`pwsh.exe`) installed on Windows
- A logged-in Claude session in a Chromium browser (Brave, Chrome, Edge)

## Setup

### 1. Install the scraper script

The `claude-usage` script scrapes `claude.ai/settings/usage` via a headless browser and outputs JSON.

Create `~/.local/bin/claude-usage` (or any directory in your PATH):

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# On WSL, use PowerShell + Chromium to scrape (cookies are on Windows side)
if grep -qiE "microsoft|wsl" /proc/version 2>/dev/null; then
  PS_SCRAPER="$(wslpath -w "$SCRIPT_DIR/claude-usage-scraper.ps1")"
  output=$(pwsh.exe -NoProfile -ExecutionPolicy Bypass -File "$PS_SCRAPER" | tr -d '\r') || true
else
  echo "Error: only WSL is supported for now" >&2
  exit 1
fi

if [[ -z "$output" ]]; then
  echo "Error: scraper returned no output." >&2
  exit 1
fi

echo "$output"
```

### 2. Create the PowerShell scraper

Create `~/.local/bin/claude-usage-scraper.ps1`:

```powershell
# Scrapes claude.ai/settings/usage via Chromium DevTools Protocol
# Requires a logged-in session in Brave/Chrome/Edge

$ErrorActionPreference = "Stop"

# Find browser with Claude session cookies
$browserPaths = @(
    "$env:LOCALAPPDATA\BraveSoftware\Brave-Browser\User Data"
    "$env:LOCALAPPDATA\Google\Chrome\User Data"
    "$env:LOCALAPPDATA\Microsoft\Edge\User Data"
)

$userDataDir = $browserPaths | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $userDataDir) {
    Write-Output '{"error":"no_browser_found"}'
    exit 1
}

# Find browser executable
$browserExes = @(
    "$env:PROGRAMFILES\BraveSoftware\Brave-Browser\Application\brave.exe"
    "$env:PROGRAMFILES(x86)\BraveSoftware\Brave-Browser\Application\brave.exe"
    "$env:PROGRAMFILES\Google\Chrome\Application\chrome.exe"
    "$env:PROGRAMFILES(x86)\Google\Chrome\Application\chrome.exe"
    "$env:PROGRAMFILES(x86)\Microsoft\Edge\Application\msedge.exe"
)

$browserExe = $browserExes | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $browserExe) {
    Write-Output '{"error":"no_browser_exe"}'
    exit 1
}

$debugPort = 9234
$tempProfile = Join-Path $env:TEMP "claude-usage-profile"

# Launch headless browser with existing cookies
$proc = Start-Process -FilePath $browserExe -ArgumentList @(
    "--headless=new"
    "--disable-gpu"
    "--remote-debugging-port=$debugPort"
    "--user-data-dir=$tempProfile"
    "--profile-directory=Default"
    "--no-first-run"
    "https://claude.ai/settings/usage"
) -PassThru -WindowStyle Hidden

Start-Sleep -Seconds 3

try {
    # Connect to DevTools and extract usage data
    $response = Invoke-RestMethod "http://localhost:$debugPort/json"
    $wsUrl = $response[0].webSocketDebuggerUrl

    # Use CDP to evaluate JavaScript on the page
    $js = @"
    (function() {
        const bars = document.querySelectorAll('[class*="UsageBar"], [class*="progress"]');
        // Extract usage data from page DOM
        const data = { plan: '', bars: [] };
        // ... scraping logic specific to claude.ai DOM
        return JSON.stringify(data);
    })()
"@

    # Simplified: just fetch the page source and parse
    Start-Sleep -Seconds 2
    $pageResponse = Invoke-RestMethod "http://localhost:$debugPort/json"
    
    # Return scraped data
    Write-Output $result
}
finally {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $tempProfile -ErrorAction SilentlyContinue
}
```

> **Note:** The actual scraper implementation depends on Claude's DOM structure which may change.
> The script above is a skeleton. For a working implementation, see the [dotfiles example](https://github.com/ysaunier).

### 3. Make it executable

```bash
chmod +x ~/.local/bin/claude-usage
```

### 4. Test it

```bash
claude-usage --json
```

Expected output:

```json
{
  "plan": "Max (5x)",
  "bars": [
    {"label": "Session actuelle", "percentage": 61, "reset": "Reset in 42 min"},
    {"label": "Tous les modeles", "percentage": 13, "reset": "Reset Mon 17:00"},
    {"label": "Sonnet seulement", "percentage": 4, "reset": "Reset Tue 18:00"},
    {"label": "Routines quotidiennes", "percentage": 0, "reset": ""},
    {"label": "Usage supplementaire", "percentage": 100, "reset": "Renewal May 1"}
  ]
}
```

### 5. Add to wtch config

```yaml
watch:
  - name: Claude Usage
    type: script
    command: claude-usage --json
    shell: wsl
    format: json
    interval: "5m"
    link: https://console.anthropic.com
    expand: true
    conditions:
      error: ".bars[0].percentage > 90"
      warning: ".bars[0].percentage > 70"
      success: ".bars[0].percentage <= 70"
    display:
      style: progress
      title: Claude
      description: "{{ .bars[0].percentage }}% ({{ .bars[0].reset }})"
      value: ".bars[0].percentage"
      max: "100"
    details:
      - title: "{{ .bars[1].label }}"
        style: progress
        value: ".bars[1].percentage"
        max: "100"
        description: "{{ .bars[1].reset }}"
      - title: "{{ .bars[2].label }}"
        style: progress
        value: ".bars[2].percentage"
        max: "100"
        description: "{{ .bars[2].reset }}"
      - title: "{{ .bars[4].label }}"
        style: progress
        value: ".bars[4].percentage"
        max: "100"
        description: "{{ .bars[4].reset }}"
```

The watch shows:
- Main progress bar: current session usage
- Expanded details: all models, Sonnet-only, and extra usage quotas
- Warning at 70%, error at 90%
- Click the name to open the Anthropic console
