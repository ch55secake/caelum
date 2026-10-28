# OpenCode provider

Caelum integrates with the local OpenCode v2 server API. It starts one `opencode serve` process per project, bound to `127.0.0.1` on an available local port, and uses the server's generated HTTP API and `/api/event` SSE stream. The API currently identifies itself in `/openapi.json` as an experimental HTTP surface; this adapter targets that v2 API rather than older `/session` endpoints.

## Setup

1. Install OpenCode and make `opencode` available on `PATH`. Caelum also checks common Homebrew and user-local install paths.
2. Configure and sign in to at least one provider in OpenCode. Caelum relies on OpenCode's existing provider configuration and credentials and does not copy or store them.
3. Open a project in Caelum and select **OpenCode** in the provider card. Caelum starts its local server when sessions are loaded or created.

OpenCode's managed v2 server always uses Basic authentication: when `OPENCODE_SERVER_PASSWORD` is unset, the CLI generates a random password and prints it during startup. Caelum captures that startup password privately and authenticates using OpenCode's default `opencode` username. If `OPENCODE_SERVER_PASSWORD` is set for Caelum, the managed server inherits and reuses it instead. You do not need to configure a password manually. Authentication failures and HTTP/API errors are shown in the desktop error banner.

## Protocol behavior

- The adapter verifies the local server by listing sessions through `GET /api/session` and scopes all session operations to the canonical project directory.
- Sessions are created, restored, and listed through the v2 `/api/session` endpoints. Prompts use `/api/session/{sessionID}/prompt`; cancellation uses `/interrupt`.
- `/api/event` streams native server events. Text deltas and tool lifecycle events are forwarded to the desktop; session history is loaded from the message endpoint on resume.
- `permission.asked` events are shown in the shared approval dialog. **Allow once** maps to OpenCode's `once` decision; denial maps to `reject`. Caelum never sends the `always` decision.
- Managed server processes are registered for application shutdown and restarted on the next provider request after an SSE disconnect.

The integration follows the API exposed by the installed OpenCode binary, so API changes may require an adapter update. The running server's `/openapi.json` is the authoritative schema for that installation.
