# Reviewed Convert All / Swap All and exact checked heads

Status: isolated models/UI/patches prepared; Rust Cargo tests not run yet.
Original fourteen simple conversion pairs already existed through Transform::Convert;
this increment closes whole-chart and exact checked-head interaction gaps.

## Source evidence

`scripts/editor-note-convert-reference.py` executes fourteen unchanged original
UIEditEvents Convert methods, complete ActionChangeNoteType and EventConfig
factories, using the previously documented inert host adapters. Real .NET
execution produced 56 action cases, each with actual Fumen SM/SSC writer/readback
(112 outputs). Requests cover all/subset/reversed/empty selected source events.
Fixtures note-convert.* record provenance. No attributed Keysound/Attack target
or physical UI parity is claimed.

## One conversion rule

`note_operation::convert_note` is the single native reference rule for kind changes:
positive sustain ends, owners, note decoration and StepF2 fake-bit coherence survive;
simple destinations remove sustain tail data. Core hook patch factors existing
Transform::Convert and note_swap to call it. Existing row-range commands retain
their behavior. Reviewed Operation::Convert/Swap plans build only changed head
edits; there is no second codec or independent fourteen-pair implementation.

Plan scopes are whole chart (all players or explicit owner), explicit row range,
and exact BTreeSet<Position>. The mixed Selection bridge validates its revision
before capturing only checked Note identities. Same-row unchecked lanes, other
players, timing, attacks and patterns cannot leak into a note operation. A source
kind swap is simultaneous; zero-duration sustains are refused atomically, without
inventing a duration. Grid validation checks candidate replacements before any
live mutation or visible prepared review. Bounds/owner validation and monotone
IntentGuard prevent stale/foreign/changed-focus publication, including edit→Undo.

The sparse scan has a default 2,000,000 head bound and a cancellable explicit-bound
API. Cooperative checks occur during scan/validation; native Grid loading/cloning
remains synchronous. This is not a peak-memory bound or a promise of an
interruptible allocator. Grid cloning is needed for existing collision validation.

## Optional keyboard review

F8 inside the existing note conversion inspector opens a five-row optional panel
for Convert or Swap, scope, prepare/counts, Apply and Cancel. Types/duration/pair
come from the existing inspector rather than a duplicate editor. Default scope
is Whole chart. F8 from the mixed checklist uses its exact checked heads by default.
Whole-chart/checked/range choices are explicit; unavailable checked scope is skipped.
Preparing shows selected/matching/changed counts without editing. Changing operation
or scope clears that review. Apply confirms the exact current plan, uses one Undo
entry, closes the consumed selection and refreshes the chart. Escape returns to
its original inspector/checklist without modifying the song.

## Prepared validation and files

- `patches/editor-note-operation-core.patch`: module declaration, shared conversion
  factor, mixed Selection and Workspace bridges.
- `patches/editor-note-operation-native.patch`: optional panel, F8 routing, EN/FR.
- `src/note_operation.rs` and native `edition/note_operation_review.rs`: isolated,
  undeclared until hooks apply.
- `/tmp/deadsync-note-operation-core-tests.rs`: four core tests: 112 original
  format cases; exact checked owner/all-owner; zero-duration/stale/foreign refusal
  preserving redo; limits/cancellation atomicity.
- `/tmp/deadsync-note-operation-native-tests.rs`: helper for four EN/FR workflows:
  reviewed/cancelled ConvertAll and save/reopen; simultaneous SwapAll; exact checked
  Routine P2 while same-row P1 stays intact; missing/invalidated review refusal.

Test files are held outside Cargo discovery while the parent's Windows --tests
check is active. They must be copied into the normal test tree only after module
hooks are applied. Proposed targets: `note_operation_workflows`,
`edition_note_operation_workflow`, `edition_note_operation_fr_workflow`. The two
native wrappers set `const FRENCH: bool` and load the shared helper from
`tests/support/note_operation/mod.rs`.
