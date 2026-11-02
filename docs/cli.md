# CLI

The standalone `caelum` CLI shares the desktop's Rust core and provider adapters. Terminal workflows work without the desktop app.

```sh
caelum [PROJECT]
caelum chat [PROJECT] --provider codex
caelum chat [PROJECT] --provider opencode
caelum run "Fix the failing tests" --project .
caelum sessions [PROJECT] --provider opencode
```

Without a subcommand, `caelum [PROJECT]` opens the project in the installed Caelum desktop app on macOS. The project defaults to the current directory; the provider defaults to Codex.

Use `--session ID` with `chat` or `run` to resume a provider-owned session. `/quit` exits interactive chat. Provider permissions are denied unless explicitly approved once in the terminal.

See [development](development.md) to build the CLI, and the [Codex](codex.md) and [OpenCode](opencode.md) setup guides for provider requirements.
