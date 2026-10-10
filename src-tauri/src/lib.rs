mod ai;
mod db;
mod git;
mod images;
mod models;
mod reports;

use models::*;
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::{LogicalSize, Manager, PhysicalPosition, PhysicalSize, State};
use tauri_plugin_dialog::DialogExt;

struct AppState {
    database: Mutex<Connection>,
    storage_path: String,
    generation: tokio::sync::Mutex<()>,
}

fn database(state: &AppState) -> Result<MutexGuard<'_, Connection>, String> {
    state
        .database
        .lock()
        .map_err(|_| "本地数据库暂时不可用".into())
}

#[tauri::command]
fn load_workspace(state: State<'_, AppState>) -> Result<Workspace, String> {
    let mut workspace = db::load(&*database(&state)?, state.storage_path.clone())?;
    workspace.settings.has_api_key = ai::read_key().ok().flatten().is_some();
    workspace.settings.git.has_token = git::read_token().ok().flatten().is_some();
    Ok(workspace)
}

#[tauri::command]
fn save_note(
    state: State<'_, AppState>,
    note: Note,
    images: Option<Vec<ImageUpload>>,
) -> Result<(), String> {
    db::save_note_with_images(&*database(&state)?, &note, &images.unwrap_or_default())
}

#[tauri::command]
fn read_note_image(state: State<'_, AppState>, id: String) -> Result<String, String> {
    images::read(&*database(&state)?, &id)
}

#[tauri::command]
fn delete_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    db::delete(&*database(&state)?, "notes", &id)
}

#[tauri::command]
fn save_goal(state: State<'_, AppState>, goal: Goal) -> Result<(), String> {
    db::save_goal(&*database(&state)?, &goal)
}

#[tauri::command]
fn save_goals(state: State<'_, AppState>, goals: Vec<Goal>) -> Result<(), String> {
    db::save_goals(&*database(&state)?, &goals)
}

#[tauri::command]
fn delete_goal(state: State<'_, AppState>, id: String) -> Result<(), String> {
    db::delete(&*database(&state)?, "goals", &id)
}

#[tauri::command]
fn delete_report(state: State<'_, AppState>, id: String) -> Result<(), String> {
    db::delete(&*database(&state)?, "reports", &id)
}

#[tauri::command]
fn save_ai_settings(
    state: State<'_, AppState>,
    mut settings: AiSettings,
    api_key: String,
    clear_key: bool,
    git_token: String,
    clear_git_token: bool,
) -> Result<(), String> {
    ai::endpoint(&settings.endpoint)?;
    settings.report_templates.validate()?;
    if settings.model.chars().count() > 200 || api_key.len() > 8192 || git_token.len() > 8192 {
        return Err("模型名称或密钥过长".into());
    }
    git::normalize(&mut settings.git)?;
    let db = database(&state)?;
    let previous = db::settings(&db)?;
    settings.endpoint = settings.endpoint.trim().trim_end_matches('/').to_string();
    settings.model = settings.model.trim().to_string();
    let git_changed = settings.git.provider != previous.git.provider
        || (settings.git.provider == GitProvider::Gitlab
            && settings.git.gitlab_url != previous.git.gitlab_url);
    if clear_git_token || (git_changed && git_token.trim().is_empty()) {
        git::clear_token()?;
    }
    if !git_token.trim().is_empty() && !clear_git_token {
        git::credential()?
            .set_password(git_token.trim())
            .map_err(|_| "无法将令牌保存到系统凭据存储")?;
    }
    if clear_key || (settings.endpoint != previous.endpoint && api_key.trim().is_empty()) {
        ai::clear_key()?;
    }
    if !api_key.trim().is_empty() && !clear_key {
        ai::credential()?
            .set_password(api_key.trim())
            .map_err(|_| "无法将密钥保存到系统凭据存储")?;
    }
    db::save_settings(&db, &settings)
}

#[tauri::command]
async fn test_git_connection(
    state: State<'_, AppState>,
    git: GitSettings,
    token: String,
) -> Result<String, String> {
    if token.len() > 8192 {
        return Err("令牌过长".into());
    }
    let mut git = git;
    git::normalize(&mut git)?;
    let token = if token.trim().is_empty() {
        let previous = db::settings(&*database(&state)?)?.git;
        if previous.provider != git.provider
            || (git.provider == GitProvider::Gitlab && previous.gitlab_url != git.gitlab_url)
        {
            return Err("请输入新的访问令牌".into());
        }
        git::read_token()?.ok_or("请先填写访问令牌")?
    } else {
        token.trim().to_string()
    };
    git::test(&git, &token).await
}

#[tauri::command]
fn open_git_token_page(git: GitSettings) -> Result<(), String> {
    git::open_token_page(&git)
}

#[tauri::command]
async fn list_ai_models(
    state: State<'_, AppState>,
    endpoint: String,
    api_key: String,
    clear_key: bool,
) -> Result<Vec<String>, String> {
    if endpoint.len() > 1000 || api_key.len() > 8192 {
        return Err("接口地址或密钥过长".into());
    }
    let requested_endpoint = ai::endpoint(&endpoint)?;
    let _guard = state
        .generation
        .try_lock()
        .map_err(|_| "AI 服务正在请求中，请稍后刷新模型")?;
    let key = if clear_key {
        None
    } else if !api_key.trim().is_empty() {
        Some(api_key.trim().to_string())
    } else {
        let previous = db::settings(&*database(&state)?)?;
        if ai::endpoint(&previous.endpoint)? == requested_endpoint {
            ai::read_key()?
        } else {
            None
        }
    };
    ai::list_models(&endpoint, key.as_deref()).await
}

#[tauri::command]
async fn test_ai_connection(state: State<'_, AppState>) -> Result<String, String> {
    let _guard = state
        .generation
        .try_lock()
        .map_err(|_| "正在生成报告，请完成后再测试连接")?;
    let (config, key) = {
        let db = database(&state)?;
        (db::settings(&db)?, ai::read_key()?)
    };
    ai::test(&config, key.as_deref()).await
}

#[tauri::command]
async fn generate_report(
    state: State<'_, AppState>,
    request: ReportRequest,
) -> Result<Report, String> {
    let _guard = state
        .generation
        .try_lock()
        .map_err(|_| "已有报告正在生成，请稍候")?;
    let (start, end) = reports::period(&request)?;
    let use_commits = request.mode == GenerationMode::Ai && request.include_commits;
    let (workspace, key, git_token) = {
        let db = database(&state)?;
        let workspace = db::load(&db, state.storage_path.clone())?;
        let key = if request.mode == GenerationMode::Ai {
            ai::read_key()?
        } else {
            None
        };
        let git_token = if use_commits {
            git::read_token()?
        } else {
            None
        };
        (workspace, key, git_token)
    };
    let goals: Vec<_> = workspace
        .goals
        .into_iter()
        .filter(|goal| goal.date >= start && goal.date <= end)
        .collect();
    let mut report = reports::make_report(&request, vec![], goals)?;
    report.markdown = reports::render_template(&report, &workspace.settings.report_templates)?;
    if use_commits {
        let token = git_token.ok_or("请先在设置中配置 Git 访问令牌")?;
        if !workspace.settings.git.enabled {
            return Err("请先在设置中启用提交记录".into());
        }
        report.sources.commits =
            git::fetch_commits(&workspace.settings.git, &token, &start, &end).await?;
    }
    if request.mode == GenerationMode::Ai {
        report.markdown =
            ai::generate(&workspace.settings, key.as_deref(), &report, &request).await?;
        report.model = workspace.settings.model;
    }
    db::save_report(&*database(&state)?, &report)?;
    Ok(report)
}

async fn save_file(
    app: tauri::AppHandle,
    name: String,
    content: String,
    extension: &'static str,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = app
            .dialog()
            .file()
            .set_title("保存文件")
            .set_file_name(name)
            .add_filter("文件", &[extension])
            .blocking_save_file();
        let Some(file) = file else {
            return Ok(false);
        };
        let path = file.into_path().map_err(|_| "保存路径无效")?;
        std::fs::write(path, content.as_bytes()).map_err(|e| format!("无法保存文件：{e}"))?;
        Ok(true)
    })
    .await
    .map_err(|_| "保存操作异常")?
}

#[tauri::command]
async fn export_report(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let workspace = db::load(&*database(&state)?, state.storage_path.clone())?;
    let report = workspace
        .reports
        .into_iter()
        .find(|report| report.id == id)
        .ok_or("报告不存在")?;
    let filename = format!("日迹-{}-{}.md", report.r#type.label(), report.start_date);
    save_file(app, filename, report.markdown, "md").await
}

#[tauri::command]
async fn export_csv(app: tauri::AppHandle, name: String, content: String) -> Result<bool, String> {
    if content.len() > 32 * 1024 * 1024 {
        return Err("导出内容过大，请缩小日期范围".into());
    }
    let name = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-");
    if name.is_empty() || name.chars().count() > 120 {
        return Err("文件名不正确".into());
    }
    save_file(app, name, content, "csv").await
}

#[tauri::command]
async fn export_backup(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let (workspace, images) = {
        let connection = database(&state)?;
        let workspace = db::load(&connection, state.storage_path.clone())?;
        let images = images::export(&connection, &workspace)?;
        (workspace, images)
    };
    let backup = Backup {
        report_templates: Some(workspace.settings.report_templates),
        version: 2,
        notes: workspace.notes,
        goals: workspace.goals,
        reports: workspace.reports,
        images,
    };
    let content = serde_json::to_string_pretty(&backup).map_err(|_| "无法整理备份数据")?;
    save_file(
        app,
        format!("日迹备份-{}.json", chrono::Local::now().format("%Y-%m-%d")),
        content,
        "json",
    )
    .await
}

#[tauri::command]
async fn import_backup(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let content =
        tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
            let file = app
                .dialog()
                .file()
                .set_title("导入日迹备份")
                .add_filter("日迹备份", &["json"])
                .blocking_pick_file();
            let Some(file) = file else {
                return Ok(None);
            };
            let path = file.into_path().map_err(|_| "备份路径无效")?;
            if std::fs::metadata(&path)
                .map_err(|_| "无法读取备份文件")?
                .len()
                > 256 * 1024 * 1024
            {
                return Err("备份文件不能超过 256 MB".into());
            }
            std::fs::read_to_string(path)
                .map(Some)
                .map_err(|_| "无法读取备份文件".to_string())
        })
        .await
        .map_err(|_| "导入操作异常")??;
    let Some(content) = content else {
        return Ok(false);
    };
    let backup: Backup = serde_json::from_str(&content).map_err(|_| "备份文件格式不正确")?;
    db::import(&mut *database(&state)?, &backup)?;
    Ok(true)
}

// WebView2 的常规自动填充默认开启，会在自由文本输入框下弹出「保存的信息」面板。
// Tauri 未暴露该设置，只能通过 WebView2 COM 接口关闭；失败不阻断启动。
#[cfg(target_os = "windows")]
fn disable_autofill(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use windows_core::Interface;

    let result = window.with_webview(|webview| {
        let applied = (|| -> windows_core::Result<bool> {
            let core = unsafe { webview.controller().CoreWebView2() }?;
            let settings = unsafe { core.Settings() }?;
            let settings = settings.cast::<ICoreWebView2Settings4>()?;
            unsafe { settings.SetIsGeneralAutofillEnabled(false)? };
            let mut enabled = windows_core::BOOL(1);
            unsafe { settings.IsGeneralAutofillEnabled(&mut enabled)? };
            Ok(enabled.as_bool())
        })();
        report_webview_message(match applied {
            Ok(false) => "WebView2 常规自动填充已关闭".into(),
            Ok(true) => "WebView2 常规自动填充仍处于开启状态".into(),
            Err(error) => format!("关闭 WebView2 自动填充失败：{error}"),
        });
    });
    if let Err(error) = result {
        report_webview_message(format!("无法访问 WebView2 设置：{error}"));
    }
}

#[cfg(target_os = "windows")]
fn report_webview_message(message: String) {
    #[cfg(debug_assertions)]
    println!("{message}");
    #[cfg(not(debug_assertions))]
    let _ = message;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let path = directory.join("riji.db");
            let connection = db::open(&path).map_err(std::io::Error::other)?;
            app.manage(AppState {
                database: Mutex::new(connection),
                storage_path: path.to_string_lossy().to_string(),
                generation: tokio::sync::Mutex::new(()),
            });
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                disable_autofill(&window);
                if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
                    let area = monitor.work_area();
                    let size = PhysicalSize::new(1600, 900);
                    let logical = size.to_logical::<f64>(monitor.scale_factor());
                    window.set_min_size(Some(LogicalSize::new(
                        logical.width.min(640.0),
                        logical.height.min(360.0),
                    )))?;
                    // Windows finalizes hidden frame offsets when the window is shown.
                    window.show()?;
                    window.set_size(size)?;
                    let outer = window.outer_size()?;
                    window.set_position(PhysicalPosition::new(
                        area.position.x + (area.size.width.saturating_sub(outer.width) / 2) as i32,
                        area.position.y
                            + (area.size.height.saturating_sub(outer.height) / 2) as i32,
                    ))?;
                } else {
                    window.show()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_workspace,
            save_note,
            read_note_image,
            delete_note,
            save_goal,
            save_goals,
            delete_goal,
            delete_report,
            save_ai_settings,
            test_git_connection,
            open_git_token_page,
            list_ai_models,
            test_ai_connection,
            generate_report,
            export_report,
            export_csv,
            export_backup,
            import_backup
        ])
        .run(tauri::generate_context!())
        .expect("日迹启动失败");
}
