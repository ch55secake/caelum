# Caelum Rune candidate

The native replacement candidate, using Caelum's shared Rust providers and
application coordinator. Tauri remains the shipping client during the migration.

From the repository root:

```sh
nix develop -c cargo run --locked --manifest-path apps/rune-prototype/Cargo.toml
```

Use **Open project** (Cmd+O), or append `-- --project /path/to/project`. Installed,
authenticated Codex/OpenCode providers supply sessions, paged history, streaming,
tools and approvals. **Session options** selects the provider for a new session,
model, reasoning effort and agent. Refresh reconnects and reloads history.

For the original deterministic comparison, append `-- --fixtures`. Only fixture
mode simulates replies and displays the sample diff.

## Current milestone

Stage A implements real-session I/O, scoped background activity, drafts retained
while navigating, explicit recovery of failed submissions and one-shot approvals.
Pending questions are visible and can be dismissed; native answer forms are still
pending. Provider-owned history is persistent; app-owned drafts, navigation and
preferences currently last for this run only.

Queues, attachments, full reconnect reconciliation, native question forms,
files/worktrees, GitHub workflows and saved settings remain migration work. This
is not yet the 90% evaluation build. See the [migration plan](../../docs/design/rune-migration.md)
and [comparison guide](../../docs/design/README.md) for checkpoints and checks.
