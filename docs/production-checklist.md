# Production Readiness Checklist

## Build Quality

- [ ] `cargo fmt --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets`
- [ ] `cargo package --allow-dirty --no-verify`

## Security

- [ ] No auth materials (`storage_state.json`, cookies, tokens) committed
- [ ] `SECURITY.md` and `docs/security.md` reviewed
- [ ] Logging policy avoids credential leakage

## API/CLI Contract

- [ ] Typed option enums and payload builders covered by tests
- [ ] Output contract verified for `--output json|table|tsv`
- [ ] Error variants mapped and handled by caller paths

## Operational Confidence

- [ ] Retry policy chosen explicitly for target environment
- [ ] Representative fixture tests added for parser-sensitive paths
- [ ] Smoke run executed on target platform

## Release

- [ ] `CHANGELOG.md` updated
- [ ] Version bumped (if publishing)
- [ ] Tag/release notes prepared
