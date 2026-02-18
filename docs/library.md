# Library Usage

## Client Construction

```rust
use notebooklm_api::client::NotebookLmClient;
use notebooklm_api::{Result, RetryPolicy};

#[tokio::main]
async fn main() -> Result<()> {
    let client = NotebookLmClient::from_storage(None)
        .await?
        .with_retry_policy(RetryPolicy {
            max_retries: 4,
            base_delay_ms: 300,
            max_delay_ms: 8000,
            jitter_ms: 250,
        });

    let notebooks = client.notebooks().list().await?;
    println!("{} notebooks", notebooks.len());
    Ok(())
}
```

## API Groups

- `client.notebooks()`
- `client.sources()`
- `client.chat()`
- `client.artifacts()`
- `client.research()`
- `client.sharing()`
- `client.settings()`

## Artifact Generation (Typed)

```rust
use notebooklm_api::{
    AudioFormat, AudioGenerationOptions, AudioLength,
    ReportFormat, ReportGenerationOptions,
};

let audio = client.artifacts().generate_audio(
    notebook_id,
    AudioGenerationOptions {
        source_ids: None,
        language: "en".into(),
        instructions: Some("Keep it concise".into()),
        format: Some(AudioFormat::Brief),
        length: Some(AudioLength::Short),
    },
).await?;

let report = client.artifacts().generate_report(
    notebook_id,
    ReportGenerationOptions {
        source_ids: None,
        language: "en".into(),
        format: ReportFormat::StudyGuide,
        custom_prompt: None,
    },
).await?;
```

## Error Handling

Main error type: `notebooklm_api::NotebookLmError`.

Important variants:
- `Auth`, `StaleAuth`
- `RateLimit { retry_after }`
- `Timeout`, `Network`
- `Client`, `Server`
- `Rpc`, `RpcDecode`, `DecodeShape`

Pattern:

```rust
use notebooklm_api::{NotebookLmError, Result};

match client.notebooks().list().await {
    Ok(v) => println!("{}", v.len()),
    Err(NotebookLmError::RateLimit { retry_after, .. }) => {
        eprintln!("rate limited: retry_after={retry_after:?}");
    }
    Err(e) => return Err(e),
}
```

## Notes

- Treat RPC payload/response shape as unstable.
- Pin crate versions and keep parser tests with fixtures.
- Use typed option structs over raw payload manipulation.
