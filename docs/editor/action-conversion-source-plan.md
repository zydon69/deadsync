# Note swaps and timing conversions

## Evidence and state

Original GA `48e18f71655d0f0d86144c63239331833501fda2`, SML
`bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`, Fumen
`0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89` are pinned in the executable
reference scripts. Fixture reference JSON records source blobs, host adapters,
program/assembly and input/output hashes. No Rust Cargo validation has run for
this lot. `note_swap.rs` and `note_swap_workflows.rs` are isolated, undeclared.

## Note swaps

`editor-note-swap-reference.py` executes unchanged `ActionChangeNoteType`, ten
unchanged UIEditEvents swap method bodies and six EventConfig factory bodies.
Forty actions cover all/subset/reversed/empty selections. All forty undo and redo
states match their original snapshots. Eighty actual Fumen SM/SSC writer and
reader results accompany those actions.

The ten menu pairs are Tap/Mine, Tap/Fake, Tap/Lift, Mine/Fake, Mine/Lift,
Hold/Roll, Hold/Tap, Hold/Mine, Roll/Tap and Roll/Mine. They are simultaneous;
composing two conversions would incorrectly reconvert the first side.

Ten actions create a zero-length sustain from a simple note. All twenty format
readbacks then contain zero charts, despite successful source Save. Native swap
therefore rejects that entire operation and directs the caller to Convert with
an explicit duration. It invents no duration. Selected existing positive holds
can still convert to simple notes or rolls. Owner, position and positive ends
remain intact. Existing native decoration preservation and StepF2 fake-bit
coherence are retained extensions; source factories reconstruct simpler notes.

Prepared Rust tests: two workflows, including all forty action cases in both
formats (sixty conforming results, twenty safe atomic refusals), exact Undo/Redo,
strict save/reopen and a mixed hold/tap refusal preserving a preexisting redo.

## Warp / negative Stop

`editor-timing-conversion-reference.py` executes both complete original action
classes using the original extracted timing refresh/rate mapping methods. Host
adapters provide list-backed lookups and mutations, factories and selection;
this is not a full GUI or writer round-trip oracle. Twenty-three cases execute;
all twenty-three Undo and Redo chart states match exactly.

Originals are removed first. Their original event times remain the conversion
start times; querying the mutated chart at the start row is incorrect. Each new
event is added sequentially, so subsequent queries see its clock effects.
Existing opposite-kind events at the same row prevent replacement **after the
original was deleted**. A review must disclose that deletion and never describe
this transformation as lossless. A selected warp inside an unselected covering
warp can yield a zero Stop. Negative Stop conversion uses ties-to-even rounding,
then the original Warp constructor clamps the result to at least one row.
Same-row delays and interior delays/BPM changes are separate fixture cases.

Original `NewEvents` is not cleared on Redo: selected result references double
while the actual chart remains correct. Native should return the current new
selection only and explicitly document this presentation correction. Rust timing
conversion implementation now exists in the isolated `timing_edit/conversion.rs` module. The plan captures the original event times before deleting originals, then reconstructs the clock after each replacement. A zero-duration Stop refuses the complete operation. It captures the native session/revision ownership token and requires exact acknowledgement of every collision row.


## Prepared native keyboard integration (not yet executed)

`patches/editor-action-conversion-core.patch` declares the two isolated models.
`patches/editor-note-swap-native.patch` extends the existing note conversion
inspector with ten simultaneous swap pairs and a separate Apply swap action.
Original conversion indices 0–4 remain stable; Escape cancels. Workspace uses
the selected row range, or current row, over every player's heads just like its
existing Convert command.

`patches/editor-timing-conversion-native.patch` adds an optional F7 view from
within the existing timing inspector. Pending inspector fields block entry.
Direction and current selection/all-events scope changes invalidate the detached
plan and every prior acknowledgement. Preparation never edits the chart. Its
review counts converted, collision-deleted and overlapping events; collisions
are browsed one by one with Left/Right and individually acknowledged with Enter.
Apply requires the exact acknowledgement set and the still-current chart/token.
Escape discards the plan. Source originals deleted by collisions remain explicit
and Undo restores them. An empty conversion is a true no-op.

New actor/keyboard workflows in `edition_action_conversion_workflow` and the
separate French target each cover four scenarios: simultaneous swap/undo/redo/
save; atomic invalid sustain refusal; detached timing cancel/apply/undo/save;
and two collision deletions requiring two separate acknowledgements. These
checks have NOT run yet. They do not establish physical keyboard, audio or GPU
correctness. Both patches are prepared against the shared working source and
may require regeneration if another shared hook changes their context.
