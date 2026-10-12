use crate::{images, models::*, reports::validate_date};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|e| format!("无法打开本地数据库：{e}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > 3 {
        return Err("数据库来自更新版本，请使用新版日迹打开".into());
    }
    connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; BEGIN IMMEDIATE;
        CREATE TABLE IF NOT EXISTS notes (id TEXT PRIMARY KEY, date TEXT NOT NULL, payload TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS notes_date ON notes(date);
        CREATE TABLE IF NOT EXISTS goals (id TEXT PRIMARY KEY, date TEXT NOT NULL, payload TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS goals_date ON goals(date);
        CREATE TABLE IF NOT EXISTS reports (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK (id = 1), payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS images (id TEXT PRIMARY KEY, payload TEXT NOT NULL, data BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS plans (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS plan_weeks (plan_id TEXT NOT NULL REFERENCES plans(id), week_start TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(plan_id, week_start));
        PRAGMA user_version = 3; COMMIT;") .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn read_list<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<T>, String> {
    let mut statement = connection.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|row| {
        let value = row.map_err(|e| e.to_string())?;
        serde_json::from_str(&value).map_err(|e| format!("本地数据格式异常：{e}"))
    })
    .collect()
}

pub fn load(connection: &Connection, storage_path: String) -> Result<Workspace, String> {
    let notes = read_list(connection, "SELECT payload FROM notes ORDER BY date, id")?;
    let goals = read_list(connection, "SELECT payload FROM goals ORDER BY date, id")?;
    let reports = read_list(
        connection,
        "SELECT payload FROM reports ORDER BY created_at DESC",
    )?;
    Ok(Workspace {
        plans: read_list(connection, "SELECT payload FROM plans ORDER BY id")?,
        plan_weeks: read_list(
            connection,
            "SELECT payload FROM plan_weeks ORDER BY week_start, plan_id",
        )?,
        notes,
        goals,
        reports,
        settings: settings(connection)?,
        storage_path,
    })
}

pub fn settings(connection: &Connection) -> Result<AiSettings, String> {
    let raw: Option<String> = connection
        .query_row("SELECT payload FROM settings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let mut config: AiSettings = raw
        .map(|value| serde_json::from_str(&value).map_err(|e| e.to_string()))
        .unwrap_or_else(|| Ok(AiSettings::default()))?;
    config.report_templates.upgrade_defaults();
    Ok(config)
}

pub fn validate_id(id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "记录标识不正确".into())
}

pub fn validate_note(note: &Note) -> Result<(), String> {
    validate_id(&note.id)?;
    validate_date(&note.date)?;
    if note.content.trim().is_empty() && note.images.is_empty() {
        return Err("请填写记录内容或添加图片".into());
    }
    if note.content.chars().count() > 32000 {
        return Err("记录内容不能超过 32000 字".into());
    }
    if note.images.len() > 9 {
        return Err("每条记录最多添加 9 张图片".into());
    }
    let mut ids = std::collections::HashSet::new();
    for image in &note.images {
        images::validate_metadata(image)?;
        if !ids.insert(&image.id) {
            return Err("记录中的图片重复".into());
        }
    }
    if note.tag.chars().count() > 40 {
        return Err("标签不能超过 40 字".into());
    }
    validate_times(&note.created_at, &note.updated_at)
}

pub fn validate_goal(goal: &Goal) -> Result<(), String> {
    validate_id(&goal.id)?;
    validate_date(&goal.date)?;
    if goal.title.trim().is_empty() || goal.title.chars().count() > 300 {
        return Err("待办需为 1 至 300 字".into());
    }
    if goal.project.chars().count() > 40 || goal.project.contains('\n') {
        return Err("项目名称不能超过 40 字，且需在一行内".into());
    }
    if goal.risk.chars().count() > 300 || goal.risk.contains('\n') {
        return Err("风险／原因不能超过 300 字，且需在一行内".into());
    }
    for minutes in [goal.estimate_min, goal.actual_min].into_iter().flatten() {
        if minutes > MAX_MINUTES {
            return Err("耗时需在 0 至 100000 分钟之间".into());
        }
    }
    match (&goal.completed_at, goal.completed) {
        (Some(_), false) => return Err("未完成的待办不能有完成时间".into()),
        (Some(value), true) => {
            chrono::DateTime::parse_from_rfc3339(value).map_err(|_| "完成时间不正确")?;
        }
        _ => {}
    }
    if let Some(origin) = &goal.origin_date {
        validate_date(origin)?;
    }
    if goal.carry_count > 1000 {
        return Err("顺延次数不正确".into());
    }
    validate_times(&goal.created_at, &goal.updated_at)
}

const MAX_MINUTES: u32 = 100_000;

/// 在一个事务中保存多条待办，用于批量顺延；任一条无效时全部不写入。
pub fn save_goals(connection: &Connection, goals: &[Goal]) -> Result<(), String> {
    if goals.is_empty() || goals.len() > 500 {
        return Err("一次最多处理 500 条待办".into());
    }
    for goal in goals {
        validate_goal(goal)?;
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    for goal in goals {
        save_goal(&transaction, goal)?;
    }
    transaction.commit().map_err(|e| e.to_string())
}

fn validate_times(created: &str, updated: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(created).map_err(|_| "创建时间不正确")?;
    chrono::DateTime::parse_from_rfc3339(updated).map_err(|_| "更新时间不正确")?;
    Ok(())
}

#[cfg(test)]
pub fn save_note(connection: &Connection, note: &Note) -> Result<(), String> {
    save_note_with_images(connection, note, &[])
}

pub fn save_note_with_images(
    connection: &Connection,
    note: &Note,
    uploads: &[ImageUpload],
) -> Result<(), String> {
    validate_note(note)?;
    if uploads.len() > note.images.len() {
        return Err("上传的图片数量不正确".into());
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    let mut ids = std::collections::HashSet::new();
    for upload in uploads {
        if !ids.insert(&upload.id) {
            return Err("上传的图片重复".into());
        }
        let image = note
            .images
            .iter()
            .find(|image| image.id == upload.id)
            .ok_or("上传图片未关联记录")?;
        images::store(&transaction, image, &upload.data)?;
    }
    images::check_references(&transaction, &note.images)?;
    let payload = serde_json::to_string(note).map_err(|e| e.to_string())?;
    transaction.execute("INSERT INTO notes(id, date, payload) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET date = excluded.date, payload = excluded.payload", params![note.id, note.date, payload]).map_err(|e| e.to_string())?;
    images::prune(&transaction)?;
    transaction.commit().map_err(|e| e.to_string())
}

pub fn save_goal(connection: &Connection, goal: &Goal) -> Result<(), String> {
    validate_goal(goal)?;
    let payload = serde_json::to_string(goal).map_err(|e| e.to_string())?;
    connection.execute("INSERT INTO goals(id, date, payload) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET date = excluded.date, payload = excluded.payload", params![goal.id, goal.date, payload]).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_plan_week(connection: &Connection, row: &PlanWeek) -> Result<(), String> {
    connection.execute("INSERT INTO plan_weeks(plan_id,week_start,payload) VALUES (?1,?2,?3) ON CONFLICT(plan_id,week_start) DO UPDATE SET payload=excluded.payload", params![row.plan_id,row.week_start,serde_json::to_string(row).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    Ok(())
}

fn validate_links(connection: &Connection, row: &PlanWeek) -> Result<(), String> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM plans WHERE id=?1)",
            params![row.plan_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err("计划不存在".into());
    }
    for id in &row.goal_ids {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goals WHERE id=?1)",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            return Err("关联待办已不存在，请重新选择".into());
        }
    }
    Ok(())
}

pub fn save_plan(connection: &Connection, plan: &Plan, row: &PlanWeek) -> Result<(), String> {
    crate::plans::validate_plan(plan)?;
    crate::plans::validate_row(row)?;
    if plan.id != row.plan_id {
        return Err("计划标识不一致".into());
    }
    let tx = connection
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO plans(id,payload) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![plan.id,serde_json::to_string(plan).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    validate_links(&tx, row)?;
    write_plan_week(&tx, row)?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn archive_plan(connection: &Connection, id: &str, archived: bool) -> Result<(), String> {
    validate_id(id)?;
    let raw: String = connection
        .query_row("SELECT payload FROM plans WHERE id=?1", params![id], |r| {
            r.get(0)
        })
        .map_err(|_| "计划不存在")?;
    let mut plan: Plan = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    plan.archived = archived;
    connection
        .execute(
            "UPDATE plans SET payload=?2 WHERE id=?1",
            params![id, serde_json::to_string(&plan).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn carry_plans(connection: &Connection, through: &str) -> Result<(), String> {
    let tx = connection
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    let plans: Vec<Plan> = read_list(&tx, "SELECT payload FROM plans")?;
    let rows: Vec<PlanWeek> = read_list(&tx, "SELECT payload FROM plan_weeks")?;
    let existing: std::collections::HashSet<_> =
        rows.iter().map(|r| (&r.plan_id, &r.week_start)).collect();
    for row in crate::plans::carry(&plans, &rows, through)? {
        if !existing.contains(&(&row.plan_id, &row.week_start)) {
            write_plan_week(&tx, &row)?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn save_report(connection: &Connection, report: &Report) -> Result<(), String> {
    for note in &report.sources.notes {
        images::check_references(connection, &note.images)?;
    }
    let payload = serde_json::to_string(report).map_err(|e| e.to_string())?;
    connection
        .execute(
            "INSERT INTO reports(id, created_at, payload) VALUES (?1, ?2, ?3)",
            params![report.id, report.created_at, payload],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(connection: &Connection, entity: &str, id: &str) -> Result<(), String> {
    validate_id(id)?;
    let sql = match entity {
        "notes" => "DELETE FROM notes WHERE id = ?1",
        "goals" => "DELETE FROM goals WHERE id = ?1",
        "reports" => "DELETE FROM reports WHERE id = ?1",
        _ => return Err("记录类型不正确".into()),
    };
    let transaction = connection
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    transaction
        .execute(sql, params![id])
        .map_err(|e| e.to_string())?;
    if entity == "goals" {
        let rows: Vec<PlanWeek> = read_list(&transaction, "SELECT payload FROM plan_weeks")?;
        for mut row in rows {
            if row.goal_ids.iter().any(|g| g == id) {
                row.goal_ids.retain(|g| g != id);
                write_plan_week(&transaction, &row)?;
            }
        }
    }
    images::prune(&transaction)?;
    transaction.commit().map_err(|e| e.to_string())
}

pub fn save_settings(connection: &Connection, settings: &AiSettings) -> Result<(), String> {
    settings.report_templates.validate()?;
    let mut config = settings.clone();
    config.has_api_key = false;
    config.git.has_token = false;
    let payload = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    connection.execute("INSERT INTO settings(id, payload) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload", params![payload]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn import(connection: &mut Connection, backup: &Backup) -> Result<(), String> {
    if !matches!(backup.version, 1 | 2 | 3) {
        return Err("不支持的备份版本".into());
    }
    if backup.notes.len()
        + backup.goals.len()
        + backup.reports.len()
        + backup.plans.len()
        + backup.plan_weeks.len()
        > 100000
    {
        return Err("备份记录过多".into());
    }
    for plan in &backup.plans {
        crate::plans::validate_plan(plan)?;
    }
    for row in &backup.plan_weeks {
        crate::plans::validate_row(row)?;
    }
    for note in &backup.notes {
        validate_note(note)?;
    }
    for goal in &backup.goals {
        validate_goal(goal)?;
    }
    for report in &backup.reports {
        validate_id(&report.id)?;
        validate_date(&report.start_date)?;
        validate_date(&report.end_date)?;
        if report.start_date > report.end_date
            || report.markdown.chars().count() > 500000
            || report.title.trim().is_empty()
        {
            return Err("报告内容不正确".into());
        }
        chrono::DateTime::parse_from_rfc3339(&report.created_at)
            .map_err(|_| "报告创建时间不正确")?;
        for note in &report.sources.notes {
            validate_note(note)?;
        }
        for goal in &report.sources.goals {
            validate_goal(goal)?;
        }
        for comparison in &report.sources.plan_comparisons {
            crate::plans::validate_plan(&comparison.plan)?;
            crate::plans::validate_row(&comparison.current)?;
            if comparison.plan.id != comparison.current.plan_id {
                return Err("计划来源标识不一致".into());
            }
            if let Some(previous) = &comparison.previous {
                crate::plans::validate_row(previous)?;
                if previous.plan_id != comparison.plan.id
                    || (validate_date(&comparison.current.week_start)? - chrono::Duration::days(7))
                        .to_string()
                        != previous.week_start
                {
                    return Err("计划对照日期不正确".into());
                }
            }
            for goal in &comparison.goals {
                validate_goal(goal)?;
            }
        }
        if report.sources.commits.len() > 2000 {
            return Err("报告来源提交过多".into());
        }
    }
    if let Some(templates) = &backup.report_templates {
        templates.validate()?;
    }
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let mut ids = std::collections::HashSet::new();
    for asset in &backup.images {
        if !ids.insert(&asset.image.id) {
            return Err("备份包含重复图片".into());
        }
        images::store(&transaction, &asset.image, &asset.data)?;
    }
    for note in &backup.notes {
        images::check_references(&transaction, &note.images)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO notes(id, date, payload) VALUES (?1, ?2, ?3)",
                params![
                    note.id,
                    note.date,
                    serde_json::to_string(note).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    for goal in &backup.goals {
        transaction
            .execute(
                "INSERT OR IGNORE INTO goals(id, date, payload) VALUES (?1, ?2, ?3)",
                params![
                    goal.id,
                    goal.date,
                    serde_json::to_string(goal).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    for plan in &backup.plans {
        transaction
            .execute(
                "INSERT OR IGNORE INTO plans(id,payload) VALUES (?1,?2)",
                params![
                    plan.id,
                    serde_json::to_string(plan).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    for row in &backup.plan_weeks {
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM plan_weeks WHERE plan_id=?1 AND week_start=?2)",
                params![row.plan_id, row.week_start],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists {
            continue;
        }
        validate_links(&transaction, row)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO plan_weeks(plan_id,week_start,payload) VALUES (?1,?2,?3)",
                params![
                    row.plan_id,
                    row.week_start,
                    serde_json::to_string(row).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    for report in &backup.reports {
        for note in &report.sources.notes {
            images::check_references(&transaction, &note.images)?;
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO reports(id, created_at, payload) VALUES (?1, ?2, ?3)",
                params![
                    report.id,
                    report.created_at,
                    serde_json::to_string(report).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
    }
    if let Some(templates) = &backup.report_templates {
        let mut config = settings(&transaction)?;
        config.report_templates = templates.clone();
        save_settings(&transaction, &config)?;
    }
    images::prune(&transaction)?;
    transaction.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note() -> Note {
        Note {
            id: uuid::Uuid::new_v4().to_string(),
            date: "2026-10-09".into(),
            content: "真实工作记录".into(),
            kind: NoteKind::Note,
            tag: String::new(),
            created_at: "2026-10-09T09:00:00+08:00".into(),
            updated_at: "2026-10-09T09:00:00+08:00".into(),
            images: vec![],
        }
    }
    #[test]
    fn edit_persists_without_duplicate_and_backup_preserves_existing() {
        let mut connection = open(Path::new(":memory:")).unwrap();
        let mut original = note();
        save_note(&connection, &original).unwrap();
        original.content = "已修改".into();
        save_note(&connection, &original).unwrap();
        let mut incoming = original.clone();
        incoming.content = "旧内容".into();
        import(
            &mut connection,
            &Backup {
                plans: vec![],
                plan_weeks: vec![],
                report_templates: None,
                version: 1,
                notes: vec![incoming],
                goals: vec![],
                reports: vec![],
                images: vec![],
            },
        )
        .unwrap();
        let workspace = load(&connection, String::new()).unwrap();
        assert_eq!(workspace.notes.len(), 1);
        assert_eq!(workspace.notes[0].content, "已修改");
    }
    #[test]
    fn invalid_backup_does_not_partially_import() {
        let mut connection = open(Path::new(":memory:")).unwrap();
        let mut invalid = note();
        invalid.date = "2026-02-30".into();
        assert!(import(
            &mut connection,
            &Backup {
                plans: vec![],
                plan_weeks: vec![],
                report_templates: None,
                version: 1,
                notes: vec![note(), invalid],
                goals: vec![],
                reports: vec![],
                images: vec![],
            }
        )
        .is_err());
        assert!(load(&connection, String::new()).unwrap().notes.is_empty());
    }

    fn image() -> BackupImage {
        BackupImage {
            image: NoteImage { id: uuid::Uuid::new_v4().to_string(), name: "截图.png".into(), mime: "image/png".into(), width: 1, height: 1 },
            data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl2V5sAAAAASUVORK5CYII=".into(),
        }
    }

    #[test]
    fn image_only_note_persists_and_snapshot_retains_image_until_report_is_deleted() {
        let connection = open(Path::new(":memory:")).unwrap();
        let mut note = note();
        let asset = image();
        note.content.clear();
        note.images.push(asset.image.clone());
        save_note_with_images(
            &connection,
            &note,
            &[ImageUpload {
                id: asset.image.id.clone(),
                data: asset.data.clone(),
            }],
        )
        .unwrap();
        let workspace = load(&connection, String::new()).unwrap();
        assert_eq!(workspace.notes[0].images, note.images);
        let request = ReportRequest {
            r#type: ReportType::Daily,
            anchor_date: note.date.clone(),
            start_date: None,
            end_date: None,
            style: ReportStyle::Brief,
            mode: GenerationMode::Local,
            include_notes: true,
            include_goals: false,
            include_commits: false,
            instruction: String::new(),
        };
        let mut report = Report {
            id: uuid::Uuid::new_v4().to_string(),
            title: "旧版图片报告".into(),
            r#type: request.r#type,
            style: request.style,
            mode: request.mode,
            start_date: note.date.clone(),
            end_date: note.date.clone(),
            markdown: "附图 1 张".into(),
            statistics: crate::reports::statistics(&workspace.notes, &[]),
            created_at: note.created_at.clone(),
            model: String::new(),
            sources: Sources::default(),
        };
        report.sources.notes = workspace.notes;
        save_report(&connection, &report).unwrap();
        let before = images::read(&connection, &asset.image.id).unwrap();
        note.content = "保留文字，移除图片".into();
        note.images.clear();
        save_note(&connection, &note).unwrap();
        delete(&connection, "notes", &note.id).unwrap();
        assert_eq!(images::read(&connection, &asset.image.id).unwrap(), before);
        let backup_images =
            images::export(&connection, &load(&connection, String::new()).unwrap()).unwrap();
        assert_eq!(backup_images.len(), 1);
        delete(&connection, "reports", &report.id).unwrap();
        assert!(images::read(&connection, &asset.image.id).is_err());
    }

    #[test]
    fn image_backup_roundtrip_and_missing_reference_rollback() {
        let mut original = open(Path::new(":memory:")).unwrap();
        let asset = image();
        let mut note = note();
        note.images.push(asset.image.clone());
        let backup = Backup {
            plans: vec![],
            plan_weeks: vec![],
            report_templates: None,
            version: 2,
            notes: vec![note.clone()],
            goals: vec![],
            reports: vec![],
            images: vec![asset],
        };
        import(&mut original, &backup).unwrap();
        let workspace = load(&original, String::new()).unwrap();
        let encoded = serde_json::to_string(&Backup {
            plans: vec![],
            plan_weeks: vec![],
            report_templates: None,
            version: 2,
            images: images::export(&original, &workspace).unwrap(),
            notes: workspace.notes,
            goals: workspace.goals,
            reports: workspace.reports,
        })
        .unwrap();
        let mut restored = open(Path::new(":memory:")).unwrap();
        import(&mut restored, &serde_json::from_str(&encoded).unwrap()).unwrap();
        assert_eq!(
            images::read(&original, &note.images[0].id).unwrap(),
            images::read(&restored, &note.images[0].id).unwrap()
        );
        let mut empty = open(Path::new(":memory:")).unwrap();
        let missing = Backup {
            plans: vec![],
            plan_weeks: vec![],
            report_templates: None,
            version: 2,
            notes: vec![note],
            goals: vec![],
            reports: vec![],
            images: vec![],
        };
        assert!(import(&mut empty, &missing).is_err());
        assert!(load(&empty, String::new()).unwrap().notes.is_empty());
    }

    #[test]
    fn invalid_upload_rolls_back_and_legacy_note_still_loads() {
        let connection = open(Path::new(":memory:")).unwrap();
        let asset = image();
        let mut note = note();
        let legacy = serde_json::to_value(&note).unwrap();
        assert!(legacy.get("images").is_none());
        assert!(serde_json::from_value::<Note>(legacy)
            .unwrap()
            .images
            .is_empty());
        note.images.push(asset.image.clone());
        assert!(save_note_with_images(
            &connection,
            &note,
            &[ImageUpload {
                id: asset.image.id.clone(),
                data: "invalid".into()
            }]
        )
        .is_err());
        assert!(load(&connection, String::new()).unwrap().notes.is_empty());
        assert!(images::read(&connection, &asset.image.id).is_err());
        assert!(save_note(&connection, &note).is_err());
        note.images = vec![asset.image; 10];
        assert!(validate_note(&note).is_err());
    }
    fn goal() -> Goal {
        Goal {
            id: uuid::Uuid::new_v4().to_string(),
            date: "2026-10-09".into(),
            title: "整理接口文档".into(),
            created_at: "2026-10-09T09:00:00+08:00".into(),
            updated_at: "2026-10-09T09:00:00+08:00".into(),
            ..Goal::default()
        }
    }

    #[test]
    fn legacy_goal_payload_loads_and_new_fields_round_trip() {
        let connection = open(Path::new(":memory:")).unwrap();
        let legacy = r#"{"id":"11111111-1111-4111-8111-111111111111","date":"2026-10-09","title":"旧待办","completed":true,"createdAt":"2026-10-09T09:00:00+08:00","updatedAt":"2026-10-09T09:00:00+08:00"}"#;
        connection
            .execute(
                "INSERT INTO goals(id, date, payload) VALUES (?1, ?2, ?3)",
                params!["11111111-1111-4111-8111-111111111111", "2026-10-09", legacy],
            )
            .unwrap();
        let loaded = load(&connection, String::new()).unwrap().goals;
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].project.is_empty() && loaded[0].actual_min.is_none());
        assert!(loaded[0].completed_at.is_none());
        assert!(loaded[0].risk.is_empty() && !loaded[0].ad_hoc);
        let mut rich = goal();
        rich.project = "官网改版".into();
        rich.priority = Some(Priority::High);
        rich.estimate_min = Some(90);
        rich.actual_min = Some(120);
        rich.completed = true;
        rich.completed_at = Some("2026-10-09T11:00:00+08:00".into());
        rich.origin_date = Some("2026-10-07".into());
        rich.carry_count = 2;
        rich.risk = "依赖外部评审，本日无法完成".into();
        rich.ad_hoc = true;
        save_goal(&connection, &rich).unwrap();
        let stored = load(&connection, String::new())
            .unwrap()
            .goals
            .into_iter()
            .find(|item| item.id == rich.id)
            .unwrap();
        assert_eq!(stored.project, "官网改版");
        assert_eq!(stored.priority, Some(Priority::High));
        assert_eq!(
            (stored.estimate_min, stored.actual_min),
            (Some(90), Some(120))
        );
        assert_eq!(stored.origin_date.as_deref(), Some("2026-10-07"));
        assert_eq!(stored.carry_count, 2);
        assert_eq!(stored.risk, "依赖外部评审，本日无法完成");
        assert!(stored.ad_hoc);
    }

    #[test]
    fn goal_validation_rejects_bad_new_fields() {
        let mut item = goal();
        item.project = "项".repeat(41);
        assert!(validate_goal(&item).is_err());
        item.project = "两\n行".into();
        assert!(validate_goal(&item).is_err());
        item = goal();
        item.actual_min = Some(100_001);
        assert!(validate_goal(&item).is_err());
        item = goal();
        item.completed_at = Some("2026-10-09T11:00:00+08:00".into());
        assert!(validate_goal(&item).is_err(), "未完成不能有完成时间");
        item.completed = true;
        item.completed_at = Some("昨天".into());
        assert!(validate_goal(&item).is_err());
        item = goal();
        item.origin_date = Some("2026-02-30".into());
        assert!(validate_goal(&item).is_err());
        item = goal();
        item.risk = "风".repeat(301);
        assert!(validate_goal(&item).is_err());
        item.risk = "两\n行".into();
        assert!(validate_goal(&item).is_err());
    }

    #[test]
    fn batch_save_is_atomic() {
        let connection = open(Path::new(":memory:")).unwrap();
        let good = goal();
        let mut bad = goal();
        bad.title.clear();
        assert!(save_goals(&connection, &[good.clone(), bad]).is_err());
        assert!(load(&connection, String::new()).unwrap().goals.is_empty());
        save_goals(&connection, &[good]).unwrap();
        assert_eq!(load(&connection, String::new()).unwrap().goals.len(), 1);
    }

    #[test]
    fn old_default_templates_upgrade_without_changing_custom_templates() {
        let connection = open(Path::new(":memory:")).unwrap();
        let mut config = AiSettings::default();
        config.report_templates.daily = "# {{标题}}\n\n{{范围}}\n\n## 今日概况\n{{概况}}\n\n## 今日完成\n{{已完成待办}}\n\n## 待继续推进\n{{未完成待办}}".into();
        config.report_templates.weekly = "自定义周报\n{{已完成待办}}".into();
        config.report_templates.monthly = "月报{{日期范围}}完成任务进度和计划\n\n【完成情况】：\n{{项目完成情况}}\n\n【遇到问题】\n无\n\n【下月计划】\n{{项目计划}}".into();
        save_settings(&connection, &config).unwrap();
        let loaded = settings(&connection).unwrap();
        assert_eq!(
            loaded.report_templates.daily,
            ReportTemplates::default().daily
        );
        assert_eq!(
            loaded.report_templates.weekly,
            config.report_templates.weekly
        );
        assert_eq!(loaded.endpoint, config.endpoint);
        assert_eq!(
            loaded.report_templates.monthly,
            ReportTemplates::default().monthly
        );
    }

    #[test]
    fn templates_persist_and_backup_restores_them_without_replacing_ai_config() {
        let mut connection = open(Path::new(":memory:")).unwrap();
        let mut config = AiSettings::default();
        config.model = "existing-model".into();
        config.report_templates.daily = "自定义日模板\n{{已完成待办}}".into();
        save_settings(&connection, &config).unwrap();
        let loaded = settings(&connection).unwrap();
        assert_eq!(loaded.report_templates.daily, config.report_templates.daily);
        let backup = Backup {
            plans: vec![],
            plan_weeks: vec![],
            version: 2,
            notes: vec![],
            goals: vec![],
            reports: vec![],
            images: vec![],
            report_templates: Some(loaded.report_templates),
        };
        let encoded = serde_json::to_string(&backup).unwrap();
        let mut incoming: Backup = serde_json::from_str(&encoded).unwrap();
        incoming.report_templates.as_mut().unwrap().weekly = "备份周模板\n{{概况}}".into();
        import(&mut connection, &incoming).unwrap();
        let loaded = settings(&connection).unwrap();
        assert_eq!(loaded.report_templates.weekly, "备份周模板\n{{概况}}");
        assert_eq!(loaded.model, "existing-model");
        incoming.report_templates.as_mut().unwrap().monthly.clear();
        assert!(import(&mut connection, &incoming).is_err());
        assert_eq!(
            settings(&connection).unwrap().report_templates.monthly,
            config.report_templates.monthly
        );
        let legacy: Backup =
            serde_json::from_str(r#"{"version":1,"notes":[],"goals":[],"reports":[]}"#).unwrap();
        import(&mut connection, &legacy).unwrap();
        assert_eq!(
            settings(&connection).unwrap().report_templates.weekly,
            "备份周模板\n{{概况}}"
        );
    }
}
