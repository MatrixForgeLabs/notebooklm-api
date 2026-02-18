use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub created_at_unix: Option<i64>,
    pub is_owner: bool,
}

impl Notebook {
    pub fn from_api_response(data: &Value) -> Self {
        let title = data
            .get(0)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace("thought\n", "")
            .trim()
            .to_string();

        let id = data
            .get(2)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let created_at_unix = data
            .get(5)
            .and_then(Value::as_array)
            .and_then(|meta| meta.get(5))
            .and_then(Value::as_array)
            .and_then(|ts| ts.first())
            .and_then(Value::as_i64);

        let is_owner = data
            .get(5)
            .and_then(Value::as_array)
            .and_then(|meta| meta.get(1))
            .map(|v| !v.as_bool().unwrap_or(false))
            .unwrap_or(true);

        Self {
            id,
            title,
            created_at_unix,
            is_owner,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub type_code: Option<i64>,
    pub created_at_unix: Option<i64>,
    pub status: i64,
}

impl Source {
    pub fn from_api_response(data: &Value) -> Option<Self> {
        let arr = data.as_array()?;

        if arr.is_empty() {
            return None;
        }

        if arr.first().and_then(Value::as_array).is_some() {
            let entry = arr.first()?.as_array()?;
            let source_id = if let Some(id_arr) = entry.first().and_then(Value::as_array) {
                id_arr.first().and_then(Value::as_str).unwrap_or_default()
            } else {
                entry.first().and_then(Value::as_str).unwrap_or_default()
            }
            .to_string();

            let title = entry
                .get(1)
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let metadata = entry.get(2).and_then(Value::as_array);
            let url = metadata
                .and_then(|m| m.get(7))
                .and_then(Value::as_array)
                .and_then(|u| u.first())
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let type_code = metadata.and_then(|m| m.get(4)).and_then(Value::as_i64);

            return Some(Self {
                id: source_id,
                title,
                url,
                type_code,
                created_at_unix: None,
                status: 2,
            });
        }

        let id = arr
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let title = arr.get(1).and_then(Value::as_str).map(ToString::to_string);

        Some(Self {
            id,
            title,
            url: None,
            type_code: None,
            created_at_unix: None,
            status: 2,
        })
    }

    pub fn from_notebook_source(data: &Value) -> Option<Self> {
        let arr = data.as_array()?;
        if arr.is_empty() {
            return None;
        }

        let source_id = match arr.first() {
            Some(Value::Array(id_arr)) => id_arr
                .first()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };

        let title = arr.get(1).and_then(Value::as_str).map(ToString::to_string);

        let meta = arr.get(2).and_then(Value::as_array);
        let created_at_unix = meta
            .and_then(|m| m.get(2))
            .and_then(Value::as_array)
            .and_then(|ts| ts.first())
            .and_then(Value::as_i64);
        let type_code = meta.and_then(|m| m.get(4)).and_then(Value::as_i64);
        let url = meta
            .and_then(|m| m.get(7))
            .and_then(Value::as_array)
            .and_then(|u| u.first())
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let status = arr
            .get(3)
            .and_then(Value::as_array)
            .and_then(|s| s.get(1))
            .and_then(Value::as_i64)
            .unwrap_or(2);

        Some(Self {
            id: source_id,
            title,
            url,
            type_code,
            created_at_unix,
            status,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFulltext {
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub type_code: Option<i64>,
    pub url: Option<String>,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub query: String,
    pub answer: String,
    pub turn_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResult {
    pub answer: String,
    pub conversation_id: String,
    pub turn_number: usize,
    pub is_follow_up: bool,
    pub raw_response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactKind {
    Audio,
    Report,
    Video,
    Quiz,
    Flashcards,
    MindMap,
    Infographic,
    SlideDeck,
    DataTable,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum ArtifactExportType {
    Report = 1,
    DataTable = 2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InteractiveOutputFormat {
    Json,
    Markdown,
    Html,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum AudioFormat {
    DeepDive = 1,
    Brief = 2,
    Critique = 3,
    Debate = 4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum AudioLength {
    Short = 1,
    Default = 2,
    Long = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum VideoFormat {
    Explainer = 1,
    Brief = 2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum VideoStyle {
    AutoSelect = 1,
    Custom = 2,
    Classic = 3,
    Whiteboard = 4,
    Kawaii = 5,
    Anime = 6,
    Watercolor = 7,
    RetroPrint = 8,
    Heritage = 9,
    PaperCraft = 10,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum QuizDifficulty {
    Easy = 1,
    Medium = 2,
    Hard = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuizQuantity {
    Fewer,
    Standard,
    More,
}

impl QuizQuantity {
    pub fn code(self) -> i64 {
        match self {
            QuizQuantity::Fewer => 1,
            QuizQuantity::Standard | QuizQuantity::More => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum InfographicOrientation {
    Landscape = 1,
    Portrait = 2,
    Square = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum InfographicDetail {
    Concise = 1,
    Standard = 2,
    Detailed = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum SlideDeckFormat {
    DetailedDeck = 1,
    PresenterSlides = 2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i64)]
pub enum SlideDeckLength {
    Default = 1,
    Short = 2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportFormat {
    BriefingDoc,
    StudyGuide,
    BlogPost,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MindMapOutputFormat {
    Json,
    PrettyJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub instructions: Option<String>,
    pub format: Option<AudioFormat>,
    pub length: Option<AudioLength>,
}

impl Default for AudioGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            instructions: None,
            format: None,
            length: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub instructions: Option<String>,
    pub format: Option<VideoFormat>,
    pub style: Option<VideoStyle>,
}

impl Default for VideoGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            instructions: None,
            format: None,
            style: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub format: ReportFormat,
    pub custom_prompt: Option<String>,
}

impl Default for ReportGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            format: ReportFormat::BriefingDoc,
            custom_prompt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub instructions: Option<String>,
    pub quantity: Option<QuizQuantity>,
    pub difficulty: Option<QuizDifficulty>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlashcardsGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub instructions: Option<String>,
    pub quantity: Option<QuizQuantity>,
    pub difficulty: Option<QuizDifficulty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfographicGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub instructions: Option<String>,
    pub orientation: Option<InfographicOrientation>,
    pub detail_level: Option<InfographicDetail>,
}

impl Default for InfographicGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            instructions: None,
            orientation: None,
            detail_level: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideDeckGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub instructions: Option<String>,
    pub format: Option<SlideDeckFormat>,
    pub length: Option<SlideDeckLength>,
}

impl Default for SlideDeckGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            instructions: None,
            format: None,
            length: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTableGenerationOptions {
    pub source_ids: Option<Vec<String>>,
    pub language: String,
    pub instructions: Option<String>,
}

impl Default for DataTableGenerationOptions {
    fn default() -> Self {
        Self {
            source_ids: None,
            language: "en".to_string(),
            instructions: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MindMapGenerationOptions {
    pub source_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindMapGenerationResult {
    pub mind_map: Option<Value>,
    pub note_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 250,
            max_delay_ms: 5000,
            jitter_ms: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub title: String,
    pub artifact_type: i64,
    pub variant: Option<i64>,
    pub status: i64,
    pub created_at_unix: Option<i64>,
}

impl Artifact {
    pub fn kind(&self) -> ArtifactKind {
        match self.artifact_type {
            1 => ArtifactKind::Audio,
            2 => ArtifactKind::Report,
            3 => ArtifactKind::Video,
            4 => match self.variant {
                Some(1) => ArtifactKind::Flashcards,
                Some(2) => ArtifactKind::Quiz,
                _ => ArtifactKind::Unknown,
            },
            5 => ArtifactKind::MindMap,
            7 => ArtifactKind::Infographic,
            8 => ArtifactKind::SlideDeck,
            9 => ArtifactKind::DataTable,
            _ => ArtifactKind::Unknown,
        }
    }

    pub fn status_str(&self) -> &'static str {
        artifact_status_to_str(self.status)
    }

    pub fn from_api_response(data: &Value) -> Option<Self> {
        let arr = data.as_array()?;
        let id = arr
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let title = arr
            .get(1)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let artifact_type = arr.get(2).and_then(Value::as_i64).unwrap_or(0);
        let status = arr.get(4).and_then(Value::as_i64).unwrap_or(0);
        let created_at_unix = arr
            .get(15)
            .and_then(Value::as_array)
            .and_then(|m| m.first())
            .and_then(Value::as_i64);
        let variant = arr
            .get(9)
            .and_then(Value::as_array)
            .and_then(|x| x.get(1))
            .and_then(Value::as_array)
            .and_then(|x| x.first())
            .and_then(Value::as_i64);

        Some(Self {
            id,
            title,
            artifact_type,
            variant,
            status,
            created_at_unix,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStatus {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchStartResult {
    pub task_id: String,
    pub report_id: Option<String>,
    pub notebook_id: String,
    pub query: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPollResult {
    pub task_id: Option<String>,
    pub status: String,
    pub query: String,
    pub sources: Vec<ResearchSource>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchImportedSource {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResearchSourceType {
    Web,
    Drive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResearchMode {
    Fast,
    Deep,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(i64)]
pub enum ShareAccess {
    Restricted = 0,
    AnyoneWithLink = 1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(i64)]
pub enum ShareViewLevel {
    FullNotebook = 0,
    ChatOnly = 1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(i64)]
pub enum SharePermission {
    Owner = 1,
    Editor = 2,
    Viewer = 3,
    Remove = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedUser {
    pub email: String,
    pub permission: SharePermission,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareStatus {
    pub notebook_id: String,
    pub is_public: bool,
    pub access: ShareAccess,
    pub view_level: ShareViewLevel,
    pub shared_users: Vec<SharedUser>,
    pub share_url: Option<String>,
}

impl GenerationStatus {
    pub fn is_complete(&self) -> bool {
        self.status == "completed"
    }

    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }
}

pub fn extract_notebook_summary(data: &Value) -> String {
    let arr = match data.as_array() {
        Some(a) => a,
        None => return String::new(),
    };

    if let Some(Value::String(s)) = arr.first() {
        return s.clone();
    }

    if let Some(Value::Array(summary_arr)) = arr.first() {
        return summary_arr
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }

    String::new()
}

pub fn artifact_status_to_str(status_code: i64) -> &'static str {
    match status_code {
        1 => "in_progress",
        2 => "pending",
        3 => "completed",
        4 => "failed",
        _ => "unknown",
    }
}

pub fn extract_fulltext_content(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::String(s) if !s.is_empty() => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                extract_fulltext_content(item, out);
            }
        }
        _ => {}
    }
}
