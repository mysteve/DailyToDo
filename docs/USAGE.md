# 日迹使用说明

[返回项目首页](../README.md)

一个本地工作留痕桌面应用，使用 Rust + Tauri 2 + Vue 3 + TypeScript。日常记录无需联网，工作数据保存在本机 SQLite 数据库。

## 已实现

- 按日期记录备忘、工作进展与问题，添加标签，编辑和删除。
- 按日期维护待办清单，勾选完成状态。
- 写记录支持粘贴和上传图片，多图预览、移除和纯图片记录；图片随记录保存在本机。
- 日历跳转、全文搜索、按月统计记录天数、手记数量、待办完成率和标签分布。
- 用户主动选择日报、周报或月报，选择日期、来源和报告风格。
- 简洁汇报、详细汇报与个人复盘；AI 模式支持补充要求。
- 离线本地整理，或调用用户配置的 OpenAI 兼容 Chat Completions 接口。
- 报告历史、生成时的来源快照、复制、Markdown 导出。
- JSON 备份与合并导入。重复 ID 保留当前数据，整个导入经过验证后在同一事务中完成。

## 启动

Windows 上可直接运行打包后的 `src-tauri/target/release/riji.exe`，或安装 `src-tauri/target/release/bundle/nsis/` 下的安装程序。也可双击根目录的 `启动日迹.bat`。

开发环境需要 Node.js 22.12+、Rust 1.90+、Windows C++ 构建工具和 WebView2。其他系统的依赖参见 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)。

```powershell
npm install
npm run desktop
```

浏览器预览：

```powershell
npm run dev
```

访问 `http://127.0.0.1:1493`。预览使用独立的浏览器本地存储，与桌面数据库分开；AI 调用与系统凭据存储在桌面端运行。首次使用是空工作空间，可主动载入示例记录。

## AI 配置

在数据与设置中填写接口地址、模型名称和可选 API Key。接口必须兼容 `/chat/completions`，并返回 `choices[0].message.content`。

- 远程接口填写 HTTPS 地址，例如服务商提供的 `/v1` 基地址。
- 本地 Ollama 可使用 `http://127.0.0.1:11434/v1`，模型填写本机已安装的名称。
- 接口地址也可直接填写完整的 `/chat/completions` 地址。
- 使用保存并测试连接会发起一次小型连接测试，不发送工作记录。
- AI 报告仅在点击生成后发送选中日期范围与勾选的内容，生成前可查看来源。通常会产生服务商的调用费用。
- Windows 密钥保存在系统凭据管理器；macOS 使用 Keychain，Linux 使用 Secret Service。密钥不会写入 SQLite、浏览器存储或备份。
- 修改接口地址并留空密钥时会清除旧密钥，避免误用。更换服务请重新填写密钥。
- 单次输入限制为 60000 字符，超出时明确报错，不会静默截断。请求超时、认证失败或输出截断时不保存不完整报告。

报告统计由本地程序计算，AI 只负责整理文字。AI 的文字结论仍需用户确认。没有配置 AI 也可以使用本地整理生成完整报告。

## 日期与数据

日报使用所选日期；周报默认周一至周日，月报默认自然月。周报和月报可在日期范围日历中点选起止日、拖动框选，或直接填写开始与结束日期，支持跨月、跨年，包含起止两天。右侧所选范围会带入生成弹窗，本地整理与 AI 使用相同的范围筛选手记和待办。统计中的记录天数按有手记或待办的日期去重，待办完成率为已完成待办数 / 所选待办数；没有待办时显示暂无待办。

Windows 数据默认保存至 `%APPDATA%\com.riji.worklog\riji.db`。具体路径在数据与设置中显示。导出备份包含手记、图片、待办、报告和来源快照，不包含 AI 地址或密钥。导入合并已有记录，不覆盖相同 ID。

图片支持 PNG、JPG、WebP、GIF、BMP 与 AVIF。每条记录最多 9 张，单张上传不超过 20 MB；保存前缩放至最长边 2048 像素并压缩。图片以二进制独立保存在 SQLite，编辑或删除手记后，历史报告仍保留原始图片来源。备份格式为版本 2，兼容导入版本 1。AI 报告使用文字与图片附件信息，不发送图片内容。

快捷键：`Ctrl + N` 写记录，`Ctrl + K` 搜索，记录编辑时 `Ctrl + Enter` 保存，`Esc` 关闭弹窗。

## 验证与打包

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

前端测试覆盖周期边界、来源筛选、统计和目标状态；Rust 测试覆盖 SQLite 更新、备份原子性和报告规则。浏览器预览检查桌面及 390px 窄屏，Windows 桌面端单独检查 Rust 调用与重启后的数据保留。AI 接口使用本地模拟服务检查成功与认证失败路径；尚未使用真实服务商的凭据进行模型质量验收。

2026-10-09 验证：前端 5 项测试与 Rust 6 项测试通过；Windows 发布版在 `http://tauri.localhost/` 使用内置资源启动成功。可执行程序约 5.7 MiB，NSIS 安装包约 2.3 MiB。模拟 AI 联调共 3 次请求：连接测试、只纳入手记的报告生成、认证失败；临时密钥与测试记录已清理。

原生文件保存与导入对话框已实现，尚未完成自动操作验收。浏览器预览下载在本次自动化环境中被取消，该检查不能验证原生文件导出。

2026-10-09 界面更新：应用冷灰蓝方案，统一每日记录、工作报告、统计概览与设置弹窗，精简页面文案。记录页支持直接选择日报、周报、月报及切换报告周期，生成弹窗会沿用该选择。前端 5 项测试与 Windows 打包通过；已实际查看 1440×960、800×640、390×844 的页面及 Windows 发布版，检查记录筛选、记录保存与删除、目标勾选、搜索、周期联动和本地月报生成。预览使用独立浏览器示例数据。最终截图位于 `designs/implemented/`。

2026-10-09 窗口与图标更新：隐藏系统标题栏，最小化、最大化／还原和关闭按钮融入顶部工具区。顶部空白与标题区域支持拖动及双击最大化；弹窗打开时窗口按钮仍可使用。已在 Windows 发布版实际验证窗口操作与滚动后的按钮位置，并查看 800×640 窗口及 390×844 浏览器预览。修正紧凑日历的横向溢出。Q 版女生图标由内置 imagegen 生成，原图与生成提示词位于 `designs/imagegen/riji-girl-icon.png` 和 `riji-girl-icon-prompts.json`，已替换页面、程序与安装包图标，并实际查看 32px 与 128px 图标及程序、安装包内嵌图标。发布程序约 5.9 MiB，安装包约 2.5 MiB，最新截图为 `designs/implemented/frameless-girl-desktop.png`。

2026-10-09 图标定稿：采用参考可露希尔生成的 B 版眨眼形象。原图与内置 imagegen 提示词保存为 `designs/imagegen/riji-closure-icon.png` 和 `riji-closure-icon-prompts.json`。已同步页面、Windows 程序与 NSIS 图标资源，前端构建和 Windows 打包通过。实际查看 32px、128px 图标、程序及安装包内嵌图标、运行窗口图标，以及最大化和 800×640 窗口下的显示。发布程序约 5.97 MiB，安装包约 2.59 MiB，最新截图为 `designs/implemented/closure-wink-desktop.png`。

2026-10-09 报告范围更新：右侧周报、月报及生成弹窗支持自选起止日期，日历支持点选、拖选和跨月导航。前端 7 项测试、Rust 9 项测试与 Windows 打包通过；实际查看 1440×960、800×640、390×844 页面及 Windows 发布版，检查日期确认、取消、倒序校验与范围传递。桌面端生成并从 SQLite 读取跨年周报和月报，确认仅包含起止范围内的手记和目标；临时验证记录与报告已清理。最新截图为 `designs/implemented/report-range-desktop.png` 和 `report-range-generator.png`。

实现遵循 [Tauri Rust 命令接口](https://v2.tauri.app/develop/calling-rust/)与[系统文件对话框](https://v2.tauri.app/plugin/dialog/)的官方用法。
窗口控制遵循 [Tauri 自定义窗口](https://v2.tauri.app/learn/window-customization/)的官方用法。

2026-10-09 图片记录与待办更新：去除顶部保存状态，统一待办名称。记录编辑器支持上传、多图粘贴、图片预览和纯图片记录，保存按钮在小窗口下保持可见。前端 8 项测试、Rust 12 项测试及 Windows 打包通过；已实际查看 1440×960、800×640、390×844 浏览器页面及 Windows 发布版。桌面端验证上传、图片粘贴处理、编辑移除、纯图片保存、重启回显、报告图片来源保留与删除后的清理；备份图片往返与旧版数据兼容由 Rust 测试覆盖。临时验证记录已清理。最新截图为 `designs/implemented/note-images-editor.png` 与 `note-images-desktop.png`。
