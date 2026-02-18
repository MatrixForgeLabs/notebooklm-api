use clap::{Parser, Subcommand, ValueEnum};
use notebooklm_api::Result;
use notebooklm_api::client::NotebookLmClient;
use notebooklm_api::{Artifact, Notebook, ResearchSource, Source};
use notebooklm_api::{
    ArtifactExportType, AudioFormat, AudioGenerationOptions, AudioLength,
    DataTableGenerationOptions, FlashcardsGenerationOptions, InfographicDetail,
    InfographicGenerationOptions, InfographicOrientation, InteractiveOutputFormat,
    MindMapGenerationOptions, MindMapOutputFormat, QuizDifficulty, QuizGenerationOptions,
    QuizQuantity, ReportFormat, ReportGenerationOptions, ResearchMode, ResearchSourceType,
    SharePermission, ShareViewLevel, SlideDeckFormat, SlideDeckGenerationOptions, SlideDeckLength,
    VideoFormat, VideoGenerationOptions, VideoStyle,
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "notebooklm")]
#[command(about = "NotebookLM CLI (Rust)")]
struct Cli {
    #[arg(long, global = true)]
    storage: Option<PathBuf>,

    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(long, value_enum, global = true, default_value_t = CliOutputMode::Tsv)]
    output: CliOutputMode,

    #[arg(long, global = true, default_value_t = false)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    AuthStatus,
    Notebook {
        #[command(subcommand)]
        command: NotebookCommands,
    },
    Source {
        notebook_id: String,
        #[command(subcommand)]
        command: SourceCommands,
    },
    Artifact {
        notebook_id: String,
        #[command(subcommand)]
        command: ArtifactCommands,
    },
    Research {
        notebook_id: String,
        #[command(subcommand)]
        command: ResearchCommands,
    },
    Settings {
        #[command(subcommand)]
        command: SettingsCommands,
    },
    Share {
        notebook_id: String,
        #[command(subcommand)]
        command: ShareCommands,
    },
    Chat {
        notebook_id: String,
        question: String,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long)]
        source_id: Vec<String>,
    },
    ChatHistory {
        notebook_id: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    List,
    Create {
        title: String,
    },
}

#[derive(Debug, Subcommand)]
enum NotebookCommands {
    List,
    Create {
        title: String,
    },
    Get {
        notebook_id: String,
    },
    Rename {
        notebook_id: String,
        new_title: String,
    },
    Delete {
        notebook_id: String,
    },
    Summary {
        notebook_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum SourceCommands {
    List,
    Get {
        source_id: String,
    },
    AddUrl {
        url: String,
    },
    Rename {
        source_id: String,
        new_title: String,
    },
    Delete {
        source_id: String,
    },
    Refresh {
        source_id: String,
    },
    Fulltext {
        source_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ArtifactCommands {
    List,
    Get {
        artifact_id: String,
    },
    GenerateAudio {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        audio_format: Option<CliAudioFormat>,
        #[arg(long, value_enum)]
        audio_length: Option<CliAudioLength>,
    },
    GenerateVideo {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        video_format: Option<CliVideoFormat>,
        #[arg(long, value_enum)]
        video_style: Option<CliVideoStyle>,
    },
    GenerateReport {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long, value_enum, default_value_t = CliReportFormat::BriefingDoc)]
        report_format: CliReportFormat,
        #[arg(long)]
        custom_prompt: Option<String>,
    },
    GenerateQuiz {
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        quantity: Option<CliQuizQuantity>,
        #[arg(long, value_enum)]
        difficulty: Option<CliQuizDifficulty>,
    },
    GenerateFlashcards {
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        quantity: Option<CliQuizQuantity>,
        #[arg(long, value_enum)]
        difficulty: Option<CliQuizDifficulty>,
    },
    GenerateInfographic {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        orientation: Option<CliInfographicOrientation>,
        #[arg(long, value_enum)]
        detail_level: Option<CliInfographicDetail>,
    },
    GenerateSlideDeck {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        instructions: Option<String>,
        #[arg(long, value_enum)]
        slide_format: Option<CliSlideDeckFormat>,
        #[arg(long, value_enum)]
        slide_length: Option<CliSlideDeckLength>,
    },
    GenerateDataTable {
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        instructions: Option<String>,
    },
    GenerateMindMap,
    DownloadAudio {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadVideo {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadInfographic {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadSlideDeck {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadReport {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadDataTable {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
    },
    DownloadQuiz {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long, value_enum, default_value_t = CliInteractiveOutputFormat::Json)]
        format: CliInteractiveOutputFormat,
    },
    DownloadFlashcards {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long, value_enum, default_value_t = CliInteractiveOutputFormat::Json)]
        format: CliInteractiveOutputFormat,
    },
    DownloadMindMap {
        output_path: String,
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long, value_enum, default_value_t = CliMindMapOutputFormat::PrettyJson)]
        format: CliMindMapOutputFormat,
    },
    ExportReport {
        artifact_id: String,
        #[arg(long, default_value = "Export")]
        title: String,
    },
    ExportDataTable {
        artifact_id: String,
        #[arg(long, default_value = "Export")]
        title: String,
    },
    Export {
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long)]
        content: Option<String>,
        #[arg(long, default_value = "Export")]
        title: String,
        #[arg(long, value_enum, default_value_t = CliArtifactExportType::Report)]
        export_type: CliArtifactExportType,
    },
    Poll {
        task_id: String,
    },
    Wait {
        task_id: String,
        #[arg(long, default_value_t = 300.0)]
        timeout: f64,
    },
    Rename {
        artifact_id: String,
        new_title: String,
    },
    Delete {
        artifact_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ResearchCommands {
    Start {
        query: String,
        #[arg(long, value_enum, default_value_t = CliResearchSourceType::Web)]
        source: CliResearchSourceType,
        #[arg(long, value_enum, default_value_t = CliResearchMode::Fast)]
        mode: CliResearchMode,
    },
    Poll,
    Import {
        task_id: String,
        #[arg(long = "url", required = true)]
        urls: Vec<String>,
        #[arg(long = "title")]
        titles: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SettingsCommands {
    GetLanguage,
    SetLanguage { language: String },
}

#[derive(Debug, Subcommand)]
enum ShareCommands {
    Status,
    SetPublic {
        public: bool,
    },
    SetViewLevel {
        #[arg(value_enum, default_value_t = CliShareViewLevel::FullNotebook)]
        level: CliShareViewLevel,
    },
    AddUser {
        email: String,
        #[arg(value_enum, default_value_t = CliSharePermission::Viewer)]
        permission: CliSharePermission,
        #[arg(long, default_value_t = true)]
        notify: bool,
        #[arg(long, default_value = "")]
        welcome_message: String,
    },
    UpdateUser {
        email: String,
        #[arg(value_enum, default_value_t = CliSharePermission::Viewer)]
        permission: CliSharePermission,
    },
    RemoveUser {
        email: String,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum CliShareViewLevel {
    FullNotebook,
    ChatOnly,
}

impl From<CliShareViewLevel> for ShareViewLevel {
    fn from(value: CliShareViewLevel) -> Self {
        match value {
            CliShareViewLevel::FullNotebook => ShareViewLevel::FullNotebook,
            CliShareViewLevel::ChatOnly => ShareViewLevel::ChatOnly,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CliSharePermission {
    Editor,
    Viewer,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliOutputMode {
    Json,
    Table,
    Tsv,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliInteractiveOutputFormat {
    Json,
    Markdown,
    Html,
}

impl From<CliInteractiveOutputFormat> for InteractiveOutputFormat {
    fn from(value: CliInteractiveOutputFormat) -> Self {
        match value {
            CliInteractiveOutputFormat::Json => InteractiveOutputFormat::Json,
            CliInteractiveOutputFormat::Markdown => InteractiveOutputFormat::Markdown,
            CliInteractiveOutputFormat::Html => InteractiveOutputFormat::Html,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CliArtifactExportType {
    Report,
    DataTable,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliAudioFormat {
    DeepDive,
    Brief,
    Critique,
    Debate,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliAudioLength {
    Short,
    Default,
    Long,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliVideoFormat {
    Explainer,
    Brief,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliVideoStyle {
    AutoSelect,
    Custom,
    Classic,
    Whiteboard,
    Kawaii,
    Anime,
    Watercolor,
    RetroPrint,
    Heritage,
    PaperCraft,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliReportFormat {
    BriefingDoc,
    StudyGuide,
    BlogPost,
    Custom,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliQuizQuantity {
    Fewer,
    Standard,
    More,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliQuizDifficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliInfographicOrientation {
    Landscape,
    Portrait,
    Square,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliInfographicDetail {
    Concise,
    Standard,
    Detailed,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliSlideDeckFormat {
    DetailedDeck,
    PresenterSlides,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliSlideDeckLength {
    Default,
    Short,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliMindMapOutputFormat {
    Json,
    PrettyJson,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliResearchSourceType {
    Web,
    Drive,
}

impl From<CliResearchSourceType> for ResearchSourceType {
    fn from(value: CliResearchSourceType) -> Self {
        match value {
            CliResearchSourceType::Web => ResearchSourceType::Web,
            CliResearchSourceType::Drive => ResearchSourceType::Drive,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CliResearchMode {
    Fast,
    Deep,
}

impl From<CliResearchMode> for ResearchMode {
    fn from(value: CliResearchMode) -> Self {
        match value {
            CliResearchMode::Fast => ResearchMode::Fast,
            CliResearchMode::Deep => ResearchMode::Deep,
        }
    }
}

impl From<CliArtifactExportType> for ArtifactExportType {
    fn from(value: CliArtifactExportType) -> Self {
        match value {
            CliArtifactExportType::Report => ArtifactExportType::Report,
            CliArtifactExportType::DataTable => ArtifactExportType::DataTable,
        }
    }
}

impl From<CliAudioFormat> for AudioFormat {
    fn from(value: CliAudioFormat) -> Self {
        match value {
            CliAudioFormat::DeepDive => AudioFormat::DeepDive,
            CliAudioFormat::Brief => AudioFormat::Brief,
            CliAudioFormat::Critique => AudioFormat::Critique,
            CliAudioFormat::Debate => AudioFormat::Debate,
        }
    }
}

impl From<CliAudioLength> for AudioLength {
    fn from(value: CliAudioLength) -> Self {
        match value {
            CliAudioLength::Short => AudioLength::Short,
            CliAudioLength::Default => AudioLength::Default,
            CliAudioLength::Long => AudioLength::Long,
        }
    }
}

impl From<CliVideoFormat> for VideoFormat {
    fn from(value: CliVideoFormat) -> Self {
        match value {
            CliVideoFormat::Explainer => VideoFormat::Explainer,
            CliVideoFormat::Brief => VideoFormat::Brief,
        }
    }
}

impl From<CliVideoStyle> for VideoStyle {
    fn from(value: CliVideoStyle) -> Self {
        match value {
            CliVideoStyle::AutoSelect => VideoStyle::AutoSelect,
            CliVideoStyle::Custom => VideoStyle::Custom,
            CliVideoStyle::Classic => VideoStyle::Classic,
            CliVideoStyle::Whiteboard => VideoStyle::Whiteboard,
            CliVideoStyle::Kawaii => VideoStyle::Kawaii,
            CliVideoStyle::Anime => VideoStyle::Anime,
            CliVideoStyle::Watercolor => VideoStyle::Watercolor,
            CliVideoStyle::RetroPrint => VideoStyle::RetroPrint,
            CliVideoStyle::Heritage => VideoStyle::Heritage,
            CliVideoStyle::PaperCraft => VideoStyle::PaperCraft,
        }
    }
}

impl From<CliReportFormat> for ReportFormat {
    fn from(value: CliReportFormat) -> Self {
        match value {
            CliReportFormat::BriefingDoc => ReportFormat::BriefingDoc,
            CliReportFormat::StudyGuide => ReportFormat::StudyGuide,
            CliReportFormat::BlogPost => ReportFormat::BlogPost,
            CliReportFormat::Custom => ReportFormat::Custom,
        }
    }
}

impl From<CliQuizQuantity> for QuizQuantity {
    fn from(value: CliQuizQuantity) -> Self {
        match value {
            CliQuizQuantity::Fewer => QuizQuantity::Fewer,
            CliQuizQuantity::Standard => QuizQuantity::Standard,
            CliQuizQuantity::More => QuizQuantity::More,
        }
    }
}

impl From<CliQuizDifficulty> for QuizDifficulty {
    fn from(value: CliQuizDifficulty) -> Self {
        match value {
            CliQuizDifficulty::Easy => QuizDifficulty::Easy,
            CliQuizDifficulty::Medium => QuizDifficulty::Medium,
            CliQuizDifficulty::Hard => QuizDifficulty::Hard,
        }
    }
}

impl From<CliInfographicOrientation> for InfographicOrientation {
    fn from(value: CliInfographicOrientation) -> Self {
        match value {
            CliInfographicOrientation::Landscape => InfographicOrientation::Landscape,
            CliInfographicOrientation::Portrait => InfographicOrientation::Portrait,
            CliInfographicOrientation::Square => InfographicOrientation::Square,
        }
    }
}

impl From<CliInfographicDetail> for InfographicDetail {
    fn from(value: CliInfographicDetail) -> Self {
        match value {
            CliInfographicDetail::Concise => InfographicDetail::Concise,
            CliInfographicDetail::Standard => InfographicDetail::Standard,
            CliInfographicDetail::Detailed => InfographicDetail::Detailed,
        }
    }
}

impl From<CliSlideDeckFormat> for SlideDeckFormat {
    fn from(value: CliSlideDeckFormat) -> Self {
        match value {
            CliSlideDeckFormat::DetailedDeck => SlideDeckFormat::DetailedDeck,
            CliSlideDeckFormat::PresenterSlides => SlideDeckFormat::PresenterSlides,
        }
    }
}

impl From<CliSlideDeckLength> for SlideDeckLength {
    fn from(value: CliSlideDeckLength) -> Self {
        match value {
            CliSlideDeckLength::Default => SlideDeckLength::Default,
            CliSlideDeckLength::Short => SlideDeckLength::Short,
        }
    }
}

impl From<CliMindMapOutputFormat> for MindMapOutputFormat {
    fn from(value: CliMindMapOutputFormat) -> Self {
        match value {
            CliMindMapOutputFormat::Json => MindMapOutputFormat::Json,
            CliMindMapOutputFormat::PrettyJson => MindMapOutputFormat::PrettyJson,
        }
    }
}

impl From<CliSharePermission> for SharePermission {
    fn from(value: CliSharePermission) -> Self {
        match value {
            CliSharePermission::Editor => SharePermission::Editor,
            CliSharePermission::Viewer => SharePermission::Viewer,
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let storage = cli.storage.as_deref();
    let output_mode = cli.output.clone();
    let quiet = cli.quiet;

    match cli.command {
        Commands::AuthStatus => {
            let client = NotebookLmClient::from_storage(storage).await?;
            println!("authenticated: true");
            let sid = client.auth().session_id.as_str();
            let sid_prefix = sid.chars().take(12).collect::<String>();
            println!("session id prefix: {sid_prefix}");
        }
        Commands::Notebook { command } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                NotebookCommands::List => {
                    let notebooks = client.notebooks().list().await?;
                    match &output_mode {
                        CliOutputMode::Json => {
                            println!("{}", serde_json::to_string_pretty(&notebooks)?)
                        }
                        CliOutputMode::Table | CliOutputMode::Tsv => {
                            print_notebooks_tsv(&notebooks)
                        }
                    }
                }
                NotebookCommands::Create { title } => {
                    let notebook = client.notebooks().create(&title).await?;
                    print_single_notebook(&notebook, &output_mode);
                }
                NotebookCommands::Get { notebook_id } => {
                    let notebook = client.notebooks().get(&notebook_id).await?;
                    print_single_notebook(&notebook, &output_mode);
                }
                NotebookCommands::Rename {
                    notebook_id,
                    new_title,
                } => {
                    let notebook = client.notebooks().rename(&notebook_id, &new_title).await?;
                    print_single_notebook(&notebook, &output_mode);
                }
                NotebookCommands::Delete { notebook_id } => {
                    client.notebooks().delete(&notebook_id).await?;
                    if !quiet {
                        println!("deleted\t{notebook_id}");
                    }
                }
                NotebookCommands::Summary { notebook_id } => {
                    let summary = client.notebooks().get_summary(&notebook_id).await?;
                    println!("{summary}");
                }
            }
        }
        Commands::Source {
            notebook_id,
            command,
        } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                SourceCommands::List => {
                    let sources = client.sources().list(&notebook_id).await?;
                    match &output_mode {
                        CliOutputMode::Json => {
                            println!("{}", serde_json::to_string_pretty(&sources)?)
                        }
                        CliOutputMode::Table | CliOutputMode::Tsv => print_sources_tsv(&sources),
                    }
                }
                SourceCommands::Get { source_id } => {
                    let source = client.sources().get(&notebook_id, &source_id).await?;
                    if let Some(src) = source {
                        println!("{}\t{}", src.id, src.title.unwrap_or_default());
                    } else {
                        println!("not found\t{source_id}");
                    }
                }
                SourceCommands::AddUrl { url } => {
                    let source = client.sources().add_url(&notebook_id, &url).await?;
                    println!("{}\t{}", source.id, source.title.unwrap_or_default());
                }
                SourceCommands::Rename {
                    source_id,
                    new_title,
                } => {
                    let source = client
                        .sources()
                        .rename(&notebook_id, &source_id, &new_title)
                        .await?;
                    println!("{}\t{}", source.id, source.title.unwrap_or_default());
                }
                SourceCommands::Delete { source_id } => {
                    client.sources().delete(&notebook_id, &source_id).await?;
                    println!("deleted\t{source_id}");
                }
                SourceCommands::Refresh { source_id } => {
                    client.sources().refresh(&notebook_id, &source_id).await?;
                    println!("refreshed\t{source_id}");
                }
                SourceCommands::Fulltext { source_id } => {
                    let fulltext = client
                        .sources()
                        .get_fulltext(&notebook_id, &source_id)
                        .await?;
                    println!("{}\t{}", fulltext.source_id, fulltext.char_count);
                    println!("{}", fulltext.content);
                }
            }
        }
        Commands::Artifact {
            notebook_id,
            command,
        } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                ArtifactCommands::List => {
                    let artifacts = client.artifacts().list(&notebook_id, None).await?;
                    match &output_mode {
                        CliOutputMode::Json => {
                            println!("{}", serde_json::to_string_pretty(&artifacts)?)
                        }
                        CliOutputMode::Table | CliOutputMode::Tsv => {
                            print_artifacts_tsv(&artifacts)
                        }
                    }
                }
                ArtifactCommands::Get { artifact_id } => {
                    let artifact = client.artifacts().get(&notebook_id, &artifact_id).await?;
                    if let Some(art) = artifact {
                        println!("{}\t{}\t{}", art.id, art.title, art.status_str());
                    } else {
                        println!("not found\t{artifact_id}");
                    }
                }
                ArtifactCommands::GenerateAudio {
                    language,
                    instructions,
                    audio_format,
                    audio_length,
                } => {
                    let status = client
                        .artifacts()
                        .generate_audio(
                            &notebook_id,
                            AudioGenerationOptions {
                                source_ids: None,
                                language,
                                instructions,
                                format: audio_format.map(Into::into),
                                length: audio_length.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateVideo {
                    language,
                    instructions,
                    video_format,
                    video_style,
                } => {
                    let status = client
                        .artifacts()
                        .generate_video(
                            &notebook_id,
                            VideoGenerationOptions {
                                source_ids: None,
                                language,
                                instructions,
                                format: video_format.map(Into::into),
                                style: video_style.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateReport {
                    language,
                    report_format,
                    custom_prompt,
                } => {
                    let status = client
                        .artifacts()
                        .generate_report(
                            &notebook_id,
                            ReportGenerationOptions {
                                source_ids: None,
                                language,
                                format: report_format.into(),
                                custom_prompt,
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateQuiz {
                    instructions,
                    quantity,
                    difficulty,
                } => {
                    let status = client
                        .artifacts()
                        .generate_quiz(
                            &notebook_id,
                            QuizGenerationOptions {
                                source_ids: None,
                                instructions,
                                quantity: quantity.map(Into::into),
                                difficulty: difficulty.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateFlashcards {
                    instructions,
                    quantity,
                    difficulty,
                } => {
                    let status = client
                        .artifacts()
                        .generate_flashcards(
                            &notebook_id,
                            FlashcardsGenerationOptions {
                                source_ids: None,
                                instructions,
                                quantity: quantity.map(Into::into),
                                difficulty: difficulty.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateInfographic {
                    language,
                    instructions,
                    orientation,
                    detail_level,
                } => {
                    let status = client
                        .artifacts()
                        .generate_infographic(
                            &notebook_id,
                            InfographicGenerationOptions {
                                source_ids: None,
                                language,
                                instructions,
                                orientation: orientation.map(Into::into),
                                detail_level: detail_level.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateSlideDeck {
                    language,
                    instructions,
                    slide_format,
                    slide_length,
                } => {
                    let status = client
                        .artifacts()
                        .generate_slide_deck(
                            &notebook_id,
                            SlideDeckGenerationOptions {
                                source_ids: None,
                                language,
                                instructions,
                                format: slide_format.map(Into::into),
                                length: slide_length.map(Into::into),
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateDataTable {
                    language,
                    instructions,
                } => {
                    let status = client
                        .artifacts()
                        .generate_data_table(
                            &notebook_id,
                            DataTableGenerationOptions {
                                source_ids: None,
                                language,
                                instructions,
                            },
                        )
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::GenerateMindMap => {
                    let result = client
                        .artifacts()
                        .generate_mind_map(&notebook_id, MindMapGenerationOptions::default())
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                ArtifactCommands::DownloadAudio {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_audio(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadVideo {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_video(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadInfographic {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_infographic(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadSlideDeck {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_slide_deck(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadReport {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_report(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadDataTable {
                    output_path,
                    artifact_id,
                } => {
                    let path = client
                        .artifacts()
                        .download_data_table(&notebook_id, &output_path, artifact_id.as_deref())
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadQuiz {
                    output_path,
                    artifact_id,
                    format,
                } => {
                    let path = client
                        .artifacts()
                        .download_quiz(
                            &notebook_id,
                            &output_path,
                            artifact_id.as_deref(),
                            format.into(),
                        )
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadFlashcards {
                    output_path,
                    artifact_id,
                    format,
                } => {
                    let path = client
                        .artifacts()
                        .download_flashcards(
                            &notebook_id,
                            &output_path,
                            artifact_id.as_deref(),
                            format.into(),
                        )
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::DownloadMindMap {
                    output_path,
                    artifact_id,
                    format,
                } => {
                    let path = client
                        .artifacts()
                        .download_mind_map_with_format(
                            &notebook_id,
                            &output_path,
                            artifact_id.as_deref(),
                            format.into(),
                        )
                        .await?;
                    println!("{path}");
                }
                ArtifactCommands::ExportReport { artifact_id, title } => {
                    let result = client
                        .artifacts()
                        .export_report(&notebook_id, &artifact_id, &title)
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                ArtifactCommands::ExportDataTable { artifact_id, title } => {
                    let result = client
                        .artifacts()
                        .export_data_table(&notebook_id, &artifact_id, &title)
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                ArtifactCommands::Export {
                    artifact_id,
                    content,
                    title,
                    export_type,
                } => {
                    let result = client
                        .artifacts()
                        .export(
                            &notebook_id,
                            artifact_id.as_deref(),
                            content.as_deref(),
                            &title,
                            export_type.into(),
                        )
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                ArtifactCommands::Poll { task_id } => {
                    let status = client
                        .artifacts()
                        .poll_status(&notebook_id, &task_id)
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::Wait { task_id, timeout } => {
                    let status = client
                        .artifacts()
                        .wait_for_completion(&notebook_id, &task_id, timeout)
                        .await?;
                    println!("task_id\t{}", status.task_id);
                    println!("status\t{}", status.status);
                }
                ArtifactCommands::Rename {
                    artifact_id,
                    new_title,
                } => {
                    client
                        .artifacts()
                        .rename(&notebook_id, &artifact_id, &new_title)
                        .await?;
                    println!("renamed\t{artifact_id}");
                }
                ArtifactCommands::Delete { artifact_id } => {
                    client
                        .artifacts()
                        .delete(&notebook_id, &artifact_id)
                        .await?;
                    println!("deleted\t{artifact_id}");
                }
            }
        }
        Commands::Research {
            notebook_id,
            command,
        } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                ResearchCommands::Start {
                    query,
                    source,
                    mode,
                } => {
                    let started = client
                        .research()
                        .start(&notebook_id, &query, source.into(), mode.into())
                        .await?;
                    if let Some(task) = started {
                        println!("{}", serde_json::to_string_pretty(&task)?);
                    } else {
                        println!("null");
                    }
                }
                ResearchCommands::Poll => {
                    let polled = client.research().poll(&notebook_id).await?;
                    println!("{}", serde_json::to_string_pretty(&polled)?);
                }
                ResearchCommands::Import {
                    task_id,
                    urls,
                    titles,
                } => {
                    let sources: Vec<ResearchSource> = urls
                        .iter()
                        .enumerate()
                        .map(|(i, url)| ResearchSource {
                            url: url.clone(),
                            title: titles
                                .get(i)
                                .cloned()
                                .unwrap_or_else(|| "Untitled".to_string()),
                        })
                        .collect();

                    let imported = client
                        .research()
                        .import_sources(&notebook_id, &task_id, &sources)
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&imported)?);
                }
            }
        }
        Commands::Settings { command } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                SettingsCommands::GetLanguage => {
                    let language = client.settings().get_output_language().await?;
                    match language {
                        Some(lang) => println!("{lang}"),
                        None => println!("null"),
                    }
                }
                SettingsCommands::SetLanguage { language } => {
                    let updated = client.settings().set_output_language(&language).await?;
                    match updated {
                        Some(lang) => println!("{lang}"),
                        None => println!("null"),
                    }
                }
            }
        }
        Commands::Share {
            notebook_id,
            command,
        } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            match command {
                ShareCommands::Status => {
                    let status = client.sharing().get_status(&notebook_id).await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                ShareCommands::SetPublic { public } => {
                    let status = client.sharing().set_public(&notebook_id, public).await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                ShareCommands::SetViewLevel { level } => {
                    let status = client
                        .sharing()
                        .set_view_level(&notebook_id, level.into())
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                ShareCommands::AddUser {
                    email,
                    permission,
                    notify,
                    welcome_message,
                } => {
                    let status = client
                        .sharing()
                        .add_user(
                            &notebook_id,
                            &email,
                            permission.into(),
                            notify,
                            &welcome_message,
                        )
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                ShareCommands::UpdateUser { email, permission } => {
                    let status = client
                        .sharing()
                        .update_user(&notebook_id, &email, permission.into())
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                ShareCommands::RemoveUser { email } => {
                    let status = client.sharing().remove_user(&notebook_id, &email).await?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
            }
        }
        Commands::Chat {
            notebook_id,
            question,
            conversation_id,
            source_id,
        } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            let source_ids = if source_id.is_empty() {
                None
            } else {
                Some(source_id)
            };
            let result = client
                .chat()
                .ask(&notebook_id, &question, source_ids, conversation_id)
                .await?;
            println!("conversation_id\t{}", result.conversation_id);
            println!("turn\t{}", result.turn_number);
            println!();
            println!("{}", result.answer);
        }
        Commands::ChatHistory { notebook_id, limit } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            let history = client.chat().get_history(&notebook_id, limit).await?;
            println!("{}", serde_json::to_string_pretty(&history)?);
        }
        Commands::List => {
            let client = NotebookLmClient::from_storage(storage).await?;
            let notebooks = client.notebooks().list().await?;
            match &output_mode {
                CliOutputMode::Json => println!("{}", serde_json::to_string_pretty(&notebooks)?),
                CliOutputMode::Table | CliOutputMode::Tsv => print_notebooks_tsv(&notebooks),
            }
        }
        Commands::Create { title } => {
            let client = NotebookLmClient::from_storage(storage).await?;
            let notebook = client.notebooks().create(&title).await?;
            print_single_notebook(&notebook, &output_mode);
        }
    }

    Ok(())
}

fn print_notebooks_tsv(notebooks: &[Notebook]) {
    for nb in notebooks {
        println!("{}\t{}", nb.id, nb.title);
    }
}

fn print_sources_tsv(sources: &[Source]) {
    for src in sources {
        println!(
            "{}\t{}\t{}",
            src.id,
            src.title.clone().unwrap_or_default(),
            src.status
        );
    }
}

fn print_artifacts_tsv(artifacts: &[Artifact]) {
    for art in artifacts {
        println!("{}\t{}\t{}", art.id, art.title, art.status_str());
    }
}

fn print_single_notebook(notebook: &Notebook, output_mode: &CliOutputMode) {
    match output_mode {
        CliOutputMode::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(notebook).unwrap_or_else(|_| "null".to_string())
            );
        }
        CliOutputMode::Table | CliOutputMode::Tsv => {
            println!("{}\t{}", notebook.id, notebook.title)
        }
    }
}
