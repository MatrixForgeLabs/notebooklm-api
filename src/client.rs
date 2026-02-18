use crate::auth::{AuthTokens, default_storage_path};
use crate::error::{NotebookLmError, Result};
use crate::rpc::{
    BATCHEXECUTE_URL, QUERY_URL, RpcMethod, build_request_body, decode_response, encode_rpc_request,
};
use crate::types::{
    Artifact, ArtifactExportType, ArtifactKind, AskResult, AudioGenerationOptions,
    ConversationTurn, DataTableGenerationOptions, FlashcardsGenerationOptions, GenerationStatus,
    InfographicGenerationOptions, InteractiveOutputFormat, MindMapGenerationOptions,
    MindMapGenerationResult, MindMapOutputFormat, Notebook, QuizGenerationOptions, ReportFormat,
    ReportGenerationOptions, ResearchImportedSource, ResearchMode, ResearchPollResult,
    ResearchSource, ResearchSourceType, ResearchStartResult, RetryPolicy, ShareAccess,
    SharePermission, ShareStatus, ShareViewLevel, SharedUser, SlideDeckGenerationOptions, Source,
    SourceFulltext, VideoGenerationOptions, artifact_status_to_str, extract_fulltext_content,
    extract_notebook_summary,
};
use regex::Regex;
use reqwest::header::{CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const DEFAULT_QUERY_BL: &str = "boq_labs-tailwind-frontend_20251221.14_p0";

#[derive(Debug, Clone)]
pub struct NotebookLmClient {
    auth: AuthTokens,
    http: reqwest::Client,
    reqid_counter: Arc<AtomicU64>,
    conversation_cache: Arc<Mutex<HashMap<String, Vec<ConversationTurn>>>>,
    retry_policy: RetryPolicy,
}

impl NotebookLmClient {
    pub async fn from_storage(path: Option<&Path>) -> Result<Self> {
        let auth = AuthTokens::from_storage(path).await?;
        Self::new(auth)
    }

    pub fn new(auth: AuthTokens) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&auth.cookie_header())
                .map_err(|e| NotebookLmError::Auth(format!("invalid cookie header: {e}")))?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            auth,
            http,
            reqid_counter: Arc::new(AtomicU64::new(100_000)),
            conversation_cache: Arc::new(Mutex::new(HashMap::new())),
            retry_policy: RetryPolicy::default(),
        })
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn notebooks(&self) -> NotebooksApi<'_> {
        NotebooksApi { client: self }
    }

    pub fn sources(&self) -> SourcesApi<'_> {
        SourcesApi { client: self }
    }

    pub fn chat(&self) -> ChatApi<'_> {
        ChatApi { client: self }
    }

    pub fn artifacts(&self) -> ArtifactsApi<'_> {
        ArtifactsApi { client: self }
    }

    pub fn research(&self) -> ResearchApi<'_> {
        ResearchApi { client: self }
    }

    pub fn settings(&self) -> SettingsApi<'_> {
        SettingsApi { client: self }
    }

    pub fn sharing(&self) -> SharingApi<'_> {
        SharingApi { client: self }
    }

    pub fn auth(&self) -> &AuthTokens {
        &self.auth
    }

    pub async fn refresh_auth(&mut self) -> Result<()> {
        let path = default_storage_path()?;
        let refreshed = AuthTokens::from_storage(Some(&path)).await?;
        self.auth = refreshed;

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&self.auth.cookie_header())
                .map_err(|e| NotebookLmError::Auth(format!("invalid cookie header: {e}")))?,
        );
        self.http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(())
    }

    pub async fn rpc_call(
        &self,
        method: RpcMethod,
        params: Value,
        source_path: &str,
        allow_null: bool,
    ) -> Result<Value> {
        let rpc_request = encode_rpc_request(method, params)?;
        let mut active_auth = self.auth.clone();
        let mut active_client = self.http.clone();
        let mut auth_refreshed = false;

        for attempt in 0..=self.retry_policy.max_retries {
            let body = build_request_body(&rpc_request, &active_auth.csrf_token)?;
            let url = build_rpc_url(&active_auth, method, source_path)?;
            let send_result = active_client.post(url).body(body).send().await;

            let response = match send_result {
                Ok(resp) => resp,
                Err(err) => {
                    if should_retry_transport_error(&err) && attempt < self.retry_policy.max_retries
                    {
                        sleep_with_backoff(&self.retry_policy, attempt, None).await;
                        continue;
                    }
                    return Err(map_transport_error(method.id(), err));
                }
            };

            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let text = response.text().await?;

            if status.is_success() {
                return decode_response(&text, method.id(), allow_null);
            }

            if is_auth_status(status) && !auth_refreshed {
                let refreshed = AuthTokens::from_storage(None).await?;
                active_client = build_http_client_with_auth(&refreshed)?;
                active_auth = refreshed;
                auth_refreshed = true;
                continue;
            }

            if is_auth_status(status) && auth_refreshed {
                return Err(NotebookLmError::StaleAuth(format!(
                    "{}: authentication failed after token refresh",
                    method.id()
                )));
            }

            if is_retryable_http_status(status) && attempt < self.retry_policy.max_retries {
                sleep_with_backoff(&self.retry_policy, attempt, retry_after).await;
                continue;
            }

            return Err(map_http_error(
                method.id(),
                status,
                &text,
                retry_after.map(|v| v.to_string()).as_deref(),
            ));
        }

        Err(NotebookLmError::Rpc {
            method_id: method.id().to_string(),
            message: "retry attempts exhausted".to_string(),
            code: None,
        })
    }

    async fn query_call(&self, params: Value) -> Result<String> {
        let params_json = serde_json::to_string(&params)?;
        let f_req = serde_json::to_string(&json!([null, params_json]))?;
        let reqid = self.reqid_counter.fetch_add(100_000, Ordering::Relaxed) + 100_000;
        let reqid_str = reqid.to_string();
        let mut active_auth = self.auth.clone();
        let mut active_client = self.http.clone();
        let mut auth_refreshed = false;

        for attempt in 0..=self.retry_policy.max_retries {
            let body = format!(
                "f.req={}&at={}&",
                urlencoding::encode(&f_req),
                urlencoding::encode(&active_auth.csrf_token)
            );
            let url = build_query_url(&active_auth, reqid_str.as_str())?;
            let send_result = active_client.post(url).body(body).send().await;

            let response = match send_result {
                Ok(resp) => resp,
                Err(err) => {
                    if should_retry_transport_error(&err) && attempt < self.retry_policy.max_retries
                    {
                        sleep_with_backoff(&self.retry_policy, attempt, None).await;
                        continue;
                    }
                    return Err(map_transport_error("query", err));
                }
            };

            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let text = response.text().await?;

            if status.is_success() {
                return Ok(text);
            }

            if is_auth_status(status) && !auth_refreshed {
                let refreshed = AuthTokens::from_storage(None).await?;
                active_client = build_http_client_with_auth(&refreshed)?;
                active_auth = refreshed;
                auth_refreshed = true;
                continue;
            }

            if is_auth_status(status) && auth_refreshed {
                return Err(NotebookLmError::StaleAuth(
                    "query authentication failed after token refresh".to_string(),
                ));
            }

            if is_retryable_http_status(status) && attempt < self.retry_policy.max_retries {
                sleep_with_backoff(&self.retry_policy, attempt, retry_after).await;
                continue;
            }

            return Err(map_http_error(
                "query",
                status,
                &text,
                retry_after.map(|v| v.to_string()).as_deref(),
            ));
        }

        Err(NotebookLmError::Rpc {
            method_id: "query".to_string(),
            message: "retry attempts exhausted".to_string(),
            code: None,
        })
    }

    fn cache_conversation_turn(&self, conversation_id: &str, query: &str, answer: &str) {
        let mut cache = self
            .conversation_cache
            .lock()
            .expect("conversation cache poisoned");
        let entry = cache.entry(conversation_id.to_string()).or_default();
        entry.push(ConversationTurn {
            query: query.to_string(),
            answer: answer.to_string(),
            turn_number: entry.len() + 1,
        });
    }

    fn get_cached_conversation(&self, conversation_id: &str) -> Vec<ConversationTurn> {
        let cache = self
            .conversation_cache
            .lock()
            .expect("conversation cache poisoned");
        cache.get(conversation_id).cloned().unwrap_or_default()
    }

    fn clear_conversation_cache(&self, conversation_id: Option<&str>) -> bool {
        let mut cache = self
            .conversation_cache
            .lock()
            .expect("conversation cache poisoned");
        match conversation_id {
            Some(id) => cache.remove(id).is_some(),
            None => {
                cache.clear();
                true
            }
        }
    }

    async fn get_source_ids(&self, notebook_id: &str) -> Result<Vec<String>> {
        Ok(self
            .sources()
            .list(notebook_id)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect())
    }
}

pub struct NotebooksApi<'a> {
    client: &'a NotebookLmClient,
}

impl NotebooksApi<'_> {
    pub async fn list(&self) -> Result<Vec<Notebook>> {
        let params = json!([null, 1, null, [2]]);
        let result = self
            .client
            .rpc_call(RpcMethod::ListNotebooks, params, "/", false)
            .await?;

        let notebooks_raw = if let Some(arr) = result.as_array() {
            if arr.first().map(|v| v.is_array()).unwrap_or(false) {
                arr.first()
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            } else {
                arr.clone()
            }
        } else {
            Vec::new()
        };

        Ok(notebooks_raw
            .iter()
            .map(Notebook::from_api_response)
            .collect())
    }

    pub async fn create(&self, title: &str) -> Result<Notebook> {
        let params = json!([title, null, null, [2], [1]]);
        let result = self
            .client
            .rpc_call(RpcMethod::CreateNotebook, params, "/", false)
            .await?;

        Ok(Notebook::from_api_response(&result))
    }

    pub async fn get(&self, notebook_id: &str) -> Result<Notebook> {
        let params = json!([notebook_id, null, [2], null, 0]);
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;

        let nb_info = result
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Array(Vec::new()));

        Ok(Notebook::from_api_response(&nb_info))
    }

    pub async fn delete(&self, notebook_id: &str) -> Result<bool> {
        let params = json!([[notebook_id], [2]]);
        self.client
            .rpc_call(RpcMethod::DeleteNotebook, params, "/", false)
            .await?;
        Ok(true)
    }

    pub async fn rename(&self, notebook_id: &str, new_title: &str) -> Result<Notebook> {
        let params = json!([notebook_id, [[null, null, null, [null, new_title]]]]);
        self.client
            .rpc_call(RpcMethod::RenameNotebook, params, "/", true)
            .await?;

        self.get(notebook_id).await
    }

    pub async fn get_summary(&self, notebook_id: &str) -> Result<String> {
        let params = json!([notebook_id, [2]]);
        let result = self
            .client
            .rpc_call(
                RpcMethod::Summarize,
                params,
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;

        Ok(extract_notebook_summary(&result))
    }
}

pub struct SourcesApi<'a> {
    client: &'a NotebookLmClient,
}

impl SourcesApi<'_> {
    pub async fn list(&self, notebook_id: &str) -> Result<Vec<Source>> {
        let params = json!([notebook_id, null, [2], null, 0]);
        let notebook = self
            .client
            .rpc_call(
                RpcMethod::GetNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;

        let sources = notebook
            .as_array()
            .and_then(|outer| outer.first())
            .and_then(Value::as_array)
            .and_then(|nb| nb.get(1))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        Ok(sources
            .iter()
            .filter_map(Source::from_notebook_source)
            .collect())
    }

    pub async fn get(&self, notebook_id: &str, source_id: &str) -> Result<Option<Source>> {
        let sources = self.list(notebook_id).await?;
        Ok(sources.into_iter().find(|s| s.id == source_id))
    }

    pub async fn add_url(&self, notebook_id: &str, url: &str) -> Result<Source> {
        let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");

        let (params, allow_null) = if is_youtube {
            (
                json!([
                    [[
                        null,
                        null,
                        null,
                        null,
                        null,
                        null,
                        null,
                        [url],
                        null,
                        null,
                        1
                    ]],
                    notebook_id,
                    [2],
                    [1, null, null, null, null, null, null, null, null, null, [1]]
                ]),
                true,
            )
        } else {
            (
                json!([
                    [[null, null, [url], null, null, null, null, null]],
                    notebook_id,
                    [2],
                    null,
                    null
                ]),
                false,
            )
        };

        let result = self
            .client
            .rpc_call(
                RpcMethod::AddSource,
                params,
                &format!("/notebook/{notebook_id}"),
                allow_null,
            )
            .await?;

        Source::from_api_response(&result).ok_or_else(|| {
            NotebookLmError::RpcDecode("failed to parse source from add_url response".to_string())
        })
    }

    pub async fn delete(&self, notebook_id: &str, source_id: &str) -> Result<bool> {
        let params = json!([[[source_id]]]);
        self.client
            .rpc_call(
                RpcMethod::DeleteSource,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(true)
    }

    pub async fn rename(
        &self,
        notebook_id: &str,
        source_id: &str,
        new_title: &str,
    ) -> Result<Source> {
        let params = json!([null, [source_id], [[[new_title]]]]);
        let result = self
            .client
            .rpc_call(
                RpcMethod::UpdateSource,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        if let Some(source) = Source::from_api_response(&result) {
            return Ok(source);
        }

        Ok(Source {
            id: source_id.to_string(),
            title: Some(new_title.to_string()),
            url: None,
            type_code: None,
            created_at_unix: None,
            status: 2,
        })
    }

    pub async fn refresh(&self, notebook_id: &str, source_id: &str) -> Result<bool> {
        let params = json!([null, [source_id], [2]]);
        self.client
            .rpc_call(
                RpcMethod::RefreshSource,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(true)
    }

    pub async fn get_fulltext(&self, notebook_id: &str, source_id: &str) -> Result<SourceFulltext> {
        let params = json!([[source_id], [2], [2]]);
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetSource,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        let title = result
            .as_array()
            .and_then(|r| r.first())
            .and_then(Value::as_array)
            .and_then(|s| s.get(1))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let type_code = result
            .as_array()
            .and_then(|r| r.first())
            .and_then(Value::as_array)
            .and_then(|s| s.get(2))
            .and_then(Value::as_array)
            .and_then(|meta| meta.get(4))
            .and_then(Value::as_i64);

        let url = result
            .as_array()
            .and_then(|r| r.first())
            .and_then(Value::as_array)
            .and_then(|s| s.get(2))
            .and_then(Value::as_array)
            .and_then(|meta| meta.get(7))
            .and_then(Value::as_array)
            .and_then(|u| u.first())
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let mut texts = Vec::new();
        if let Some(content_root) = result
            .as_array()
            .and_then(|r| r.get(3))
            .and_then(Value::as_array)
            .and_then(|c| c.first())
        {
            extract_fulltext_content(content_root, &mut texts);
        }
        let content = texts.join("\n");

        Ok(SourceFulltext {
            source_id: source_id.to_string(),
            title,
            content: content.clone(),
            type_code,
            url,
            char_count: content.len(),
        })
    }
}

pub struct ChatApi<'a> {
    client: &'a NotebookLmClient,
}

pub struct ArtifactsApi<'a> {
    client: &'a NotebookLmClient,
}

pub struct ResearchApi<'a> {
    client: &'a NotebookLmClient,
}

pub struct SettingsApi<'a> {
    client: &'a NotebookLmClient,
}

pub struct SharingApi<'a> {
    client: &'a NotebookLmClient,
}

impl ChatApi<'_> {
    pub async fn ask(
        &self,
        notebook_id: &str,
        question: &str,
        source_ids: Option<Vec<String>>,
        conversation_id: Option<String>,
    ) -> Result<AskResult> {
        let source_ids = match source_ids {
            Some(ids) => ids,
            None => self
                .client
                .sources()
                .list(notebook_id)
                .await?
                .into_iter()
                .map(|s| s.id)
                .collect(),
        };

        let is_follow_up = conversation_id.is_some();
        let conversation_id = conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        let conversation_history = if is_follow_up {
            let turns = self.client.get_cached_conversation(&conversation_id);
            if turns.is_empty() {
                Value::Null
            } else {
                let mut history = Vec::with_capacity(turns.len() * 2);
                for turn in turns {
                    history.push(json!([turn.answer, null, 2]));
                    history.push(json!([turn.query, null, 1]));
                }
                Value::Array(history)
            }
        } else {
            Value::Null
        };

        let sources_array: Vec<Value> = source_ids.iter().map(|sid| json!([[sid]])).collect();
        let params = json!([
            sources_array,
            question,
            conversation_history,
            [2, null, [1]],
            conversation_id
        ]);

        let raw_response = self.client.query_call(params).await?;
        let answer = extract_chat_answer(&raw_response);

        if !answer.is_empty() {
            self.client
                .cache_conversation_turn(&conversation_id, question, &answer);
        }

        let turn_number = self.client.get_cached_conversation(&conversation_id).len();

        Ok(AskResult {
            answer,
            conversation_id,
            turn_number,
            is_follow_up,
            raw_response: raw_response.chars().take(1000).collect(),
        })
    }

    pub async fn get_history(&self, notebook_id: &str, limit: usize) -> Result<Value> {
        let params = json!([[], null, notebook_id, limit]);
        self.client
            .rpc_call(
                RpcMethod::GetConversationHistory,
                params,
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await
    }

    pub fn get_cached_turns(&self, conversation_id: &str) -> Vec<ConversationTurn> {
        self.client.get_cached_conversation(conversation_id)
    }

    pub fn clear_cache(&self, conversation_id: Option<&str>) -> bool {
        self.client.clear_conversation_cache(conversation_id)
    }
}

impl ResearchApi<'_> {
    pub async fn start(
        &self,
        notebook_id: &str,
        query: &str,
        source: ResearchSourceType,
        mode: ResearchMode,
    ) -> Result<Option<ResearchStartResult>> {
        if matches!(mode, ResearchMode::Deep) && matches!(source, ResearchSourceType::Drive) {
            return Err(NotebookLmError::Config(
                "deep research only supports web source".to_string(),
            ));
        }

        let source_type = match source {
            ResearchSourceType::Web => 1,
            ResearchSourceType::Drive => 2,
        };

        let (method, params) = if matches!(mode, ResearchMode::Fast) {
            (
                RpcMethod::StartFastResearch,
                json!([[query, source_type], null, 1, notebook_id]),
            )
        } else {
            (
                RpcMethod::StartDeepResearch,
                json!([null, [1], [query, source_type], 5, notebook_id]),
            )
        };

        let result = self
            .client
            .rpc_call(method, params, &format!("/notebook/{notebook_id}"), false)
            .await?;

        let Some(arr) = result.as_array() else {
            return Ok(None);
        };
        if arr.is_empty() {
            return Ok(None);
        }

        let task_id = arr
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if task_id.is_empty() {
            return Ok(None);
        }
        let report_id = arr.get(1).and_then(Value::as_str).map(ToString::to_string);

        Ok(Some(ResearchStartResult {
            task_id,
            report_id,
            notebook_id: notebook_id.to_string(),
            query: query.to_string(),
            mode: match mode {
                ResearchMode::Fast => "fast",
                ResearchMode::Deep => "deep",
            }
            .to_string(),
        }))
    }

    pub async fn poll(&self, notebook_id: &str) -> Result<ResearchPollResult> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::PollResearch,
                json!([null, null, notebook_id]),
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;

        let mut root = result.clone();
        if root.as_array().is_some_and(|r| !r.is_empty())
            && root
                .as_array()
                .and_then(|r| r.first())
                .is_some_and(Value::is_array)
            && root
                .as_array()
                .and_then(|r| r.first())
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty() && a.first().is_some_and(Value::is_array))
        {
            root = root
                .as_array()
                .and_then(|r| r.first())
                .cloned()
                .unwrap_or(Value::Array(Vec::new()));
        }

        let Some(tasks) = root.as_array() else {
            return Ok(ResearchPollResult {
                task_id: None,
                status: "no_research".to_string(),
                query: String::new(),
                sources: Vec::new(),
                summary: String::new(),
            });
        };

        for task in tasks {
            let Some(task_arr) = task.as_array() else {
                continue;
            };
            if task_arr.len() < 2 {
                continue;
            }

            let task_id = task_arr
                .first()
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let Some(info) = task_arr.get(1).and_then(Value::as_array) else {
                continue;
            };

            let query = info
                .get(1)
                .and_then(Value::as_array)
                .and_then(|q| q.first())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            let source_summary = info
                .get(3)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let sources_data = source_summary
                .first()
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let summary = source_summary
                .get(1)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            let mut sources = Vec::new();
            for src in sources_data {
                let Some(sarr) = src.as_array() else {
                    continue;
                };
                if sarr.len() < 2 {
                    continue;
                }
                let (url, title) = if sarr.first().is_some_and(Value::is_null) {
                    (
                        String::new(),
                        sarr.get(1)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    )
                } else {
                    (
                        sarr.first()
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        sarr.get(1)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    )
                };
                if !url.is_empty() || !title.is_empty() {
                    sources.push(ResearchSource { url, title });
                }
            }

            let status_code = info.get(4).and_then(Value::as_i64).unwrap_or(1);
            let status = if status_code == 2 {
                "completed"
            } else {
                "in_progress"
            }
            .to_string();

            return Ok(ResearchPollResult {
                task_id,
                status,
                query,
                sources,
                summary,
            });
        }

        Ok(ResearchPollResult {
            task_id: None,
            status: "no_research".to_string(),
            query: String::new(),
            sources: Vec::new(),
            summary: String::new(),
        })
    }

    pub async fn import_sources(
        &self,
        notebook_id: &str,
        task_id: &str,
        sources: &[ResearchSource],
    ) -> Result<Vec<ResearchImportedSource>> {
        let valid_sources: Vec<&ResearchSource> =
            sources.iter().filter(|s| !s.url.is_empty()).collect();
        if valid_sources.is_empty() {
            return Ok(Vec::new());
        }

        let source_array: Vec<Value> = valid_sources
            .iter()
            .map(|src| {
                json!([
                    null,
                    null,
                    [
                        src.url,
                        if src.title.is_empty() {
                            "Untitled"
                        } else {
                            &src.title
                        }
                    ],
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    2
                ])
            })
            .collect();

        let result = self
            .client
            .rpc_call(
                RpcMethod::ImportResearch,
                json!([null, [1], task_id, notebook_id, source_array]),
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;

        let mut imported = Vec::new();
        let mut root = result.clone();
        if root.as_array().is_some_and(|r| !r.is_empty())
            && root
                .as_array()
                .and_then(|r| r.first())
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty() && a.first().is_some_and(Value::is_array))
        {
            root = root
                .as_array()
                .and_then(|r| r.first())
                .cloned()
                .unwrap_or(Value::Array(Vec::new()));
        }

        if let Some(items) = root.as_array() {
            for item in items {
                let Some(arr) = item.as_array() else {
                    continue;
                };
                if arr.len() < 2 {
                    continue;
                }
                let title = arr
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let id = arr
                    .first()
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !id.is_empty() {
                    imported.push(ResearchImportedSource { id, title });
                }
            }
        }

        Ok(imported)
    }
}

impl SettingsApi<'_> {
    pub async fn get_output_language(&self) -> Result<Option<String>> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetUserSettings,
                json!([
                    null,
                    [1, null, null, null, null, null, null, null, null, null, [1]]
                ]),
                "/",
                false,
            )
            .await?;

        Ok(extract_nested_string(&result, &[0, 2, 4, 0]))
    }

    pub async fn set_output_language(&self, language: &str) -> Result<Option<String>> {
        if language.is_empty() {
            return Err(NotebookLmError::Config(
                "language cannot be empty".to_string(),
            ));
        }

        let result = self
            .client
            .rpc_call(
                RpcMethod::SetUserSettings,
                json!([[[null, [[null, null, null, null, [language]]]]]]),
                "/",
                false,
            )
            .await?;

        Ok(extract_nested_string(&result, &[2, 4, 0]))
    }
}

impl SharingApi<'_> {
    pub async fn get_status(&self, notebook_id: &str) -> Result<ShareStatus> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetShareStatus,
                json!([notebook_id, [2]]),
                &format!("/notebook/{notebook_id}"),
                false,
            )
            .await?;
        Ok(parse_share_status(&result, notebook_id, None))
    }

    pub async fn set_public(&self, notebook_id: &str, public: bool) -> Result<ShareStatus> {
        let access = if public {
            ShareAccess::AnyoneWithLink
        } else {
            ShareAccess::Restricted
        };
        let access_value = access as i64;
        let params = json!([
            [[notebook_id, null, [access_value], [access_value, ""]]],
            1,
            null,
            [2]
        ]);
        self.client
            .rpc_call(
                RpcMethod::ShareNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        self.get_status(notebook_id).await
    }

    pub async fn set_view_level(
        &self,
        notebook_id: &str,
        level: ShareViewLevel,
    ) -> Result<ShareStatus> {
        let level_value = level as i64;
        let params = json!([
            notebook_id,
            [[
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [[level_value]]
            ]]
        ]);
        self.client
            .rpc_call(
                RpcMethod::RenameNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        let mut status = self.get_status(notebook_id).await?;
        status.view_level = level;
        Ok(status)
    }

    pub async fn add_user(
        &self,
        notebook_id: &str,
        email: &str,
        permission: SharePermission,
        notify: bool,
        welcome_message: &str,
    ) -> Result<ShareStatus> {
        if matches!(permission, SharePermission::Owner) {
            return Err(NotebookLmError::Config(
                "cannot assign owner permission".to_string(),
            ));
        }
        if matches!(permission, SharePermission::Remove) {
            return Err(NotebookLmError::Config(
                "use remove_user for remove operation".to_string(),
            ));
        }

        let message_flag = if welcome_message.is_empty() { 1 } else { 0 };
        let notify_flag = if notify { 1 } else { 0 };
        let permission_value = permission as i64;
        let params = json!([
            [[
                notebook_id,
                [[email, null, permission_value]],
                null,
                [message_flag, welcome_message]
            ]],
            notify_flag,
            null,
            [2]
        ]);
        self.client
            .rpc_call(
                RpcMethod::ShareNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        self.get_status(notebook_id).await
    }

    pub async fn update_user(
        &self,
        notebook_id: &str,
        email: &str,
        permission: SharePermission,
    ) -> Result<ShareStatus> {
        self.add_user(notebook_id, email, permission, false, "")
            .await
    }

    pub async fn remove_user(&self, notebook_id: &str, email: &str) -> Result<ShareStatus> {
        let remove_permission = SharePermission::Remove as i64;
        let params = json!([
            [[
                notebook_id,
                [[email, null, remove_permission]],
                null,
                [0, ""]
            ]],
            0,
            null,
            [2]
        ]);
        self.client
            .rpc_call(
                RpcMethod::ShareNotebook,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        self.get_status(notebook_id).await
    }
}

impl ArtifactsApi<'_> {
    pub async fn list(
        &self,
        notebook_id: &str,
        artifact_kind: Option<ArtifactKind>,
    ) -> Result<Vec<Artifact>> {
        let raw = self.list_raw(notebook_id).await?;
        let mut artifacts: Vec<Artifact> =
            raw.iter().filter_map(Artifact::from_api_response).collect();

        if artifact_kind.is_none() || artifact_kind == Some(ArtifactKind::MindMap) {
            let mind_maps = self.list_raw_mind_maps(notebook_id).await?;
            artifacts.extend(mind_maps);
        }

        if let Some(kind) = artifact_kind {
            artifacts.retain(|a| a.kind() == kind);
        }

        Ok(artifacts)
    }

    pub async fn get(&self, notebook_id: &str, artifact_id: &str) -> Result<Option<Artifact>> {
        Ok(self
            .list(notebook_id, None)
            .await?
            .into_iter()
            .find(|a| a.id == artifact_id))
    }

    pub async fn generate_audio(
        &self,
        notebook_id: &str,
        options: AudioGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_audio_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_video(
        &self,
        notebook_id: &str,
        options: VideoGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_video_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_report(
        &self,
        notebook_id: &str,
        options: ReportGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_report_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_quiz(
        &self,
        notebook_id: &str,
        options: QuizGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_quiz_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_flashcards(
        &self,
        notebook_id: &str,
        options: FlashcardsGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_flashcards_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_infographic(
        &self,
        notebook_id: &str,
        options: InfographicGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_infographic_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_slide_deck(
        &self,
        notebook_id: &str,
        options: SlideDeckGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_slide_deck_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_data_table(
        &self,
        notebook_id: &str,
        options: DataTableGenerationOptions,
    ) -> Result<GenerationStatus> {
        let option_source_ids = options.source_ids.clone();
        let source_ids = self
            .resolve_source_ids(notebook_id, option_source_ids)
            .await?;
        let params = build_generate_data_table_params(notebook_id, &source_ids, &options);
        self.call_generate(notebook_id, params).await
    }

    pub async fn generate_mind_map(
        &self,
        notebook_id: &str,
        options: MindMapGenerationOptions,
    ) -> Result<MindMapGenerationResult> {
        let source_ids = self
            .resolve_source_ids(notebook_id, options.source_ids)
            .await?;
        let source_ids_nested: Vec<Value> = source_ids.iter().map(|sid| json!([[sid]])).collect();

        let params = json!([
            source_ids_nested,
            null,
            null,
            null,
            null,
            ["interactive_mindmap", [["[CONTEXT]", ""]], ""],
            null,
            [2, null, [1]]
        ]);

        let result = self
            .client
            .rpc_call(
                RpcMethod::GenerateMindMap,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        let maybe_json = result
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_array)
            .and_then(|x| x.first())
            .and_then(Value::as_str);

        let mut mind_map: Option<Value> = None;
        let mut note_id: Option<String> = None;

        if let Some(json_str) = maybe_json {
            let parsed = serde_json::from_str::<Value>(json_str)
                .unwrap_or(Value::String(json_str.to_string()));
            let title = parsed
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Mind Map");

            if let Ok(created_note_id) = self.create_note(notebook_id).await {
                let _ = self
                    .update_note(notebook_id, &created_note_id, json_str, title)
                    .await;
                note_id = Some(created_note_id);
            }
            mind_map = Some(parsed);
        }

        Ok(MindMapGenerationResult { mind_map, note_id })
    }

    pub async fn poll_status(&self, notebook_id: &str, task_id: &str) -> Result<GenerationStatus> {
        let artifacts = self.list_raw(notebook_id).await?;
        for art in artifacts {
            let Some(arr) = art.as_array() else {
                continue;
            };
            let id = arr.first().and_then(Value::as_str).unwrap_or_default();
            if id != task_id {
                continue;
            }

            let status_code = arr.get(4).and_then(Value::as_i64).unwrap_or(2);
            return Ok(GenerationStatus {
                task_id: task_id.to_string(),
                status: artifact_status_to_str(status_code).to_string(),
                error: None,
                error_code: None,
            });
        }

        Ok(GenerationStatus {
            task_id: task_id.to_string(),
            status: "pending".to_string(),
            error: None,
            error_code: None,
        })
    }

    pub async fn wait_for_completion(
        &self,
        notebook_id: &str,
        task_id: &str,
        timeout_secs: f64,
    ) -> Result<GenerationStatus> {
        let start = tokio::time::Instant::now();
        let mut interval_secs = 2.0f64;

        loop {
            let status = self.poll_status(notebook_id, task_id).await?;
            if status.is_complete() || status.is_failed() {
                return Ok(status);
            }

            if start.elapsed().as_secs_f64() > timeout_secs {
                return Err(NotebookLmError::RpcDecode(format!(
                    "task {task_id} timed out after {timeout_secs}s"
                )));
            }

            tokio::time::sleep(tokio::time::Duration::from_secs_f64(interval_secs)).await;
            interval_secs = (interval_secs * 2.0).min(10.0);
        }
    }

    pub async fn download_audio(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let url = self
            .find_media_url(notebook_id, 1, artifact_id, find_audio_url)
            .await?
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("no completed audio artifact found".to_string())
            })?;
        self.download_url_to_file(&url, output_path).await
    }

    pub async fn download_video(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let url = self
            .find_media_url(notebook_id, 3, artifact_id, find_video_url)
            .await?
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("no completed video artifact found".to_string())
            })?;
        self.download_url_to_file(&url, output_path).await
    }

    pub async fn download_infographic(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let url = self
            .find_media_url(notebook_id, 7, artifact_id, find_infographic_url)
            .await?
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("no completed infographic artifact found".to_string())
            })?;
        self.download_url_to_file(&url, output_path).await
    }

    pub async fn download_slide_deck(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let url = self
            .find_media_url(notebook_id, 8, artifact_id, find_slide_deck_url)
            .await?
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("no completed slide deck artifact found".to_string())
            })?;
        self.download_url_to_file(&url, output_path).await
    }

    pub async fn download_report(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let artifacts = self.list_raw(notebook_id).await?;
        let art = select_completed_artifact(&artifacts, 2, artifact_id).ok_or_else(|| {
            NotebookLmError::RpcDecode("no completed report artifact found".to_string())
        })?;
        let markdown = art
            .get(7)
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(Value::as_str)
            .or_else(|| art.get(7).and_then(Value::as_str))
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("could not parse report content".to_string())
            })?;

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, markdown)?;
        Ok(output_path.to_string())
    }

    pub async fn download_mind_map(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        self.download_mind_map_with_format(
            notebook_id,
            output_path,
            artifact_id,
            MindMapOutputFormat::PrettyJson,
        )
        .await
    }

    pub async fn download_mind_map_with_format(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
        output_format: MindMapOutputFormat,
    ) -> Result<String> {
        let mind_maps = self.list_raw_mind_map_items(notebook_id).await?;
        if mind_maps.is_empty() {
            return Err(NotebookLmError::RpcDecode("no mind maps found".to_string()));
        }

        let selected = select_mind_map_item(&mind_maps, artifact_id)?;

        let json_str = selected
            .as_array()
            .and_then(|a| a.get(1))
            .and_then(Value::as_array)
            .and_then(|inner| inner.get(1))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("mind map content unavailable".to_string())
            })?;

        let parsed: Value = serde_json::from_str(json_str)?;
        let content = match output_format {
            MindMapOutputFormat::Json => serde_json::to_string(&parsed)?,
            MindMapOutputFormat::PrettyJson => serde_json::to_string_pretty(&parsed)?,
        };

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, content)?;
        Ok(output_path.to_string())
    }

    pub async fn download_data_table(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
    ) -> Result<String> {
        let artifacts = self.list_raw(notebook_id).await?;
        let art = select_completed_artifact(&artifacts, 9, artifact_id).ok_or_else(|| {
            NotebookLmError::RpcDecode("no completed data table artifact found".to_string())
        })?;

        let raw_data = art.get(18).ok_or_else(|| {
            NotebookLmError::RpcDecode("could not parse data table structure".to_string())
        })?;
        let (headers, rows) = parse_data_table(raw_data)?;

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut csv_out = String::new();
        csv_out.push_str(&csv_row(&headers));
        csv_out.push('\n');
        for row in rows {
            csv_out.push_str(&csv_row(&row));
            csv_out.push('\n');
        }

        fs::write(output_path, csv_out)?;
        Ok(output_path.to_string())
    }

    pub async fn download_quiz(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
        output_format: InteractiveOutputFormat,
    ) -> Result<String> {
        self.download_interactive_artifact(
            notebook_id,
            output_path,
            artifact_id,
            output_format,
            true,
        )
        .await
    }

    pub async fn download_flashcards(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
        output_format: InteractiveOutputFormat,
    ) -> Result<String> {
        self.download_interactive_artifact(
            notebook_id,
            output_path,
            artifact_id,
            output_format,
            false,
        )
        .await
    }

    pub async fn export_report(
        &self,
        notebook_id: &str,
        artifact_id: &str,
        title: &str,
    ) -> Result<Value> {
        self.export(
            notebook_id,
            Some(artifact_id),
            None,
            title,
            ArtifactExportType::Report,
        )
        .await
    }

    pub async fn export_data_table(
        &self,
        notebook_id: &str,
        artifact_id: &str,
        title: &str,
    ) -> Result<Value> {
        self.export(
            notebook_id,
            Some(artifact_id),
            None,
            title,
            ArtifactExportType::DataTable,
        )
        .await
    }

    pub async fn export(
        &self,
        notebook_id: &str,
        artifact_id: Option<&str>,
        content: Option<&str>,
        title: &str,
        export_type: ArtifactExportType,
    ) -> Result<Value> {
        let params = json!([null, artifact_id, content, title, export_type as i64]);
        self.client
            .rpc_call(
                RpcMethod::ExportArtifact,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await
    }

    pub async fn export_raw(
        &self,
        notebook_id: &str,
        artifact_id: Option<&str>,
        content: Option<&str>,
        title: &str,
        export_type_code: i64,
    ) -> Result<Value> {
        let params = json!([null, artifact_id, content, title, export_type_code]);
        self.client
            .rpc_call(
                RpcMethod::ExportArtifact,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await
    }

    pub async fn delete(&self, notebook_id: &str, artifact_id: &str) -> Result<bool> {
        let params = json!([[2], artifact_id]);
        self.client
            .rpc_call(
                RpcMethod::DeleteArtifact,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(true)
    }

    pub async fn rename(
        &self,
        notebook_id: &str,
        artifact_id: &str,
        new_title: &str,
    ) -> Result<()> {
        let params = json!([[artifact_id, new_title], [["title"]]]);
        self.client
            .rpc_call(
                RpcMethod::RenameArtifact,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(())
    }

    async fn resolve_source_ids(
        &self,
        notebook_id: &str,
        source_ids: Option<Vec<String>>,
    ) -> Result<Vec<String>> {
        match source_ids {
            Some(ids) => Ok(ids),
            None => self.client.get_source_ids(notebook_id).await,
        }
    }

    async fn create_note(&self, notebook_id: &str) -> Result<String> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::CreateNote,
                json!([notebook_id, "", [1], null, "New Note"]),
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        if let Some(id) = result
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_array)
            .and_then(|x| x.first())
            .and_then(Value::as_str)
        {
            return Ok(id.to_string());
        }
        if let Some(id) = result
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
        {
            return Ok(id.to_string());
        }

        Err(NotebookLmError::DecodeShape {
            path: "[0][0]".to_string(),
            context: "create_note did not return a note id".to_string(),
        })
    }

    async fn update_note(
        &self,
        notebook_id: &str,
        note_id: &str,
        content: &str,
        title: &str,
    ) -> Result<()> {
        self.client
            .rpc_call(
                RpcMethod::UpdateNote,
                json!([notebook_id, note_id, [[[content, title, [], 0]]]]),
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(())
    }

    async fn call_generate(&self, notebook_id: &str, params: Value) -> Result<GenerationStatus> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::CreateArtifact,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;
        Ok(parse_generation_result(&result))
    }

    async fn list_raw(&self, notebook_id: &str) -> Result<Vec<Value>> {
        let params = json!([
            [2],
            notebook_id,
            "NOT artifact.status = \"ARTIFACT_STATUS_SUGGESTED\""
        ]);
        let result = self
            .client
            .rpc_call(
                RpcMethod::ListArtifacts,
                params,
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        if let Some(arr) = result.as_array() {
            if let Some(inner) = arr.first().and_then(Value::as_array) {
                return Ok(inner.clone());
            }
            return Ok(arr.clone());
        }

        Ok(Vec::new())
    }

    async fn list_raw_mind_maps(&self, notebook_id: &str) -> Result<Vec<Artifact>> {
        let raw = self.list_raw_mind_map_items(notebook_id).await?;
        Ok(raw
            .iter()
            .filter_map(artifact_from_mind_map_item)
            .collect::<Vec<_>>())
    }

    async fn list_raw_mind_map_items(&self, notebook_id: &str) -> Result<Vec<Value>> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetNotesAndMindMaps,
                json!([notebook_id]),
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        let notes_list = result
            .as_array()
            .and_then(|r| r.first())
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut mind_maps = Vec::new();
        for item in notes_list {
            let Some(arr) = item.as_array() else {
                continue;
            };
            if arr.len() >= 3
                && arr.get(1).is_some_and(Value::is_null)
                && arr.get(2).and_then(Value::as_i64) == Some(2)
            {
                continue;
            }
            let content = arr
                .get(1)
                .and_then(|v| {
                    if v.is_string() {
                        v.as_str().map(ToString::to_string)
                    } else {
                        v.as_array()
                            .and_then(|inner| inner.get(1))
                            .and_then(Value::as_str)
                            .map(ToString::to_string)
                    }
                })
                .unwrap_or_default();

            if content.contains("\"children\":") || content.contains("\"nodes\":") {
                mind_maps.push(item);
            }
        }

        Ok(mind_maps)
    }

    async fn get_artifact_html_content(
        &self,
        notebook_id: &str,
        artifact_id: &str,
    ) -> Result<Option<String>> {
        let result = self
            .client
            .rpc_call(
                RpcMethod::GetInteractiveHtml,
                json!([artifact_id]),
                &format!("/notebook/{notebook_id}"),
                true,
            )
            .await?;

        Ok(result
            .as_array()
            .and_then(|_| json_at(&result, &[0, 9, 0]))
            .and_then(Value::as_str)
            .map(ToString::to_string))
    }

    async fn download_interactive_artifact(
        &self,
        notebook_id: &str,
        output_path: &str,
        artifact_id: Option<&str>,
        output_format: InteractiveOutputFormat,
        is_quiz: bool,
    ) -> Result<String> {
        let artifacts = self
            .list(
                notebook_id,
                Some(if is_quiz {
                    ArtifactKind::Quiz
                } else {
                    ArtifactKind::Flashcards
                }),
            )
            .await?;
        let completed: Vec<Artifact> = artifacts.into_iter().filter(|a| a.status == 3).collect();
        if completed.is_empty() {
            return Err(NotebookLmError::RpcDecode(format!(
                "no completed {} artifact found",
                if is_quiz { "quiz" } else { "flashcards" }
            )));
        }

        let artifact = if let Some(id) = artifact_id {
            completed
                .iter()
                .find(|a| a.id == id)
                .ok_or_else(|| NotebookLmError::RpcDecode(format!("artifact {id} not found")))?
                .clone()
        } else {
            completed
                .iter()
                .max_by_key(|a| a.created_at_unix.unwrap_or(0))
                .cloned()
                .ok_or_else(|| {
                    NotebookLmError::RpcDecode("artifact selection failed".to_string())
                })?
        };

        let html = self
            .get_artifact_html_content(notebook_id, &artifact.id)
            .await?
            .ok_or_else(|| {
                NotebookLmError::RpcDecode("interactive artifact HTML unavailable".to_string())
            })?;

        let content = if matches!(output_format, InteractiveOutputFormat::Html) {
            html
        } else {
            let app_data = extract_app_data_from_html(&html)?;
            if is_quiz {
                let questions = app_data
                    .get("quiz")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if matches!(output_format, InteractiveOutputFormat::Markdown) {
                    format_quiz_markdown(artifact.title.as_str(), &questions)
                } else {
                    serde_json::to_string_pretty(&json!({
                        "title": artifact.title,
                        "questions": questions
                    }))?
                }
            } else {
                let cards = app_data
                    .get("flashcards")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if matches!(output_format, InteractiveOutputFormat::Markdown) {
                    format_flashcards_markdown(artifact.title.as_str(), &cards)
                } else {
                    let normalized: Vec<Value> = cards
                        .iter()
                        .map(|c| {
                            json!({
                                "front": c.get("f").and_then(Value::as_str).unwrap_or_default(),
                                "back": c.get("b").and_then(Value::as_str).unwrap_or_default()
                            })
                        })
                        .collect();
                    serde_json::to_string_pretty(&json!({
                        "title": artifact.title,
                        "cards": normalized
                    }))?
                }
            }
        };

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, content)?;
        Ok(output_path.to_string())
    }

    async fn find_media_url(
        &self,
        notebook_id: &str,
        artifact_type: i64,
        artifact_id: Option<&str>,
        extractor: fn(&Value) -> Option<String>,
    ) -> Result<Option<String>> {
        let artifacts = self.list_raw(notebook_id).await?;
        let art = select_completed_artifact(&artifacts, artifact_type, artifact_id);
        Ok(art.and_then(extractor))
    }

    async fn download_url_to_file(&self, url: &str, output_path: &str) -> Result<String> {
        let mut active_client = self.client.http.clone();
        let mut auth_refreshed = false;

        for attempt in 0..=self.client.retry_policy.max_retries {
            let send_result = active_client.get(url).send().await;
            let response = match send_result {
                Ok(resp) => resp,
                Err(err) => {
                    if should_retry_transport_error(&err)
                        && attempt < self.client.retry_policy.max_retries
                    {
                        sleep_with_backoff(&self.client.retry_policy, attempt, None).await;
                        continue;
                    }
                    return Err(map_transport_error("download", err));
                }
            };

            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let bytes = response.bytes().await?;

            if status.is_success() {
                if let Some(parent) = std::path::Path::new(output_path).parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(output_path, &bytes)?;
                return Ok(output_path.to_string());
            }

            if is_auth_status(status) && !auth_refreshed {
                let refreshed = AuthTokens::from_storage(None).await?;
                active_client = build_http_client_with_auth(&refreshed)?;
                auth_refreshed = true;
                continue;
            }

            if is_auth_status(status) && auth_refreshed {
                return Err(NotebookLmError::StaleAuth(
                    "download authentication failed after token refresh".to_string(),
                ));
            }

            if is_retryable_http_status(status) && attempt < self.client.retry_policy.max_retries {
                sleep_with_backoff(&self.client.retry_policy, attempt, retry_after).await;
                continue;
            }

            let body_preview = format!("download failed ({} bytes body)", bytes.len());
            return Err(map_http_error(
                "download",
                status,
                &body_preview,
                retry_after.map(|v| v.to_string()).as_deref(),
            ));
        }

        Err(NotebookLmError::Rpc {
            method_id: "download".to_string(),
            message: "retry attempts exhausted".to_string(),
            code: None,
        })
    }
}

fn build_http_client_with_auth(auth: &AuthTokens) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
    );
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&auth.cookie_header())
            .map_err(|e| NotebookLmError::Auth(format!("invalid cookie header: {e}")))?,
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

fn build_rpc_url(auth: &AuthTokens, method: RpcMethod, source_path: &str) -> Result<reqwest::Url> {
    reqwest::Url::parse_with_params(
        BATCHEXECUTE_URL,
        &[
            ("rpcids", method.id()),
            ("source-path", source_path),
            ("f.sid", auth.session_id.as_str()),
            ("rt", "c"),
        ],
    )
    .map_err(|e| NotebookLmError::Config(format!("invalid rpc url params: {e}")))
}

fn build_query_url(auth: &AuthTokens, reqid: &str) -> Result<reqwest::Url> {
    reqwest::Url::parse_with_params(
        QUERY_URL,
        &[
            ("bl", DEFAULT_QUERY_BL),
            ("hl", "en"),
            ("_reqid", reqid),
            ("rt", "c"),
            ("f.sid", auth.session_id.as_str()),
        ],
    )
    .map_err(|e| NotebookLmError::Config(format!("invalid query url params: {e}")))
}

fn is_auth_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN
}

fn is_retryable_http_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn should_retry_transport_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

fn map_transport_error(operation: &str, err: reqwest::Error) -> NotebookLmError {
    if err.is_timeout() {
        return NotebookLmError::Timeout {
            operation: operation.to_string(),
            message: err.to_string(),
        };
    }
    NotebookLmError::Network {
        operation: operation.to_string(),
        message: err.to_string(),
    }
}

async fn sleep_with_backoff(policy: &RetryPolicy, attempt: u32, retry_after_secs: Option<u64>) {
    let shift = attempt.min(20);
    let scale = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
    let mut delay_ms = policy.base_delay_ms.saturating_mul(scale);
    if delay_ms > policy.max_delay_ms {
        delay_ms = policy.max_delay_ms;
    }

    let jitter = if policy.jitter_ms > 0 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0);
        nanos % (policy.jitter_ms + 1)
    } else {
        0
    };

    let retry_after_delay = retry_after_secs.unwrap_or(0).saturating_mul(1000);
    let final_delay = delay_ms.saturating_add(jitter).max(retry_after_delay);
    tokio::time::sleep(tokio::time::Duration::from_millis(final_delay)).await;
}

fn map_http_error(
    method_id: &str,
    status: reqwest::StatusCode,
    text: &str,
    retry_after_header: Option<&str>,
) -> NotebookLmError {
    let status_u16 = status.as_u16();
    let message = format!("http status {status}: {text}");

    if is_auth_status(status) {
        return NotebookLmError::Auth(format!("{method_id}: {message}"));
    }

    if status_u16 == 429 {
        let retry_after = retry_after_header.and_then(|h| h.parse::<u64>().ok());
        return NotebookLmError::RateLimit {
            message: format!("{method_id}: {message}"),
            retry_after,
        };
    }

    if (400..500).contains(&status_u16) {
        return NotebookLmError::Client {
            status: status_u16,
            message: format!("{method_id}: {message}"),
        };
    }

    if status_u16 >= 500 {
        return NotebookLmError::Server {
            status: status_u16,
            message: format!("{method_id}: {message}"),
        };
    }

    NotebookLmError::Rpc {
        method_id: method_id.to_string(),
        message,
        code: Some(i64::from(status_u16)),
    }
}

fn source_ids_triple(source_ids: &[String]) -> Vec<Value> {
    source_ids.iter().map(|sid| json!([[sid]])).collect()
}

fn source_ids_double(source_ids: &[String]) -> Vec<Value> {
    source_ids.iter().map(|sid| json!([sid])).collect()
}

fn report_config(
    format: ReportFormat,
    custom_prompt: Option<&str>,
) -> (&'static str, &'static str, String) {
    match format {
        ReportFormat::BriefingDoc => (
            "Briefing Doc",
            "Key insights and important quotes",
            "Create a comprehensive briefing document that includes an Executive Summary, detailed analysis of key themes, important quotes with context, and actionable insights.".to_string(),
        ),
        ReportFormat::StudyGuide => (
            "Study Guide",
            "Short-answer quiz, essay questions, glossary",
            "Create a comprehensive study guide that includes key concepts, short-answer practice questions, essay prompts for deeper exploration, and a glossary of important terms.".to_string(),
        ),
        ReportFormat::BlogPost => (
            "Blog Post",
            "Insightful takeaways in readable article format",
            "Write an engaging blog post that presents the key insights in an accessible, reader-friendly format. Include an attention-grabbing introduction, well-organized sections, and a compelling conclusion with takeaways.".to_string(),
        ),
        ReportFormat::Custom => (
            "Custom Report",
            "Custom format",
            custom_prompt
                .unwrap_or("Create a report based on the provided sources.")
                .to_string(),
        ),
    }
}

fn build_generate_audio_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &AudioGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let source_ids_double = source_ids_double(source_ids);
    let format_code = options.format.map(|v| v as i64);
    let length_code = options.length.map(|v| v as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            1,
            source_ids_triple,
            null,
            null,
            [
                null,
                [
                    options.instructions,
                    length_code,
                    null,
                    source_ids_double,
                    options.language,
                    null,
                    format_code
                ]
            ]
        ]
    ])
}

fn build_generate_video_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &VideoGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let source_ids_double = source_ids_double(source_ids);
    let format_code = options.format.map(|v| v as i64);
    let style_code = options.style.map(|v| v as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            3,
            source_ids_triple,
            null,
            null,
            null,
            null,
            [
                null,
                null,
                [
                    source_ids_double,
                    options.language,
                    options.instructions,
                    null,
                    format_code,
                    style_code
                ]
            ]
        ]
    ])
}

fn build_generate_report_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &ReportGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let source_ids_double = source_ids_double(source_ids);
    let (title, description, prompt) =
        report_config(options.format, options.custom_prompt.as_deref());
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            2,
            source_ids_triple,
            null,
            null,
            null,
            [
                null,
                [
                    title,
                    description,
                    null,
                    source_ids_double,
                    options.language,
                    prompt,
                    null,
                    true
                ]
            ]
        ]
    ])
}

fn build_generate_quiz_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &QuizGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let quantity_code = options.quantity.map(|q| q.code());
    let difficulty_code = options.difficulty.map(|d| d as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            4,
            source_ids_triple,
            null,
            null,
            null,
            null,
            null,
            [
                null,
                [
                    2,
                    null,
                    options.instructions,
                    null,
                    null,
                    null,
                    null,
                    [quantity_code, difficulty_code]
                ]
            ]
        ]
    ])
}

fn build_generate_flashcards_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &FlashcardsGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let quantity_code = options.quantity.map(|q| q.code());
    let difficulty_code = options.difficulty.map(|d| d as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            4,
            source_ids_triple,
            null,
            null,
            null,
            null,
            null,
            [
                null,
                [
                    1,
                    null,
                    options.instructions,
                    null,
                    null,
                    null,
                    [difficulty_code, quantity_code]
                ]
            ]
        ]
    ])
}

fn build_generate_infographic_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &InfographicGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let orientation_code = options.orientation.map(|v| v as i64);
    let detail_code = options.detail_level.map(|v| v as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            7,
            source_ids_triple,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [[
                options.instructions,
                options.language,
                null,
                orientation_code,
                detail_code
            ]]
        ]
    ])
}

fn build_generate_slide_deck_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &SlideDeckGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    let format_code = options.format.map(|v| v as i64);
    let length_code = options.length.map(|v| v as i64);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            8,
            source_ids_triple,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [[
                options.instructions,
                options.language,
                format_code,
                length_code
            ]]
        ]
    ])
}

fn build_generate_data_table_params(
    notebook_id: &str,
    source_ids: &[String],
    options: &DataTableGenerationOptions,
) -> Value {
    let source_ids_triple = source_ids_triple(source_ids);
    json!([
        [2],
        notebook_id,
        [
            null,
            null,
            9,
            source_ids_triple,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [null, [options.instructions, options.language]]
        ]
    ])
}

fn select_completed_artifact<'a>(
    artifacts: &'a [Value],
    artifact_type: i64,
    artifact_id: Option<&str>,
) -> Option<&'a Value> {
    let mut candidates: Vec<&Value> = artifacts
        .iter()
        .filter(|a| {
            let Some(arr) = a.as_array() else {
                return false;
            };
            let t = arr.get(2).and_then(Value::as_i64).unwrap_or_default();
            let status = arr.get(4).and_then(Value::as_i64).unwrap_or_default();
            t == artifact_type && status == 3
        })
        .collect();

    if let Some(id) = artifact_id {
        return candidates.into_iter().find(|a| {
            a.as_array()
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
                == Some(id)
        });
    }

    candidates.sort_by_key(|a| {
        a.as_array()
            .and_then(|arr| arr.get(15))
            .and_then(Value::as_array)
            .and_then(|t| t.first())
            .and_then(Value::as_i64)
            .unwrap_or(0)
    });
    candidates.pop()
}

fn select_mind_map_item<'a>(
    mind_maps: &'a [Value],
    artifact_id: Option<&str>,
) -> Result<&'a Value> {
    if let Some(id) = artifact_id {
        return mind_maps
            .iter()
            .find(|item| {
                item.as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    == Some(id)
            })
            .ok_or_else(|| NotebookLmError::RpcDecode(format!("mind map {id} not found")));
    }

    mind_maps
        .iter()
        .max_by_key(|item| {
            item.as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_array)
                .and_then(|inner| inner.get(2))
                .and_then(Value::as_array)
                .and_then(|t| t.first())
                .and_then(Value::as_i64)
                .unwrap_or(0)
        })
        .ok_or_else(|| NotebookLmError::RpcDecode("no selectable mind maps".to_string()))
}

fn find_audio_url(artifact: &Value) -> Option<String> {
    let arr = artifact.as_array()?;
    let media_list = arr.get(6)?.as_array()?.get(5)?.as_array()?;
    for item in media_list {
        let entry = item.as_array()?;
        if entry.get(2).and_then(Value::as_str) == Some("audio/mp4") {
            return entry
                .first()
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }
    }
    media_list
        .first()
        .and_then(Value::as_array)
        .and_then(|e| e.first())
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn find_video_url(artifact: &Value) -> Option<String> {
    let arr = artifact.as_array()?;
    let metadata = arr.get(8)?.as_array()?;
    for group in metadata {
        let items = group.as_array()?;
        let has_http = items.iter().any(|i| {
            i.as_array()
                .and_then(|x| x.first())
                .and_then(Value::as_str)
                .map(|s| s.starts_with("http"))
                .unwrap_or(false)
        });
        if !has_http {
            continue;
        }
        for item in items {
            let entry = item.as_array()?;
            if entry.get(2).and_then(Value::as_str) == Some("video/mp4") {
                return entry
                    .first()
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
            }
        }
        return items
            .first()
            .and_then(Value::as_array)
            .and_then(|e| e.first())
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    None
}

fn find_infographic_url(artifact: &Value) -> Option<String> {
    let arr = artifact.as_array()?;
    for item in arr.iter().rev() {
        let outer = item.as_array()?;
        let content_list = outer.get(2)?.as_array()?;
        let img_data = content_list
            .first()
            .and_then(Value::as_array)
            .and_then(|x| x.get(1))
            .and_then(Value::as_array)?;
        let url = img_data.first().and_then(Value::as_str)?;
        if url.starts_with("http") {
            return Some(url.to_string());
        }
    }
    None
}

fn find_slide_deck_url(artifact: &Value) -> Option<String> {
    artifact
        .as_array()?
        .get(16)?
        .as_array()?
        .get(3)?
        .as_str()
        .map(ToString::to_string)
}

fn artifact_from_mind_map_item(item: &Value) -> Option<Artifact> {
    let arr = item.as_array()?;
    let id = arr.first().and_then(Value::as_str)?.to_string();

    if arr.len() >= 3
        && arr.get(1).is_some_and(Value::is_null)
        && arr.get(2).and_then(Value::as_i64) == Some(2)
    {
        return None;
    }

    let inner = arr.get(1).and_then(Value::as_array)?;
    let title = inner
        .get(4)
        .and_then(Value::as_str)
        .unwrap_or("Mind Map")
        .to_string();
    let created_at_unix = inner
        .get(2)
        .and_then(Value::as_array)
        .and_then(|m| m.get(2))
        .and_then(Value::as_array)
        .and_then(|t| t.first())
        .and_then(Value::as_i64);

    Some(Artifact {
        id,
        title,
        artifact_type: 5,
        variant: None,
        status: 3,
        created_at_unix,
    })
}

fn extract_nested_string(data: &Value, path: &[usize]) -> Option<String> {
    json_at(data, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn json_at<'a>(data: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = data;
    for idx in path {
        current = current.as_array()?.get(*idx)?;
    }
    Some(current)
}

fn required_json_at<'a>(data: &'a Value, path: &[usize], context: &str) -> Result<&'a Value> {
    json_at(data, path).ok_or_else(|| NotebookLmError::DecodeShape {
        path: format!("{path:?}"),
        context: context.to_string(),
    })
}

fn parse_share_status(
    data: &Value,
    notebook_id: &str,
    view_level_override: Option<ShareViewLevel>,
) -> ShareStatus {
    let mut users: Vec<SharedUser> = Vec::new();
    if let Some(user_list) = data
        .as_array()
        .and_then(|d| d.first())
        .and_then(Value::as_array)
    {
        for user in user_list {
            let Some(arr) = user.as_array() else {
                continue;
            };
            let email = arr
                .first()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let permission = match arr.get(1).and_then(Value::as_i64).unwrap_or(3) {
                1 => SharePermission::Owner,
                2 => SharePermission::Editor,
                4 => SharePermission::Remove,
                _ => SharePermission::Viewer,
            };
            let display_name = arr
                .get(3)
                .and_then(Value::as_array)
                .and_then(|u| u.first())
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let avatar_url = arr
                .get(3)
                .and_then(Value::as_array)
                .and_then(|u| u.get(1))
                .and_then(Value::as_str)
                .map(ToString::to_string);
            users.push(SharedUser {
                email,
                permission,
                display_name,
                avatar_url,
            });
        }
    }

    let is_public = data
        .as_array()
        .and_then(|d| d.get(1))
        .and_then(Value::as_array)
        .and_then(|x| x.first())
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let access = if is_public {
        ShareAccess::AnyoneWithLink
    } else {
        ShareAccess::Restricted
    };
    let view_level = view_level_override.unwrap_or(ShareViewLevel::FullNotebook);
    let share_url = if is_public {
        Some(format!(
            "https://notebooklm.google.com/notebook/{notebook_id}"
        ))
    } else {
        None
    };

    ShareStatus {
        notebook_id: notebook_id.to_string(),
        is_public,
        access,
        view_level,
        shared_users: users,
        share_url,
    }
}

fn csv_row(values: &[String]) -> String {
    values
        .iter()
        .map(|v| {
            if v.contains(',') || v.contains('"') || v.contains('\n') || v.contains('\r') {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn extract_cell_text(cell: &Value) -> String {
    match cell {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(extract_cell_text)
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn parse_data_table(raw_data: &Value) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let rows_array = required_json_at(
        raw_data,
        &[0, 0, 0, 0, 4, 2],
        "invalid data table structure",
    )?
    .as_array()
    .ok_or_else(|| NotebookLmError::DecodeShape {
        path: "[0, 0, 0, 0, 4, 2]".to_string(),
        context: "rows array is not an array".to_string(),
    })?;

    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for (idx, row_section) in rows_array.iter().enumerate() {
        let cells = row_section
            .as_array()
            .and_then(|r| r.get(2))
            .and_then(Value::as_array)
            .ok_or_else(|| NotebookLmError::DecodeShape {
                path: "[row][2]".to_string(),
                context: "invalid row structure".to_string(),
            })?;

        let row_values: Vec<String> = cells.iter().map(extract_cell_text).collect();
        if idx == 0 {
            headers = row_values;
        } else {
            rows.push(row_values);
        }
    }

    if headers.is_empty() {
        return Err(NotebookLmError::DecodeShape {
            path: "[rows][0]".to_string(),
            context: "failed to extract data table headers".to_string(),
        });
    }

    Ok((headers, rows))
}

fn html_unescape_basic(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn extract_app_data_from_html(html: &str) -> Result<Value> {
    let re = Regex::new(r#"data-app-data="([^"]+)""#)
        .map_err(|e| NotebookLmError::RpcDecode(format!("regex compile failed: {e}")))?;
    let encoded = re
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| {
            NotebookLmError::RpcDecode("data-app-data attribute not found".to_string())
        })?;

    let decoded = html_unescape_basic(encoded);
    Ok(serde_json::from_str::<Value>(&decoded)?)
}

fn format_quiz_markdown(title: &str, questions: &[Value]) -> String {
    let mut lines = vec![format!("# {title}"), String::new()];
    for (idx, q) in questions.iter().enumerate() {
        let question = q
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        lines.push(format!("## Question {}", idx + 1));
        lines.push(question);
        lines.push(String::new());

        if let Some(options) = q.get("answerOptions").and_then(Value::as_array) {
            for opt in options {
                let is_correct = opt
                    .get("isCorrect")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let marker = if is_correct { "[x]" } else { "[ ]" };
                let text = opt.get("text").and_then(Value::as_str).unwrap_or_default();
                lines.push(format!("- {marker} {text}"));
            }
        }

        if let Some(hint) = q.get("hint").and_then(Value::as_str) {
            lines.push(String::new());
            lines.push(format!("**Hint:** {hint}"));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

fn format_flashcards_markdown(title: &str, cards: &[Value]) -> String {
    let mut lines = vec![format!("# {title}"), String::new()];
    for (idx, card) in cards.iter().enumerate() {
        let front = card.get("f").and_then(Value::as_str).unwrap_or_default();
        let back = card.get("b").and_then(Value::as_str).unwrap_or_default();
        lines.push(format!("## Card {}", idx + 1));
        lines.push(String::new());
        lines.push(format!("**Q:** {front}"));
        lines.push(String::new());
        lines.push(format!("**A:** {back}"));
        lines.push(String::new());
        lines.push("---".to_string());
        lines.push(String::new());
    }
    lines.join("\n")
}

fn parse_generation_result(result: &Value) -> GenerationStatus {
    if let Some(artifact_data) = result.as_array().and_then(|arr| arr.first())
        && let Some(artifact_arr) = artifact_data.as_array()
    {
        let artifact_id = artifact_arr
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let status_code = artifact_arr.get(4).and_then(Value::as_i64).unwrap_or(2);

        if !artifact_id.is_empty() {
            return GenerationStatus {
                task_id: artifact_id,
                status: artifact_status_to_str(status_code).to_string(),
                error: None,
                error_code: None,
            };
        }
    }

    GenerationStatus {
        task_id: String::new(),
        status: "failed".to_string(),
        error: Some("generation failed - no artifact id returned".to_string()),
        error_code: None,
    }
}

fn extract_chat_answer(response_text: &str) -> String {
    let stripped = if response_text.starts_with(")]}'") {
        response_text.trim_start_matches(")]}'")
    } else {
        response_text
    };

    let lines: Vec<&str> = stripped
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let mut longest_answer = String::new();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let maybe_json_line = if line.parse::<usize>().is_ok() {
            i += 1;
            if i < lines.len() {
                Some(lines[i])
            } else {
                None
            }
        } else {
            Some(line)
        };

        if let Some(json_line) = maybe_json_line
            && let Some(candidate) = extract_answer_from_chunk(json_line)
            && candidate.len() > longest_answer.len()
        {
            longest_answer = candidate;
        }

        i += 1;
    }

    longest_answer
}

fn extract_answer_from_chunk(json_str: &str) -> Option<String> {
    let data = serde_json::from_str::<Value>(json_str).ok()?;
    let items = data.as_array()?;

    for item in items {
        let item_arr = item.as_array()?;
        if item_arr.first().and_then(Value::as_str) != Some("wrb.fr") {
            continue;
        }

        let inner_json = item_arr.get(2).and_then(Value::as_str)?;
        let inner_data = serde_json::from_str::<Value>(inner_json).ok()?;
        let first = inner_data.as_array()?.first()?.as_array()?;

        let text = first.first().and_then(Value::as_str)?;
        if text.len() <= 20 {
            continue;
        }

        let is_answer = first
            .get(4)
            .and_then(Value::as_array)
            .and_then(|type_info| type_info.last())
            .and_then(Value::as_i64)
            == Some(1);

        if is_answer {
            return Some(text.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_string_by_path() {
        let data = json!([[null, [null, [null, "en"]]]]);
        let extracted = extract_nested_string(&data, &[0, 1, 1, 1]);
        assert_eq!(extracted.as_deref(), Some("en"));
    }

    #[test]
    fn parses_share_status_with_users() {
        let data = json!([
            [
                [
                    "a@example.com",
                    3,
                    [],
                    ["Alice", "https://avatar.example/a.png"]
                ],
                ["b@example.com", 2, [], ["Bob", null]]
            ],
            [true],
            1000
        ]);

        let status = parse_share_status(&data, "nb123", None);
        assert!(status.is_public);
        assert!(matches!(status.access, ShareAccess::AnyoneWithLink));
        assert_eq!(status.shared_users.len(), 2);
        assert_eq!(status.shared_users[0].email, "a@example.com");
        assert!(matches!(
            status.shared_users[0].permission,
            SharePermission::Viewer
        ));
        assert_eq!(
            status.shared_users[0].display_name.as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn escapes_csv_rows() {
        let row = vec![
            "plain".to_string(),
            "has,comma".to_string(),
            "has\"quote".to_string(),
            "has\nnewline".to_string(),
        ];
        let csv = csv_row(&row);
        assert_eq!(csv, "plain,\"has,comma\",\"has\"\"quote\",\"has\nnewline\"");
    }

    #[test]
    fn parses_data_table_structure() {
        // Structure expected at raw_data[0][0][0][0][4][2]
        let raw = json!([[[[[
            null,
            null,
            null,
            null,
            [
                null,
                null,
                [[0, 1, ["Col A", "Col B"]], [1, 2, ["V1", "V2"]]]
            ]
        ]]]]]);

        let (headers, rows) = parse_data_table(&raw).expect("should parse table");
        assert_eq!(headers, vec!["Col A".to_string(), "Col B".to_string()]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], vec!["V1".to_string(), "V2".to_string()]);
    }

    #[test]
    fn extracts_app_data_from_html_attribute() {
        let html = r#"<div data-app-data="{&quot;quiz&quot;:[{&quot;question&quot;:&quot;Q1&quot;}]}"></div>"#;
        let app_data = extract_app_data_from_html(html).expect("extract app data");
        assert_eq!(
            app_data["quiz"][0]["question"].as_str(),
            Some("Q1"),
            "question should parse from HTML-encoded JSON"
        );
    }

    #[test]
    fn selects_latest_completed_artifact_when_no_id() {
        let artifacts = vec![
            json!([
                "a1",
                "Old",
                1,
                null,
                3,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [10]
            ]),
            json!([
                "a2",
                "New",
                1,
                null,
                3,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [20]
            ]),
            json!([
                "a3",
                "Pending",
                1,
                null,
                2,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [30]
            ]),
        ];

        let selected = select_completed_artifact(&artifacts, 1, None).expect("selected artifact");
        assert_eq!(selected[0].as_str(), Some("a2"));
    }

    #[test]
    fn parses_generation_result_task_id_and_status() {
        let input = json!([["task-1", "Title", 1, null, 3]]);
        let status = parse_generation_result(&input);
        assert_eq!(status.task_id, "task-1");
        assert_eq!(status.status, "completed");
        assert!(!status.is_failed());
    }

    #[test]
    fn maps_auth_error_status() {
        let err = map_http_error(
            "test_method",
            reqwest::StatusCode::UNAUTHORIZED,
            "unauthorized",
            None,
        );
        match err {
            NotebookLmError::Auth(msg) => assert!(msg.contains("test_method")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_rate_limit_error_status() {
        let err = map_http_error(
            "test_method",
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "too many requests",
            Some("120"),
        );
        match err {
            NotebookLmError::RateLimit {
                retry_after: Some(120),
                ..
            } => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_server_error_status() {
        let err = map_http_error(
            "test_method",
            reqwest::StatusCode::BAD_GATEWAY,
            "bad gateway",
            None,
        );
        match err {
            NotebookLmError::Server { status: 502, .. } => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn retryable_status_detection() {
        assert!(is_retryable_http_status(
            reqwest::StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(is_retryable_http_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(!is_retryable_http_status(reqwest::StatusCode::BAD_REQUEST));
    }

    #[test]
    fn builds_audio_payload_with_typed_options() {
        let options = AudioGenerationOptions {
            source_ids: None,
            language: "en".to_string(),
            instructions: Some("focus on chapter 3".to_string()),
            format: Some(crate::types::AudioFormat::Debate),
            length: Some(crate::types::AudioLength::Long),
        };
        let payload =
            build_generate_audio_params("nb_1", &["s1".to_string(), "s2".to_string()], &options);
        assert_eq!(json_at(&payload, &[2, 2]).and_then(Value::as_i64), Some(1));
        assert_eq!(
            json_at(&payload, &[2, 6, 1, 1]).and_then(Value::as_i64),
            Some(3)
        );
        assert_eq!(
            json_at(&payload, &[2, 6, 1, 6]).and_then(Value::as_i64),
            Some(4)
        );
    }

    #[test]
    fn builds_report_payload_for_study_guide() {
        let options = ReportGenerationOptions {
            source_ids: None,
            language: "en".to_string(),
            format: ReportFormat::StudyGuide,
            custom_prompt: None,
        };
        let payload = build_generate_report_params("nb_1", &["s1".to_string()], &options);
        assert_eq!(json_at(&payload, &[2, 2]).and_then(Value::as_i64), Some(2));
        assert_eq!(
            json_at(&payload, &[2, 7, 1, 0]).and_then(Value::as_str),
            Some("Study Guide")
        );
    }

    #[test]
    fn parses_fixture_share_status() {
        let raw = include_str!("../tests/fixtures/share_status.json");
        let value: Value = serde_json::from_str(raw).expect("fixture json");
        let status = parse_share_status(&value, "nb_fixture", None);
        assert!(status.is_public);
        assert_eq!(status.shared_users.len(), 1);
        assert_eq!(status.shared_users[0].email, "a@example.com");
    }

    #[test]
    fn parses_fixture_data_table() {
        let raw = include_str!("../tests/fixtures/data_table.json");
        let value: Value = serde_json::from_str(raw).expect("fixture json");
        let (headers, rows) = parse_data_table(&value).expect("parse fixture data table");
        assert_eq!(headers, vec!["Col A".to_string(), "Col B".to_string()]);
        assert_eq!(rows[0], vec!["V1".to_string(), "V2".to_string()]);
    }

    #[test]
    fn parses_fixture_notebook_list_shape() {
        let raw = include_str!("../tests/fixtures/notebooks_list.json");
        let value: Value = serde_json::from_str(raw).expect("fixture json");
        let first = json_at(&value, &[0, 0]).expect("first notebook");
        let parsed = Notebook::from_api_response(first);
        assert_eq!(parsed.id, "nb_123");
        assert_eq!(parsed.title, "My Notebook");
    }

    #[test]
    fn parse_data_table_returns_decode_shape_for_bad_shape() {
        let value = json!({"not":"table"});
        let err = parse_data_table(&value).expect_err("expected error");
        match err {
            NotebookLmError::DecodeShape { .. } => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn selects_latest_mind_map_by_default() {
        let items = vec![
            json!(["m1", ["m1", "{\"name\":\"One\"}", [10], null, "One"]]),
            json!(["m2", ["m2", "{\"name\":\"Two\"}", [20], null, "Two"]]),
        ];
        let selected = select_mind_map_item(&items, None).expect("selected");
        assert_eq!(
            selected
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str),
            Some("m2")
        );
    }

    #[test]
    fn selects_mind_map_by_id() {
        let items = vec![
            json!(["m1", ["m1", "{\"name\":\"One\"}", [10], null, "One"]]),
            json!(["m2", ["m2", "{\"name\":\"Two\"}", [20], null, "Two"]]),
        ];
        let selected = select_mind_map_item(&items, Some("m1")).expect("selected");
        assert_eq!(
            selected
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str),
            Some("m1")
        );
    }
}
