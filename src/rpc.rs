use crate::error::{NotebookLmError, Result};
use serde_json::Value;

pub const BATCHEXECUTE_URL: &str =
    "https://notebooklm.google.com/_/LabsTailwindUi/data/batchexecute";
pub const QUERY_URL: &str = "https://notebooklm.google.com/_/LabsTailwindUi/data/google.internal.labs.tailwind.orchestration.v1.LabsTailwindOrchestrationService/GenerateFreeFormStreamed";

#[derive(Debug, Clone, Copy)]
pub enum RpcMethod {
    ListNotebooks,
    CreateNotebook,
    GetNotebook,
    RenameNotebook,
    DeleteNotebook,
    Summarize,
    AddSource,
    DeleteSource,
    GetSource,
    RefreshSource,
    UpdateSource,
    GetConversationHistory,
    CreateArtifact,
    ListArtifacts,
    DeleteArtifact,
    RenameArtifact,
    ExportArtifact,
    GenerateMindMap,
    CreateNote,
    UpdateNote,
    GetInteractiveHtml,
    GetNotesAndMindMaps,
    StartFastResearch,
    StartDeepResearch,
    PollResearch,
    ImportResearch,
    GetUserSettings,
    SetUserSettings,
    GetShareStatus,
    ShareNotebook,
}

impl RpcMethod {
    pub fn id(self) -> &'static str {
        match self {
            RpcMethod::ListNotebooks => "wXbhsf",
            RpcMethod::CreateNotebook => "CCqFvf",
            RpcMethod::GetNotebook => "rLM1Ne",
            RpcMethod::RenameNotebook => "s0tc2d",
            RpcMethod::DeleteNotebook => "WWINqb",
            RpcMethod::Summarize => "VfAZjd",
            RpcMethod::AddSource => "izAoDd",
            RpcMethod::DeleteSource => "tGMBJ",
            RpcMethod::GetSource => "hizoJc",
            RpcMethod::RefreshSource => "FLmJqe",
            RpcMethod::UpdateSource => "b7Wfje",
            RpcMethod::GetConversationHistory => "hPTbtc",
            RpcMethod::CreateArtifact => "R7cb6c",
            RpcMethod::ListArtifacts => "gArtLc",
            RpcMethod::DeleteArtifact => "V5N4be",
            RpcMethod::RenameArtifact => "rc3d8d",
            RpcMethod::ExportArtifact => "Krh3pd",
            RpcMethod::GenerateMindMap => "yyryJe",
            RpcMethod::CreateNote => "CYK0Xb",
            RpcMethod::UpdateNote => "cYAfTb",
            RpcMethod::GetInteractiveHtml => "v9rmvd",
            RpcMethod::GetNotesAndMindMaps => "cFji9",
            RpcMethod::StartFastResearch => "Ljjv0c",
            RpcMethod::StartDeepResearch => "QA9ei",
            RpcMethod::PollResearch => "e3bVqc",
            RpcMethod::ImportResearch => "LBwxtb",
            RpcMethod::GetUserSettings => "ZwVcOc",
            RpcMethod::SetUserSettings => "hT54vc",
            RpcMethod::GetShareStatus => "JFMDGd",
            RpcMethod::ShareNotebook => "QDyure",
        }
    }
}

pub fn encode_rpc_request(method: RpcMethod, params: Value) -> Result<Value> {
    let params_json = serde_json::to_string(&params)?;
    Ok(serde_json::json!([[[
        method.id(),
        params_json,
        null,
        "generic"
    ]]]))
}

pub fn build_request_body(rpc_request: &Value, csrf_token: &str) -> Result<String> {
    let f_req = serde_json::to_string(rpc_request)?;
    let body = format!(
        "f.req={}&at={}&",
        urlencoding::encode(&f_req),
        urlencoding::encode(csrf_token)
    );
    Ok(body)
}

pub fn decode_response(raw: &str, rpc_id: &str, allow_null: bool) -> Result<Value> {
    let stripped = strip_anti_xssi(raw);
    let chunks = parse_chunked_response(&stripped)?;
    let result = extract_rpc_result(&chunks, rpc_id)?;

    if result.is_null() && !allow_null {
        return Err(NotebookLmError::RpcDecode(format!(
            "null response for rpc method {rpc_id}"
        )));
    }

    Ok(result)
}

fn strip_anti_xssi(response: &str) -> String {
    if response.starts_with(")]}'") {
        if let Some(idx) = response.find('\n') {
            return response[idx + 1..].to_string();
        }
    }
    response.to_string()
}

fn parse_chunked_response(response: &str) -> Result<Vec<Value>> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = response
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];

        if line.parse::<usize>().is_ok() {
            i += 1;
            if i < lines.len() {
                if let Ok(chunk) = serde_json::from_str::<Value>(lines[i]) {
                    chunks.push(chunk);
                }
            }
            i += 1;
            continue;
        }

        if let Ok(chunk) = serde_json::from_str::<Value>(line) {
            chunks.push(chunk);
        }
        i += 1;
    }

    Ok(chunks)
}

fn extract_rpc_result(chunks: &[Value], rpc_id: &str) -> Result<Value> {
    for chunk in chunks {
        let items: Vec<&Value> = if chunk.is_array()
            && chunk
                .as_array()
                .and_then(|a| a.first())
                .map(|v| v.is_array())
                .unwrap_or(false)
        {
            chunk
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default()
        } else {
            vec![chunk]
        };

        for item in items {
            let Some(arr) = item.as_array() else {
                continue;
            };
            if arr.len() < 3 {
                continue;
            }

            let tag = arr.first().and_then(Value::as_str).unwrap_or_default();
            let id = arr.get(1).and_then(Value::as_str).unwrap_or_default();

            if id != rpc_id {
                continue;
            }

            if tag == "er" {
                let code = arr.get(2).and_then(Value::as_i64);
                let message = match code {
                    Some(401) => "authentication required".to_string(),
                    Some(403) => "forbidden".to_string(),
                    Some(404) => "not found".to_string(),
                    Some(429) => "rate limited".to_string(),
                    Some(c) => format!("rpc error code {c}"),
                    None => "unknown rpc error".to_string(),
                };
                return Err(NotebookLmError::Rpc {
                    method_id: rpc_id.to_string(),
                    message,
                    code,
                });
            }

            if tag == "wrb.fr" {
                let Some(data) = arr.get(2) else {
                    return Ok(Value::Null);
                };

                if let Some(data_str) = data.as_str() {
                    return serde_json::from_str::<Value>(data_str)
                        .or(Ok(Value::String(data_str.to_string())));
                }
                return Ok(data.clone());
            }
        }
    }

    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_minimal_wrb_response() {
        let response = ")]}'\n57\n[[\"wrb.fr\",\"wXbhsf\",\"[[[]]]\",null,null,null,\"generic\"]]";
        let decoded = decode_response(response, "wXbhsf", false).expect("decode should work");
        assert_eq!(decoded, serde_json::json!([[[]]]));
    }
}
