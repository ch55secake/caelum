.DEFAULT_GOAL := help

PROJECT ?= .

.PHONY: help install dev app frontend build cli cli-run format lint test check

help:
	@printf '%s\n' \
	  'Caelum local development:' \
	  '  make install             Install frontend dependencies' \
	  '  make dev                 Run the Tauri desktop app in development mode' \
	  '  make app                 Build the macOS .app bundle' \
	  '  make frontend            Build the frontend only' \
	  '  make cli                 Build the standalone CLI' \
	  '  make cli-run PROJECT=.   Run the CLI against a project path' \
	  '  make check               Run formatting, lint, tests, and frontend build'

install:
	pnpm install --frozen-lockfile

dev: install
	pnpm tauri:dev

app: install
	pnpm --filter @caelum/desktop tauri build --bundles app

frontend: install
	pnpm desktop:build

build: app cli

cli:
	cargo build -p caelum-cli

cli-run:
	cargo run -p caelum-cli -- "$(PROJECT)"

format:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

check: format lint test frontend
