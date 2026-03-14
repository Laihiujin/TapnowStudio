# TapnowStudio

Windows launcher + installer for TapnowStudio.

## What Setup contains

- `TapnowStudio.exe`
- `runtime\Tapnow-Studio-PP` (includes built `dist`)
- `runtime\jimeng-api`
- `runtime\bin\node.exe`
- `runtime\bin\python` (embedded Python runtime for localserver)

## Build Setup

```powershell
cd D:\Siuyechu\TapnowStudio\Tapnow-Studio-PP\tapnow-launcher
.\scripts\build-setup.ps1
```

Output:

`dist\TapnowStudio_Setup.exe`

## Runtime behavior

- Starts `jimeng-api` in hidden mode
- Starts local server `9527` with bundled Python runtime
- Starts embedded static frontend server on `8080`
- Opens browser in a **new window**

## Optional env vars

- `TAPNOW_STUDIO_DIR`
- `JIMENG_API_DIR`
- `TAPNOW_FRONTEND_PORT` (default `8080`)
- `JIMENG_API_PORT` (default `5100`)
- `TAPNOW_LOCALSERVER_PORT` (default `9527`)
- `TAPNOW_ENABLE_LOCALSERVER` (default `true`)
- `TAPNOW_LOCALSERVER_SCRIPT`

Logs:

`%LOCALAPPDATA%\TapnowStudio\logs`
