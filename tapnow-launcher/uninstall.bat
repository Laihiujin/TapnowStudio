@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\uninstall.ps1"
if %errorlevel% neq 0 (
  echo Uninstall failed.
  pause
  exit /b %errorlevel%
)
echo Uninstall completed.
pause
