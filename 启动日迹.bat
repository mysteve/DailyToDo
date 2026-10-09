@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"
if exist "src-tauri\target\release\riji.exe" (
  start "" "src-tauri\target\release\riji.exe"
  exit /b 0
)
if not exist "node_modules\@tauri-apps\cli" (
  echo 请先安装项目依赖：npm install
  pause
  exit /b 1
)
call npm run desktop
