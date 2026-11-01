# OpenCode provider

Caelum integrates with the local OpenCode v2 server API and `/api/event` SSE stream. It attaches to a healthy registered background service so sessions running in other OpenCode clients update live. If no healthy service is available, it starts an owned `opencode serve` process per project, bound to `127.0.0.1` on an available local port.

## Setup

1. Install OpenCode and make `opencode` available on `PATH`. Caelum also checks common Homebrew and user-local install paths.
2. Configure and sign in to at least one provider in OpenCode. Caelum relies on OpenCode's existing provider configuration and credentials and does not copy or store them.
3. Open a project in Caelum and select **OpenCode** in the new-session composer. Caelum connects when sessions are loaded or created.

Service discovery reads `${XDG_STATE_HOME:-~/.local/state}/opencode/service.json`, verifies the loopback endpoint's readiness, PID, and version through `/api/info`, and uses the registered Basic authentication password when present. The shared service remains running when Caelum closes. An explicitly configured executable uses an owned server instead.

OpenCode's managed v2 server always uses Basic authentication: when `OPENCODE_SERVER_PASSWORD` is unset, the CLI generates a random password and prints it during startup. Caelum captures that startup password privately and authenticates using OpenCode's default `opencode` username. If `OPENCODE_SERVER_PASSWORD` is set for Caelum, the managed server inherits and reuses it instead. You do not need to configure a password manually. Authentication failures and HTTP/API errors are shown in the desktop error banner.

## Protocol behavior

- The adapter verifies the local server by listing sessions through `GET /api/session` and scopes all session operations to the canonical project directory.
- Sessions are created, restored, and listed through the v2 `/api/session` endpoints. Prompts use `/api/session/{sessionID}/prompt`; cancellation uses `/interrupt`.
- Archiving uses `PATCH /api/session/{sessionID}` with `archived: true`; Undo sends `false`. Active sessions cannot be archived, and listings request `archived=false`. The desktop removes the scoped saved session/selection, invalidates pending snapshots, and retains the latest archived draft/settings for Undo.
- Sidebar rows and PR discovery load optional details without blocking navigation. `/api/session/{sessionID}/diff?context=0` supplies added/deleted line counts for the newest user-message turn. Caelum records the local branch on `session.execution.started` and succeeded/failed/interrupted completion events, persisting associations across restart and archive/undo. Directory-wide `/api/vcs` metadata is not used to attribute branches to historical sessions; older sessions gain an association the next time they execute while connected. Concurrent sessions in one checkout suppress ambiguous updates. Missing diff metadata leaves counts absent; completed turns and refreshes update them.
- `/api/event` streams native server events. Caelum waits for the first stream frame before loading snapshots or sending prompts. Text, reasoning, and tool lifecycle events retain stable identities and their original order; text and reasoning ordinals are scoped by message and fragment kind. Thinking appears in expandable Markdown with a compact preview.
- History pages retain native message content, tool input/output, errors, timing, and current session activity. The initial message page uses `order=desc`; subsequent pages use only the opaque cursor and limit. The desktop refreshes the selected session on focus, selection, and periodically to recover missed events, reconciling snapshots with any newer live updates.
- The composer outline pulses throughout thinking, responses, tool calls, and retries, with a static cue for required approvals. Running tools show elapsed time. A completed model step does not end the session's working state, and reduced-motion preferences disable the pulse.
- Available models, model-specific effort variants, and primary agents come from `/api/model` and `/api/agent`; `/api/model/default` resolves the default model. New-session model/agent selections are passed to session creation. Pending model and effort selections are applied through `/api/session/{sessionID}/model` before the next prompt, with effort encoded as `model.variant`. Default effort omits the variant; switching models resets it. Failed submissions retain selections and the draft for retry. Model-selection events and history refreshes keep the configured model and effort current across clients. Prompts carry a client message ID so optimistic messages reconcile with persisted history.
- `permission.asked` events are shown inline in the requesting session. **Allow once** maps to OpenCode's `once` decision; denial maps to `reject`. Caelum never sends the `always` decision.
- Owned server processes are registered for application shutdown and restarted on the next provider request after an SSE disconnect. Reconnecting to a shared service rediscovers its current registration.

The integration follows the API exposed by the installed OpenCode binary, so API changes may require an adapter update. The running server's `/openapi.json` is the authoritative schema for that installation.
