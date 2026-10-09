use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Note,
    Progress,
    Blocker,
}

impl NoteKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Note => "备忘",
            Self::Progress => "进展",
            Self::Blocker => "问题",
        }
    }
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
}

impl Note {
    pub fn summary(&self) -> String {
        if self.images.is_empty() {
            return self.content.clone();
        }
        if self.content.is_empty() {
            format!("附图 {} 张", self.images.len())
        } else {
            format!("{} · 附图 {} 张", self.content, self.images.len())
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub date: String,
    pub title: String,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: String,
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
            Self::Brief => "生成简洁汇报，按工作概况、工作成果、问题与阻碍、待推进待办组织，控制在 600 字以内。",
            Self::Detailed => "生成详细汇报，按工作概况、成果与进展、每日工作记录、问题与阻碍、待推进待办组织。",
            Self::Review => "生成个人复盘，按待办完成情况、过程与收获、问题与阻碍、下一步行动组织。推测必须标注为建议，未记录的收获不得编造。",
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sources {
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
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
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            model: String::new(),
            has_api_key: false,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
    pub reports: Vec<Report>,
    pub settings: AiSettings,
    pub storage_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Backup {
    pub version: u32,
    pub notes: Vec<Note>,
    pub goals: Vec<Goal>,
    pub reports: Vec<Report>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<BackupImage>,
}
