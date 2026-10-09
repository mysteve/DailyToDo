<p align="center">
  <img src="public/app-icon.png" alt="日迹应用图标" width="112" height="112" />
</p>

<h1 align="center">日迹 · DailyToDo</h1>

<p align="center">记录日常工作，管理待办，按需整理工作报告。</p>

日迹是一款本地优先的桌面备忘录应用。日常记录无需联网，数据保存在本机，支持离线整理和可选的 AI 报告生成。

## 功能

- 📝 **每日记录**：按日期记录备忘、工作进展与问题，支持标签、编辑和搜索。
- ✅ **待办清单**：管理每日待办，勾选完成状态，回顾任务进度。
- 🖼️ **图片附件**：上传或粘贴图片，支持多图预览和纯图片记录。
- 📄 **工作报告**：生成日报、周报、月报，自选日期范围和来源，支持本地整理或 AI 生成。
- 📊 **统计回顾**：查看记录天数、待办完成率、标签分布和报告历史。
- 💾 **备份导出**：复制或导出 Markdown 报告，通过 JSON 备份与合并导入本地数据。

## 启动

开发环境：Node.js 22.12+、Rust 1.90+；Windows 还需 C++ 构建工具和 WebView2。

```powershell
npm ci
npm run desktop
```

仅预览界面可运行 `npm run dev`，访问 `http://127.0.0.1:1493`。浏览器预览数据与桌面数据独立，AI 功能在桌面端使用。

使用 `npm run desktop:build` 构建 Windows 程序与安装包，产物位于 `src-tauri/target/release/`。

## AI 与数据

AI 为可选功能，支持 OpenAI 兼容接口和本地 Ollama。生成时仅发送用户选中的记录、统计及图片附件信息，不发送图片内容；远程服务可能产生调用费用。API Key 保存在系统凭据库，不进入备份。

## 项目文档

- [详细使用说明](docs/USAGE.md)：AI 配置、日期规则、图片限制、数据位置与验证记录。
- [Agent 协作规范](Agent.md)：项目结构、开发约定与验证要求。

技术栈：Vue 3 · TypeScript · Vite · Tauri 2 · Rust · SQLite
