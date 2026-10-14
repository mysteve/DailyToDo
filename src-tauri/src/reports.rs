use crate::models::*;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::collections::HashSet;

pub fn validate_date(value: &str) -> Result<NaiveDate, String> {
    let date =
        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "日期格式不正确".to_string())?;
    if date.format("%Y-%m-%d").to_string() != value || !(1900..=9999).contains(&date.year()) {
        return Err("日期必须在 1900 至 9999 年之间".into());
    }
    Ok(date)
}

pub fn period(request: &ReportRequest) -> Result<(String, String), String> {
    let anchor = validate_date(&request.anchor_date)?;
    if request.r#type != ReportType::Daily {
        match (&request.start_date, &request.end_date) {
            (Some(start), Some(end)) => {
                validate_date(start)?;
                validate_date(end)?;
                if start > end {
                    return Err("结束日期不能早于开始日期".into());
                }
                return Ok((start.clone(), end.clone()));
            }
            (None, None) => {}
            _ => return Err("请选择开始日期和结束日期".into()),
        }
    }
    let (start, end) = match request.r#type {
        ReportType::Daily => (anchor, anchor),
        ReportType::Weekly => {
            let start = anchor - Duration::days(anchor.weekday().num_days_from_monday() as i64);
            (start, start + Duration::days(6))
        }
        ReportType::Monthly => {
            let start = anchor.with_day(1).ok_or("日期不正确")?;
            let next = if anchor.month() == 12 {
                NaiveDate::from_ymd_opt(anchor.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(anchor.year(), anchor.month() + 1, 1)
            }
            .ok_or("日期超出支持范围")?;
            (start, next - Duration::days(1))
        }
    };
    let range = (start.to_string(), end.to_string());
    validate_date(&range.0)?;
    validate_date(&range.1)?;
    Ok(range)
}

pub fn statistics(notes: &[Note], goals: &[Goal]) -> Statistics {
    let completed = goals.iter().filter(|goal| goal.completed).count();
    let days: HashSet<&str> = notes
        .iter()
        .map(|note| note.date.as_str())
        .chain(goals.iter().map(|goal| goal.date.as_str()))
        .collect();
    Statistics {
        days: days.len(),
        notes: notes.len(),
        goals: goals.len(),
        completed,
        blockers: notes
            .iter()
            .filter(|note| note.kind == NoteKind::Blocker)
            .count(),
        completion_rate: if goals.is_empty() {
            0
        } else {
            ((completed as f64 / goals.len() as f64) * 100.0).round() as usize
        },
        estimate_minutes: goals
            .iter()
            .map(|goal| goal.estimate_min.unwrap_or(0) as u64)
            .sum(),
        actual_minutes: goals
            .iter()
            .map(|goal| goal.actual_min.unwrap_or(0) as u64)
            .sum(),
    }
}

pub fn format_minutes(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} 分钟"),
        (h, 0) => format!("{h} 小时"),
        (h, m) => format!("{h} 小时 {m} 分钟"),
    }
}

/// 已记录的风险或未完成原因，按填写顺序编号；没有填写时返回 None。
fn recorded_reasons(goals: &[&Goal]) -> Option<String> {
    let lines: Vec<String> = goals
        .iter()
        .filter(|goal| !goal.risk.trim().is_empty())
        .enumerate()
        .map(|(index, goal)| {
            format!(
                "{}{}：{}",
                item_number(index),
                goal.title.trim(),
                goal.risk.trim()
            )
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// 已记录耗时的概述；没有任何耗时记录时返回 None，避免写出 0 分钟。
fn time_summary(goals: &[Goal]) -> Option<String> {
    let recorded = goals
        .iter()
        .filter(|goal| goal.actual_min.is_some())
        .count();
    if recorded == 0 {
        return None;
    }
    let actual: u64 = goals
        .iter()
        .map(|goal| goal.actual_min.unwrap_or(0) as u64)
        .sum();
    Some(format!(
        "{recorded} 项待办记录了实际耗时，合计 {}。",
        format_minutes(actual)
    ))
}

pub fn make_report(
    request: &ReportRequest,
    _notes: Vec<Note>,
    goals: Vec<Goal>,
) -> Result<Report, String> {
    make_report_with_plans(request, goals, vec![])
}

pub fn make_report_with_plans(
    request: &ReportRequest,
    goals: Vec<Goal>,
    comparisons: Vec<PlanComparison>,
) -> Result<Report, String> {
    if request.instruction.chars().count() > 2000 {
        return Err("补充要求不能超过 2000 字".into());
    }
    let (start_date, end_date) = period(request)?;
    let goals: Vec<_> = goals
        .into_iter()
        .filter(|goal| {
            goal.date <= end_date
                && (goal.date >= start_date
                    || request.r#type == ReportType::Daily && !goal.completed)
        })
        .collect();
    if goals.is_empty() && comparisons.is_empty() {
        return Err("所选范围内没有可用于生成报告的待办事项".into());
    }
    let title = if start_date == end_date {
        format!("{} 工作{}", start_date, request.r#type.label())
    } else {
        format!(
            "{} — {} 工作{}",
            start_date,
            end_date,
            request.r#type.label()
        )
    };
    let stats = statistics(&[], &goals);
    let mut report = Report {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        r#type: request.r#type.clone(),
        style: request.style.clone(),
        mode: request.mode.clone(),
        start_date,
        end_date,
        markdown: String::new(),
        statistics: stats,
        created_at: Utc::now().to_rfc3339(),
        model: String::new(),
        sources: Sources {
            plan_comparisons: comparisons,
            notes: vec![],
            goals,
            commits: vec![],
        },
    };
    report.markdown = render_template(&report, &ReportTemplates::default())?;
    Ok(report)
}

fn item_number(index: usize) -> String {
    if index < 20 {
        char::from_u32(0x2460 + index as u32).unwrap().to_string()
    } else {
        format!("{}、", index + 1)
    }
}

fn project_items(goals: Vec<&Goal>, detail: Option<(&str, &str)>) -> String {
    project_items_layout(goals, detail, false)
}

fn project_items_layout(
    goals: Vec<&Goal>,
    detail: Option<(&str, &str)>,
    single_line: bool,
) -> String {
    let mut projects: Vec<(&str, Vec<String>)> = Vec::new();
    for goal in goals {
        let title = goal.title.trim();
        let spent = goal
            .actual_min
            .map(|minutes| format!("（实际耗时 {}）", format_minutes(minutes as u64)))
            .unwrap_or_default();
        if !goal.project.trim().is_empty() {
            let project = goal.project.trim();
            let content = format!("{title}{spent}");
            if let Some((_, items)) = projects.iter_mut().find(|(name, _)| *name == project) {
                items.push(content);
            } else {
                projects.push((project, vec![content]));
            }
            continue;
        }
        let labelled = [('【', '】'), ('[', ']')]
            .iter()
            .find_map(|(open, close)| title.strip_prefix(*open)?.split_once(*close));
        let grouped = labelled
            .or_else(|| title.split_once(['：', ':']))
            .map(|(project, content)| (project.trim(), content.trim()))
            .filter(|(project, content)| {
                !project.is_empty()
                    && !content.is_empty()
                    && !project.contains('\n')
                    && !content.contains('\n')
            });
        let (project, content) = grouped.unwrap_or(("待办事项", title));
        let content = format!("{content}{spent}");
        if let Some((_, items)) = projects.iter_mut().find(|(name, _)| *name == project) {
            items.push(content);
        } else {
            projects.push((project, vec![content]));
        }
    }
    if projects.is_empty() {
        return "暂无".into();
    }
    projects
        .iter()
        .enumerate()
        .map(|(index, (project, items))| {
            if single_line {
                return format!(
                    "{project}\n{}",
                    items
                        .iter()
                        .enumerate()
                        .map(|(i, content)| format!("{}、{content}", i + 1))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            let mut lines = items
                .iter()
                .enumerate()
                .map(|(item, content)| {
                    let label = detail
                        .map(|(label, _)| format!("{label}："))
                        .unwrap_or_default();
                    format!("{}{label}{content}", item_number(item))
                })
                .collect::<Vec<_>>();
            if let Some((_, confirmation)) = detail {
                lines.push(format!(
                    "{}{confirmation}：待确认",
                    item_number(items.len())
                ));
            }
            format!("{}、{project}：\n{}", index + 1, lines.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub const DAILY_RISK_SLOT: &str = "RIJI_DAILY_RISK_SLOT";
pub const DAILY_PLAN_SLOT: &str = "RIJI_DAILY_PLAN_SLOT";

pub fn daily_risks(report: &Report) -> String {
    recorded_reasons(&report.sources.goals.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| "无".into())
}

pub fn daily_plan(report: &Report) -> String {
    let lines = report
        .sources
        .goals
        .iter()
        .filter(|g| !g.completed)
        .enumerate()
        .map(|(i, g)| format!("{}、{}", i + 1, g.title))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "暂无".into()
    } else {
        lines.join("\n")
    }
}

pub fn daily_report_template(template: &str) -> String {
    let has_plan = template.split("{{").skip(1).any(|part| {
        part.split_once("}}").is_some_and(|(key, _)| {
            matches!(
                key.trim(),
                "项目计划" | "下阶段重点任务与交付" | "未完成待办"
            )
        })
    });
    if has_plan {
        template.into()
    } else {
        format!(
            "{}\n\n## 【明日重点计划】\n{{{{项目计划}}}}",
            template.trim_end()
        )
    }
}

pub fn daily_ai_template(template: &str) -> String {
    let template = daily_report_template(template);
    // Parse template tokens so whitespace accepted by local reports also works for AI.
    let mut output = String::new();
    let mut rest = template.as_str();
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let token = &rest[start + 2..];
        let Some((key, tail)) = token.split_once("}}") else {
            output.push_str(&rest[start..]);
            rest = "";
            break;
        };
        match key.trim() {
            "风险与协调事项" => output.push_str(DAILY_RISK_SLOT),
            "项目计划" | "下阶段重点任务与交付" | "未完成待办" => {
                output.push_str(DAILY_PLAN_SLOT)
            }
            _ => output.push_str(&rest[start..start + 2 + key.len() + 2]),
        }
        rest = tail;
    }
    output.push_str(rest);
    output
}

pub fn finish_daily_ai(content: &str, report: &Report, template: &str) -> Result<String, String> {
    let template = daily_ai_template(template);
    for slot in [DAILY_RISK_SLOT, DAILY_PLAN_SLOT] {
        if template.contains(slot) && !content.lines().any(|line| line.trim() == slot) {
            return Err("AI 未保留日报风险或计划位置，请重新生成".into());
        }
    }
    // Only replace standalone markers, never marker text inside a todo title.
    let risks = daily_risks(report);
    let plan = daily_plan(report);
    let content = flatten_daily_completed_items(content);
    Ok(content
        .lines()
        .map(|line| match line.trim() {
            DAILY_RISK_SLOT => risks.as_str(),
            DAILY_PLAN_SLOT => plan.as_str(),
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn flatten_daily_completed_items(content: &str) -> String {
    let mut output = Vec::new();
    let mut in_completed = false;
    let mut has_summary = false;
    let mut number = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        let heading = trimmed
            .trim_start_matches('#')
            .trim()
            .trim_end_matches(['：', ':']);
        if matches!(
            heading,
            "【重点工作与交付】" | "【完成情况】" | "重点工作与交付" | "完成情况"
        ) {
            in_completed = true;
            has_summary = false;
            number = 0;
        } else if trimmed == DAILY_RISK_SLOT
            || trimmed == DAILY_PLAN_SLOT
            || (trimmed.starts_with("## ") || trimmed.starts_with("【") && trimmed.ends_with('】'))
        {
            in_completed = false;
        }
        if in_completed {
            if let Some((prefix, summary)) = trimmed.split_once('、') {
                if let Ok(index) = prefix.parse::<usize>() {
                    let summary = summary.trim();
                    has_summary = summary
                        .split_once(['：', ':'])
                        .is_some_and(|(_, description)| !description.trim().is_empty());
                    number = index;
                    if !summary.is_empty() && summary.ends_with(['：', ':']) {
                        output.push(summary.trim_end_matches(['：', ':']).to_string());
                        number = 0;
                        continue;
                    }
                }
            } else if let Some(first) = trimmed.chars().next().filter(|c| ('①'..='⑳').contains(c))
            {
                // A numbered summary is the complete item; discard its nested details.
                if has_summary {
                    continue;
                }
                // If the model supplied only a project heading, retain each actual item.
                number += 1;
                output.push(format!("{number}、{}", trimmed[first.len_utf8()..].trim()));
                continue;
            } else if !trimmed.is_empty() {
                has_summary = false;
                number = 0;
            }
        }
        output.push(line.to_string());
    }
    output.join("\n")
}

pub fn render_template(report: &Report, templates: &ReportTemplates) -> Result<String, String> {
    templates.validate()?;
    let stats = &report.statistics;
    let goals = &report.sources.goals;
    let is_daily = report.r#type == ReportType::Daily;
    let list = |items: Vec<&Goal>| -> String {
        if items.is_empty() {
            return "暂无".into();
        }
        items
            .iter()
            .map(|goal| {
                let project = if goal.project.trim().is_empty() {
                    String::new()
                } else {
                    format!("【{}】", goal.project.trim())
                };
                let spent = goal
                    .actual_min
                    .map(|minutes| format!("，耗时 {}", format_minutes(minutes as u64)))
                    .unwrap_or_default();
                let adhoc = if goal.ad_hoc { "（插活）" } else { "" };
                if is_daily {
                    return format!(
                        "- [{}] {project}{}{adhoc}",
                        if goal.completed { "x" } else { " " },
                        goal.title
                    );
                }
                format!(
                    "- [{}] {project}{}{adhoc}（{}{spent}）",
                    if goal.completed { "x" } else { " " },
                    goal.title,
                    goal.date
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut dates: Vec<_> = goals.iter().map(|goal| &goal.date).collect();
    dates.sort();
    dates.dedup();
    let daily = dates
        .iter()
        .map(|date| {
            format!(
                "### {}\n\n{}",
                date,
                list(goals.iter().filter(|goal| &goal.date == *date).collect())
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let completed: Vec<_> = goals.iter().filter(|goal| goal.completed).collect();
    let unfinished: Vec<_> = goals.iter().filter(|goal| !goal.completed).collect();
    let period = match report.r#type {
        ReportType::Daily => "今日",
        ReportType::Weekly => "本周",
        ReportType::Monthly => "本月",
    };
    let time_note = if is_daily { None } else { time_summary(goals) };
    let mut summary = format!(
        "{period}共 {} 项待办，已完成 {} 项，待推进 {} 项。{}",
        stats.goals,
        stats.completed,
        unfinished.len(),
        time_note
            .as_ref()
            .map(|note| format!("\n{note}"))
            .unwrap_or_default()
    );
    let comparisons = &report.sources.plan_comparisons;
    if !comparisons.is_empty() {
        summary = format!(
            "周计划 {} 项，已完成 {} 项，阻塞 {} 项。\n{summary}",
            comparisons.len(),
            comparisons
                .iter()
                .filter(|r| r.current.status == PlanStatus::Completed)
                .count(),
            comparisons
                .iter()
                .filter(|r| r.current.status == PlanStatus::Blocked)
                .count()
        );
    }
    let mut values = [
        (
            "周计划进展对照",
            crate::plans::table(&report.sources.plan_comparisons),
        ),
        ("标题", report.title.clone()),
        ("日期", report.end_date.clone()),
        ("报告日期", report.end_date.clone()),
        ("日期年-月-日", report.end_date.clone()),
        ("汇报摘要", summary.clone()),
        (
            "月度目标达成情况",
            format!("{summary}\n月度目标、验收标准及达成情况：待确认"),
        ),
        (
            "重点项目成果与实际影响",
            project_items(completed.clone(), Some(("成果或进度", "实际影响"))),
        ),
        (
            "风险与协调事项",
            match recorded_reasons(&unfinished) {
                Some(reasons) => format!(
                    "风险、进度影响及需协调事项：\n{reasons}{}",
                    if reasons.lines().count() < unfinished.len() {
                        format!(
                            "\n其余 {} 项未记录风险或问题：待确认",
                            unfinished.len() - reasons.lines().count()
                        )
                    } else {
                        String::new()
                    }
                ),
                None => "风险、进度影响及需协调事项：待确认".into(),
            },
        ),
        (
            "下阶段重点任务与交付",
            project_items(unfinished.clone(), Some(("重点任务", "预期交付及完成时间"))),
        ),
        (
            "未完成事项与原因",
            if unfinished.is_empty() {
                "所选范围内无未完成待办。".into()
            } else {
                match recorded_reasons(&unfinished) {
                    Some(reasons) => format!(
                        "待推进事项：\n{}\n\n未完成原因：\n{reasons}",
                        project_items(unfinished.clone(), None)
                    ),
                    None => format!(
                        "待推进事项：\n{}\n\n未完成原因：待确认",
                        project_items(unfinished.clone(), None)
                    ),
                }
            },
        ),
        (
            "日期范围",
            if report.start_date == report.end_date {
                report.end_date.clone()
            } else {
                format!("{} — {}", report.start_date, report.end_date)
            },
        ),
        ("项目完成情况", project_items(completed, None)),
        ("项目计划", project_items(unfinished, None)),
        (
            "范围",
            if report.start_date == report.end_date {
                report.start_date.clone()
            } else {
                format!("{} — {}", report.start_date, report.end_date)
            },
        ),
        (
            "概况",
            format!(
                "{} 天，共 {} 项待办，已完成 {} 项，完成率 {}%。{}",
                stats.days,
                stats.goals,
                stats.completed,
                stats.completion_rate,
                time_note
                    .as_ref()
                    .map(|note| format!("\n{note}"))
                    .unwrap_or_default()
            ),
        ),
        (
            "耗时统计",
            time_note.clone().unwrap_or_else(|| "未记录耗时".into()),
        ),
        (
            "已完成待办",
            list(goals.iter().filter(|goal| goal.completed).collect()),
        ),
        (
            "未完成待办",
            list(goals.iter().filter(|goal| !goal.completed).collect()),
        ),
        ("每日待办", daily),
    ];
    if is_daily {
        let untimed: Vec<_> = goals
            .iter()
            .filter(|g| g.completed)
            .cloned()
            .map(|mut g| {
                g.actual_min = None;
                g
            })
            .collect();
        let completed = project_items_layout(untimed.iter().collect(), None, true);
        let plan = daily_plan(report);
        let risks = daily_risks(report);
        for (key, value) in &mut values {
            match *key {
                "重点项目成果与实际影响" | "项目完成情况" => {
                    *value = completed.clone()
                }
                "下阶段重点任务与交付" | "项目计划" | "未完成待办" => {
                    *value = plan.clone()
                }
                "风险与协调事项" => *value = risks.clone(),
                "未完成事项与原因" => {
                    *value = if plan == "暂无" {
                        "所选范围内无未完成待办。".into()
                    } else {
                        format!(
                            "待推进事项：\n{plan}\n\n未完成原因：{}",
                            recorded_reasons(
                                &goals.iter().filter(|g| !g.completed).collect::<Vec<_>>()
                            )
                            .unwrap_or_else(|| "待确认".into())
                        )
                    };
                }
                "耗时统计" => value.clear(),
                _ => {}
            }
        }
    }
    let blocked: Vec<_> = comparisons
        .iter()
        .filter(|r| r.current.status == PlanStatus::Blocked)
        .map(|r| format!("{}：{}", r.current.title, r.current.blocker))
        .collect();
    if !blocked.is_empty() {
        if let Some((_, value)) = values.iter_mut().find(|(key, _)| *key == "风险与协调事项")
        {
            *value = blocked.join("\n");
            if let Some(reasons) =
                recorded_reasons(&goals.iter().filter(|g| !g.completed).collect::<Vec<_>>())
            {
                value.push_str(&format!("\n{reasons}"));
            }
        }
    }
    let mut template = if is_daily {
        daily_report_template(&templates.daily)
    } else {
        templates.get(&report.r#type).to_string()
    };
    if !is_daily && report.style == ReportStyle::Detailed && !template.contains("{{每日待办}}")
    {
        template.push_str("\n\n## 每日待办\n{{每日待办}}");
    }
    if !report.sources.plan_comparisons.is_empty() && !template.contains("{{周计划进展对照}}")
    {
        template.push_str("\n\n## 周计划进展对照\n{{周计划进展对照}}");
    }
    // Substitute only template tokens; task text is never interpreted as another token.
    let mut output = String::new();
    let mut rest = template.as_str();
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let token = &rest[start + 2..];
        if let Some(end) = token.find("}}") {
            let key = token[..end].trim();
            if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
                output.push_str(value);
            } else {
                output.push_str(&format!(
                    "待填写：{}",
                    if key.is_empty() {
                        "自定义字段"
                    } else {
                        key
                    }
                ));
            }
            rest = &token[end + 2..];
        } else {
            output.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    output.push_str(rest);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_plan_includes_backlog_without_past_completions_or_future_todos() {
        let goals: Vec<_> = [
            ("2026-09-30", "积压目标：完成接口联调", false),
            ("2026-10-08", "昨日已完成", true),
            ("2026-10-09", "今日交付", true),
            ("2026-10-09", "当天未完成", false),
            ("2026-10-10", "未来待办", false),
        ]
        .into_iter()
        .map(|(date, title, completed)| Goal {
            date: date.into(),
            title: title.into(),
            completed,
            ..Goal::default()
        })
        .collect();
        for style in [
            ReportStyle::Brief,
            ReportStyle::Detailed,
            ReportStyle::Review,
        ] {
            let mut req = request("2026-10-09", ReportType::Daily);
            req.style = style;
            let report = make_report(&req, vec![], goals.clone()).unwrap();
            let (completed, planned) = report.markdown.split_once("【明日重点计划】").unwrap();
            assert!(completed.contains("今日交付"));
            assert!(!completed.contains("积压目标"));
            assert_eq!(planned.trim(), "1、积压目标：完成接口联调\n2、当天未完成");
            assert!(!report.markdown.contains("昨日已完成"));
            assert!(!report.markdown.contains("未来待办"));
            assert_eq!(report.sources.goals.len(), 3);
            assert_eq!(report.statistics.completed, 1);
        }
        let req = request("2026-10-09", ReportType::Daily);
        let report = make_report(&req, vec![], vec![goals[0].clone()]).unwrap();
        assert_eq!(daily_plan(&report), "1、积压目标：完成接口联调");
        assert_eq!(
            make_report(
                &request("2026-10-09", ReportType::Weekly),
                vec![],
                goals.clone()
            )
            .unwrap()
            .sources
            .goals
            .len(),
            4
        );
        assert_eq!(
            make_report(&request("2026-10-09", ReportType::Monthly), vec![], goals)
                .unwrap()
                .sources
                .goals
                .len(),
            4
        );
    }

    #[test]
    fn custom_daily_templates_keep_backlog_in_local_and_ai_plans() {
        let report = make_report(
            &request("2026-10-09", ReportType::Daily),
            vec![],
            vec![Goal {
                date: "2026-10-08".into(),
                title: "往日未完成".into(),
                ..Goal::default()
            }],
        )
        .unwrap();
        let templates = ReportTemplates {
            daily: "# 我的日报\n{{项目完成情况}}".into(),
            ..ReportTemplates::default()
        };
        assert_eq!(
            render_template(&report, &templates).unwrap(),
            "# 我的日报\n暂无\n\n## 【明日重点计划】\n1、往日未完成"
        );
        assert_eq!(
            daily_ai_template(&templates.daily),
            "# 我的日报\n{{项目完成情况}}\n\n## 【明日重点计划】\nRIJI_DAILY_PLAN_SLOT"
        );
        assert!(finish_daily_ai("# 我的日报\n暂无", &report, &templates.daily).is_err());
        assert_eq!(
            finish_daily_ai(
                "# 我的日报\n暂无\n\n## 【明日重点计划】\nRIJI_DAILY_PLAN_SLOT",
                &report,
                &templates.daily
            )
            .unwrap(),
            render_template(&report, &templates).unwrap()
        );
        for token in ["项目计划", "下阶段重点任务与交付", "未完成待办"] {
            let template = format!("# 日报\n## 明日计划\n{{{{ {token} }}}}");
            assert_eq!(daily_report_template(&template), template);
            assert_eq!(
                daily_ai_template(&template),
                "# 日报\n## 明日计划\nRIJI_DAILY_PLAN_SLOT"
            );
        }
    }

    #[test]
    fn daily_summaries_use_one_line_and_heading_only_groups_keep_their_work() {
        let duplicated = "# 日报 2026-10-12\n\n## 【重点工作与交付】\nAI教学工作站\n1、岗位图谱数据：完成岗位图谱数据添加侧重于广西方面。\n①岗位图谱数据添加侧重于广西方面。\n2、岗位图谱样式：完成岗位图谱样式变更。\n①岗位图谱样式变更。\n3、岗位图谱搜索栏：完成岗位图谱搜索栏变更。\n①岗位图谱搜索栏变更。\n\n## 【风险与需协调事项】\nRIJI_DAILY_RISK_SLOT\n\n## 【明日重点计划】\nRIJI_DAILY_PLAN_SLOT";
        let report = make_report(
            &request("2026-10-12", ReportType::Daily),
            vec![],
            vec![Goal {
                date: "2026-10-12".into(),
                title: "研究使用presenton的PPT生成开源项目".into(),
                ..Goal::default()
            }],
        )
        .unwrap();
        let output =
            finish_daily_ai(duplicated, &report, &ReportTemplates::default().daily).unwrap();
        assert!(output.contains("AI教学工作站\n1、岗位图谱数据：完成岗位图谱数据添加侧重于广西方面。\n2、岗位图谱样式：完成岗位图谱样式变更。\n3、岗位图谱搜索栏：完成岗位图谱搜索栏变更。"));
        assert!(!output.contains('①'));
        assert_eq!(output.matches("岗位图谱数据添加侧重于广西方面").count(), 1);
        assert!(output.contains("【风险与需协调事项】\n无"));
        assert!(output.ends_with("【明日重点计划】\n1、研究使用presenton的PPT生成开源项目"));
        let grouped = "## 【重点工作与交付】\n1、官网：\n①完成登录\n②完成搜索\n另一个项目\n1、新功能：完成导出\n①导出实现细节\n## 【风险与需协调事项】\n①风险原文";
        assert_eq!(flatten_daily_completed_items(grouped), "## 【重点工作与交付】\n官网\n1、完成登录\n2、完成搜索\n另一个项目\n1、新功能：完成导出\n## 【风险与需协调事项】\n①风险原文");
    }

    #[test]
    fn daily_content_risks_and_verbatim_plan_are_stable_across_styles() {
        let goals = vec![
            Goal {
                title: "岗位图谱样式变更".into(),
                project: "AI教学工作站".into(),
                date: "2026-10-12".into(),
                completed: true,
                actual_min: Some(90),
                ..Goal::default()
            },
            Goal {
                title: "研究使用presenton的PPT生成开源项目".into(),
                date: "2026-10-12".into(),
                ..Goal::default()
            },
        ];
        for style in [
            ReportStyle::Brief,
            ReportStyle::Detailed,
            ReportStyle::Review,
        ] {
            let mut req = request("2026-10-12", ReportType::Daily);
            req.style = style;
            let mut report = make_report(&req, vec![], goals.clone()).unwrap();
            assert_eq!(report.markdown, "# 日报 2026-10-12\n\n## 【重点工作与交付】\nAI教学工作站\n1、岗位图谱样式变更\n\n## 【风险与需协调事项】\n无\n\n## 【明日重点计划】\n1、研究使用presenton的PPT生成开源项目");
            assert_eq!(report.sources.goals[0].actual_min, Some(90));
            report.sources.goals[0].risk = "待确认字段".into();
            report.sources.goals[1].risk = "尚未找到可用模板".into();
            assert_eq!(daily_risks(&report), "①岗位图谱样式变更：待确认字段\n②研究使用presenton的PPT生成开源项目：尚未找到可用模板");
            let template = ReportTemplates::default();
            assert_eq!(
                finish_daily_ai(
                    "风险\nRIJI_DAILY_RISK_SLOT\n计划\nRIJI_DAILY_PLAN_SLOT",
                    &report,
                    &template.daily
                )
                .unwrap(),
                format!(
                    "风险\n{}\n计划\n{}",
                    daily_risks(&report),
                    daily_plan(&report)
                )
            );
            assert!(finish_daily_ai("风险与计划已被模型改写", &report, &template.daily).is_err());
        }
        let old = "# 日报 {{日期年-月-日}}\n\n## 【总体情况】\n{{汇报摘要}}\n\n## 【重点工作与交付】\n{{重点项目成果与实际影响}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【明日重点计划】\n{{下阶段重点任务与交付}}";
        let mut templates = ReportTemplates {
            daily: old.into(),
            ..ReportTemplates::default()
        };
        templates.upgrade_defaults();
        assert_eq!(templates.daily, ReportTemplates::default().daily);
        templates.daily = format!("{old}\n我的要求");
        templates.upgrade_defaults();
        assert_eq!(templates.daily, format!("{old}\n我的要求"));
    }
    fn request(date: &str, kind: ReportType) -> ReportRequest {
        ReportRequest {
            r#type: kind,
            anchor_date: date.into(),
            start_date: None,
            end_date: None,
            style: ReportStyle::Brief,
            mode: GenerationMode::Local,
            include_notes: true,
            include_goals: true,
            include_commits: false,
            instruction: String::new(),
        }
    }
    #[test]
    fn weeks_cross_year_and_months_handle_leap_days() {
        assert_eq!(
            period(&request("2027-01-01", ReportType::Weekly)).unwrap(),
            ("2026-12-28".into(), "2027-01-03".into())
        );
        assert_eq!(
            period(&request("2024-02-20", ReportType::Monthly)).unwrap(),
            ("2024-02-01".into(), "2024-02-29".into())
        );
        assert!(validate_date("2026-02-30").is_err());
    }
    #[test]
    fn empty_sources_cannot_produce_reports() {
        assert!(make_report(&request("2026-10-09", ReportType::Daily), vec![], vec![]).is_err());
    }
    #[test]
    fn custom_ranges_keep_the_selected_dates_and_report_metadata() {
        for kind in [ReportType::Weekly, ReportType::Monthly] {
            let mut custom = request("2026-10-09", kind);
            custom.start_date = Some("2026-12-31".into());
            custom.end_date = Some("2027-01-02".into());
            assert_eq!(
                period(&custom).unwrap(),
                ("2026-12-31".into(), "2027-01-02".into())
            );
            let goals = vec![Goal {
                id: "test-goal".into(),
                date: "2027-01-02".into(),
                title: "完成阶段待办".into(),
                completed: true,
                created_at: String::new(),
                updated_at: String::new(),
                ..Goal::default()
            }];
            let report = make_report(&custom, vec![], goals).unwrap();
            assert_eq!(report.start_date, "2026-12-31");
            assert_eq!(report.end_date, "2027-01-02");
            assert!(report.markdown.contains("2026-12-31 — 2027-01-02"));
            assert_eq!(report.statistics.completed, 1);
            custom.start_date = Some("2027-01-02".into());
            assert_eq!(
                period(&custom).unwrap(),
                ("2027-01-02".into(), "2027-01-02".into())
            );
        }
    }

    #[test]
    fn custom_ranges_reject_missing_invalid_and_reversed_dates() {
        for (start, end) in [
            (Some("2026-10-09"), None),
            (Some(""), Some("2026-10-09")),
            (Some("2026-10-10"), Some("2026-10-09")),
            (Some("2026-02-30"), Some("2026-03-01")),
            (Some("1899-12-31"), Some("1900-01-01")),
        ] {
            let mut custom = request("2026-10-09", ReportType::Weekly);
            custom.start_date = start.map(String::from);
            custom.end_date = end.map(String::from);
            assert!(period(&custom).is_err());
        }
    }

    #[test]
    fn legacy_requests_still_deserialize_and_daily_reports_use_one_date() {
        let legacy = serde_json::json!({
            "type": "weekly", "anchorDate": "2026-10-09", "style": "brief",
            "mode": "local", "includeNotes": true, "includeGoals": true, "instruction": ""
        });
        let parsed: ReportRequest = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            period(&parsed).unwrap(),
            ("2026-10-05".into(), "2026-10-11".into())
        );
        let mut daily = request("2026-10-09", ReportType::Daily);
        daily.start_date = Some("2026-10-01".into());
        daily.end_date = Some("2026-10-31".into());
        assert_eq!(
            period(&daily).unwrap(),
            ("2026-10-09".into(), "2026-10-09".into())
        );
    }
    #[test]
    fn completed_and_unfinished_goals_stay_distinct() {
        let goals: Vec<_> = [true, false, false]
            .iter()
            .enumerate()
            .map(|(i, done)| Goal {
                id: i.to_string(),
                date: "2026-10-09".into(),
                title: format!("待办{}", i),
                completed: *done,
                created_at: String::new(),
                updated_at: String::new(),
                ..Goal::default()
            })
            .collect();
        let report = make_report(&request("2026-10-09", ReportType::Daily), vec![], goals).unwrap();
        assert_eq!(report.statistics.completion_rate, 33);
        let (completed, planned) = report.markdown.split_once("【明日重点计划】").unwrap();
        assert!(completed.contains("1、待办0"));
        assert!(!completed.contains("待办1"));
        assert!(planned.contains("1、待办1\n2、待办2"));
        assert!(!planned.contains("待办0"));
    }
    #[test]
    fn notes_never_enter_reports_and_templates_are_type_specific() {
        let note = Note {
            id: "note".into(),
            date: "2026-10-09".into(),
            content: "记录私有内容".into(),
            goal_id: Some("goal".into()),
            kind: NoteKind::Blocker,
            tag: String::new(),
            images: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let goal = Goal {
            id: "goal".into(),
            date: note.date.clone(),
            title: "处理 {{未完成待办}}".into(),
            completed: true,
            created_at: String::new(),
            updated_at: String::new(),
            ..Goal::default()
        };
        let templates = ReportTemplates {
            daily: "日定制\n{{已完成待办}}".into(),
            weekly: "周定制\n{{概况}}".into(),
            monthly: "月定制\n{{每日待办}}".into(),
        };
        for (kind, heading) in [
            (ReportType::Daily, "日定制"),
            (ReportType::Weekly, "周定制"),
            (ReportType::Monthly, "月定制"),
        ] {
            let mut req = request("2026-10-09", kind);
            req.include_goals = false;
            assert!(make_report(&req, vec![note.clone()], vec![]).is_err());
            let report = make_report(&req, vec![note.clone()], vec![goal.clone()]).unwrap();
            assert!(report.sources.notes.is_empty());
            assert_eq!(report.statistics.notes, 0);
            assert_eq!(report.statistics.blockers, 0);
            let output = render_template(&report, &templates).unwrap();
            assert!(output.starts_with(heading));
            assert!(!output.contains("记录私有内容"));
            if report.r#type == ReportType::Daily {
                assert!(output.contains("处理 {{未完成待办}}"));
            }
        }
        assert_eq!(statistics(&[note], &[]).notes, 1);
        let legacy: AiSettings =
            serde_json::from_str(r#"{"endpoint":"https://example.com/v1","model":"test"}"#)
                .unwrap();
        assert_eq!(
            legacy.report_templates.daily,
            ReportTemplates::default().daily
        );
        let mut invalid = templates;
        invalid.weekly = "{{工作记录}}".into();
        assert!(invalid.validate().is_ok());
    }
    #[test]
    fn custom_placeholders_save_and_local_reports_mark_unknown_fields() {
        let template = "{{日期年-月-日}}完成任务进度和计划\n【完成情况】：\n1、{{项目名称}}\n①{{完成的事项1}}\n【明日计划】\n{{计划完成的事项1}}\n{{已完成待办}}";
        let templates = ReportTemplates {
            weekly: template.into(),
            ..ReportTemplates::default()
        };
        templates.validate().unwrap();
        let goal = Goal {
            id: "goal".into(),
            date: "2026-10-09".into(),
            title: "完成模板修复".into(),
            completed: true,
            created_at: String::new(),
            updated_at: String::new(),
            ..Goal::default()
        };
        let report = make_report(
            &request("2026-10-09", ReportType::Weekly),
            vec![],
            vec![goal],
        )
        .unwrap();
        let output = render_template(&report, &templates).unwrap();
        assert!(output.starts_with("2026-10-11完成任务进度和计划"));
        assert!(output.contains("待填写：项目名称"));
        assert!(output.contains("待填写：完成的事项1"));
        assert!(output.contains("待填写：计划完成的事项1"));
        assert!(output.contains("- [x] 完成模板修复"));
    }

    #[test]
    fn default_reports_group_projects_and_use_the_correct_period_and_plan() {
        let goals: Vec<_> = [
            ("项目名称1：完成的内容或进度1", true),
            ("【项目名称1】完成的内容或进度2", true),
            ("[项目名称2]完成的内容或进度3", true),
            ("项目名称1：计划内容1", false),
            ("未填写项目的事项", false),
        ]
        .iter()
        .map(|(title, completed)| Goal {
            id: uuid::Uuid::new_v4().to_string(),
            date: "2026-10-09".into(),
            title: (*title).into(),
            completed: *completed,
            created_at: String::new(),
            updated_at: String::new(),
            ..Goal::default()
        })
        .collect();
        for (kind, header, plan) in [
            (
                ReportType::Weekly,
                "# 周报\n\n2026-10-05 — 2026-10-11",
                "下周重点计划",
            ),
            (
                ReportType::Monthly,
                "# 月报\n\n2026-10-01 — 2026-10-31",
                "下月重点计划",
            ),
        ] {
            let report = make_report(&request("2026-10-09", kind), vec![], goals.clone()).unwrap();
            assert!(report.markdown.starts_with(&format!("{header}\n")));
            let plan_at = report.markdown.find(&format!("## 【{plan}】")).unwrap();
            let results_at = report
                .markdown
                .find("①成果或进度：完成的内容或进度1")
                .unwrap();
            assert!(plan_at > results_at);
            assert!(report.markdown.contains("1、项目名称1：\n①成果或进度：完成的内容或进度1\n②成果或进度：完成的内容或进度2\n③实际影响：待确认\n2、项目名称2：\n①成果或进度：完成的内容或进度3\n②实际影响：待确认"));
            assert!(report.markdown[plan_at..].contains("1、项目名称1：\n①重点任务：计划内容1\n②预期交付及完成时间：待确认\n2、待办事项：\n①重点任务：未填写项目的事项"));
            assert!(!report.markdown[plan_at..].contains("完成的内容或进度"));
            assert!(report
                .markdown
                .contains("风险、进度影响及需协调事项：待确认"));
            assert!(!report.markdown.contains("{{"));
            assert!(!report.markdown.contains("目标达成率"));
            if report.r#type == ReportType::Monthly {
                assert!(report
                    .markdown
                    .contains("月度目标、验收标准及达成情况：待确认"));
                assert!(report.markdown.contains("未完成原因：待确认"));
            }
            assert_eq!(report.statistics.completion_rate, 60);
        }
        let report = make_report(
            &request("2026-10-09", ReportType::Daily),
            vec![],
            vec![goals[3].clone()],
        )
        .unwrap();
        assert!(report.markdown.contains("【重点工作与交付】\n暂无"));
        let report = make_report(
            &request("2026-10-09", ReportType::Daily),
            vec![],
            vec![goals[0].clone()],
        )
        .unwrap();
        assert!(report.markdown.contains("【明日重点计划】\n暂无"));
        let report = make_report(
            &request("2026-10-09", ReportType::Monthly),
            vec![],
            vec![goals[0].clone()],
        )
        .unwrap();
        assert!(report.markdown.contains("所选范围内无未完成待办。"));
        assert!(report
            .markdown
            .contains("月度目标、验收标准及达成情况：待确认"));
    }

    #[test]
    fn project_field_flows_into_daily_reports_without_time_annotations() {
        let mk = |title: &str, project: &str, done: bool, actual: Option<u32>| Goal {
            id: uuid::Uuid::new_v4().to_string(),
            date: "2026-10-09".into(),
            title: title.into(),
            completed: done,
            project: project.into(),
            estimate_min: actual.map(|m| m / 2),
            actual_min: actual,
            ..Goal::default()
        };
        let goals = vec![
            mk("接口联调", "官网改版", true, Some(150)),
            mk("【旧写法】标题里的项目", "官网改版", true, None),
            mk("采购询价", "", false, None),
        ];
        let report = make_report(&request("2026-10-09", ReportType::Daily), vec![], goals).unwrap();
        assert!(report.markdown.contains("官网改版\n1、接口联调"));
        assert!(!report.markdown.contains("耗时"));
        assert_eq!(report.statistics.actual_minutes, 150);
        assert_eq!(report.statistics.estimate_minutes, 75);
        let untimed = vec![mk("只有标题", "", true, None)];
        let report =
            make_report(&request("2026-10-09", ReportType::Daily), vec![], untimed).unwrap();
        assert!(!report.markdown.contains("分钟") && !report.markdown.contains("小时"));
        assert_eq!(format_minutes(0), "0 分钟");
        assert_eq!(format_minutes(60), "1 小时");
    }

    #[test]
    fn recorded_risks_fill_the_pending_sections_and_adhoc_items_are_marked() {
        let mk = |title: &str, risk: &str| Goal {
            id: uuid::Uuid::new_v4().to_string(),
            date: "2026-10-09".into(),
            title: title.into(),
            created_at: String::new(),
            updated_at: String::new(),
            risk: risk.into(),
            ..Goal::default()
        };
        let mut request = request("2026-10-09", ReportType::Monthly);
        request.style = ReportStyle::Detailed;
        let report = make_report(
            &request,
            vec![],
            vec![
                mk("接口联调", "依赖外部联调环境，本日无法完成"),
                mk("采购询价", ""),
            ],
        )
        .unwrap();
        assert!(report.markdown.contains(
            "风险、进度影响及需协调事项：\n①接口联调：依赖外部联调环境，本日无法完成\n其余 1 项未记录风险或问题：待确认"
        ));
        assert!(report
            .markdown
            .contains("未完成原因：\n①接口联调：依赖外部联调环境，本日无法完成"));
        assert!(!report
            .markdown
            .contains("风险、进度影响及需协调事项：待确认"));
        let plain = make_report(&request, vec![], vec![mk("接口联调", "")]).unwrap();
        assert!(plain
            .markdown
            .contains("风险、进度影响及需协调事项：待确认"));
        assert!(plain.markdown.contains("未完成原因：待确认"));
        let adhoc = vec![Goal {
            ad_hoc: true,
            ..mk("临时插入", "")
        }];
        let report = make_report(&request, vec![], adhoc).unwrap();
        assert!(report.markdown.contains("临时插入（插活）"));
    }
}
