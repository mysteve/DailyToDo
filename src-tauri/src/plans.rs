use crate::{models::*, reports::validate_date};
use chrono::{Datelike, Duration, Utc};

pub fn week(date: &str) -> Result<String, String> {
    let date = validate_date(date)?;
    Ok((date - Duration::days(date.weekday().num_days_from_monday() as i64)).to_string())
}
fn shift(date: &str, days: i64) -> Result<String, String> {
    Ok((validate_date(date)? + Duration::days(days)).to_string())
}
pub fn validate_plan(plan: &Plan) -> Result<(), String> {
    crate::db::validate_id(&plan.id)?;
    chrono::DateTime::parse_from_rfc3339(&plan.created_at).map_err(|_| "计划创建时间不正确")?;
    Ok(())
}
pub fn validate_row(row: &PlanWeek) -> Result<(), String> {
    if let Some(percent) = row.completion_percent {
        if percent > 100 {
            return Err("完成度需为 0 至 100 的整数".into());
        }
        if (percent == 100) != (row.status == PlanStatus::Completed)
            || (row.status == PlanStatus::Pending && percent != 0)
        {
            return Err("完成度与计划状态不一致".into());
        }
    }
    crate::db::validate_id(&row.plan_id)?;
    if week(&row.week_start)? != row.week_start {
        return Err("计划日期需为有效的周一".into());
    }
    if row.title.trim().is_empty()
        || row.title.chars().count() > 300
        || row.project.chars().count() > 40
    {
        return Err("计划名称需为 1 至 300 字，项目最多 40 字".into());
    }
    if row.progress.chars().count() > 2000 || row.blocker.chars().count() > 300 {
        return Err("进展最多 2000 字，阻塞原因最多 300 字".into());
    }
    if row.status == PlanStatus::Blocked && row.blocker.trim().is_empty() {
        return Err("请填写阻塞原因".into());
    }
    if row.status != PlanStatus::Blocked && !row.blocker.trim().is_empty() {
        return Err("解除阻塞后请清空阻塞原因".into());
    }
    if row.goal_ids.len() > 500
        || row
            .goal_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != row.goal_ids.len()
    {
        return Err("关联待办不正确".into());
    }
    for id in &row.goal_ids {
        crate::db::validate_id(id)?;
    }
    chrono::DateTime::parse_from_rfc3339(&row.updated_at).map_err(|_| "计划更新时间不正确")?;
    Ok(())
}
pub fn carry(plans: &[Plan], rows: &[PlanWeek], through: &str) -> Result<Vec<PlanWeek>, String> {
    let target = week(through)?;
    let mut result = rows.to_vec();
    for plan in plans.iter().filter(|p| !p.archived) {
        let mut saved: Vec<_> = rows
            .iter()
            .filter(|r| r.plan_id == plan.id && r.week_start <= target)
            .collect();
        saved.sort_by(|a, b| a.week_start.cmp(&b.week_start));
        let Some(first) = saved.first() else {
            continue;
        };
        let mut previous = (*first).clone();
        let mut date = shift(&previous.week_start, 7)?;
        while date <= target {
            if let Some(row) = saved.iter().find(|r| r.week_start == date) {
                previous = (*row).clone();
            } else if previous.status != PlanStatus::Completed {
                let mut row = previous.clone();
                row.week_start = date.clone();
                row.progress.clear();
                row.carried = true;
                row.updated_at = Utc::now().to_rfc3339();
                result.push(row.clone());
                previous = row;
            }
            date = shift(&date, 7)?;
        }
    }
    result.sort_by(|a, b| (&a.plan_id, &a.week_start).cmp(&(&b.plan_id, &b.week_start)));
    Ok(result)
}
pub fn comparisons(
    workspace: &Workspace,
    start: &str,
    end: &str,
) -> Result<Vec<PlanComparison>, String> {
    let mut result = vec![];
    for row in &workspace.plan_weeks {
        if row.week_start.as_str() < start || shift(&row.week_start, 6)?.as_str() > end {
            continue;
        }
        let Some(plan) = workspace.plans.iter().find(|p| p.id == row.plan_id) else {
            continue;
        };
        let prev = shift(&row.week_start, -7)?;
        let previous = workspace
            .plan_weeks
            .iter()
            .find(|p| p.plan_id == row.plan_id && p.week_start == prev)
            .cloned();
        let goals = workspace
            .goals
            .iter()
            .filter(|g| {
                row.goal_ids.contains(&g.id)
                    || previous
                        .as_ref()
                        .is_some_and(|p| p.goal_ids.contains(&g.id))
            })
            .cloned()
            .collect();
        result.push(PlanComparison {
            plan: plan.clone(),
            current: row.clone(),
            previous,
            goals,
        });
    }
    result.sort_by(|a, b| {
        (&a.current.week_start, &a.current.plan_id)
            .cmp(&(&b.current.week_start, &b.current.plan_id))
    });
    Ok(result)
}
fn cell(s: &str) -> String {
    s.replace('|', "｜")
        .replace(['\r', '\n'], "；")
        .replace('<', "＜")
        .replace('>', "＞")
}
fn completion_comparison(current: &PlanWeek, previous: Option<&PlanWeek>) -> String {
    let known = |row: &PlanWeek| {
        row.completion_percent
            .or_else(|| (row.status == PlanStatus::Completed).then_some(100))
    };
    let before = previous.and_then(known);
    let after = known(current);
    let label = |value: Option<u8>| {
        value
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "未记录".into())
    };
    let mut result = format!(
        "{} → {}",
        if previous.is_some() {
            label(before)
        } else {
            "本周新增".into()
        },
        label(after)
    );
    let mut details = vec![];
    if let (Some(before), Some(after)) = (before, after) {
        let change = i16::from(after) - i16::from(before);
        details.push(if change == 0 {
            "持平".into()
        } else {
            format!("{change:+} 个百分点")
        });
    }
    if current.carried {
        details.push("自动延续".into());
    }
    if !details.is_empty() {
        result.push_str(&format!("（{}）", details.join("；")));
    }
    result
}

pub fn table(rows: &[PlanComparison]) -> String {
    let weeks: std::collections::BTreeSet<_> = rows.iter().map(|r| &r.current.week_start).collect();
    weeks.into_iter().map(|week| {
        let items: Vec<_> = rows.iter().filter(|r| &r.current.week_start == week).collect();
        let lines = items.iter().map(|r| {
            let c = &r.current;
            let previous = r.previous.as_ref().map(|p| format!("{}；{}", p.status.label(), if p.progress.is_empty() { if p.carried { "尚未更新" } else { "未记录" } } else { &p.progress })).unwrap_or_else(|| "本周新增".into());
            let title = if c.project.is_empty() { c.title.clone() } else { format!("{}：{}",c.project,c.title) };
            let progress = if c.progress.is_empty() { if c.carried { "尚未更新（自动延续）" } else { "未记录" } } else { &c.progress };
            let completion = completion_comparison(c, r.previous.as_ref());
            format!("| {} |", [title.as_str(), &completion, &previous, progress, c.status.label(), if c.status == PlanStatus::Blocked { &c.blocker } else { "—" }].map(cell).join(" | "))
        }).collect::<Vec<_>>().join("\n");
        format!("### {week} — {}\n\n计划 {} 项 · 已完成 {} 项 · 阻塞 {} 项\n\n| 计划 | 完成度（上周 → 本周） | 上周状态与进展 | 本周推进 | 当前状态 | 阻塞原因 |\n| --- | --- | --- | --- | --- | --- |\n{lines}", shift(week,6).unwrap_or_default(),items.len(),items.iter().filter(|r|r.current.status == PlanStatus::Completed).count(),items.iter().filter(|r|r.current.status == PlanStatus::Blocked).count())
    }).collect::<Vec<_>>().join("\n\n")
}

/// A reserved slot is replaced after model output; the model never authors the comparison table.
pub const SLOT: &str = "RIJI_WEEKLY_PLAN_COMPARISON_SLOT";
pub fn finish(markdown: &str, rows: &[PlanComparison]) -> String {
    let content = table(rows);
    if markdown.contains(SLOT) {
        return markdown.replace(SLOT, &content);
    }
    if content.is_empty() {
        markdown.to_owned()
    } else {
        format!("{markdown}\n\n## 周计划进展对照\n\n{content}")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db;
    use std::path::Path;
    pub fn fixture() -> (Plan, PlanWeek) {
        let plan = Plan {
            id: "00000000-0000-4000-8000-000000000001".into(),
            archived: false,
            created_at: "2026-10-05T09:00:00+08:00".into(),
        };
        let row = PlanWeek {
            plan_id: plan.id.clone(),
            week_start: "2026-10-05".into(),
            title: "接口联调".into(),
            project: "平台".into(),
            status: PlanStatus::Blocked,
            completion_percent: None,
            progress: "完成字段映射".into(),
            blocker: "等待测试环境".into(),
            goal_ids: vec![],
            carried: false,
            updated_at: plan.created_at.clone(),
        };
        (plan, row)
    }
    pub fn request() -> ReportRequest {
        ReportRequest {
            r#type: ReportType::Weekly,
            anchor_date: "2026-10-12".into(),
            start_date: None,
            end_date: None,
            style: ReportStyle::Brief,
            mode: GenerationMode::Local,
            include_notes: false,
            include_goals: true,
            include_commits: false,
            instruction: String::new(),
        }
    }
    #[test]
    fn missed_weeks_are_idempotent_and_never_copy_progress() {
        let (plan, row) = fixture();
        let rows = carry(&[plan.clone()], &[row.clone()], "2026-10-26").unwrap();
        assert_eq!(rows.len(), 4);
        assert!(rows[1..]
            .iter()
            .all(|r| r.carried && r.progress.is_empty() && r.blocker == row.blocker));
        let again = carry(&[plan], &rows, "2026-10-26").unwrap();
        assert_eq!(
            serde_json::to_value(rows).unwrap(),
            serde_json::to_value(again).unwrap()
        );
    }
    #[test]
    fn completed_and_archived_stop_carry_and_saved_future_is_preserved() {
        let (mut plan, mut row) = fixture();
        plan.archived = true;
        assert_eq!(
            carry(&[plan.clone()], &[row.clone()], "2026-10-26")
                .unwrap()
                .len(),
            1
        );
        plan.archived = false;
        row.status = PlanStatus::Completed;
        row.blocker.clear();
        let mut future = row.clone();
        future.week_start = "2026-10-19".into();
        future.status = PlanStatus::Active;
        future.progress = "重新开始".into();
        let rows = carry(&[plan], &[row, future], "2026-10-26").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].progress, "重新开始");
        assert_eq!(rows[2].status, PlanStatus::Active);
    }
    #[test]
    fn crosses_years_and_validates_blockers() {
        let (plan, mut row) = fixture();
        row.week_start = "2025-12-29".into();
        let rows = carry(&[plan], &[row.clone()], "2026-01-12").unwrap();
        assert_eq!(rows[2].week_start, "2026-01-12");
        row.blocker.clear();
        assert!(validate_row(&row).is_err());
        row.status = PlanStatus::Active;
        assert!(validate_row(&row).is_ok());
        row.week_start = "2026-10-06".into();
        assert!(validate_row(&row).is_err());
    }
    #[test]
    fn storage_migration_backup_and_invalid_import_are_atomic() {
        let mut connection = db::open(Path::new(":memory:")).unwrap();
        let (plan, row) = fixture();
        db::save_plan(&connection, &plan, &row).unwrap();
        db::carry_plans(&connection, "2026-10-26").unwrap();
        let workspace = db::load(&connection, String::new()).unwrap();
        assert_eq!(workspace.plan_weeks.len(), 4);
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 3);
        let backup = Backup {
            version: 3,
            plans: workspace.plans,
            plan_weeks: workspace.plan_weeks,
            notes: vec![],
            goals: vec![],
            reports: vec![],
            images: vec![],
            report_templates: None,
        };
        let encoded = serde_json::to_string(&backup).unwrap();
        let mut restored = db::open(Path::new(":memory:")).unwrap();
        db::import(&mut restored, &serde_json::from_str(&encoded).unwrap()).unwrap();
        db::import(&mut restored, &backup).unwrap();
        assert_eq!(
            db::load(&restored, String::new()).unwrap().plan_weeks.len(),
            4
        );
        db::archive_plan(&connection, &plan.id, true).unwrap();
        db::carry_plans(&connection, "2026-11-02").unwrap();
        assert_eq!(
            db::load(&connection, String::new())
                .unwrap()
                .plan_weeks
                .len(),
            4
        );
        db::archive_plan(&connection, &plan.id, false).unwrap();
        db::carry_plans(&connection, "2026-11-02").unwrap();
        assert_eq!(
            db::load(&connection, String::new())
                .unwrap()
                .plan_weeks
                .len(),
            5
        );
        let mut invalid: Backup = serde_json::from_str(&encoded).unwrap();
        let mut extra = plan.clone();
        extra.id = uuid::Uuid::new_v4().to_string();
        invalid.plans.push(extra);
        invalid.plan_weeks[0].week_start = "2026-11-09".into();
        invalid.plan_weeks[0]
            .goal_ids
            .push(uuid::Uuid::new_v4().to_string());
        assert!(db::import(&mut connection, &invalid).is_err());
        assert_eq!(db::load(&connection, String::new()).unwrap().plans.len(), 1);
    }
    #[test]
    fn plan_only_reports_snapshot_and_exclude_partial_weeks() {
        let connection = db::open(Path::new(":memory:")).unwrap();
        let (plan, row) = fixture();
        db::save_plan(&connection, &plan, &row).unwrap();
        db::carry_plans(&connection, "2026-10-12").unwrap();
        let workspace = db::load(&connection, String::new()).unwrap();
        assert!(comparisons(&workspace, "2026-10-13", "2026-10-18")
            .unwrap()
            .is_empty());
        let compared = comparisons(&workspace, "2026-10-12", "2026-10-18").unwrap();
        let report = crate::reports::make_report_with_plans(&request(), vec![], compared).unwrap();
        assert_eq!(report.statistics.goals, 0);
        assert!(report.markdown.contains("尚未更新（自动延续）"));
        assert!(report.markdown.contains("完成字段映射"));
        db::save_report(&connection, &report).unwrap();
        let mut changed = row;
        changed.progress = "改写".into();
        db::save_plan(&connection, &plan, &changed).unwrap();
        let saved = db::load(&connection, String::new()).unwrap();
        assert_eq!(
            saved.reports[0].sources.plan_comparisons[0]
                .previous
                .as_ref()
                .unwrap()
                .progress,
            "完成字段映射"
        );
        assert!(finish("# AI 汇报", &report.sources.plan_comparisons).contains("等待测试环境"));
        assert!(!finish(SLOT, &report.sources.plan_comparisons).contains(SLOT));
    }
    #[test]
    fn deleted_goal_cleans_links_but_keeps_report_snapshot() {
        let connection = db::open(Path::new(":memory:")).unwrap();
        let (plan, mut row) = fixture();
        let goal = Goal {
            id: uuid::Uuid::new_v4().to_string(),
            title: "字段核对".into(),
            date: "2026-10-05".into(),
            created_at: plan.created_at.clone(),
            updated_at: plan.created_at.clone(),
            ..Goal::default()
        };
        db::save_goal(&connection, &goal).unwrap();
        row.goal_ids.push(goal.id.clone());
        db::save_plan(&connection, &plan, &row).unwrap();
        db::carry_plans(&connection, "2026-10-12").unwrap();
        let w = db::load(&connection, String::new()).unwrap();
        let report = crate::reports::make_report_with_plans(
            &request(),
            vec![],
            comparisons(&w, "2026-10-12", "2026-10-18").unwrap(),
        )
        .unwrap();
        db::save_report(&connection, &report).unwrap();
        db::delete(&connection, "goals", &goal.id).unwrap();
        let saved = db::load(&connection, String::new()).unwrap();
        assert!(saved.plan_weeks.iter().all(|r| r.goal_ids.is_empty()));
        assert_eq!(
            saved.reports[0].sources.plan_comparisons[0].goals[0].title,
            "字段核对"
        );
    }

    #[test]
    fn version_two_database_upgrades_in_place_and_reopens() {
        let path = std::env::temp_dir().join(format!("riji-weekly-{}.db", uuid::Uuid::new_v4()));
        let (plan, row) = fixture();
        {
            let old = rusqlite::Connection::open(&path).unwrap();
            old.execute_batch("CREATE TABLE goals(id TEXT PRIMARY KEY,date TEXT NOT NULL,payload TEXT NOT NULL); PRAGMA user_version=2;").unwrap();
            let goal = Goal {
                id: uuid::Uuid::new_v4().to_string(),
                title: "迁移前待办".into(),
                date: row.week_start.clone(),
                created_at: plan.created_at.clone(),
                updated_at: plan.created_at.clone(),
                ..Goal::default()
            };
            old.execute(
                "INSERT INTO goals VALUES (?1,?2,?3)",
                rusqlite::params![goal.id, goal.date, serde_json::to_string(&goal).unwrap()],
            )
            .unwrap();
        }
        {
            let connection = db::open(&path).unwrap();
            assert_eq!(
                db::load(&connection, String::new()).unwrap().goals[0].title,
                "迁移前待办"
            );
            db::save_plan(&connection, &plan, &row).unwrap();
        }
        {
            let connection = db::open(&path).unwrap();
            let workspace = db::load(&connection, String::new()).unwrap();
            assert_eq!(workspace.goals.len(), 1);
            assert_eq!(workspace.plan_weeks[0].blocker, "等待测试环境");
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(version, 3);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn completion_validates_legacy_and_invalid_payloads() {
        let (_, mut row) = fixture();
        let legacy = serde_json::to_value(&row).unwrap();
        assert!(legacy.get("completionPercent").is_none());
        let old: PlanWeek = serde_json::from_value(legacy.clone()).unwrap();
        assert!(old.completion_percent.is_none());
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(256),
        ] {
            let mut value = legacy.clone();
            value["completionPercent"] = invalid;
            assert!(serde_json::from_value::<PlanWeek>(value).is_err());
        }
        row.completion_percent = Some(101);
        assert!(validate_row(&row).is_err());
        row.completion_percent = Some(100);
        assert!(validate_row(&row).is_err());
        row.status = PlanStatus::Completed;
        row.blocker.clear();
        validate_row(&row).unwrap();
        row.completion_percent = Some(65);
        assert!(validate_row(&row).is_err());
        row.status = PlanStatus::Active;
        validate_row(&row).unwrap();
        row.status = PlanStatus::Pending;
        assert!(validate_row(&row).is_err());
    }

    #[test]
    fn completion_survives_reopen_carry_backup_and_snapshot() {
        let path =
            std::env::temp_dir().join(format!("riji-completion-{}.db", uuid::Uuid::new_v4()));
        let (plan, mut row) = fixture();
        row.completion_percent = Some(65);
        {
            let connection = db::open(&path).unwrap();
            db::save_plan(&connection, &plan, &row).unwrap();
            db::carry_plans(&connection, "2026-10-12").unwrap();
        }
        {
            let connection = db::open(&path).unwrap();
            let w = db::load(&connection, String::new()).unwrap();
            assert!(w
                .plan_weeks
                .iter()
                .all(|r| r.completion_percent == Some(65)));
            let compared = comparisons(&w, "2026-10-12", "2026-10-18").unwrap();
            let report =
                crate::reports::make_report_with_plans(&request(), vec![], compared).unwrap();
            db::save_report(&connection, &report).unwrap();
            let mut completed = w.plan_weeks[1].clone();
            completed.completion_percent = Some(100);
            completed.status = PlanStatus::Completed;
            completed.blocker.clear();
            db::save_plan(&connection, &plan, &completed).unwrap();
            db::carry_plans(&connection, "2026-10-26").unwrap();
            let w = db::load(&connection, String::new()).unwrap();
            assert_eq!(w.plan_weeks.len(), 2);
            assert_eq!(w.plan_weeks[0].completion_percent, Some(65));
            assert_eq!(
                w.reports[0].sources.plan_comparisons[0]
                    .current
                    .completion_percent,
                Some(65)
            );
            let backup = Backup {
                version: 3,
                plans: w.plans,
                plan_weeks: w.plan_weeks,
                notes: vec![],
                goals: vec![],
                reports: w.reports,
                images: vec![],
                report_templates: None,
            };
            let encoded = serde_json::to_string(&backup).unwrap();
            let mut restored = db::open(Path::new(":memory:")).unwrap();
            db::import(&mut restored, &serde_json::from_str(&encoded).unwrap()).unwrap();
            let w = db::load(&restored, String::new()).unwrap();
            assert_eq!(w.plan_weeks[1].completion_percent, Some(100));
            assert_eq!(
                w.reports[0].sources.plan_comparisons[0]
                    .current
                    .completion_percent,
                Some(65)
            );
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn report_completion_matches_shared_browser_cases() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/plan-completion-comparison.json"
        ))
        .unwrap();
        for sample in cases.as_array().unwrap() {
            let (plan, row) = fixture();
            let make_row = |patch: &serde_json::Value| -> PlanWeek {
                let mut value = serde_json::to_value(&row).unwrap();
                value
                    .as_object_mut()
                    .unwrap()
                    .extend(patch.as_object().unwrap().clone());
                serde_json::from_value(value).unwrap()
            };
            let current = make_row(&sample["current"]);
            let previous = if sample["previous"].is_null() {
                None
            } else {
                Some(make_row(&sample["previous"]))
            };
            let expected = sample["expected"].as_str().unwrap();
            assert_eq!(
                completion_comparison(&current, previous.as_ref()),
                expected,
                "{}",
                sample["name"]
            );
            let markdown = table(&[PlanComparison {
                plan,
                current,
                previous,
                goals: vec![],
            }]);
            assert!(markdown.contains("| 完成度（上周 → 本周） |"));
            assert!(markdown.contains(&format!("| {expected} |")));
        }
    }
}
