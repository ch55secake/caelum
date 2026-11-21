# State machine audit

Encode a phase and its required data together. Keep independent lifecycles
separate: a session can fetch history while the provider is still executing.
Keep request identity checks at the point where asynchronous results enter the
state machine; types cannot establish that an external event is current.

## Converted

| Owner | Typed invariant |
| --- | --- |
| [Conversation turn timing](../crates/caelum-core/src/application/conversation/turn.rs) | Only an in-progress timing record has a start time, captured agent, and output IDs. Provider start is a separate phase that cannot reinitialize an already confirmed turn. |
| [OpenCode timing](../crates/caelum-core/src/application/conversation/opencode.rs) and [history reconstruction](../crates/caelum-core/src/application/conversation/timing.rs) | Step model and timestamp travel with the message ID; history keeps timing data only inside a reconstructed turn. |
| [Approvals](../crates/caelum-core/src/approvals.rs) and [processes](../crates/caelum-core/src/processes.rs) | Closed registries do not own pending approvals, subscribers, or children. Waiter cleanup also verifies the approval attempt, not just the provider request ID. |
| [CLI event handling](../apps/cli/src/main.rs) | A provider event has one outcome, and completion carries a typed terminal status rather than a string. |
| [Workspace persistence](../crates/caelum-core/src/application/workspace/persistence.rs) | Only a loaded store can save. A save owns its snapshot and held commands; only its matching completion can release them. |

## Remaining candidates

| Owner | State currently split across fields | Invariant to encode |
| --- | --- | --- |
| [Workspace submission](../crates/caelum-core/src/application/workspace/submission.rs) | Submission, stop, in-flight prompt, queued origin, and pending request | Creating a session and awaiting send admission each own the captured prompt and options; a stop waits on the relevant phase instead of a separate flag. Native commands can overlap a provider turn. |
| [Workspace history](../crates/caelum-core/src/application/workspace/history.rs) | Loaded/loading flags, next cursor, disconnect, pending read | Distinguish initial load, refresh of usable data, and pagination. Keep the cursor with the loaded history, and attach the request identity to the read phase. |
| [Session archive and deletion](../crates/caelum-core/src/application/workspace/archive.rs) | Archived, archiving, deleting, forgotten, discovery barrier | Only eligible source phases can mutate; a failed mutation restores its source phase. A forgotten slot remains addressable internally but cannot act as an open session. |
| [Archive notice](../crates/caelum-core/src/application/workspace/archive.rs) and [session titles](../crates/caelum-core/src/application/workspace/titles.rs) | Notice target, clock, and replacement stored separately; manual mode, turn count, and pending regeneration | A visible notice owns its target and timer. Only automatically titled sessions accumulate turns; regeneration owns the captured session key. |
| [Inbox](../crates/caelum-core/src/application/inbox.rs) | Pending/removing flags and error | Distinguish enqueue from removal, preserving the underlying admission state where operations overlap. Convert the existing snapshot representation at the boundary. |
| [Conversation activity and entries](../crates/caelum-core/src/application/conversation.rs) | Working/retrying flags, entry kind, completion, provider status, background | Model provider activity separately from timing. Give each entry kind the lifecycle and payload it supports; accept unknown provider statuses at the decoding boundary. |
| [Review and service requests](../crates/caelum-core/src/application/services.rs) | Review source and details are separate; request slots share latest/loading/error maps | Tie cached/live details and file pages to a verified revision. A loading slot owns its request; refresh can retain visible data while a new request is pending. |
| [Questions](../crates/caelum-core/src/questions.rs) | Disabled broker flag and per-entry submitting flag | A disabled broker cannot own an answerable request; only an awaiting request can begin submission. Keep callback identity checks on reply. |
| [Branch attribution](../crates/caelum-core/src/session_branches.rs) | Active entry, optional turn ID, ambiguous flag | Separate scoped attribution from ambiguity and retain the provider identity needed to reject a late completion. |
| [Desktop worktree form](../apps/desktop/src/worktree.rs) and [panels](../apps/desktop/src/panels.rs) | Form initialized/submitted flags, screen tags and optional payloads, file/search/review bookkeeping | Bind overlay contents and resource owners to their screens. In-flight creation must retain the submitted task rather than infer it from mutable fields. |
| [Desktop completion](../apps/desktop/src/completion.rs), [sidebar](../apps/desktop/src/live/sidebar.rs), and [remote UI](../apps/desktop/src/remote/mod.rs) | Pending tuples, scheduled deadlines, optional owner-specific UI data | Keep request and timer identities with the states they can advance. Sort out independent focus, navigation, and rendering state before merging fields. |
| [Desktop preview](../apps/desktop/src/state.rs) | Optional stream and separate turn flags | A streaming preview response owns its turn and clock rather than acting on whichever turn is last. |

Updates, OpenCode connection slots, conversation controls, relocation, and remote
checkout and turn lifecycles already use phase enums. Review their event
admission separately from representation changes. Persisted snapshots and
provider protocol structs are compatibility boundaries, not authoritative
runtime state.
