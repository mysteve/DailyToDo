mod ai;
mod db;
mod images;
mod models;
mod reports;

use models::*;
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::{Manager, State};
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
) -> Result<(), String> {
    ai::endpoint(&settings.endpoint)?;
    if settings.model.chars().count() > 200 || api_key.len() > 8192 {
        return Err("模型名称或密钥过长".into());
    }
    let db = database(&state)?;
    let previous = db::settings(&db)?;
    settings.endpoint = settings.endpoint.trim().trim_end_matches('/').to_string();
    settings.model = settings.model.trim().to_string();
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
    let (workspace, key) = {
        let db = database(&state)?;
        let workspace = db::load(&db, state.storage_path.clone())?;
        let key = if request.mode == GenerationMode::Ai {
            ai::read_key()?
        } else {
            None
        };
        (workspace, key)
    };
    let notes: Vec<_> = workspace
        .notes
        .into_iter()
        .filter(|note| request.include_notes && note.date >= start && note.date <= end)
        .collect();
    let goals: Vec<_> = workspace
        .goals
        .into_iter()
        .filter(|goal| request.include_goals && goal.date >= start && goal.date <= end)
        .collect();
    let mut report = reports::make_report(&request, notes, goals)?;
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
async fn export_backup(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let (workspace, images) = {
        let connection = database(&state)?;
        let workspace = db::load(&connection, state.storage_path.clone())?;
        let images = images::export(&connection, &workspace)?;
        (workspace, images)
    };
    let backup = Backup {
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

#[tauri::command]
fn import_examples(
    state: State<'_, AppState>,
    notes: Vec<Note>,
    goals: Vec<Goal>,
) -> Result<(), String> {
    let mut db = database(&state)?;
    let current = db::load(&db, String::new())?;
    if !current.notes.is_empty() || !current.goals.is_empty() {
        return Err("已有工作记录，无法载入示例".into());
    }
    db::import(
        &mut db,
        &Backup {
            version: 1,
            notes,
            goals,
            reports: vec![],
            images: vec![],
        },
    )
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_workspace,
            save_note,
            read_note_image,
            delete_note,
            save_goal,
            delete_goal,
            delete_report,
            save_ai_settings,
            test_ai_connection,
            generate_report,
            export_report,
            export_backup,
            import_backup,
            import_examples
        ])
        .run(tauri::generate_context!())
        .expect("日迹启动失败");
}
