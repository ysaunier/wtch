# wtch dev script
# Run from PowerShell on Windows: .\devops\run\dev.ps1
# Starts Tauri in dev mode with hot reload

$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path "$PSScriptRoot\..\.."

Write-Host "wtch dev" -ForegroundColor Cyan
Write-Host "Project: $ProjectRoot"

# Save current PATH before VS env loading (VS overwrites it)
$PreVsPath = $env:PATH

# Set up Visual Studio environment for MSVC linker
$VsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $VsWhere) {
    $VsPath = & $VsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($VsPath) {
        $VcVarsAll = Join-Path $VsPath "VC\Auxiliary\Build\vcvarsall.bat"
        if (Test-Path $VcVarsAll) {
            Write-Host "`nLoading Visual Studio environment..." -ForegroundColor Yellow
            $EnvOutput = cmd /c "`"$VcVarsAll`" x64 >nul 2>&1 && set"
            foreach ($Line in $EnvOutput) {
                if ($Line -match '^([^=]+)=(.*)$') {
                    [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
                }
            }
        }
    }
}

# Restore paths that VS env might have dropped (node, cargo, npm)
foreach ($Dir in $PreVsPath -split ';') {
    if ($Dir -and ($env:PATH -notlike "*$Dir*")) {
        $env:PATH = "$env:PATH;$Dir"
    }
}

# Check and install Rust
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    if (-not (Get-Command "rustup" -ErrorAction SilentlyContinue)) {
        Write-Host "`nRust not found. Installing via rustup..." -ForegroundColor Yellow
        Invoke-RestMethod -Uri "https://win.rustup.rs" -OutFile "$env:TEMP\rustup-init.exe"
        & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable
    }
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: cargo not found. Restart your terminal." -ForegroundColor Red
        exit 1
    }
}

# Check and install Node.js
if (-not (Get-Command "node" -ErrorAction SilentlyContinue)) {
    Write-Host "`nNode.js not found. Installing via winget..." -ForegroundColor Yellow
    winget install OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements
    $env:PATH = "$env:ProgramFiles\nodejs;$env:PATH"
    if (-not (Get-Command "node" -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: node not found. Restart your terminal." -ForegroundColor Red
        exit 1
    }
}

Write-Host "  cargo: $(cargo --version)" -ForegroundColor DarkGray
Write-Host "  node:  $(node --version)" -ForegroundColor DarkGray

# Clean stale node_modules and install
Push-Location $ProjectRoot
if (Test-Path "node_modules") {
    # Check if modules are valid for this platform
    $EsbuildBin = Join-Path "node_modules" ".package-lock.json"
    $NeedsReinstall = $false
    if (-not (Test-Path (Join-Path "node_modules" ".bin\vite.ps1"))) {
        $NeedsReinstall = $true
    }
    if ($NeedsReinstall) {
        Write-Host "`nRemoving stale node_modules..." -ForegroundColor Yellow
        cmd /c "rmdir /s /q node_modules" 2>$null
    }
}

Write-Host "`nInstalling npm dependencies..." -ForegroundColor Yellow
npm install

Write-Host "`nStarting tauri dev..." -ForegroundColor Green
npx tauri dev

Pop-Location
