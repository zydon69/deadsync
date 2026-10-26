# Mixed-event Shift Earlier / Shift Later

## Status

Core implemented and declared: all six domain workflows passed parent run92119,
including nineteen source-oracle format/case comparisons (12SSC +7SM).
The real original C# oracle executed twelve cases successfully under official
.NET SDK10.0.403. Workspace and native mixed-picker controls are now connected;
all ten native EN/FR workflows passed parent execution. No physical GUI, keyboard or
GPU QA is claimed by these headless checks.

## Source contract

Pins: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
StepManiaLibrary `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`,
Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.
The original menu/action dispatch and references are detailed in
`menu-comparison-next.md`.

The whole unchanged ActionShiftSelectionRow and ForceAddSideEffect execute in
`scripts/editor-event-shift-oracle`. Immutable selected events are ignored.
Original EditorChart rate initialization marks its first tempo/signature
immutable (`EditorChart.cs:1229–1329`, `EditorRateAlteringEvent.cs:57–75`);
CanBeRepositioned simply reads that flag. Resident Rust timing always supplies
row-zero initial nodes, which are therefore protected in this plan.

All movable selected events are removed before valid destinations are force
added. Negative destinations are deleted, not clamped or retained; undo restores
them. Unsupported Pattern destinations are likewise removed. Same-type misc
conflicts replace the old event. ForceAdd performs real hold trim/short-hold-to-
tap/overlapping-note removal. Pattern region overlap without a shared head does
not clear notes or other definitions. Owners remain the original owners.

## Rust draft

`Plan::capture(session, selection, rows)` validates the snapshot through the
existing detached selection boundary, records immutable/deleted/moved choices,
checks arithmetic and captures only valid shifted payloads. Single-owner note
clipboard remapping is explicitly supplied with that original owner, so a P2-only
shift does not silently change it to P1. A mixed-owner selection retains owners.
`Plan::apply` binds to one Session/revision and publishes deletion and typed
force paste in one chart/history gesture. It returns changed state, counts and
the successfully transformed selection keys. The existing notes-only Transform
command remains separate and unchanged.

Zero shift with valid data is a no-op preserving redo and tokens. A zero shift
can still delete pre-existing unsupported multiplayer Patterns, matching the
original action's existence predicate. Positive head or sustain/Pattern-end
overflow is refused before mutation, an explicit safety difference from original
unchecked integer arithmetic. Negative heads delete complete events even if a
sustain's tail or a Pattern region would have remained nonnegative.

## Prepared checks and limits

- Twelve real original cases compare complete represented events and transformed
  selection membership, before/after/undo/redo/reopen. Seven are also representable
  in SM. Original Label fixtures remain intact and run only in SSC.
- Dedicated workflows cover negative deletion, immutable nodes, zero no-op with
  reusable token/redo, P2-only StepF2 sustain owner/head/tail attributes and other
  owners, unknown nested Pattern/chart metadata, stale/foreign identity and
  checked head/region overflow.
- StepF2 attributes and unknown metadata are DeadSync preservation contracts.
  The C# adapter does not model those attributes, so these Rust checks do not
  claim an original attribute oracle. Existing user-approved preservation must
  not be removed to imitate legacy decoration loss in conflict/conversion paths.
- The C# adapter runs the real action and ForceAdd bodies with explicit event
  storage, flag initialization and inert timing callbacks. Full clock dependency
  math, original GUI or writer execution is not established by it.
- SM and legacy SSC retain the existing shared-song timing model: moving a
  timing event affects that shared scope and resident attacks are anchored by
  the existing typed transactions. This is not independent per-chart SM timing.
- Rust uses a deterministic total presentation order. The original Fumen
  lane/type comparator can be nontransitive around Pattern; exact pathological
  presentation order is not claimed.

## Native integration and focused validation

Inside the existing mixed picker, Ctrl/Cmd+Left/Right shifts checked choices by
the current snap. Global shortcuts remain unchanged. Help states negative-row
deletion and initial-node protection. Workspace returns the direct Report;
the panel refreshes and recaptures a window covering the original and moved
heads, selecting **only** report.selection_after. Immutable initial events stay
in the document but are unchecked. The first transformed choice receives focus.
Membership uses a BTreeSet rather than quadratic list searches. Empty choices
and failures have localized status. A successful atomic edit followed by a
presentation-refresh failure is explicitly distinguished from rollback.

The ten passing native workflows (five each in EN/FR) cover repeated shifts
and subsequent copying of only the transformed selection, negative deletion,
immutable-only zero-change behavior, force-trimming an unselected hold,
localized overflow refusal, exact undo/redo/save/reopen and SM/SSC variants.
This is a bounded command increment, not editor-wide parity.
