@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\install.ps1"
if %errorlevel% neq 0 (
  echo Install failed.
  pause
  exit /b %errorlevel%
)
echo Install completed.
pause
