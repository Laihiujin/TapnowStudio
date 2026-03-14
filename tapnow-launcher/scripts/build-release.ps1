Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $projectRoot

Write-Host "[Build] cargo build --release"
cargo build --release

$exePath = Join-Path $projectRoot "target\release\TapnowStudio.exe"
if (-not (Test-Path $exePath)) {
    throw "Build succeeded but executable not found: $exePath"
}

Write-Host "[Done] $exePath"
