@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-setup.ps1"
if %errorlevel% neq 0 (
  echo Build setup failed.
  pause
  exit /b %errorlevel%
)
echo Build setup completed.
pause
