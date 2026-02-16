.PHONY: test test-verbose test-watch test-coverage lint clippy fmt fmt-check clean help

test:
	cargo test --all-features --no-fail-fast

test-verbose:
	cargo test --all-features --no-fail-fast -- --nocapture

test-watch:
	cargo watch -x test --all-features

test-coverage:
	cargo tarpaulin --all-features --out Xml --output-dir target/tarpaulin

lint:
	cargo clippy --all-features -- -D warnings

clippy:
	cargo clippy --all-features -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

clean:
	cargo clean

help:
	@echo "Available targets:"
	@echo "  test              - Run all tests"
	@echo "  test-verbose      - Run tests with verbose output"
	@echo "  test-watch        - Run tests in watch mode"
	@echo "  test-coverage     - Generate code coverage report"
	@echo "  lint/clippy       - Run linter"
	@echo "  fmt               - Format code"
	@echo "  fmt-check         - Check code formatting"
	@echo "  clean             - Clean build artifacts"
