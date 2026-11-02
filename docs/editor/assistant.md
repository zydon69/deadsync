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
- **F5** opens a detached visual preview of a changed chart. Up/Down scroll by
  a beat, Page Up/Down by four measures; Escape returns to the conversation.
- **Page Up / Page Down** scroll through the response.
- **F4** restarts the MCP snapshot from the current editor document. Reconnect any
  external client. The stable connection path now points to the new snapshot.
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
The implementation follows the installed CLI schema (0.163.0-alpha.11.1). Changes
to this interface may require updating the host adapter.

A private temporary Codex home reuses the existing `auth.json` sign-in. Shell,
apps, multi-agent and web search tools are disabled; repository instructions and
other configured MCP servers are not inherited. This authentication adapter
currently requires a Unix platform and a file-backed Codex login. It reports an
error if either is unavailable. No credentials are embedded in the game.

Prompts, chart metadata, note data returned by tools, and analysis summaries are
sent to the configured Codex service. Audio decoding and onset analysis run
locally; the audio file is not uploaded. Service usage is subject to the user's
Codex account limits. No API key is required by this adapter.

## External MCP access to the open editor

Opening the assistant starts an authenticated loopback bridge and displays a stable
connection-file path. Its token belongs to this editor instance. The most recently opened editor owns
discovery. After it closes, the stable file may remain as an expired connection;
opening the assistant publishes a fresh endpoint and token. Do not share the
connection file. The bridge is available only on loopback.

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

The bridge owns a detached snapshot captured when it starts. Tool calls inspect
and modify that snapshot. `apply_preview`, `undo` and `redo` offer the resulting
snapshot to the native panel; **F2 in the game** is still required to publish it.
A stale snapshot cannot overwrite subsequent editor changes. Restart with F4 to
capture a fresh snapshot. Bridge connections cannot save files.

The helper can alternatively open an explicitly selected song without a running
game:

```sh
deadsync-mcp --song /absolute/path/song.ssc
```

This standalone mode operates on its own in-memory session. Add `--allow-save`
only when you want the `save_chart` tool to be allowed to persist that song. It
does not provide live editor synchronization. Without that flag, saves fail.
The helper emits newline-delimited JSON-RPC only on stdout; diagnostics use stderr.

## Tools

| Tool | Purpose |
| --- | --- |
| `inspect_song` | Song metadata, chart identities and revision |
| `inspect_chart` | Bounded pages of note rows and attributes |
| `inspect_timing` | Chart/song BPM nodes and inherited timing |
| `analyze_chart` | RSSP analysis of unsaved chart data |
| `generate_chart` | GrooveAuthor conversion from an existing chart |
| `preview_note_edits` | Validate a batch of taps, holds, rolls, mines, lifts, fakes or deletions |
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
