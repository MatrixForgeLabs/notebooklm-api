# Configuration

## Authentication

The client reads NotebookLM auth cookies from a Playwright `storage_state.json`.

CLI:
- `--storage <PATH>` to override the default location.

Library:
- `NotebookLmClient::from_storage(Some(path))`
- `NotebookLmClient::from_storage(None)` for default path discovery.

## Retry Policy

Retry policy is configurable at client construction:

```rust
use notebooklm_api::RetryPolicy;

let client = NotebookLmClient::from_storage(None)
    .await?
    .with_retry_policy(RetryPolicy {
        max_retries: 3,
        base_delay_ms: 250,
        max_delay_ms: 5000,
        jitter_ms: 200,
    });
```

Applied to:
- RPC calls
- Query endpoint calls
- Binary download calls

Behavior:
- One-shot auth refresh on 401/403
- Exponential backoff + jitter for retryable failures
- Retryable statuses: 429 and 5xx

## Output Modes (CLI)

Global `--output`:
- `json`
- `table`
- `tsv`

`--quiet` suppresses non-essential status lines where applicable.
