# Caelum

An AI coding workspace for macOS, with a native Rune desktop app and standalone Rust CLI. Work with project-scoped **Codex** and **OpenCode** sessions, browse files, review changes and manage GitHub pull requests in one place.

- Independent session drafts, streamed responses, tool activity and restored history.
- Project and worktree navigation with remembered sessions and live Git status.
- File search, read-only previews and file/selection attachments.
- Pull-request reviews, checks, branch updates and merging through GitHub CLI.
- Local themes, model favourites and provider defaults.

## Get started

[Download a release](https://github.com/ch55secake/caelum/releases), then configure [Codex](docs/codex.md) or [OpenCode](docs/opencode.md). Open a project and send your first message.

For local development, install Rust 1.95.0 and Xcode Command Line Tools, then run:

```sh
make dev
```

The repository also provides a `nix develop` shell. See [development setup](docs/development.md) for build commands and [contributing](docs/contributing.md) for checks.

## Documentation

- [Desktop guide](docs/desktop.md) — sessions, commands, files, worktrees, settings and shortcuts
- [Pull requests](docs/pull-requests.md) — discovery, review feedback, checks and merging
- [CLI guide](docs/cli.md) — terminal sessions and opening desktop projects
- [Codex setup](docs/codex.md) · [OpenCode setup](docs/opencode.md)
- [Architecture](docs/architecture.md) · [Security](docs/security.md)
- [Development](docs/development.md) · [Contributing](docs/contributing.md)
- [Releasing](docs/releasing.md) · [Roadmap](docs/roadmap.md)
- [Workspace design](docs/design/README.md) — native surfaces, previews and interaction fixtures

## Repository

```text
apps/desktop/          Native Rune desktop application
apps/cli/              Standalone CLI
crates/caelum-core/    Shared application logic and provider adapters
crates/caelum-protocol/ Serializable interface types
docs/                 User and developer documentation
```

[MIT License](LICENSE).
