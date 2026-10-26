# Keyboard note press/release transactions

Status: core `lane_entry.rs` is declared in lib.rs and its 13 workflow tests
passed central execution. Native State/Workspace key routing is now connected.
Five native gesture workflows, three minimap State regressions and all 37
general editor workflows passed. Focus/modal cancellation, detached previews,
per-lane release history and immediate playing/advance commits are exercised.
The broader playback regression and source Snap Off remain separate increments.

## Original behavior

GrooveAuthor pin `48e18f71655d0f0d86144c63239331833501fda2`, MIT.

- `ActiveEditorChart.cs:2697–2748`: down validates lane, rejects negative chart
  position, takes nearest row, snaps its midpoint upward, stages deletion of an
  existing head at that position, then creates a pending tap/mine. Playing
  commits immediately. AdvanceBySnap commits then moves forward, except an
  initial deletion does not advance.
- `EditorPosition.cs:199–205`: nearest row truncates a nonnegative exact half
  downward (`> 0.5`, not `>=`). Snap is a subsequent distinct operation.
- `Editor.cs:6386–6405`: MoveLaterInChart moves to
  floor(rawChartPosition/snapRows)×snapRows+snapRows. It does not add snapRows to
  the rounded note position. This differs for an off-grid fractional cursor.
- `ActiveEditorChart.cs:2946–2990`: held movement uses nearest row without a
  second snap, preserves the starting anchor and captured participant, creates
  a hold in either direction, and converts it back to tap at the anchor. A
  gesture that began as deletion uses the current participant on first becoming
  a hold; subsequent movement retains that participant.
- `LaneEditState.cs:168–218`: modification swaps tap/mine and hold/roll.
- `ActiveEditorChart.cs:2775–2943`: release resolves overlaps. An interior tap
  shortens a hold by 12 rows (source MaxValidDenominator/4). Holds merge partial
  edge overlaps and remove inner tap/mine/fake/lift notes across participants in
  the same physical lane. A fully contained same-type hold is a no-op; a type
  change also applies the new participant.
- `EventTree.cs:177–216`: FindNoteAt scans backward from the latest physical-lane
  head; a later nonoverlapping head blocks an older hold. Source Fumen
  SMEventComparer orders same-row tap/fake/lift, then hold starts, then generic
  LaneNote (including mines), before comparing the participant.
- `LaneEditState.Commit` performs the group as one ActionMultiple. Escape calls
  CancelLaneInput (`Editor.cs:2283`), reverting provisional actions.

## Detached model and native boundary

`Inputs` binds to one source chart GUID and exact content. Pending `Draft` notes
and `deleted_heads()` are presentation overlays; they do not mutate Session,
serialized bytes, undo history or timing indices. Release resolves on a cloned
Grid and submits all changed heads through one `Session.edit_notes` call.
Cancellation only discards overlays. This adapts the source's provisional
ActionMultiple edits to the existing Session transaction boundary; it is not an
assertion that the source uses detached drafts.

Own commits rebind the controller so other held physical lanes remain active.
Unrelated content edits invalidate it before any release mutation. Native chart
switch, panel ownership, save/edit-disable and focus-loss transitions must cancel
before relinquishing keyboard ownership, then discard/re-capture the controller
when appropriate. Source focus-loss early-out is not claimed to prove automatic
cancellation: this is an explicit native policy to prevent a delayed held key
from editing on return. Key releases must always clear the receptor input state.

Each lane release is its own atomic undo entry, including simultaneous chords.
A net-identical result uses Session's existing no-op behavior and creates no
history entry; the original ActionMultiple can still record a reversible action
sequence with net-identical bytes. This distinction does not change document
content. Fresh taps/holds use source-factory empty attributes; unaffected heads
and modified existing sustains retain their note attributes. Unsupported note
representations remain subject to Session's existing validation.

## Evidence and remaining work

`editor-lane-entry-oracle.py` executed seven .NET control-flow sequences using
unchanged OnLaneInputDown, SetLaneInputDownNote, movement, cancel, nearest-row and
MoveLaterInChart methods, each byte-verified against git show. Event/action
storage and release recording are explicit adapters: this oracle does not run
original undo actions, overlap resolution, timing or the complete event tree.
Its fixture is replayed by one of the 13 Rust tests. Separate Rust cases cover
collisions, stale data, cancellation, owners, atomic undo and independent lanes.

The prepared native bridge uses the default Tap tool for the source gesture;
explicit Hold/Roll/Mine/Lift/Fake tools retain their existing Place commands.
Left Shift is the source ArrowModification binding; Ctrl/Super/Alt cancel or
block the pending transaction. Releases run before modal early returns, and
Escape/focus/chart/save/modal ownership cancel the preview. Cursor updates
recompute only held drafts; generic refresh with an unchanged row does not turn
a snapped tap into a hold. Own commits rebind the precise fractional position.

The overlay uses source ActiveEditEventAlpha=0.8 (Utils.cs:94), masks initial
deletions, and draws drafts without Session mutation. It reserves at most 512
strips from the focused view's existing note budget rather than adding another
full budget; truncated overlays use the existing partial-view notice.

`edition_lane_entry_workflow` has five prepared State tests covering detached
tap/undo, held/backward/return-to-anchor and Roll, cancellation boundaries,
Routine chord participants and per-lane undo, playing/advance and explicit
Hold-tool preservation. They require the bridge before compilation/execution.
The native module and temporary patch have not been validated in Rust; no native
port of this transaction is claimed complete yet.
