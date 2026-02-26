# Contributing

## Setup

```bash
git clone <repo>
cd notebooklm-api
cargo check
cargo test
```

## Development Rules

- Prefer typed enums/option structs over raw numeric/string params.
- Add or update tests for payload/parser behavior.
- Keep CLI and library behavior aligned.
- Avoid introducing breaking changes without explicit version bump.

## Quality Gates

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Pull Request Checklist

- [ ] Feature/fix implemented
- [ ] Tests added/updated
- [ ] Docs updated (`README.md`, as needed)
- [ ] No secrets committed
