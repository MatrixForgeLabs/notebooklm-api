use thiserror::Error;

pub type Result<T> = std::result::Result<T, NotebookLmError>;

#[derive(Debug, Error)]
pub enum NotebookLmError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("rate limited: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<u64>,
    },

    #[error("client error {status}: {message}")]
    Client { status: u16, message: String },

    #[error("server error {status}: {message}")]
    Server { status: u16, message: String },

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("network error ({operation}): {message}")]
    Network { operation: String, message: String },

    #[error("timeout ({operation}): {message}")]
    Timeout { operation: String, message: String },

    #[error("stale auth session: {0}")]
    StaleAuth(String),

    #[error("RPC error ({method_id}): {message}")]
    Rpc {
        method_id: String,
        message: String,
        code: Option<i64>,
    },

    #[error("RPC response parse error: {0}")]
    RpcDecode(String),

    #[error("unexpected response shape at {path}: {context}")]
    DecodeShape { path: String, context: String },
}
