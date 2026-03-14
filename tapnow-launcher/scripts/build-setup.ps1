Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$installerScript = Join-Path $projectRoot "installer\TapnowLauncher.iss"
$cargoToml = Join-Path $projectRoot "Cargo.toml"
$distDir = Join-Path $projectRoot "dist"
$buildDir = Join-Path $projectRoot "build\bundle"
$runtimeDir = Join-Path $buildDir "runtime"
$tapnowSource = (Resolve-Path (Join-Path $projectRoot "..")).Path
$jimengSource = (Resolve-Path (Join-Path $projectRoot "..\..\jimeng-api")).Path
$tapnowDest = Join-Path $runtimeDir "Tapnow-Studio-PP"
$jimengDest = Join-Path $runtimeDir "jimeng-api"
$runtimeBinDir = Join-Path $runtimeDir "bin"
$pythonRuntimeDir = Join-Path $runtimeBinDir "python"

$isccCandidates = @(
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)
$iscc = $isccCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    throw "Inno Setup compiler (ISCC.exe) not found. Please install Inno Setup 6."
}

function Invoke-Robocopy {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [string[]]$ExcludeDirs = @(),
        [string[]]$ExcludeFiles = @()
    )

    New-Item -Path $Destination -ItemType Directory -Force | Out-Null
    $args = @(
        $Source,
        $Destination,
        "/E",
        "/R:1",
        "/W:1",
        "/NFL",
        "/NDL",
        "/NJH",
        "/NJS",
        "/NP"
    )

    if ($ExcludeDirs.Count -gt 0) {
        $args += "/XD"
        $args += $ExcludeDirs
    }
    if ($ExcludeFiles.Count -gt 0) {
        $args += "/XF"
        $args += $ExcludeFiles
    }

    & robocopy @args | Out-Null
    $code = $LASTEXITCODE
    if ($code -gt 7) {
        throw "Robocopy failed ($code): $Source -> $Destination"
    }
}

function Invoke-Npm {
    param(
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string[]]$NpmArgs
    )

    Push-Location $WorkingDirectory
    try {
        & npm @NpmArgs
        if ($LASTEXITCODE -ne 0) {
            throw "npm $($NpmArgs -join ' ') failed in $WorkingDirectory"
        }
    }
    finally {
        Pop-Location
    }
}

Set-Location $projectRoot
Write-Host "[Build] cargo build --release"
cargo build --release

$cargoText = Get-Content $cargoToml -Raw
$versionMatch = [regex]::Match($cargoText, 'version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "Unable to parse version from Cargo.toml"
}
$appVersion = $versionMatch.Groups[1].Value

Write-Host "[Build] preparing frontend/jimeng build artifacts"
if (-not (Test-Path (Join-Path $tapnowSource "node_modules"))) {
    Invoke-Npm -WorkingDirectory $tapnowSource -NpmArgs @("install", "--no-fund", "--no-audit")
}
Invoke-Npm -WorkingDirectory $tapnowSource -NpmArgs @("exec", "vite", "build")

if (-not (Test-Path (Join-Path $jimengSource "node_modules"))) {
    Invoke-Npm -WorkingDirectory $jimengSource -NpmArgs @("install", "--no-fund", "--no-audit")
}
if (-not (Test-Path (Join-Path $jimengSource "dist\index.js"))) {
    Invoke-Npm -WorkingDirectory $jimengSource -NpmArgs @("run", "build")
}

Write-Host "[Build] resolving node runtime"
$nodeCmd = Get-Command node -ErrorAction SilentlyContinue
if (-not $nodeCmd) {
    throw "node executable not found in PATH"
}
$nodeExe = $nodeCmd.Source
if (-not (Test-Path $nodeExe)) {
    throw "node executable path invalid: $nodeExe"
}

Write-Host "[Build] resolving python runtime"
$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCmd) {
    throw "python executable not found in PATH"
}
$pythonExe = $pythonCmd.Source
if (-not (Test-Path $pythonExe)) {
    throw "python executable path invalid: $pythonExe"
}
$pythonRoot = Split-Path $pythonExe -Parent
$pythonLibSource = Join-Path $pythonRoot "Lib"
$pythonDllsSource = Join-Path $pythonRoot "DLLs"
if (-not (Test-Path $pythonLibSource)) {
    throw "python Lib directory not found: $pythonLibSource"
}
if (-not (Test-Path $pythonDllsSource)) {
    throw "python DLLs directory not found: $pythonDllsSource"
}

Write-Host "[Build] preparing bundle directory"
if (Test-Path $buildDir) {
    Remove-Item $buildDir -Recurse -Force
}
New-Item -Path $runtimeDir -ItemType Directory -Force | Out-Null
New-Item -Path $runtimeBinDir -ItemType Directory -Force | Out-Null
New-Item -Path $pythonRuntimeDir -ItemType Directory -Force | Out-Null

Copy-Item (Join-Path $projectRoot "target\release\TapnowStudio.exe") (Join-Path $buildDir "TapnowStudio.exe") -Force
Copy-Item (Join-Path $projectRoot "README.md") (Join-Path $buildDir "README.txt") -Force
Copy-Item $nodeExe (Join-Path $runtimeBinDir "node.exe") -Force

$pythonCoreGlobs = @("python*.exe", "python*.dll", "vcruntime*.dll")
foreach ($pattern in $pythonCoreGlobs) {
    Get-ChildItem -Path $pythonRoot -Filter $pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item $_.FullName (Join-Path $pythonRuntimeDir $_.Name) -Force
    }
}

Invoke-Robocopy -Source $pythonLibSource -Destination (Join-Path $pythonRuntimeDir "Lib") -ExcludeDirs @(
    (Join-Path $pythonLibSource "site-packages"),
    (Join-Path $pythonLibSource "test"),
    (Join-Path $pythonLibSource "tkinter"),
    (Join-Path $pythonLibSource "idlelib")
)
Invoke-Robocopy -Source $pythonDllsSource -Destination (Join-Path $pythonRuntimeDir "DLLs")

Write-Host "[Build] bundling Tapnow-Studio-PP runtime files"
Invoke-Robocopy -Source $tapnowSource -Destination $tapnowDest -ExcludeDirs @(
    (Join-Path $tapnowSource ".git"),
    (Join-Path $tapnowSource "tapnow-launcher"),
    (Join-Path $tapnowSource "node_modules")
)

Write-Host "[Build] bundling jimeng-api runtime files"
Invoke-Robocopy -Source $jimengSource -Destination $jimengDest -ExcludeDirs @(
    (Join-Path $jimengSource ".git")
)

New-Item -Path $distDir -ItemType Directory -Force | Out-Null

Write-Host "[Build] ISCC - version $appVersion"
& $iscc "/DMyAppVersion=$appVersion" "/O$distDir" $installerScript
if ($LASTEXITCODE -ne 0) {
    throw "ISCC build failed with exit code $LASTEXITCODE"
}

$setupPath = Join-Path $distDir "TapnowStudio_Setup.exe"
if (-not (Test-Path $setupPath)) {
    throw "Setup output not found: $setupPath"
}

Write-Host "[Done] Setup created: $setupPath"
