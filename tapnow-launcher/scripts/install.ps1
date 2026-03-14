param(
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$installRoot = Join-Path $env:LOCALAPPDATA "TapnowStudio"
$exeSource = Join-Path $projectRoot "target\release\TapnowStudio.exe"
$exeTarget = Join-Path $installRoot "TapnowStudio.exe"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "TapnowStudio.lnk"
$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\TapnowStudio"
$startMenuShortcut = Join-Path $startMenuDir "TapnowStudio.lnk"

if (-not $SkipBuild) {
    Write-Host "[Install] Build release executable"
    & (Join-Path $PSScriptRoot "build-release.ps1")
}

if (-not (Test-Path $exeSource)) {
    throw "Executable not found: $exeSource"
}

New-Item -Path $installRoot -ItemType Directory -Force | Out-Null
New-Item -Path $startMenuDir -ItemType Directory -Force | Out-Null
Copy-Item $exeSource $exeTarget -Force

$shell = New-Object -ComObject WScript.Shell

$desktop = $shell.CreateShortcut($desktopShortcut)
$desktop.TargetPath = $exeTarget
$desktop.WorkingDirectory = $installRoot
$desktop.IconLocation = "$env:SystemRoot\System32\SHELL32.dll,220"
$desktop.Save()

$menu = $shell.CreateShortcut($startMenuShortcut)
$menu.TargetPath = $exeTarget
$menu.WorkingDirectory = $installRoot
$menu.IconLocation = "$env:SystemRoot\System32\SHELL32.dll,220"
$menu.Save()

Write-Host "[Done] Installed to: $installRoot"
Write-Host "[Done] Desktop shortcut: $desktopShortcut"
Write-Host "[Done] Start menu shortcut: $startMenuShortcut"
