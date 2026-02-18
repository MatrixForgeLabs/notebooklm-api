# Operations Runbook

## Build and Verify

```bash
cargo fmt
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Common Failure Classes

- `Auth` / `StaleAuth`: session expired
- `RateLimit`: temporary throttling
- `DecodeShape` / `RpcDecode`: upstream response shape changed
- `Network` / `Timeout`: transient transport errors

## Incident Response

1. Reproduce with `--output json` and capture payload context.
2. Check whether failure is:
   - auth issue,
   - rate limiting,
   - parser shape drift,
   - transport issue.
3. If parser drift: add/adjust fixture test and update parser path guards.
4. If RPC method drift: verify method IDs and payload builders.

## Reliability Recommendations

- Keep retry policy explicit for production workloads.
- Wrap API calls with your own service-level retries/circuit breakers.
- Log method + status + error variant (avoid logging cookies/tokens).
