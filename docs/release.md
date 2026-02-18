# Release Process

## Preconditions

- `cargo fmt` clean
- `cargo check` clean
- `cargo test` clean
- `cargo clippy --all-targets --all-features -- -D warnings` clean
- docs updated (`README.md`, `PORTING.md`, `docs/*`)

## Versioning

Use semantic versioning.

- Patch: fixes and non-breaking parser updates
- Minor: additive APIs/options
- Major: breaking API or CLI contract changes

## Steps

1. Update `Cargo.toml` version.
2. Update `CHANGELOG.md`.
3. Tag release (`vX.Y.Z`).
4. Publish crate if desired:
   ```bash
   cargo publish
   ```
5. Build release binaries for distribution:
   ```bash
   cargo build --release
   ```

## GitHub Actions Release Automation

Tag push workflow: `.github/workflows/release.yml`

- `verify`: fmt/check/clippy/test on Linux + Windows (package check on Linux)
- `publish`: runs `cargo publish --locked` when `CARGO_REGISTRY_TOKEN` is configured
- `release-binaries`: uploads Linux and Windows `notebooklm` binaries to GitHub release

Required secret for crate publish:
- `CARGO_REGISTRY_TOKEN`

## Post-release

- Validate CLI smoke commands on clean environment.
- Confirm auth + notebook list + one artifact generation path.
