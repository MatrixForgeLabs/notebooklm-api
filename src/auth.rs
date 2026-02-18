use crate::error::{NotebookLmError, Result};
use regex::Regex;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const NOTEBOOKLM_HOME_URL: &str = "https://notebooklm.google.com/";

#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub cookies: HashMap<String, String>,
    pub csrf_token: String,
    pub session_id: String,
}

impl AuthTokens {
    pub async fn from_storage(path: Option<&Path>) -> Result<Self> {
        let cookies = load_auth_from_storage(path)?;
        let (csrf_token, session_id) = fetch_tokens(&cookies).await?;
        Ok(Self {
            cookies,
            csrf_token,
            session_id,
        })
    }

    pub fn cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Debug, Deserialize)]
struct StorageState {
    cookies: Vec<StorageCookie>,
}

#[derive(Debug, Deserialize)]
struct StorageCookie {
    name: String,
    value: String,
    domain: String,
}

pub fn default_storage_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        NotebookLmError::Config("failed to resolve home directory for storage path".to_string())
    })?;
    Ok(home.join(".notebooklm").join("storage_state.json"))
}

pub fn load_auth_from_storage(path: Option<&Path>) -> Result<HashMap<String, String>> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => default_storage_path()?,
    };

    if !path.exists() {
        return Err(NotebookLmError::Config(format!(
            "storage_state.json not found at {}",
            path.display()
        )));
    }

    let content = std::fs::read_to_string(&path)?;
    let state: StorageState = serde_json::from_str(&content)?;

    let mut cookies = HashMap::new();
    for cookie in state.cookies {
        if is_allowed_auth_domain(&cookie.domain) {
            cookies.insert(cookie.name, cookie.value);
        }
    }

    if !cookies.contains_key("SID") {
        return Err(NotebookLmError::Auth(
            "missing SID cookie in storage state; run `notebooklm login`".to_string(),
        ));
    }

    Ok(cookies)
}

fn is_allowed_auth_domain(domain: &str) -> bool {
    domain == ".google.com"
        || domain == "notebooklm.google.com"
        || domain == ".googleusercontent.com"
        || domain.starts_with(".google.")
}

pub async fn fetch_tokens(cookies: &HashMap<String, String>) -> Result<(String, String)> {
    let cookie_header = cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ");

    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&cookie_header)
            .map_err(|e| NotebookLmError::Auth(format!("invalid cookie header: {e}")))?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let response = client.get(NOTEBOOKLM_HOME_URL).send().await?;

    let final_url = response.url().as_str().to_string();
    if final_url.contains("accounts.google.com") {
        return Err(NotebookLmError::Auth(
            "authentication expired; run `notebooklm login`".to_string(),
        ));
    }

    let body = response.text().await?;
    let csrf_re = Regex::new(r#"\"SNlM0e\":\"([^\"]+)\""#)
        .map_err(|e| NotebookLmError::RpcDecode(e.to_string()))?;
    let sid_re = Regex::new(r#"\"FdrFJe\":\"([^\"]+)\""#)
        .map_err(|e| NotebookLmError::RpcDecode(e.to_string()))?;

    let csrf_token = csrf_re
        .captures(&body)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| {
            NotebookLmError::Auth("failed to extract CSRF token (SNlM0e)".to_string())
        })?;

    let session_id = sid_re
        .captures(&body)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| {
            NotebookLmError::Auth("failed to extract session id (FdrFJe)".to_string())
        })?;

    Ok((csrf_token, session_id))
}
