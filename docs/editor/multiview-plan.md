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
input, and cleared selection on focus loss, matching the reference. Runtime rendering
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
selection. The six tests passed before the final added delay-event case; that
expanded case still needs execution.

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
All eight native tests passed. Wider playback/navigation regressions are
pending at the checkpoint below.


## Validation checkpoint for musical focus

The six domain focus tests passed before the final additional delay-event case
was added. All eight native English/French focus tests passed, including the
explicit Home reset and quantization-preserves-anchor assertions. The wider
playback/navigation regression run is still pending at this documentation
checkpoint; no physical audio-device or continuous-playback parity is claimed.

## Open-view registry domain increment prepared

`chart_views::ActiveChartViews` holds ordered resident GUIDs, dedicated flags,
per-view musical position and selection anchor, one focused GUID, and one shared
music-file time. It borrows the caller's Session for operations; it owns no
separate document, working grids, clipboard or undo stack. An owner token checks
Session identity independently of content revision, so edits and structural
undo remain in the same owner while another session is rejected even when the
registry is empty.

The bound is one temporary view and unique eligible chart identities, not an
invented fixed maximum number of dedicated tabs. Show, focus, pin/unpin, close,
move, and cycle operations follow the source distinction. In particular,
`ActiveEditorChart.cs:359` clears selection when a view loses focus; the registry
does likewise. Dedicated views retain their identity and order, not an active
unfocused selection. Closing focus prefers the right neighbor, then the left.
All view positions are remapped through their own clocks at the shared song
time. A clock failure commits no partial registry update.

Structural reconciliation removes missing/unsupported IDs and rebinds surviving
views to new source indices. A preferred source focus can reopen the chart
restored by Workspace structural undo, without claiming restoration of every
previous dedicated-tab flag. Cross-session identity rejection, temporary-view
replacement, dedicated toggling, independent order, shared/inherited timing,
atomic failure, structural undo/redo/duplication and large unique tab sets have
eight prepared domain tests. They have not yet been executed. This model is not
yet connected to native tab controls or simultaneous chart rendering.

## Native dedicated-view headers and keyboard front end prepared

The registry now has a native path. Ctrl+Shift+B pins/unpins the focused view,
Ctrl+W closes a view, Ctrl+PageUp/PageDown cycles open-view focus, and
Ctrl+Shift+Left/Right moves its header in open-view order. Draft panels handle
input before these commands, and the music-rate shortcuts require no Control
modifier, so the new commands do not share their route. Localized headers show
up to five open views around the focused header page, with dedicated/temporary
labels. Only the focused chart's lanes are currently rendered: this is not yet
simultaneous multichart drawing.

Closing the final view calls `Workspace::clear_active_chart`; it neither deletes
source charts nor closes the song. Ctrl+B still opens the complete sorted song
chart list and can reopen a view at the retained exact musical instant. Deferred
list requests use `SessionBinding` when no chart is focused, so they remain
bound to the exact song session and content without inventing a chart identity.
`Transition::open_at_time` validates the unfocused source state before atomic
reopening. Presentation commands do not dirty the source or add document undo
entries.

Four native tests per English/French target are prepared for temporary
replacement and pinning, separate open-view ordering/focus, right/left close
fallback, closing/reopening the final view, protected drafts and stale requests,
and exact retained musical time with explicit stopped playback. Two added
focus-domain tests cover the unfocused transition and session binding. These new
native/domain cases have not yet been executed at this checkpoint.

This front end conservatively refreshes view clock inputs through the registry.
Caching unchanged per-view clocks, visible-view resource budgeting, simultaneous
lane drawing, dedicated view geometry, and continued playback are subsequent
integration work; no performance parity claim follows from the current tests.

A follow-up history case ensures empty Undo/Redo leaves a closed final view
closed. Workspace refreshes active-chart focus only when Session reports an
actual history action; it does not infer that from document content revision.
The added test also exercises real preference Undo/Redo, whose audio updates
must still run while the song's content revision and dirty state remain
unchanged. The focus-domain target now contains nine cases; the new cases await
execution with the native view integration.

## Source review for simultaneous rendering and history focus

### Layout and per-view rendering

`Editor.cs:3076` positions the focused chart at the focal point, then packs
neighbor charts to its left and right using their full natural chart widths.
It does not divide the viewport into equal miniature columns or shrink every
open chart to force all tabs on screen. `ActiveEditorChart.cs:279` includes lane,
waveform, event-widget and focused-only auxiliary areas in width. `:417` culls
views completely outside the chart area. The native implementation should keep
this focal layout contract while simplifying controls.

All visible views receive receptors, markers, regions and note drawing
(`Editor.cs:2777`, `:2947`, `:2964`, `:2979`, `:3008`). Unfocused notes are still
rendered with their own chart graphics. Focus is indicated by boundaries;
focused-only selection is not a reason to hide other charts' arrows.
`ActiveEditorChart.cs:379` draws miscellaneous events for every chart.
Waveform policy has three alternatives at `:390`: focused only, every chart, or
charts sharing both music path and offset. That policy must be ported explicitly
rather than applying a single focused waveform everywhere.

`Editor.cs:1384` synchronizes by music-file time. Each view converts through its
own timing/offsets and has its own row/time/variable-spacing projection. All
receptors share focal Y; equal song times can correspond to different rows or
to a frozen row during a stop. The current native `spacing_view.rs:12` caches
one focused clock/index and the actor code at `edition.rs:2387` builds one
projection. Reusing that projection for inactive views would reproduce the
wrong-row/offset bug in another form.

### Undo focus is action-specific, not an affected-chart heuristic

`Editor.cs:6778` invokes ActionQueue Undo directly. Ordinary
`EditorActions/ActionAddEditorEvent.cs:46` modifies the event's owning chart
without calling SetChartFocused. `Editor.cs:5839` OnNoteTransformationEnd also
does not open or focus a chart: it selects transformed events only if their
chart is already focused (`:5859`), otherwise clears that chart's selection and
clears other active selections. Therefore undoing a note/transform on a closed
chart should update the shared document in the background while keeping the
current other focus, or no focus, unchanged. It must not open an arbitrary first
chart, nor automatically open the affected chart.

Explicit `ActionSelectChart` and `ActionDeleteChart.UndoImplementation` are
different: they restore the designated previous/deleted focused chart. Keep
Workspace's explicit structural focus records for those operations. A typed
Session history effect should report whether an action applied and any resident
chart-ID structure delta. Workspace can retain the current resident focus for
ordinary actions and apply an explicit structural focus directive when present,
without classifying user-visible action labels or guessing from note targets.

### Keyboard and rendering increments

1. Finish history-focus effects first. Test ordinary note Undo/Redo with its
   chart closed, another chart focused, every view closed, preference actions,
   and explicit structural delete/undo. Assert exact notes and independent focus.
2. Prepare all open grids through a mutable Workspace boundary before actor
   drawing. Actors borrow Session's live loaded grids by resident identity; they
   must not read stale serialized source for unsaved notes or clone the whole
   song per view. Resolve source indices after structural changes.
3. Extract focused rendering into a reusable read-only view renderer accepting
   grid, chart-specific metadata/projection, shared musical time, geometry,
   selection eligibility and an event budget. First use that renderer for the
   existing focused path and preserve its tests, then invoke it for all visible
   views. Preview/generated replacement grids apply only to their target view.
4. Build per-GUID clock/projection caches keyed by content and view inputs.
   Reuse unchanged clocks and geometry inputs across transport frames. Distribute
   a global note/marker/event budget across visible views, as the source does
   (`ActiveEditorChart.cs:479`), and cull off-screen views before actor work.
5. Add geometry/actor tests for Single plus Double and Couple/Routine, left/right
   placement, clipping, two different BPM/offset clocks at one song time,
   stops/warps/reverse/variable spacing, unsaved edits, and focused-only selection.
   Validate waveform placement separately with the original policy options.
6. Keyboard note commands must target only the focused resident view:
   `Editor.cs:6749` dispatches lane down/up to GetFocusedChartData. Focus loss
   clears active lane-edit gestures (`ActiveEditorChart.cs:359`). Test that a
   held key released after focus changes cannot finish a hold in the new chart,
   and that inactive charts' notes remain unchanged when typing in the focused
   chart. Current native key-down placement is not proof of the source's full
   press/release hold-editing behavior.

## Typed history focus correction prepared

Session now exposes `undo_with_effect` / `redo_with_effect`, returning an applied
flag, before/after action revisions, whether song content changed, and an
optional before/after resident chart-ID structure delta. Existing boolean
Undo/Redo APIs remain available. Workspace preserves the current resident focus
(or no focus) for ordinary actions and uses its explicit structural focus record
only when that actual structural action calls for restoration. It no longer
opens the first source chart as a side effect of ordinary history on a closed
view. Preference actions still execute even when song content does not change.

Four focused domain tests are prepared for actual note Undo/Redo on a closed
view, keeping another chart's focus, structural deletion/undo restoration, and
typed no-op/content/preference/structure outcomes. The native chart-view targets
now contain five tests per language, including an actual note edit on a chart
that is subsequently closed. These added history and native cases have not yet
been executed at this checkpoint.

### Prepared rendering budgets and immutable clocks (not yet integrated)

The new `view_layout`/`view_render`/`view_render_screen` modules keep natural
chart widths and prepare only views intersecting the viewport. The note pass
has two independent bounds: candidate notes and track/bin tree queries. This
second bound also limits sparse charts whose tracks all lie outside the view.
The deterministic prefix retains crossing sustains before heads within a track.
`NoteRenderStats` exposes both counts and `partial`; coarse row sampling also
marks the view partial. Each affected view displays the localized instruction
“Partial view. Zoom in to see more notes.” The focused note pass must use the
same helper and indication when it is connected.

Resident views now retain immutable `Arc<Clock>` handles guarded by exact
content tokens and GUID/source bindings. Time movement, focus and view order
changes reuse those clocks. Content changes (including timing edits and Undo)
rebuild them; another Session is rejected. `synchronize` combines reconciliation
and shared-time mapping into one pass. The new rendering input cache borrows
these clock handles rather than capturing them again. This is a structural
cost reduction, with no benchmark claim.

Prepared coverage: five core overview-budget tests, two additional registry
clock-handle tests, two note-render statistics tests, three geometry tests and
four native simultaneous-view actor workflows. No result is asserted here:
parent integration and central test execution remain required. Inactive
waveform policy and interactive attack widgets remain separate unfinished
layers; the current prepared inactive pass covers lanes, owner-colored notes,
sustains, beat/signature/label/pattern markers and the synchronized cursor.

### Native renderer connected after checkpoint `eff40ebbf`

`edition.rs` now prepares visible Session grids, draws the inactive natural-width
fields, and delegates focused/inactive note construction to the same bounded
renderer. `Workspace::prepare_view_grids` only initializes read caches: selection,
source bytes and history are preserved. Each chart maps the shared musical
instant through its own cached clock, with independent note-owner colors and
sustains; keyboard editing remains routed through the focused Workspace chart.
The partial-view notice is wrapped inside the visible portion of its field.
The new actor/statistics/geometry targets still await central execution; this
connection does not certify inactive waveform/attack-widget or GPU parity.

### Concrete remaining rendering differences (reference reread after restart)

Reference paths below are relative to the pinned GrooveAuthor checkout,
`StepManiaEditor/`; native paths are relative to this repository. These are
observed omissions, not inferred completed parity.

| Remaining difference | Reference evidence | Current native evidence / next contract |
| --- | --- | --- |
| Receptor graphics and lane feedback | `Editor.cs:2777` draws receptors for every visible chart; `ActiveEditorChart.cs:3027` delegates each receptor graphic. | `edition/view_render.rs::cursor` draws a horizontal focus line; no receptor row is drawn. Introduce owner/layout-aware receptor actors separately from scrolling notes, then test inactive as well as focused charts. |
| Inactive waveform placement policy | `ActiveEditorChart.cs:390` supports focused, all, and all with the same MUSIC and offset. | `edition.rs:2473` invokes waveform actors once for the focused field; `view_render::Data` only supplies an unused waveform axis for variable projections. Requires a per-view waveform source binding and explicit policy; do not reuse focused samples for charts with different MUSIC/offset. |
| Miscellaneous timing/event widgets on inactive charts | `ActiveEditorChart.cs:379` explicitly enables miscellaneous events for all charts; `Editor.cs:3008` draws each chart's visible events through its own graphic manager. | `view_render::markers` covers measure/signature/label/pattern markers; `edition.rs:2475` invokes attack actors only for the focus. BPM, stop, delay, warp, scroll/speed and attack widgets need an explicit per-chart bounded visible-event stream. |
| Note graphics and playback consumption | `Editor.cs:3038` hides consumed arrows during autoplay and clips active holds at receptors before event-specific drawing. | Shared `view_render::notes` uses the highlight-arrow texture, owner tint, letter symbols and a plain sustain body; no consumption/receptor state or hold-tail graphic. Requires a render policy based on each chart's musical position, independent of the shared audio adapter. |
| Exact horizontal padding and boundaries | `ActiveEditorChart.cs:277` accounts for waveform/misc-event widths and focused minimap preferences; `Editor.cs:2947` draws focused/inactive boundaries. | `view_layout::layout` uses natural lane widths with fixed 42/30 margins. This preserves neighboring charts without squeezing but does not reproduce preference-driven full widths or themed boundaries. Coordinate minimap placement before changing geometry. |
| Key-release lane gestures | `Editor.cs:6749` and `:6757` forward both down and up to the focused view. Focus loss clears lane edit state (`ActiveEditorChart.cs:359`). | `edition.rs` returns before processing non-modifier releases; lane keys dispatch `Command::Place` on press. Existing note tools work, but sustained key gestures and feedback are not implemented. Minimap release routing must not be mistaken for this separate lane-edit contract. |

Suggested next independent increments: receptor actors and boundaries; bounded
per-view miscellaneous-event projection; waveform source/policy; note graphics
and autoplay clipping; lane press/release gesture transaction. Existing musical
focus, registry and actor tests should remain regression targets for each.

Global note traversal bound: with `N` visible fields, each gets
`floor(NOTE_BUDGET / N)` candidate notes and four times that many track/bin
queries. Each field subtracts its cumulative query/candidate counts before
processing another visible row range. Consequently, across all fields and all
ranges, note traversal issues at most `4 * NOTE_BUDGET` track/bin queries and
returns at most `NOTE_BUDGET` candidates per draw. This statement concerns the
note traversal pass, not metadata preparation, markers, waveform or GPU time.

### End Hint widget prepared

`edition/end_hint.rs` projects positive song `LASTSECONDHINT` seconds into each
visible chart using its own music offset, SyncOffset and timing map. Reference:
`EditorEvents/EditorLastSecondHintEvent.cs::GetChartTime`,
`EditorPosition.cs:189–197`, `EditorChart.cs:1792–1804`. Nonpositive hints produce
no widget. The reference marks this widget unselectable and position-immutable;
native editing stays in the existing keyboard Song Properties / sample panel.

Both committed values and staged song-property values are reflected in all
visible widgets, with cancellation restoring the committed view. Projection
inputs are cached by exact Session/content and hint/offset/sync values plus the
visible source list. Committed projections share registry clocks. Two native
workflows cover differing BPM/offset/SyncOffset, absent hints, EN/FR labels and
staged cancel/commit/Undo. They await central execution. This closes only the
End Hint item, not the other miscellaneous-event widgets inventoried above.

### Receptors/autoplay source contract for the next rendering increment

Read-only review, no implementation started in this increment:

- `Receptor.cs:80–124` updates beat/tap/rim/glow animation and draws a
  layout-specific receptor texture, including size-zoom fading. Foreground
  effects are separate (`:131–183`). Current native cursor line is insufficient.
- `Editor.cs:3168–3180` updates autoplay for **all resident charts** only while
  playing; scrolling cancels their autoplay state. `AutoPlayer.cs:79–164` keeps
  previous next-event state by physical lane, avoids a false tap when starting
  mid-chart, activates an already-overlapping hold, and handles hold→hold or
  tap→hold transitions at equal warped time. `Stop` resets every lane and
  cancels held input (`:172–183`).
- Input candidates use chart **time**, not rounded row: overlapping holds
  first, then tap/hold/lift events, excluding fake events (`AutoPlayer.cs:196–249`).
  The hold's tail becomes the next input after its head is passed. A native
  replica must consider FAKE regions as well as `Kind::Fake`, not merely note
  kind, and must preserve owners while resolving physical-lane collisions.
- `Editor.cs:3038–3058` consumes arrows only during playback: hide events whose
  end chart-time is strictly earlier than the per-chart current time when the
  hide preference is enabled; active consumed holds are cut at the receptor.
  `EditorTapNoteEvent.cs:36` and `EditorHoldNoteEvent.cs:168` consume only
  nonfake events. This should be a render policy, not mutation of Session notes.
- Native note input currently handles only key press (`edition.rs` early
  release guard); adding receptor graphics must not claim lane gesture parity.

Suggested contracts: a pure `AutoplayState` keyed by resident GUID and physical
lane, receiving immutable time-sorted input events and exact chart-time; a
separate preference-controlled note-consumption policy; then actor rendering
and finally animation time. Tests should cover start inside hold, pause/stop,
seek/reset, time equality at warp edges, fake tap/hold/FAKE regions, lift,
multiplayer owners and per-chart clocks. Real playback/sample separation must
use the existing typed transport contract, not infer `playing` from a queued
or paused decoder request. No production source is changed by this review.
