# Run Codex or OpenCode on Boat

A Boat VM has its own Git checkout and disk. Choose it as the destination for
a normal Codex or OpenCode session. Your composer, transcript and session list
stay in the same Caelum window; the provider, files and Git run on the VM.
Configure provider credentials in [Boat → Agents & Proxying](https://boat.dev/dashboard?tab=agents).

## Create or attach a checkout

1. Connect GitHub and credentials for the provider you want to use in your
   [Boat account](https://docs.boat.dev/integrated-agents/keys.md). In Caelum,
   open **Settings → Remote machines**, enter your Boat API key and click
   **Connect Boat**. The key is stored in Keychain or Linux Secret Service.
2. Click **Create VM…** in **Settings → Remote machines** or **New session**.
   Use the current project, choose another, or open a project folder with a GitHub `origin`.
3. **Session branch** shows the current branch of the selected checkout,
   including a linked worktree. Publish it to `origin`, choose the [machine size](https://docs.boat.dev/machines.md)
   and auto-stop, then click **Create VM**. Creation prepares the checkout;
   your first **Send** starts the conversation.
   Caelum checks the published ref and Boat’s clone branch before allocating the VM.
   It uses a matching single-repo
   environment or creates a dedicated one. Unpublished commits, staged changes
   and files on your laptop stay there.

![Create VM with the checkout’s published branch and selected machine size](design/previews/rune-light-remote-create-ready-small.png)

To attach instead, right-click a project, choose **Open on remote machine…**,
and choose the machine under **Existing VM**. It must have a
current single-repo environment for the project's GitHub origin. **Resume** a
stopped machine to use its saved disk; attaching keeps its current branch.
If setup fails, **Refresh checkout** retries that checkout on the same machine.

If setup reports a missing checkout, look on the VM itself. Replace `VM_ID`
with the Boat ID shown for that machine:

```sh
boat exec VM_ID 'pwd -P; find "$HOME" -maxdepth 3 -name .git -prune -print'
```

Boat normally clones into `$HOME/<repository>`. Caelum checks the VM command
workdir and home for a Git checkout with the selected `origin`, so the folder
need not have the repository’s name. If it lives elsewhere, a link from an
unused `$HOME/<repository>` to the checkout also works.
On a newly created VM Caelum can clone a missing repository using the VM’s
GitHub access. When attaching to an existing VM, restore the missing checkout
first, then click **Refresh checkout**. If it was previously verified, restore
it at its saved path so the same sessions and files are reopened.

## Open a VM session

Click a VM in the sidebar to return to its session, or open **New session**
(**⇧⌘N**), choose a provider and pick the VM under the project. You can search
by machine name, repository or branch. **⌘N** in a VM session
starts another draft on that checkout. Select any local or VM row in **Sessions**
to switch back, even while another session streams.
The normal session list, model and effort pickers, transcript, questions,
approvals and background-shell output use the provider installed on the VM.
**Files** and **Changes** read that checkout. Opening or switching
sessions does not submit a message.

![A VM session open in the same window as local sessions](design/previews/rune-dark-native-connected-small.png)

![New session with Codex selected and a ready Boat VM listed under its project](design/previews/rune-dark-vm-session-launcher-small.png)

The Boat key needs the [`ssh` action](https://docs.boat.dev/ssh-access).
Caelum installs its matching Linux peer on first use and uses the VM’s own
provider configuration and credentials. Drafts and session settings are kept
on this computer, scoped to their account, machine and repository. **Save as
default** in a VM session sets the model, effort and agent for new sessions on
that checkout. Provider defaults, favourite models and drafts from the earlier
separate-window preview are imported on startup.

If SSH drops, you can keep editing. Caelum tries to reconnect once; if the VM
has stopped, **Resume** it from machine stats and reconnect in the same session.
The connection verifies the new host key, reloads history and
restores pending questions and approvals. Check the transcript before retrying
a message marked unconfirmed. Closing the window detaches; it does not stop
the VM. SSH activity does **not** extend Boat’s auto-stop timer.

![A disconnected native workspace with its OpenCode session and unsent draft still visible](design/previews/rune-light-native-offline-small.png)

## Connected, but Create VM returns 403

Connecting checks that the key can read the account and list VMs. Creating a VM
needs `sandbox.create` for its environment. **First-time repository setup also
needs `environment.write`** to create and configure the Boat environment.

- If Caelum reports **Create checkout environment** or **Configure checkout
  environment** and your key lacks `environment.write`, create a scoped key
  with your current permissions plus `environment.write` in [Boat → API Keys](https://boat.dev/dashboard?tab=api-keys).
  Enter it in **Settings → Remote machines → Connect Boat** for the same
  account. Reopen **one** failed checkout, or click **Refresh checkout** if it’s
  already open, to retry its original request.
- To keep a key without `environment.write`, configure an [environment in Boat](https://docs.boat.dev/environments/repositories.md)
   with just this repository, its exact clone branch, GitHub access and agent credentials enabled, and
  **Safe for third parties** off. Reopen VM setup so Caelum discovers it. The
  key still needs `environment.read` and `sandbox.create` for that environment,
  `exec` to prepare the checkout, and `ssh` to connect a native session.
  Managed Boat conversations use `agent.prompt` to send a prompt.

If the failing step is **Create VM**, check the action **and** the key’s allowed
environments, or follow the size restriction in the error. See [Boat key scopes](https://docs.boat.dev/api-keys/scopes.md).

## Continue a local session on a VM

Click the **server icon** beside **Context** in a local Codex or OpenCode composer.
On **Home** it’s beside **Send**. The picker opens at the icon; on Home, choose the
Home draft or a recent session. Finish any active local turn and queued prompts first.

For a handoff, **Session branch** is the current branch of that session’s exact
worktree (or Home’s chosen checkout). A new VM opens **that same branch** at
its published commit on `origin`. The name is shown for reference; there’s
nothing to enter. An existing VM must already be on that branch and contain the
published commit.

![Handoff showing the session’s published commit and options for a new VM](design/previews/rune-dark-remote-handoff-new-small.png)

Choose an existing VM, or choose the size and auto-stop time and click
**Create VM**. While Boat prepares the checkout, a progress card takes the
place of the message editor. The editor returns with a copy of your unsent draft.
Your original session and draft stay on the laptop.
If the VM already had a conversation, its row stays in **Sessions**.

The first time you press **Send**, Caelum includes the local user and assistant
messages as context for the native session on the VM. The VM composer starts with
the provider from your local session or Home draft. Setup shows how much chat
will travel: up to the most recent 200 messages or 64 KiB of text. Tool output,
reasoning, attachments and unpublished Git work stay local.

## Existing managed Boat chats

Managed conversations from earlier builds remain in **Sessions**. You can also
right-click the machine and select **Manage VM and Boat conversations…**.
These conversations use [Boat’s integrated agent API](https://docs.boat.dev/platform-guide/agents.md)
and keep their existing Boat conversation IDs and provider settings.

**Before your first Send**, open **Boat default** beside **Codex** or **OpenCode**
in the VM composer. Search [Boat’s model catalogue](https://docs.boat.dev/integrated-agents/harnesses-and-models.md#the-model-catalog)
and choose a model your Boat credentials support. You can change your choice
while drafting. Only **Send** starts the
conversation, using the last choice you made.

![Choosing a model with the first VM message still in the editor](design/previews/rune-dark-vm-first-model-small.png)

Each VM chat remembers its choice. Between turns you can pick another model in
the same conversation. Changing providers resets the model to **Boat default**.
That option uses your Boat settings and works even when the catalogue can’t load.

To change reasoning depth, choose a specific model, then open **Effort** beside
it. The menu lists only the levels Boat advertises for that provider and model.
For example, you can choose **High** before the first Send, then choose a lower
level for a follow-up without leaving the conversation.

![Choosing High reasoning effort in a Codex VM draft](design/previews/rune-light-vm-first-effort-small.png)

**Boat default** in the effort menu lets Boat choose the level. **None**, when
listed, explicitly requests no extra reasoning. Changing model or provider
resets effort; switching away from a chat and back keeps its choice. The model
setting **Boat default** leaves both decisions to your account. Models that
list no effort levels have no effort picker.

Reopen Caelum and select the saved conversation row to continue the same thread.
If Boat may have accepted a prompt but the acknowledgement was lost, check
Boat and reconnect using the prompt ID.

When the VM is ready and its turn has finished, choose **Conversations… →
New Boat conversation** in the management view. **Sessions**
immediately selects the new chat and keeps the previous chat in the list.
Click either row to return to its draft. Both use the same VM files and branch.
Only **Send** starts the new Boat conversation.

![A new session and an existing Boat conversation on the same machine in Sessions](design/previews/rune-light-vm-session-sidebar-small.png)

To switch providers in an existing chat, finish the current turn, then choose
**Codex** or **OpenCode** in the VM composer. The next **Send** uses that provider
in the same [Boat conversation](https://docs.boat.dev/integrated-agents/conversations.md#switch-harness-or-model-in-a-conversation).
Your draft stays in the editor, and earlier replies keep their original provider labels.

![Choosing Codex or OpenCode for the next message in a VM chat](design/previews/rune-light-remote-provider-small.png)

### See what the agent is doing

Look in the VM transcript while a turn runs. **Working on VM…** means the turn
is active but hasn’t reported a more specific action. Tool calls show their
name, status and, when available, the file or executable. Click a tool row to
see its command and text result or error. An open call updates when its result
arrives. You can reopen it after restarting Caelum as long as Boat retains the
event. Older saved rows without an event reference show their summary only.

![An expanded VM tool call with its command and test output](design/previews/rune-light-vm-tool-output-small.png)

Boat’s [managed event feed](https://docs.boat.dev/api/reference/agent/list-sandbox-events.md)
publishes replies and tool calls. It doesn’t guarantee a separate thinking
stream. If a response includes a public reasoning-summary item, Caelum shows
**Thinking…** while it’s active; click the row to read the summary. A high
effort setting alone doesn’t mean a summary is available.

### Resume or manage the machine

If you used the earlier direct-OpenCode preview,
its VM transcript and draft stay available for reference. Your next explicit
send starts a managed conversation; opening the checkout does not replay a turn.
Caelum no longer contacts the preview service. If one is still running on an
old VM, stopping that VM ends its detached process.

Find the checkout under **Remote VMs** in the project sidebar. **Stop** in the
composer interrupts just the selected turn. **Stop machine** keeps the disk
for a later resume. **Delete VM on Boat…** permanently deletes the machine and
its disk. Confirm its exact name and ID; Caelum removes the chats, drafts and
links only after Boat accepts deletion. If it fails, the checkout stays available
to retry. The key needs `sandbox.delete` or `sandbox.delete-own` for a VM it
created. A setup that has no Boat machine ID can only be discarded locally.

Click the server icon in the remote composer or the original linked session for
machine stats: capacity, the auto-stop deadline, and the busiest sampled process.
From the original session, **Open VM conversation** returns to its specific
thread. **Pin stats to VM composer** keeps process activity visible on the VM.

The default lifetime is **1 h**. For longer work, choose **8 h**, **24 h**,
or **Until stopped**. Boat automatically moves a timed VM’s deadline forward
while a managed Boat prompt is running or waiting, even if Caelum is closed.
Standalone commands and services don’t renew it. In machine stats, **Set from
now** changes the remaining time without interrupting the turn. **Until stopped**
keeps billing until you stop the VM; trials require a timer of at most 2 h.
See [Boat’s auto-stop guide](https://docs.boat.dev/long-running-tasks/auto-stop.md).

For a managed Git commit or push, open **Manage VM and Boat conversations… → Changes**.
Review and commit the VM's tracked and non-ignored new files; **Push
branch** publishes the reviewed commit to GitHub. In your local project, run
`git fetch origin` to bring the branch back.
