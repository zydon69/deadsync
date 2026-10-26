# Receptor and autoplay increment

## Source and boundary

Pinned GrooveAuthor: `48e18f71655d0f0d86144c63239331833501fda2`, MIT.
Core and actor modules are now declared, with five autoplay, four receptor and
two actor tests passed. Four playback-signal checks also passed: pending, stale,
failed, rebound, paused and sample states cannot drive autoplay. State/render
integration is now applied after checkpoint `543e3d106`; its five native
workflows await central execution. The playback helper
records the generation which actually supplied a validated position, preventing
an older position from passing after a new Opened acknowledgement.

- `crates/deadsync-editor/src/autoplay.rs`: immutable content/GUID-bound timing
  index, physical-lane next-input lookup, original autoplay callback transition
  state, and separate note-consumption/hold-lighting/hold-cutoff policies.
- `crates/deadsync-editor/src/receptors.rs`: deterministic receptor animation
  state. Callers provide monotonic animation time and exact chart-time callback
  offsets. This does not advance or infer decoder playback.
- `edition/receptor_render.rs` and `receptor_atlas.rs`: original layout-specific
  textures/rotations and separate base/rim/glow actors for a visible field.
- `assets/graphics/editor-grooveauthor`: original atlas PNG, matching root atlas
  JSON, copied MIT licence and hashes/provenance. `Content/atlas.json` is a
  different 1800px packing; it must **not** be paired with the 1280px PNG. All
  copied root atlas rectangles were checked against the PNG dimensions.

## Original rules retained

`AutoPlayer.cs:79–164` compares previous/next event times using one-ULP equality,
not event identity. Starting inside an active hold presses its receptor, while
starting after a tap does not invent a hit. Hold→hold and tap→hold equal-time
transitions retain the original callback ordering, including repeated releases.
`Stop` resets every lane and emits Cancel for each receptor. A seek or content
change must reset this state before feeding another next-input sequence.

`Timeline::capture` reuses `EditorTimeline::build_clock` and its TimedHead fake
flags: explicit fake notes, FAKE regions, warps and negative-stop retroactive
flags are not reinterpreted from rounded cursor rows. Hold tails use the note
phase at their own row. All participants share physical lanes, while stored
head Positions retain owner identities. Candidates use chart seconds; audio
file seconds must be converted with that resident view's existing clock.
Per-frame lookup uses per-lane binary searches and a Position map for render
policy; it does not rescan all notes for every drawn head.

The candidate adapter follows `EditorChart.GetHoldsOverlappingTime`: examine
latest per-lane heads, including fake/non-actionable blockers. It also preserves
`FindBestByTime`'s **global strictly preceding event**. An old hold can supply
its tail exactly when a new blocker has equal current time, but only if that
hold remains the global predecessor. A label, Pattern, attack, PreviewRegion or
End Hint between them prevents that fallback. Preview exists whenever the song
has no dedicated PREVIEW file, even with zero sample length (`EditorChart.cs:1751`).

`editor-autoplay-candidates-oracle.py` executes unchanged GetNextInputs,
GetHoldsOverlappingTime, FindBestByTime/FindGreatestAtOrBeforeChartTime and their
Ensure helpers, with unchanged Fumen RedBlackTree. Nine sequences / 90 queries
cover equal-time blockers, owners, fake flags, warped equal times and nonlane
labels/preview/end-hint predecessors. Event times, fake flags and tie ordering
are explicit adapter snapshots: this validates the searches against them, not
the complete original timing pass or EditorEvent comparator. The native index
uses the crate-private typed timed-event iterator from the existing dependency
pass; the iterator is now connected and covered by the candidate tests.

`Editor.cs:3038–3058` and `EditorHoldNoteEvent.cs:493–514` distinguish hiding
consumed notes, lighting active holds and clipping active holds. `Presentation`
therefore exposes hidden, active_hold and clip_hold independently. Nonfake end
times strictly below the current chart time are consumed; equality is retained.
Paused editing never consumes notes. Turning off lighting does not disable
cutoff when hiding is enabled.

`Receptor.cs` timing constants are translated directly: tap shrink .02s down
and .08s up, rim .2s, glow .3s, beat brightness 1→.8 over the first .3 beat,
size-zoom fade .02→.01. Manual and autoplay held states share the rim, while
Cancel clears autoplay glow without releasing a manually held key.

## Oracle and prepared tests

`scripts/editor-autoplay-oracle.py` extracts **unchanged** private AutoPlayState,
Update and Stop methods from the pinned source. Constructor glue supplies
next-input snapshots and records receptor callbacks. It generated seven
sequences / 34 stages using .NET 10, with source/method hashes stored in
`reference/autoplay-transitions.json`. It validates callback transitions only:
not candidate selection, Fumen/editor timing, graphical animation or GPU output.

Prepared, not yet run with Cargo:

- `autoplay_workflows`: five tests, including all C# transition stages and 90
  original candidate-search queries, fake regions,
  owners/lifts/holds, warp/negative stop flags, strict consumption boundaries,
  stale session/content and atomic rejection of invalid snapshots.
- `receptors_workflows`: four animation/preference/held-state tests.
- `receptor_render::tests`: two tests for every supported layout's atlas bounds,
  rotations and distinct base/rim/glow actor layers.

## Native integration contract after checkpoint

Keep one autoplay/animation slot per resident GUID with an exact content Binding.
Rebuild its immutable candidate index only when content changes. Obtain chart
seconds from that view's validated shared clock; use the existing typed chart
transport to decide whether playback is actually active. Sample audition,
queued playback and paused transport must not trigger autoplay. Reset all slots
on stop/seek/content reload, remove closed resident slots, and do not attach
native editing gestures to inactive charts. Manual lane feedback and release are connected in the native bridge below.
Full press/release note-creation gestures remain a separate source contract.

## Compile checkpoint after `c9393c835`

The core modules and native actor module are now declared for central compile
and tests only. `EditorTimeline::timed_events` supplies a crate-private typed
stream of initialized rows/times and optional head Positions, preserving the
existing entry order. The candidate index uses this stream rather than
reconstructing nonlane timing boundaries. At that checkpoint Native State remained unchanged; the later bridge below
connects slots, updates, note-consumption policy and actors.
The 5+4 core and 2 native unit tests passed central validation.

The ninth candidate sequence proves the
original equal-row ordering Preview < End Hint < real Event (SMEventComparer
sorts null-backed widgets first). The five autoplay workflows passed again with
the 90-query fixture and corrected rank; both native receptor actor tests passed.

## Validated transport gate prepared for native integration

`playback::State::chart_autoplay_transport` returns a chart music position only
when it is finite and mapped/acknowledged. Typed decoder callers additionally
require matching issued/clock/ack/mapped-position generations and an opened
Opened/Producing/Exhausted acknowledgement. Pending, failure, stop, paused state
and dedicated sample audition are excluded. `mapped_generation` is cleared at
rebinding and recorded only by successful `update_audio`, so an Opened ack alone
cannot relabel the old generation's position. Silent continuation after a valid
acknowledgement remains eligible. Legacy synthetic mapped snapshots retain their
explicit generation-free scope. All four unit tests passed central execution.

Both oracle scripts now also compare every extracted source file byte-for-byte
against `git show` of its recorded commit before compiling. The Fumen tree is
checked against its own recorded submodule commit. Hash metadata alone is not
treated as proof that a locally modified source matches the pin.

## Native bridge applied after `543e3d106`

`edition/receptor_views.rs` prepares one slot per resident GUID with a shared
immutable clock, bound candidate index, autoplay state and per-lane animation
state. Content changes invalidate candidates; manual feedback survives note
edits on the same layout so placing the pressed note does not lose its key-down
rim. Session changes and layout changes reset the appropriate state. Losing
view focus or receiving key-up clears manual feedback, including after opening
a panel/modifier. Key repeat does not invent manual presses.

The bridge now adds a State field, refresh/update hooks, validated playback updates,
stop/seek/scroll cancellation, source-layered receptor actors for visible fields,
and the default hide/hold-cutoff policy shared by focused/inactive note passes.
Manual feedback is included, but full press/release note-creation gestures are
still a distinct unported contract. Active-hold lighting policy exists in the
core; rendering the original hold-specific active textures remains a separate
graphic increment, rather than approximating it with an invented tint.

Source draw order review (`Editor.cs:2588–2604`) found base receptors must be
behind note sprites and selection; only rim/glow foreground effects overlay
them. The bridge corrects the base actor layer to 8001 and retains
foreground layers 8009/8010. Five native production-flow tests cover pending
requests, per-view clocks, hold cutoff, fake-note visibility, pause/fade, manual
release through panels/help/focus loss, focused-only edits, and captured digits
under help/settings/minimap/modifiers. All five passed central execution after
resumption on 2026-10-26. The two actor unit tests and both renderer-budget
unit tests also passed after the base-layer correction. These are synthetic
transport/actor proofs, not hardware audio or GPU-rendering validation.

Key releases are handled before modal routing. Help, settings, minimap focus
and active Ctrl/Super/Shift/Alt modifiers suppress new manual feedback. Losing
application input clears tracked modifiers and held manual state. UI animation
time may fade effects during pause, but never advances chart audio position.

### Transport regression adaptation

The playback-follow regression now tracks the same upcoming lane note across
pause/resume and end-of-stream. The old helper compared the first visible note,
which changes identity when autoplay hides a passed note and pausing reveals it
for editing. Two assertions initially exposed this fixture assumption; all ten
playback-follow workflows passed after preserving note identity. Exact position,
requested audio cuts, stale snapshots and unchanged source bytes remain checked.

The resumed checkpoint also passed 82 native regressions: mixed shift ten, help
two, minimap twenty, playback-follow ten, transport navigation three and general
editor workflows thirty-seven. The five new receptor workflows and four targeted
receptor/render-budget units are separate from that regression count.
