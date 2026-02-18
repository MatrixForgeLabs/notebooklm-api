# notebooklm-api

Rust client library and CLI for Google NotebookLM.

`notebooklm-api` ports the Python `notebooklm-py` functionality to Rust for:
- Embedding NotebookLM automation in Rust applications (library/crate)
- Scriptable terminal automation (CLI)

## Status

This project is functional and actively ported from `notebooklm-py`.
Implemented API groups:
- Notebooks
- Sources
- Chat + history
- Artifacts (generate/list/download/export)
- Research
- Sharing
- Settings

See `PORTING.md` for detailed parity tracking.

## Installation

### Library

Add to `Cargo.toml`:

```toml
[dependencies]
notebooklm-api = "0.1.0-alpha"
```

### CLI

```bash
cargo install --path .
# then
notebooklm --help
```

## Authentication

Authentication uses Playwright/Chromium storage state cookies.
Default search path is handled by the crate (same flow as existing implementation).

You can pass an explicit storage state file to CLI commands:

```bash
notebooklm --storage /path/to/storage_state.json auth-status
```

## Quickstart (CLI)

```bash
# List notebooks
notebooklm notebook list

# Create one
notebooklm notebook create "My Notebook"

# Add source
notebooklm source <notebook_id> add-url "https://example.com"

# Ask a question
notebooklm chat <notebook_id> "Summarize this"

# Generate typed artifact
notebooklm artifact <notebook_id> generate-audio \
  --audio-format deep-dive \
  --audio-length long
```

Global output controls:

```bash
notebooklm --output json notebook list
notebooklm --output tsv artifact <notebook_id> list
notebooklm --quiet notebook <notebook_id> delete
```

## Quickstart (Library)

```rust
use notebooklm_api::client::NotebookLmClient;
use notebooklm_api::{AudioGenerationOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = NotebookLmClient::from_storage(None).await?;
    let notebooks = client.notebooks().list().await?;
    println!("count={}", notebooks.len());

    if let Some(nb) = notebooks.first() {
        let status = client
            .artifacts()
            .generate_audio(&nb.id, AudioGenerationOptions::default())
            .await?;
        println!("task_id={} status={}", status.task_id, status.status);
    }

    Ok(())
}
```

## Production Notes

- API methods are based on reverse-engineered RPC identifiers and payloads.
- Breakage can occur if NotebookLM changes internal RPC schemas.
- Use retry policy tuning and robust error handling in production.
- Pin crate versions and test against your own fixtures/cassettes.

## Documentation

- `docs/cli.md`: full CLI usage
- `docs/library.md`: library usage patterns
- `docs/configuration.md`: auth/retry/output config
- `docs/architecture.md`: internal architecture
- `docs/operations.md`: runbook and observability guidance
- `docs/security.md`: security handling for credentials
- `docs/release.md`: release process
- `CONTRIBUTING.md`: contributor workflow

## Development

```bash
cargo fmt
cargo check
cargo test
cargo run --bin notebooklm -- --help
```

CI runs formatting, build, clippy, and tests.

## License

MIT
