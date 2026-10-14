@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"
set "riji_exe=%~dp0src-tauri\target\release\riji.exe"
if exist "%riji_exe%" (
  start "" "%riji_exe%"
  exit /b 0
)
if not exist "%~dp0node_modules\@tauri-apps\cli" (
  echo 请先安装项目依赖：npm install
  pause
  exit /b 1
)
call npm run desktop
