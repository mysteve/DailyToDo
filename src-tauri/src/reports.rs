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
    }
}

pub fn make_report(
    request: &ReportRequest,
    notes: Vec<Note>,
    goals: Vec<Goal>,
) -> Result<Report, String> {
    if !request.include_notes && !request.include_goals {
        return Err("请至少选择一种报告来源".into());
    }
    if request.instruction.chars().count() > 2000 {
        return Err("补充要求不能超过 2000 字".into());
    }
    let (start_date, end_date) = period(request)?;
    if notes.is_empty() && goals.is_empty() {
        return Err("所选范围内没有可用于生成报告的记录".into());
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
    let stats = statistics(&notes, &goals);
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
        sources: Sources { notes, goals },
    };
    report.markdown = local_markdown(&report);
    Ok(report)
}

fn local_markdown(report: &Report) -> String {
    let stats = &report.statistics;
    let mut lines = vec![
        format!("# {}", report.title),
        String::new(),
        "## 工作概况".into(),
        String::new(),
        format!(
            "记录 {} 天，共 {} 条备忘；{} 项待办，已完成 {} 项。",
            stats.days, stats.notes, stats.goals, stats.completed
        ),
    ];
    if stats.goals > 0 {
        lines.push(format!("待办完成率：{}%。", stats.completion_rate));
    }
    lines.push(String::new());
    let notes = &report.sources.notes;
    let goals = &report.sources.goals;
    let done: Vec<_> = goals.iter().filter(|goal| goal.completed).collect();
    if !done.is_empty() {
        lines.extend(["## 已完成待办".into(), String::new()]);
        lines.extend(
            done.iter()
                .map(|goal| format!("- [x] {}（{}）", goal.title, goal.date)),
        );
        lines.push(String::new());
    }
    if report.style == ReportStyle::Detailed {
        lines.extend(["## 每日工作记录".into(), String::new()]);
        let mut dates: Vec<_> = notes
            .iter()
            .map(|note| &note.date)
            .chain(goals.iter().map(|goal| &goal.date))
            .collect();
        dates.sort();
        dates.dedup();
        for date in dates {
            lines.extend([format!("### {}", date), String::new()]);
            lines.extend(notes.iter().filter(|note| &note.date == date).map(|note| {
                format!(
                    "- {}{}：{}",
                    note.kind.label(),
                    if note.tag.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", note.tag)
                    },
                    note.summary().replace('\n', "\n  ")
                )
            }));
            lines.extend(goals.iter().filter(|goal| &goal.date == date).map(|goal| {
                format!(
                    "- [{}] {}",
                    if goal.completed { "x" } else { " " },
                    goal.title
                )
            }));
            lines.push(String::new());
        }
    } else {
        let regular: Vec<_> = notes
            .iter()
            .filter(|note| note.kind != NoteKind::Blocker)
            .collect();
        if !regular.is_empty() {
            lines.extend([
                if report.style == ReportStyle::Review {
                    "## 过程与收获".into()
                } else {
                    "## 工作进展".into()
                },
                String::new(),
            ]);
            lines.extend(regular.iter().map(|note| {
                format!(
                    "- {}{}（{}）",
                    if note.tag.is_empty() {
                        String::new()
                    } else {
                        format!("【{}】", note.tag)
                    },
                    note.summary().replace('\n', "\n  "),
                    note.date
                )
            }));
            lines.push(String::new());
        }
    }
    let blockers: Vec<_> = notes
        .iter()
        .filter(|note| note.kind == NoteKind::Blocker)
        .collect();
    if !blockers.is_empty() {
        lines.extend(["## 问题与阻碍".into(), String::new()]);
        lines.extend(blockers.iter().map(|note| {
            format!(
                "- {}（{}）",
                note.summary().replace('\n', "\n  "),
                note.date
            )
        }));
        lines.push(String::new());
    }
    let pending: Vec<_> = goals.iter().filter(|goal| !goal.completed).collect();
    if !pending.is_empty() {
        lines.extend([
            if report.style == ReportStyle::Review {
                "## 待继续推进".into()
            } else {
                "## 未完成待办".into()
            },
            String::new(),
        ]);
        lines.extend(
            pending
                .iter()
                .map(|goal| format!("- [ ] {}（{}）", goal.title, goal.date)),
        );
        lines.push(String::new());
    }
    if report.style == ReportStyle::Review {
        lines.extend([
            "## 复盘思考".into(),
            String::new(),
            "- 哪些做法值得继续？".into(),
            "- 哪些问题需要调整？".into(),
            "- 下一阶段优先推进什么？".into(),
        ]);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
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
            })
            .collect();
        let report = make_report(&request("2026-10-09", ReportType::Daily), vec![], goals).unwrap();
        assert_eq!(report.statistics.completion_rate, 33);
        assert!(report.markdown.contains("- [ ] 待办1"));
        assert!(report.markdown.contains("- [x] 待办0"));
    }
}
