use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Note,
    Progress,
    Blocker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub date: String,
    pub content: String,
    pub kind: NoteKind,
    pub tag: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<NoteImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteImage {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageUpload {
    pub id: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupImage {
    pub image: NoteImage,
    pub data: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub date: String,
    pub title: String,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: String,
    /// 自由填写的项目名称，留空时回退到标题中的 `项目：事项` 写法。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// 预估耗时（分钟）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_min: Option<u32>,
    /// 实际耗时（分钟）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_min: Option<u32>,
    /// 勾选完成的时间（RFC 3339）；旧数据没有该字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// 首次计划日期，顺延后保留原日期。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_date: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub carry_count: u32,
    /// 风险或未完成原因，仅在用户填写时进入报告。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub risk: String,
    /// 插活：中途临时安排、不在原计划内的事项。
    #[serde(default, skip_serializing_if = "is_false")]
    pub ad_hoc: bool,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReportType {
    Daily,
    Weekly,
    Monthly,
}

impl ReportType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Daily => "日报",
            Self::Weekly => "周报",
            Self::Monthly => "月报",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReportStyle {
    Brief,
    Detailed,
    Review,
}

impl ReportStyle {
    pub fn instruction(&self) -> &'static str {
        match self {
            Self::Brief => "使用简洁汇报语气，控制在 600 字以内；章节和顺序以报告模板为准。",
            Self::Detailed => "使用详细汇报语气，说明待办日期和状态；章节和顺序以报告模板为准。",
            Self::Review => "使用个人复盘语气；章节和顺序以报告模板为准。推测必须标注为建议，未记录的收获不得编造。",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationMode {
    Local,
    Ai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRequest {
    pub r#type: ReportType,
    pub anchor_date: String,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    pub style: ReportStyle,
    pub mode: GenerationMode,
    pub include_notes: bool,
    pub include_goals: bool,
    #[serde(default)]
    pub include_commits: bool,
    pub instruction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub days: usize,
    pub notes: usize,
    pub goals: usize,
    pub completed: usize,
    pub blockers: usize,
    pub completion_rate: usize,
    #[serde(default)]
    pub estimate_minutes: u64,
    #[serde(default)]
    pub actual_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sources {
    #[serde(default, rename = "planComparisons")]
    pub plan_comparisons: Vec<PlanComparison>,
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
    #[serde(default)]
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub date: String,
    pub repository: String,
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GitProvider {
    #[default]
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitSettings {
    pub enabled: bool,
    pub provider: GitProvider,
    pub gitlab_url: String,
    pub has_token: bool,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: GitProvider::Github,
            gitlab_url: "https://gitlab.com".into(),
            has_token: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub id: String,
    pub title: String,
    pub r#type: ReportType,
    pub style: ReportStyle,
    pub mode: GenerationMode,
    pub start_date: String,
    pub end_date: String,
    pub markdown: String,
    pub statistics: Statistics,
    pub created_at: String,
    pub model: String,
    #[serde(default)]
    pub sources: Sources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub endpoint: String,
    pub model: String,
    #[serde(default)]
    pub has_api_key: bool,
    #[serde(default)]
    pub thinking: bool,
    #[serde(default)]
    pub report_templates: ReportTemplates,
    #[serde(default)]
    pub git: GitSettings,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            model: String::new(),
            has_api_key: false,
            thinking: false,
            report_templates: ReportTemplates::default(),
            git: GitSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportTemplates {
    pub daily: String,
    pub weekly: String,
    pub monthly: String,
}

impl Default for ReportTemplates {
    fn default() -> Self {
        Self {
            daily: "# 日报 {{日期年-月-日}}\n\n## 【总体情况】\n{{汇报摘要}}\n\n## 【重点工作与交付】\n{{重点项目成果与实际影响}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【明日重点计划】\n{{下阶段重点任务与交付}}".into(),
            weekly: "# 周报\n\n{{日期范围}}\n\n## 【本周总体情况】\n{{汇报摘要}}\n\n## 周计划进展对照\n{{周计划进展对照}}\n\n## 【重点项目与节点进展】\n{{重点项目成果与实际影响}}\n\n## 【进度风险与需协调事项】\n{{风险与协调事项}}\n\n## 【下周重点计划】\n{{下阶段重点任务与交付}}".into(),
            monthly: "# 月报\n\n{{日期范围}}\n\n## 【月度目标达成情况】\n{{月度目标达成情况}}\n\n## 【主要成果与业务价值】\n{{重点项目成果与实际影响}}\n\n## 【未完成事项与原因】\n{{未完成事项与原因}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【下月重点计划】\n{{下阶段重点任务与交付}}".into(),
        }
    }
}

impl ReportTemplates {
    pub fn upgrade_defaults(&mut self) {
        let defaults = Self::default();
        for (value, default, period, kind, date, plan) in [
            (
                &mut self.daily,
                defaults.daily,
                "今日",
                "日报",
                "日期年-月-日",
                "明日计划",
            ),
            (
                &mut self.weekly,
                defaults.weekly,
                "本周",
                "周报",
                "日期范围",
                "下周计划",
            ),
            (
                &mut self.monthly,
                defaults.monthly,
                "本月",
                "月报",
                "日期范围",
                "下月计划",
            ),
        ] {
            let previous = format!("# {{{{标题}}}}\n\n{{{{范围}}}}\n\n## {period}概况\n{{{{概况}}}}\n\n## {period}完成\n{{{{已完成待办}}}}\n\n## 待继续推进\n{{{{未完成待办}}}}");
            let progress = format!("{kind}{{{{{date}}}}}完成任务进度和计划\n\n【完成情况】：\n{{{{项目完成情况}}}}\n\n【遇到问题】\n无\n\n【{plan}】\n{{{{项目计划}}}}");
            if *value == previous
                || *value == progress
                || *value == default.replace("## 周计划进展对照\n{{周计划进展对照}}\n\n", "")
            {
                *value = default;
            }
        }
    }

    pub fn get(&self, kind: &ReportType) -> &str {
        match kind {
            ReportType::Daily => &self.daily,
            ReportType::Weekly => &self.weekly,
            ReportType::Monthly => &self.monthly,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        for kind in [ReportType::Daily, ReportType::Weekly, ReportType::Monthly] {
            let value = self.get(&kind);
            if value.trim().is_empty() || value.chars().count() > 10000 {
                return Err(format!("{}模板需填写，且不能超过 10000 字", kind.label()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub plans: Vec<Plan>,
    pub plan_weeks: Vec<PlanWeek>,
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
    pub reports: Vec<Report>,
    pub settings: AiSettings,
    pub storage_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Backup {
    #[serde(default)]
    pub plans: Vec<Plan>,
    #[serde(default, rename = "planWeeks")]
    pub plan_weeks: Vec<PlanWeek>,
    #[serde(
        default,
        rename = "reportTemplates",
        skip_serializing_if = "Option::is_none"
    )]
    pub report_templates: Option<ReportTemplates>,
    pub version: u32,
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
    pub reports: Vec<Report>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<BackupImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub archived: bool,
    pub created_at: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    Pending,
    Active,
    Completed,
    Blocked,
}
impl PlanStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "未开始",
            Self::Active => "进行中",
            Self::Completed => "已完成",
            Self::Blocked => "阻塞",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanWeek {
    pub plan_id: String,
    pub week_start: String,
    pub title: String,
    pub project: String,
    pub status: PlanStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_percent: Option<u8>,
    pub progress: String,
    pub blocker: String,
    pub goal_ids: Vec<String>,
    pub carried: bool,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanComparison {
    pub plan: Plan,
    pub current: PlanWeek,
    pub previous: Option<PlanWeek>,
    pub goals: Vec<Goal>,
}
