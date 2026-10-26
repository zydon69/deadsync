# Original chart-wide copy oracle

Development-only .NET 10 adapter. Run from the repository root:

```sh
python3 scripts/editor-chart-copy-reference.py \
  --editor-root /path/to/grooveauthor \
  --reference-root /path/to/StepManiaLibrary \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/grooveauthor/chart-copy
```

The generator extracts the **whole unchanged** original
`ActionCopyEventsBetweenCharts.cs` at the declared GrooveAuthor pin, plus the
original Pattern definition/clone and multiplayer predicate. Actual pinned
Fumen/StepManiaLibrary provide event objects, chart properties and comparison.
The explicit adapter supplies storage, direct BPM/signature value access,
base-event clone, input/output projection and inert dependency callbacks.
Unused ForceAdd helpers are reused from the previous oracle for shared adapter
compatibility; chart-wide copying does not invoke ForceAdd.

The 10 actual cases cover selected-type replacement, empty-source deletion,
initial BPM/signature value updates, preserving an existing first-event row,
Patterns, overlaps, multiple destinations, unsupported multiplayer Patterns and
attack/delay rows. Before/after/undo/redo and unchanged source are recorded.
This proves in-memory action behavior. It does not execute original writers,
original GUI, profile observers or timing dependency recalculation. The original
nonzero-first-node case is explicitly kept separate from Rust normalized resident
file timing. Provenance records exact pins, generated source/adapter/assembly
hashes, request/output hashes and SDK version. Build output stays ignored.
