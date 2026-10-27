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

No agent provider currently creates approval requests. This broker and UI are the required boundary for future integrations, not a claim that provider actions are already available.

## Child-process lifecycle

Provider adapters must launch a configured executable with a discrete argument vector, never by interpolating input into a shell command. Each child must be registered with `ProcessRegistry`. It can be polled for an exit status so the adapter can decide whether to recover, explicitly terminated on cancellation, and killed/reaped on app shutdown or when its managed handle is dropped.

The current application does not launch agent processes. The registry's lifecycle behavior is tested independently; process-specific restart policy will live in the adapter that owns the process.

## Tauri capabilities

The desktop capability remains limited to core defaults and the native open-dialog command. Do not add filesystem or shell plugins to the frontend capability without a reviewed use case and matching project-scope checks.
