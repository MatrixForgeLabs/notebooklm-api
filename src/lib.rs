//! `notebooklm-api` provides a Rust library and CLI for automating Google NotebookLM.
//!
//! # What You Get
//! - Typed API for notebooks, sources, chat, artifacts, research, sharing, and settings.
//! - CLI binary (`notebooklm`) for scripting and terminal workflows.
//! - Retry/backoff support and typed error taxonomy.
//!
//! # Quick Example
//! ```no_run
//! use notebooklm_api::client::NotebookLmClient;
//! use notebooklm_api::{AudioGenerationOptions, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = NotebookLmClient::from_storage(None).await?;
//!     let notebooks = client.notebooks().list().await?;
//!     if let Some(nb) = notebooks.first() {
//!         let status = client
//!             .artifacts()
//!             .generate_audio(&nb.id, AudioGenerationOptions::default())
//!             .await?;
//!         println!("{} {}", status.task_id, status.status);
//!     }
//!     Ok(())
//! }
//! ```
#![forbid(unsafe_code)]

pub mod auth;
pub mod client;
pub mod error;
pub mod rpc;
pub mod types;

pub use auth::AuthTokens;
pub use client::{
    ArtifactsApi, ChatApi, NotebookLmClient, NotebooksApi, ResearchApi, SettingsApi, SharingApi,
    SourcesApi,
};
pub use error::{NotebookLmError, Result};
pub use types::{
    Artifact, ArtifactExportType, ArtifactKind, AskResult, AudioFormat, AudioGenerationOptions,
    AudioLength, ConversationTurn, DataTableGenerationOptions, FlashcardsGenerationOptions,
    GenerationStatus, InfographicDetail, InfographicGenerationOptions, InfographicOrientation,
    InteractiveOutputFormat, MindMapGenerationOptions, MindMapGenerationResult,
    MindMapOutputFormat, Notebook, QuizDifficulty, QuizGenerationOptions, QuizQuantity,
    ReportFormat, ReportGenerationOptions, ResearchImportedSource, ResearchMode,
    ResearchPollResult, ResearchSource, ResearchSourceType, ResearchStartResult, RetryPolicy,
    ShareAccess, SharePermission, ShareStatus, ShareViewLevel, SharedUser, SlideDeckFormat,
    SlideDeckGenerationOptions, SlideDeckLength, Source, SourceFulltext, VideoFormat,
    VideoGenerationOptions, VideoStyle,
};
