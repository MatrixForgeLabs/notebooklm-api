# Architecture

## Modules

- `src/auth.rs`: storage-state auth loading and token extraction
- `src/rpc.rs`: RPC IDs, request encoding, response decode
- `src/client.rs`: high-level APIs + retry/error mapping
- `src/types.rs`: typed models and generation option enums
- `src/error.rs`: unified error taxonomy
- `src/bin/notebooklm.rs`: CLI interface

## Request Flow

1. Build RPC/query request + auth parameters
2. Execute via `reqwest`
3. Apply retry policy for retryable transport/status failures
4. Optionally refresh auth on 401/403 and retry
5. Decode to typed output

## Parsing Strategy

- Centralized path helpers (`json_at`, `required_json_at`)
- `DecodeShape` errors for malformed responses
- Fixture-backed parser tests for known payload structures

## Mind Map Persistence

Mind-map generation returns content and then persists into notes with:
- `CreateNote` RPC
- `UpdateNote` RPC

This mirrors parity behavior where generation content is explicitly saved.
