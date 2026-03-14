Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$installRoot = Join-Path $env:LOCALAPPDATA "TapnowStudio"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "TapnowStudio.lnk"
$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\TapnowStudio"
$startMenuShortcut = Join-Path $startMenuDir "TapnowStudio.lnk"

if (Test-Path $desktopShortcut) {
    Remove-Item $desktopShortcut -Force
}
if (Test-Path $startMenuShortcut) {
    Remove-Item $startMenuShortcut -Force
}
if (Test-Path $startMenuDir) {
    Remove-Item $startMenuDir -Recurse -Force
}
if (Test-Path $installRoot) {
    Remove-Item $installRoot -Recurse -Force
}

Write-Host "[Done] TapnowStudio uninstalled."
