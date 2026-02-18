.PHONY: fmt check test clippy ci run-help

fmt:
	cargo fmt

check:
	cargo check --all-targets

test:
	cargo test --all-targets

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

ci: fmt check clippy test

run-help:
	cargo run --bin notebooklm -- --help
