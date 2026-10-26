# Original mixed-event row-shift oracle

Run the development oracle with the official .NET 10 SDK:

```sh
python3 scripts/editor-event-shift-reference.py \
  --editor-root /path/to/grooveauthor \
  --reference-root /path/to/StepManiaLibrary \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/grooveauthor/event-shift
```

The whole unchanged original ActionShiftSelectionRow and ForceAddSideEffect
execute, with original ForceAddEventInternal, CanEventExistAtRow,
CanBeRepositioned and Pattern definition code. The pinned SML/Fumen assemblies
provide actual event objects, chart properties and the original comparer.

The adapter explicitly provides event storage/search, base-event row changes,
selection-notification snapshots and inert timing callbacks. First Tempo and
TimeSignature objects are marked immutable according to the original
EditorChart.cs1229–1329 initialization; the original CanBeRepositioned body
checks that flag. Full dependency-clock recomputation, GUI, writers and StepF2
attribute objects are outside this oracle's scope.

Twelve genuine cases record Before/After/Undone/Redone and selection after each
action. They cover immutable first nodes, positive/negative shifts, deletion
below row zero, same-type conflicts, overlapping Patterns, real ForceAdd hold
trim/conversion/deletion, multiple owners, unsupported multiplayer Patterns and
zero shift. Source taps in hold-conflict cases start outside the unselected hold
and are moved into it, so fixtures do not depend on invalid initial overlaps.

The Rust differential target prepares twelve SSC and seven representable SM
comparisons. SM cases containing Labels are explicitly excluded as a format
boundary, retaining complete C# fixtures. Comparisons canonicalize numeric BPM
representation without discarding values, and compare complete represented event
payloads and transformed selection membership. Ambiguous pathological comparer
presentation order is not claimed identical to Rust's deterministic total order.
