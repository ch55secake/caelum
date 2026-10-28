# Security boundaries

## Project filesystem access

- The React UI has no direct filesystem API. The desktop's only filesystem interaction is opening a project through Tauri's native directory picker.
- Treat every Tauri command argument as untrusted. `Project::open` rejects empty paths, canonicalizes selected roots, and only accepts directories.
- Filesystem operations must resolve existing paths through `Project::resolve_scoped_path`. It canonicalizes the target and rejects paths, including symlinks, that escape the selected project.
- The scoped resolver intentionally covers existing paths only. Any future file-creation API must also defend against symlink races (for example, using directory-handle-relative operations) before it is exposed.

## Provider approvals

Provider adapters submit `PermissionRequest` values that retain provider-specific details as opaque data. The UI sees only the provider name, request ID, summary, and description; provider-specific payloads are not forwarded. Caelum does not collapse provider permission modes into a single broad permission model. Adapters must clearly disclose when a request grants broad system access rather than implying that it is confined to the selected project.

- Every request requires a user decision: **Allow once** or **Deny**.
- There is no persistent allow-all decision.
- Timeouts, cancellation, app shutdown, unknown IDs, and dropped waiters resolve as denied.

The Codex adapter routes app-server command-execution and file-change approval requests through this broker. Caelum responds with a one-request `accept` or `decline`; unsupported request methods are rejected instead of being silently granted. Codex runs with its `workspaceWrite` sandbox and `unlessTrusted` approval policy. Review those provider-managed sandbox semantics separately from Caelum's own filesystem boundary.

The OpenCode adapter maps server `permission.asked` requests to the same broker and responds with OpenCode's one-shot `once` or `reject` decision. It never answers `always`. OpenCode applies its own project permissions and tool sandboxing; those provider-managed rules do not become Caelum filesystem restrictions.

## Child-process lifecycle

Provider adapters must launch a configured executable with a discrete argument vector, never by interpolating input into a shell command. Each child must be registered with `ProcessRegistry`. It can be polled for an exit status so the adapter can decide whether to recover, explicitly terminated on cancellation, and killed/reaped on app shutdown or when its managed handle is dropped.

The Codex adapter starts `codex app-server --listen stdio://` with an explicit argument vector and validated project working directory. The child is registered with `ProcessRegistry`; an unexpected exit fails outstanding requests and the next operation starts a fresh server. App shutdown terminates/reaps all registered provider children. The stdio transport avoids exposing an app-server network listener.

The OpenCode adapter starts `opencode serve --hostname 127.0.0.1 --port <available-port>` with the validated project as its working directory. The listener is not exposed on a network interface. OpenCode generates a random password by default; Caelum captures it from startup output and keeps it in memory for local Basic authentication. If `OPENCODE_SERVER_PASSWORD` is configured, Caelum reuses it instead. Provider credentials remain under OpenCode's control.

## Tauri capabilities

The desktop capability remains limited to core defaults and the native open-dialog command. Do not add filesystem or shell plugins to the frontend capability without a reviewed use case and matching project-scope checks.
