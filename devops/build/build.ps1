# wtch build script
# Run from PowerShell on Windows: .\devops\build\build.ps1
# Produces the .exe in dist/

$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path "$PSScriptRoot\..\.."
$DistDir = Join-Path $ProjectRoot "dist"

Write-Host "wtch build" -ForegroundColor Cyan
Write-Host "Project: $ProjectRoot"

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

# Check prerequisites
Write-Host "`nChecking prerequisites..." -ForegroundColor Yellow

if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: cargo not found. Install Rust: https://rustup.rs" -ForegroundColor Red
    exit 1
}

if (-not (Get-Command "node" -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: node not found. Install Node.js: https://nodejs.org" -ForegroundColor Red
    exit 1
}

Write-Host "  cargo: $(cargo --version)"
Write-Host "  node:  $(node --version)"
Write-Host "  npm:   $(npm --version)"

# Install npm deps
Write-Host "`nInstalling npm dependencies..." -ForegroundColor Yellow
Push-Location $ProjectRoot
npm ci
if ($LASTEXITCODE -ne 0) { npm install }

# Build
Write-Host "`nBuilding wtch..." -ForegroundColor Yellow
npx tauri build

if ($LASTEXITCODE -ne 0) {
    Write-Host "`nBuild failed!" -ForegroundColor Red
    Pop-Location
    exit 1
}

# Copy artifacts to dist/
Write-Host "`nCopying artifacts to dist/..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

$BundleDir = Join-Path $ProjectRoot "src-tauri\target\release\bundle"
$NsisExe = Get-ChildItem -Path "$BundleDir\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$MsiFile = Get-ChildItem -Path "$BundleDir\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1

if ($NsisExe) {
    Copy-Item $NsisExe.FullName $DistDir
    Write-Host "  $($NsisExe.Name)" -ForegroundColor Green
}
if ($MsiFile) {
    Copy-Item $MsiFile.FullName $DistDir
    Write-Host "  $($MsiFile.Name)" -ForegroundColor Green
}

# Also copy the raw exe
$RawExe = Join-Path $ProjectRoot "src-tauri\target\release\wtch.exe"
if (Test-Path $RawExe) {
    Copy-Item $RawExe $DistDir
    Write-Host "  wtch.exe" -ForegroundColor Green
}

Pop-Location
Write-Host "`nBuild complete! Artifacts in: $DistDir" -ForegroundColor Green
