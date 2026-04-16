# wtch release script
# Run from PowerShell on Windows: .\devops\build\release.ps1 -Version "0.1.0"
# Builds and creates a GitHub Release with artifacts

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,

    [switch]$Draft,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path "$PSScriptRoot\..\.."
$DistDir = Join-Path $ProjectRoot "dist"
$Tag = "v$Version"

Write-Host "wtch release $Tag" -ForegroundColor Cyan

# Check gh CLI
if (-not (Get-Command "gh" -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: gh CLI not found. Install: https://cli.github.com" -ForegroundColor Red
    exit 1
}

# Build
if (-not $SkipBuild) {
    Write-Host "`nRunning build..." -ForegroundColor Yellow
    & "$PSScriptRoot\build.ps1"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed, aborting release." -ForegroundColor Red
        exit 1
    }
}

# Verify artifacts exist
$Artifacts = Get-ChildItem -Path $DistDir -Include "*.exe","*.msi" -Recurse -ErrorAction SilentlyContinue
if ($Artifacts.Count -eq 0) {
    Write-Host "ERROR: No artifacts found in $DistDir. Run build first." -ForegroundColor Red
    exit 1
}

Write-Host "`nArtifacts to upload:" -ForegroundColor Yellow
$Artifacts | ForEach-Object { Write-Host "  $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)" }

# Create git tag
Write-Host "`nCreating tag $Tag..." -ForegroundColor Yellow
Push-Location $ProjectRoot
git tag -a $Tag -m "Release $Tag"
git push origin $Tag

# Create GitHub release
Write-Host "`nCreating GitHub Release..." -ForegroundColor Yellow
$DraftFlag = if ($Draft) { "--draft" } else { "" }
$ArtifactPaths = ($Artifacts | ForEach-Object { $_.FullName }) -join " "

$ReleaseCmd = "gh release create $Tag $ArtifactPaths --title `"$Tag`" --generate-notes $DraftFlag"
Invoke-Expression $ReleaseCmd

if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to create release." -ForegroundColor Red
    Pop-Location
    exit 1
}

Pop-Location
Write-Host "`nRelease $Tag published!" -ForegroundColor Green
Write-Host "View: gh release view $Tag --web"
