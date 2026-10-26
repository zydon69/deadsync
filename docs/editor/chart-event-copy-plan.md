# Copy selected event types between charts

## Status

Core implemented and nine workflows passed in parent validation, including
nine complete normalized SSC comparisons against the real original action.
The real C# oracle executed ten cases successfully under official .NET SDK
10.0.403; its nonzero-first-node memory case remains separately classified.
The dedicated property-only helper unit and eight native EN/FR workflows also
passed parent validation (native terminal31248). The native keyboard panel and direct Workspace report bridge
are connected through Ctrl+Shift+Y and F1; the focused keyboard workflows are validated.

## Original contract

Pinned GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
StepManiaLibrary `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`,
Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

`UI/UICopyEventsBetweenCharts.cs:47–63` exposes exactly 13 types: signature,
BPM, stop, delay, warp, scroll, speed, fake region, combo, tick, label, attack
and Pattern. Lane notes are not part of this operation. The UI requires distinct
source/destination charts and allows one or all other charts (`:298–344`).

The complete unchanged `EditorActions/ActionCopyEventsBetweenCharts.cs` executes
in `scripts/editor-chart-copy-oracle`. It selects all events of chosen concrete
types, deletes all matching destination events, then clones source events at
unchanged rows. Unlike cursor-relative paste it replaces the entire chosen type.
It retains each destination's first BPM/signature object and row and changes its
value from the source first event; the remaining nodes are replaced. Written
signature ratios remain distinct (2/2 versus 4/4). Existing destination Patterns
are deleted even when the destination is multiplayer and replacement Patterns
are therefore skipped. Unchosen types and source events remain intact. The whole
action undoes/redoes across all destinations.

## Rust draft and API

`chart_event_copy::Plan::capture(session, source, destinations, types)` binds the
selection to Session identity/revision, validates source/destination identities
and captures detached data. Duplicated destinations are deduplicated; selecting
the source as a destination is refused, matching the original UI.
`Plan::apply` validates destination content inside the atomic transaction and
replaces selected types through existing typed event operations. A specialized
property-only wrapper retains the ordinary transaction contract, refuses note/
participant/structural/file/preference changes and suppresses a net-identical
result only after comparing the source-document serialization and full resident
grids. Chart identities and preference/file changes are excluded by the
property-only change guard, not inferred from serialized bytes. Initial BPM/signature
row-zero protection is reused. Notes, chart structure and participant count are
not changed. `Report` returns changed destinations, accepted/skipped Patterns and
shared types which were already chart-independent.

Workspace::copy_chart_events returns the direct report without persistent state.
The modal panel is bound through chart_order::Binding to the source chart GUID,
song Session identity and content revision. A stale/reloaded/structurally changed
source is rejected before navigation or apply; payloads are captured only at
apply. The optional keyboard panel exposes all 13 types, individual destinations,
select-all/clear types, eight-row pages and explicit apply/cancel. Its report
shows changed destinations, skipped Patterns and shared song types left unchanged.
The panel and status are localized in EN/FR; four native workflows per locale passed.

## File model boundaries

Original action semantics are in memory. In Rust SM and SSC before version 0.7,
timing and attacks are shared song data; copying them between charts cannot
create independent per-chart values. These selected types are reported unchanged
and their source values are never rewritten. Per-chart Pattern metadata remains
independent and reversible in those formats. New tests exercise both SM and
legacy SSC. Recent SSC detaches inherited timing through existing typed APIs.

The original action can preserve a destination first BPM/signature at a nonzero
row. The oracle explicitly proves this (source first row16, destination first
row48). Rust file timing synthesizes its required initial row0 node; that case
is not falsely compared as identical resident-file behavior. Nine remaining
oracle cases contain normalized/representable events and compare complete
represented selected/unselected families before, after, undo/redo and reopen.

## Validation and remaining checks

The prepared nine tests cover multi-destination replacement, initial values,
unselected stops/notes, empty-source deletion, unsupported Pattern clearing,
source identity/revision and history preservation, shared-format no-ops plus
independent Patterns, and nine original action comparisons. The all-types workflow covers all 13
exposed choices and payloads while
retaining destination notes. Two additional workflows now cover a real corrupted
last-destination failure after the first replacement, and net-identical no-op
with redo/token preservation, pending notes and newly loaded destination caches.
A property-only helper test rejects resident note/participant mutations. All
nine core workflows, one property-helper unit and eight native EN/FR input
workflows passed. This validates this bounded command increment, with the
explicit memory/file boundaries above and headless input/actor limits. No full-editor parity is claimed.
