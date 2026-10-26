# Multiple open chart views: source contracts and next increments

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
This is an implementation plan, not a completion or runtime-validation claim.

## Observed contracts

- `Editor.cs:273` keeps `ActiveCharts` separately from `ActiveChartData` and the
  focused chart. `ActiveEditorChart.cs:32` owns per-view selection, edit/receptor
  state, dedicated-tab and focused flags, geometry, and `EditorPosition`
  (`:42`, `:52`). The document/chart object is shared, not copied per view.
- `Editor.cs:6171` opens previous/next charts in song presentation order;
  `:6225` cycles focus only within the active-view order. These are different
  commands. `UI/UIChartList.cs:66` single-select focuses a chart; double-select
  also makes its tab dedicated. Dedicated tabs can coexist with at most one
  temporary view (`Editor.cs:7142`).
- `Editor.cs:7112` shows a chart without duplicating existing views. An already
  open chart keeps its view. `:7065` and `:7087` move neighboring views without
  changing document order. `:7013` closes a focused view by focusing its right
  neighbor, falling back to its left neighbor, before removing the view.
- `Editor.cs:7176` focuses a chart while retaining the old **song time**. At
  `:7242`, the target position receives that time and converts with its own
  timing. `:1384` synchronizes every other view by song time. Equal chart rows
  are not necessarily equal song times when charts have local BPMs/offsets.
  `EditorPosition.cs:20` defines SongTime as music-file time, distinct from
  ChartTime (offset-free chart time).
- `EditorActions/ActionSelectChart.cs:20` exposes an optional undoable focus
  change and explicitly returns `AffectsFile=false`. Ordinary selection does
  not automatically enqueue that action. `ActionDeleteChart.cs:37` remembers
  whether the deleted chart was focused or the timing chart, and undo restores
  those roles. It does not promise to restore every former open-tab detail.
- `Preferences/SavedSongInformation.cs:17` explains why resident GUIDs are not
  persisted in simfiles. Reopening matches chart type, difficulty, rating, name,
  and description (`:68`) and accepts ambiguity for identical chart metadata.
  Active chart order, focused index, spacing zoom, and position are preferences
  (`:108`), not new source-format fields.

## Current DeadSync boundaries and concrete gaps

`history.rs:88` has one Session/document, working grids, resident chart GUIDs,
and undo/redo stacks. Keep that shared owner. Creating a Workspace/FileSession
for each visible view would split unsaved notes and history and is unsuitable.

`workspace.rs:60` currently owns one cursor, snap, player, tool, and anchor.
`Command::SelectChart` at `:274` retains the integral row, clamps participant,
and clears the anchor. Therefore switching two charts with different timing
moves the musical instant. This existing low-level command has many callers;
add an explicit song-time-preserving native focus API and migrate native chart
selection deliberately rather than silently redefining all callers.

`edition.rs:1276` refreshes timing labels, waveform, spacing, and markers for one
focused chart. `spacing_view.rs:182` stores one chart-specific timing cache;
`edition.rs:2387` creates a single lane projection. Separate read-only per-view
projection inputs from the mutable editing focus before drawing several lanes.
`analysis_panel.rs:18` already binds results to song/chart state. Background
results must never be installed into a newly focused view under an old index.

`playback.rs:55` retains exact fractional transport position, but
`edition.rs:1528` stops audio before ordinary command dispatch, and
`playback.rs:108` clears both playing and paused state. A focus command must
capture its anchor before that stop. Reconstructing time solely from rounded
Workspace rows loses fractional rows and time inside stops/warps.

## Proposed ownership and operations

1. **One song owner:** retain one Session and one global content history. Use
   resident GUIDs as view keys; resolve source indices immediately before each
   operation. Content revisions can move backwards on undo and need session
   identity/GUID validation. Source-index changes must not retarget open views.
2. **View registry:** ordered `OpenViews` with `{chart_guid, dedicated}` and
   `focused_guid`; at most one temporary view. Show/focus/close/move are
   presentation operations, never document reorder operations or dirty edits.
   Reconcile against live IDs after structural changes. Restore focused chart
   through the existing structural focus contract, then reconcile surviving
   views; preserve pinned surviving GUIDs. Do not claim stronger tab-restoration
   semantics than the reference demonstrates.
3. **View state:** cursor selection/edit input, projection cache, screen bounds,
   and rendering resources belong to each view. Shared clipboard, preferences,
   undo stack, and song transport remain above views. Local timing edits
   invalidate only dependent chart clocks; song timing edits invalidate all
   inheriting clocks. Revisions can initially invalidate conservatively.
4. **Musical focus anchor:** capture exact music-file time from active/paused
   playback when present, otherwise from an existing fractional focus anchor or
   `Clock::music_time(edit_row)`. Map via target `Clock::position` and retain both
   fractional presentation row and nonnegative integral edit cursor. Keep the
   original music-file time separately: row-to-time round trips are ambiguous
   in stops and zero-duration warps. Clock conversion failure leaves focus
   unchanged. Audio transport behavior must be explicit if target MUSIC differs.
5. **Input:** next/previous song chart remains separate from next/previous open
   view; open-list Enter remains normal focus, a second explicit action pins a
   dedicated tab. Keep keyboard access to pin, close, move, and focus operations.
6. **Persistence:** preferences may save descriptors and view order later;
   runtime GUIDs and sorted ordinals are not stable file identities.

## Small implementation increments and evidence

A. Add an explicit focus-position API using existing `playback::Clock`, first
without changing global `SelectChart`. Test same timing, distinct song/chart
BPMs and offsets, negative lead-in, local warps/stops/delays, fractional rows,
nonfinite time, invalid layouts, exact bytes, no dirty/history mutation, and
failed target conversion before commit. Native selection must capture position
before stopping audio; test stopped, paused and playing snapshots separately.
Where playback stops intentionally, verify its anchor is retained. Continued
playback across chart-specific music must remain unclaimed until implemented.

B. Add the ordered GUID registry with dedicated/temporary semantics, focus
traversal, closing and moving. Test structural create/delete/undo/redo,
unsupported layouts, duplicate metadata, exhausted lists and new-session IDs.
Then expose keyboard controls and visible tab headers in EN/FR.

C. Extract reusable immutable projection/render inputs and draw multiple chart
views at the shared song time. Test differing local timing at one audio instant,
shared-event invalidation, viewport clipping, only focused view accepting edit
input, and stable selection in unfocused dedicated views. Runtime rendering
validation remains necessary in addition to actor/domain tests.

D. Integrate cross-view edit workflows and optional undoable focus operations
using the global action history; preserve the reference's non-file-affecting
focus semantics. Add reopen/preferences descriptors only after runtime view
identity and structural history are proven. Record actual ambiguity behavior for
identical charts, as the reference does, without writing invented GUID tags.

## First core increment prepared

`chart_focus::Transition` is an explicit deferred operation: `at_cursor` derives
song time from the source clock; `at_time` accepts an exact transport/paused
anchor. Both prepare the target clock conversion before changing the Workspace.
`apply` rejects changed source focus, cursor, content or session. The private
Workspace commit loads the target grid before assigning chart, cursor, clamped
participant and cleared selection; no fallible operation follows assignments.
Existing `Command::SelectChart` behavior is retained for unmigrated callers.

The returned `Position` keeps original music seconds, target chart seconds and
fractional/negative presentation row. Its integral edit cursor uses the current
transport policy (floor, nonnegative, capped at i32::MAX). Its validity checks
include resident identity, source index, content token and edit cursor. Native
integration must retain this exact anchor rather than recalculate music time
from the floored cursor; native migration and playback capture are not yet
implemented by this core increment.

Six prepared domain tests cover offset/BPM changes without file or history
edits, fractional/negative positions, stop/warp ambiguity, target clock/grid
failure atomicity, stale deferred transitions, and unchanged legacy row-based
selection. Execution has not yet been recorded.

## Native focus integration prepared

Native chart-list confirmation and next/previous selection now use the explicit
Transition API. `playback::stop_audio` captures mapped chart transport or valid
paused time before removing that state; sample audition has no chart Preview
and is excluded. The resulting fractional Position remains available for view
projection and subsequent playback start, including time inside stops and
negative lead-in. Chart-list opening intentionally stops the current transport,
as existing chart navigation does. Continued playback while changing chart
music remains a parity gap.

Deferred next/previous requests additionally bind the source edit cursor, so a
cursor move invalidates a queued musical focus result before native work.
Explicit cursor/edit commands clear the fractional focus anchor even when the
integral cursor does not change (for example Home at a clamped negative row).
Ordinary refreshes and preference updates retain a still-valid anchor. Snapped
navigation stops before committing its new cursor, preventing the old transport
from reinstalling an anchor after an explicit seek.

Four native tests per English/French target are prepared for playing and paused
selection through list and shortcuts, exact playback restart cuts, preference
loading, stop/warp and negative anchors, audition exclusion, stale cursor
requests, and explicit Home reset. These are synthetic mapped audio snapshots
and command tests, not physical decoder/device or multiview rendering proof.
Their execution results have not yet been recorded.
