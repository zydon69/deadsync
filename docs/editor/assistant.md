# DeadSync chart assistant

The native editor and the `deadsync-mcp` executable share the same typed chart
commands, RSSP analysis and GrooveAuthor generation engine. They edit detached
song snapshots. A proposal enters the live editor only through an explicit host
confirmation; applying it is one undoable transaction. Saving remains a separate
editor command.

## Native assistant

Open a song in the editor, then press **Ctrl+Shift+I**. The shortcut is configurable
under the chart assistant action in keyboard settings.

- Type a request and press **Enter**.
- **Escape** cancels a running request; press it again after cancellation to close.
- **F2** applies the completed proposal to the current song in memory.
- **F3** discards the proposal.
- **F5** opens a detached visual preview of all affected charts. Left/Right
  switches between their before/after versions; the heading shows the current
  position and total. Up/Down scroll by a beat, Page Up/Down by four measures;
  Escape returns to the conversation. Only the selected grid is retained.
  This is a notes preview; review metadata and timing changes separately.
- **Page Up / Page Down** scroll through the response.
- **F6** switches the runtime between Codex and Claude while no request runs.
  The panel title shows the selected runtime. On first opening, the editor picks
  Codex when it is installed, otherwise Claude Code; set
  `DEADSYNC_ASSISTANT_RUNTIME` to `codex` or `claude` to choose explicitly.
  The F6 choice lasts until the game exits.
- **F4** enables or disables external MCP access. It is off by default.
  While enabled, the bridge follows editor revisions automatically.
- Close the panel and use the usual Undo or Save commands to revert or persist.

A proposal is rejected if its source editor session or revision has changed.
Inspect the resulting chart in the editor and play it before saving. Follow-up
messages refine the pending draft until you apply or discard it. Cancellation or
a failed request retains the preceding completed proposal.

### Codex runtime

Install Codex and sign in using `codex login`. On macOS the assistant also finds
the CLI shipped with `/Applications/ChatGPT.app`. Set `DEADSYNC_CODEX_PATH` to an
explicit executable path for another installation.

The assistant uses Codex App Server and its experimental dynamic tool interface.
The adapter accepts only the verified CLI version 0.163.0-alpha.11.1. A future
version is refused until its built-in tool set and protocol have been reviewed;
a denylist alone is not treated as a forward-compatible security boundary.
Updating ChatGPT can also replace its bundled CLI and temporarily disable the
assistant. Use `DEADSYNC_CODEX_PATH` with the supported CLI installation, or update
DeadSync once its adapter supports the new version. Never bypass the version
check merely to restore availability. Adapter maintainers must review protocol
and tool-isolation changes before approving each supported runtime version.

A private temporary Codex home copies the existing file-backed `auth.json` sign-in.
After the child exits, a refreshed credential is atomically retained only if the
original login is unchanged. The empty `auth.deadsync-lock` file is intentionally
retained beside the login: deleting it could allow processes to lock different
inodes and refresh the same credentials concurrently. It contains no credentials.
Concurrent DeadSync refreshes are serialized, and a
concurrent external account change is detected rather than overwritten. Shell,
apps, multi-agent and web search tools are disabled; repository instructions and
other configured MCP servers are not inherited. This authentication adapter
requires a file-backed Codex login. It reports an error if that login is unavailable. No credentials are embedded in the game.

The authentication lease lasts for the entire Codex request: the CLI can refresh
credentials at any point, so two DeadSync assistants cannot use the same login
concurrently. A second instance reports that the login is in use. If retaining
refreshed credentials fails after a request, the panel shows a warning and keeps
the completed proposal. It never overwrites a concurrently changed login.
Raw child-process stderr is consumed but never displayed or retained; a failed
request reports only that diagnostic output was emitted. Opaque credentials and
keywords split across reads therefore cannot leak through a stderr filter.

Prompts, chart metadata, note data returned by tools, and analysis summaries are
sent to the configured Codex service. Audio decoding and onset analysis run
locally; the audio file is not uploaded. Each assistant keeps at most one rhythm
analysis, without decoded audio. Cache reuse requires a matching SHA-256 digest
of the complete audio content, even if its path, size and modification time are
unchanged. A second digest after decoding rejects files changed during analysis.
Fingerprinting uses a 64 KiB buffer, supports cancellation, and rejects audio
files larger than 2 GiB. Service usage is subject to the user's
Codex account limits. No API key is required by this adapter.

### Claude runtime

Install Claude Code and sign in using `claude auth login`. An
`ANTHROPIC_API_KEY` is used only if it is present in the environment that
launches the game; a game opened from the Finder or Dock does not inherit
variables exported in a shell profile. The assistant looks for
`~/.local/bin/claude` (`%USERPROFILE%\.local\bin\claude.exe` on Windows), then
Homebrew locations and `PATH`; set `DEADSYNC_CLAUDE_PATH` to an explicit
executable path for another installation. Claude Code 2.1.280 or later is required.

The CLI runs headless in a private empty working directory with `--safe-mode`,
no setting sources, no built-in tools (`--tools ""`), strict MCP configuration,
no slash commands and no session persistence. Permission mode `dontAsk` allows
only the DeadSync tools; nothing prompts the host. The DeadSync tools are not a
separate process: Claude Code forwards each MCP request through its SDK control
protocol and the game answers it on the detached draft. Other control requests
are refused.

Unlike the Codex adapter, Claude is not pinned to one CLI version: the built-in
tool set is an allowlist, and before any tool runs the adapter checks the session
inventory. A session that exposes any tool other than `mcp__deadsync__*`, or any
MCP server other than DeadSync, is refused. So is a session whose DeadSync server
is not connected or lacks one of the DeadSync tools. Escape also cancels the
initial version check of either runtime, which times out after 15 seconds.
DeadSync tools run synchronously while Claude Code waits, so the adapter sets
`MCP_TOOL_TIMEOUT` to one hour, matching the external helper's response bound. Failed turns report the
CLI's error explanation, truncated to 500 characters. Claude Code manages its own
credentials; DeadSync never reads or copies them. Raw stderr is handled as for
Codex: only the error category is displayed. The same data leaves the machine as
with Codex, and usage is subject to the user's Claude plan or API limits.

## External MCP access to the open editor

Press **F4** in the assistant to explicitly start the authenticated loopback
bridge. The most recently enabled editor owns the private discovery file. The
owner removes that file when the bridge closes, under a cross-process registry
lock; closing another editor does not remove a newer owner's connection. Do not
share the connection file. The listener accepts loopback connections only.

Build the helper:

```sh
cargo build -p deadsync-editor --bin deadsync-mcp --features assistant-audio
```

Configure a local stdio MCP server in the client with the helper executable and
these arguments:

```text
--connect-editor
```

For Codex, the corresponding local registration command is:

```sh
codex mcp add deadsync -- /absolute/path/deadsync-mcp --connect-editor
```

For Claude Code:

```sh
claude mcp add deadsync -- /absolute/path/deadsync-mcp --connect-editor
```

Each MCP client owns a detached working snapshot. Host edits, Undo/Redo and
accepted proposals refresh it before the next request, without changing the port
or secret. `apply_preview`, `undo` and `redo` offer a proposal to the native panel;
**F2 in the game** is still required to publish it. A pending native proposal is
never replaced by an external proposal. The bounded proposal mailbox returns a
recoverable tool error before changing history when full. Clients can retry after
review. Rejecting an MCP-derived proposal with **F3** also resets detached drafts
from that proposal's bridge, including in-flight requests. Rejecting a native
Codex-only proposal leaves independent MCP previews intact. A native refinement
of an MCP proposal retains its MCP origin; rejecting it also abandons that MCP
work. Replacing the bridge does not let an older proposal reset the new clients.
Bridge connections cannot save files.

Editing refreshes only revision, focus and resource metadata. An authenticated
`tools/call` requests a snapshot; the next UI update captures editable state once
for that revision, while serialization and parsing run on the worker. With no
tool request, edits do not copy note grids, even while MCP is enabled. Initialization,
ping and tool discovery do not request a snapshot. Keep the editor updating while
using external tools; a missing UI response produces a recoverable error after
ten seconds.

Up to eight request workers are active at once; slow audio processing in one
client does not block another client. Busy replies use one separate worker and
a queue of eight sockets, so slow rejected peers do not block admission. Excess
connections beyond that queue are closed. A mutation that returns to the host
bytes does not publish an empty proposal. The helper permits up to one hour per
response for the local 30-minute audio limit. Recoverable bridge errors are
returned as MCP errors rather than terminating the stdio helper. The helper rereads
discovery per request, so enabling a replacement bridge does not require restarting
its stdio process. Stale previews must be regenerated after an editor change.

`inspect_chart` pages chronologically in `(row, lane, player)` order. Optional
`start_row` and `end_row` bounds are inclusive. Send `next_cursor` back as `cursor`,
keep the same row bounds, and restart pagination if `revision` changes. Note kinds
use the lowercase spellings accepted by `preview_note_edits`.

The helper can alternatively open an explicitly selected song without a running
game:

```sh
deadsync-mcp --song /absolute/path/song.ssc
```

This standalone mode operates on its own in-memory session. Add `--allow-save`
only when you want the `save_chart` tool to be allowed to persist that song. It
does not provide live editor synchronization. Without that flag, saves fail.
The helper emits newline-delimited JSON-RPC only on stdout; diagnostics use stderr.

In `--connect-editor` mode, initialization, ping and tool discovery work even
before F4 is enabled. Tool calls report a recoverable error while the editor is
unavailable; enabling F4 allows the same helper to retry without restarting.
An oversized input line receives parse error `-32700`; the rest of that line is
discarded with bounded memory, and subsequent requests remain usable. A saturated
bridge reports that its eight request slots are busy so clients can retry.

## Tools

| Tool | Purpose |
| --- | --- |
| `inspect_song` | Song metadata, chart identities and revision |
| `inspect_chart` | Bounded pages of note rows and attributes |
| `inspect_timing` | Chart/song BPM nodes and inherited timing |
| `analyze_chart` | RSSP analysis of unsaved chart data |
| `generate_chart` | GrooveAuthor conversion from an existing chart |
| `preview_note_edits` | Validate a batch of notes, attributes or deletions |
| `preview_tempo_edits` | Prepare scoped BPM node changes |
| `analyze_audio` | Local constant-tempo, phase and onset estimate |
| `generate_audio_chart` | New SSC chart from local onsets plus GrooveAuthor footwork |
| `preview_changes` | Current proposal identifier and summary |
| `apply_preview` / `discard_preview` | Accept into the detached session or discard |
| `undo` / `redo` | Detached-session history |
| `save_chart` | Standalone mode only, with host-enabled saving |

Generation accepts a deterministic seed, target layout, a built-in performed
profile (`balanced`, `stamina_single`, `stamina_double`, `no_candles`, `no_inward`,
`no_transition_limit`) and bracket interpretation (`balanced` or `none`). Profiles
are generation preferences, not guarantees about every resulting pattern.

## Audio-only generation limits

This is a bounded signal-processing proposal, not a trained music transcription
model. The decoder uses the game's playback timeline. Energy changes suggest
onsets; autocorrelation estimates a constant tempo and beat phase. The requested
subdivision and maximum notes per second constrain quantization. GrooveAuthor
then generates lane assignments for the selected layout.

Analysis exposes five-second windows of relative RMS energy and onset density.
`minimum_relative_energy` can omit quiet sections during generation; this is a
signal threshold, not an inference of musical phrasing. The proposal reports
how many detected onsets were not used.

Audio generation currently:

- Requires SSC and a music file contained in the song directory.
- Adds a new chart instead of replacing an existing chart.
- Assumes constant tempo and 4/4; it does not infer tempo changes or musical form.
- Accepts explicit BPM/phase overrides when the estimate is ambiguous.
- Reports a heuristic tempo confidence, not a calibrated probability.
- Is limited to 30 minutes of audio and supports cancellation.
- Requires human review of timing, difficulty, pauses, phrasing and playability.

Tempo edits use the editor's existing scoped timing rules. Song-scope changes can
affect several charts; the proposal reports their indices.

## macOS bundles

A normal build with `scripts/build-macos-app.py` also builds and embeds
`Contents/MacOS/deadsync-mcp`. When packaging an existing executable using
`--binary`, supply `--mcp-binary` explicitly to include the corresponding helper.
Codex itself remains a separately installed, authenticated dependency.
