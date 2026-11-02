# Original pattern worker and constructor contract

Source: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
`EditorActions/ActionAutoGeneratePatterns.cs` (MIT).
Library/Fumen pins are recorded in the fixture provenance.

## Confirmed output defect and correction

The constructor (lines54–62) creates `Patterns=[..allPatterns]`, then calls
`Patterns.AddRange(allPatterns)`, then sorts. Every selected marker runs twice.
The first occurrence is fully clipped by its identical successor, but still
runs interpretation and advances the inclusive preceding-lane counter.
The second occurrence can recount the boundary note. Removing the apparently
redundant first occurrence therefore changes search inputs and generated lanes.

The new oracle executes the entire original class, including its actual
constructor, Do/Undo/Redo, asynchronous loop, GeneratePatternAsync and helper
methods. Of 17 existing batch inputs, 10 produce different note data from the
previous reconstructed single-enumeration loop. A controlled diagnostic removes
only `Patterns.AddRange(allPatterns)` in a separate source copy: all17 then
match the old reconstructed-loop outputs. The original fixtures are never
rewritten by that diagnostic. A derived single-marker case and36 existing editor-timing inputs transformed to their actual single marker also execute the
actual class, because UIPatternEvent.cs:99–105 queues the same action for its
Generate Pattern button.

Native `RegionGenerator::generate_action` reproduces the first discarded pass
before the final pass. Both single-region jobs and batch jobs use this shared
helper. The low-level pattern search and raw context APIs remain single calls,
matching the library engine rather than its editor action wrapper. Cancellation
is checked after the discarded pass. User-facing preview reports remain per
selected marker, an explicit native presentation adaptation: the original
worker has no corresponding per-marker preview report.

## Reproduce

```
python3 scripts/editor_pattern_worker_source_oracle.py \
  --repo-root /path/to/deadsync \
  --editor-root /path/to/pinned/grooveauthor \
  --reference-root /path/to/pinned/StepManiaLibrary \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/pattern-worker \
  --diagnose-single-enumeration
```

The script builds in a temporary directory. It regenerates the existing
source-extracted clock/deletion/context adapters there; it never regenerates
files in the primary repository. Original class text changes only its namespace
and a deletion-helper type alias. Real pinned expression and performed engines
and actual Fumen note serialization execute. Config accessors, editor event
containers/factory, graph-loading host, completion observer and synchronous
main-thread dispatcher are declared plumbing. Generated notes receive the
existing editor-clock row dependencies before insertion. The dispatcher runs
in worker order; real UI interleavings are not tested. Logs are retained,
including duplicate warnings for empty patterns. Before/Undo and output/Redo
note bytes are asserted for all54 cases.

This is not the full editor, the original event factory, actual Fumen
ActionQueue, metadata writer, source profile manager persistence or a physical
thread-scheduling oracle. No claim is made that counting18 cases covers every
configuration or scoring branch. Native integration tests compare the complete
output grid, retain NPS helper assertions, and exercise real Session apply,
Undo/Redo, peer-chart isolation and persistence. Source expected outputs remain
independent of Rust.

## Generation contract coverage checkpoint

| Requirement | Original path | Evidence / remaining limit |
| --- | --- | --- |
| Independent source and source-bound expression | UIAutogenChart78–115; ActionAutoGenerateCharts36–40 | Independent picker implemented; source-bound performed catalog lifecycle is a separate active audit |
| Supported single-player graphs | Editor.GetStepGraph; EditorChart.SupportsAutogen | Native eleven-family eligibility and actual graph fixtures; not evidence for multiplayer generation (source disallows it) |
| Source expression + target performed config | ActionAutoGenerateCharts130–143 | Actual-library parity/mutation/active-witness scripts cover selected contexts; every unchanged mutation is not an active witness |
| Seeded root tiers, fallback costs, pruning | PerformedChart122–260,1212–1250 | Existing full-engine fixtures plus static numeric/tie review; threshold precision corrected in separate lot |
| Output note assembly and nonperformance copy | ActionAutoGenerateCharts145–196; SMCommon1341–1358 | Native append/cloning timing/attacks inspected; complete original chart-action metadata writer is not executed by this new worker oracle |
| Pattern defaults/config normalization | library Config.Init/PatternConfig.Init and editor profile managers | Real Init/engines run for fixture configs; all presets/default-manager startup combinations not certified here |
| Pattern range/footing/context | ActionAutoGeneratePatterns235–310,893–950 | Original worker now executes these exact methods and actual engines on54 inputs |
| Shared distribution, source constructor repetition | constructor54–62; UpdatePrecedingLaneCounts342+ | Confirmed functional gap corrected, original-vs-diagnostic outputs prove observable effect |
| Density and overlapping/empty markers | GetNps954–1022; overlap863–885 | Original worker inputs include overlap/nested/empty/fake/holds/delay and non-grid spans; full output/Undo/Redo asserted |
| Inspector single generation | UIPatternEvent99–105 | Same action class, derived one-marker original output consumed by native single Task test |
| Batch continuation and failures | worker158–183 catches per-pattern exceptions | Empty paths execute; profile-invalid exceptions and failed expression/search contexts need further original-worker fixtures |
| Cancellation and staged acceptance | source asynchronous action vs native Job/preview | Native snapshot cancellation/refusal tests retained; atomic preview is an explicit native workflow, not source thread-parity proof |
| Timing/attacks/export/history preservation | source newChart + CopyNonPerformanceEvents | Existing resident-owner/history/save workflows remain separate; this notes-worker oracle does not certify all metadata paths |
