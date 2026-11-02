# Source density graph and preferences

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
Prepared in isolated durable sidecars. No implementing-agent Cargo run.

## Implemented contracts

The graph is computed from the current native document snapshot and initialized editor timeline. It does not reuse RSSP measure densities. RSSP analysis and its panel remain available independently.

- Measures are fixed192 reference rows, independent of time signatures. Tap, Hold/Roll heads and Lift count as steps; mines, keysounds and fake heads do not. Source editor fake flags, including same-row warp rules, come from the existing EditorTimeline.
- Each measure stores individual steps and distinct rows with steps across all participants. Density accumulation selects NPS or rows/s. Stream classification has its own source Stream accumulation preference, independently of the graph's accumulation selection.
- Measure starts use the active rate including same-row events, as original RecomputeMeasureTiming does. The final next-measure time uses the original strict position query. Those phases differ at Stop boundaries and must not be replaced by note time or the continuous presentation cursor.
- Extent follows the final source event's end, not maximum earlier hold tail. Pattern regions, warps, fake regions, negative Stop end positions and End Hint participate. Preview is a stream boundary but does not extend the chart end. Source widget positions use the existing playback music/chart conversion.
- The seven source stream formatting values are consumed from the shared Stream group (defaults sixteenths/Step/min1/short4/marks '-' and '|'). This lot exposes DensityGraph's Show Stream checkbox; it does not claim to add the separate Stream Preferences editor.
- DensityGraph preferences cover visibility, stream text, Step/Row, six positions, per-position signed offsets and width offsets, height, two color modes and high/low/background RGBA. Default offsets10/110, width offsets-10/0, height90 and colors are from original constants/PostLoad.
- Shared application persistence uses a DensityGraph section consistent with the existing native Audio/Receptors/MiniMap groups. Source field names are retained inside the section; this is not a full original Preferences root importer (original root calls the group PreferencesDensityGraph).

## Executed original reference

`scripts/editor-density-reference.py` executes whole original DensityGraph and Stream preference classes, original defaults/restore Undo actions, actual Fumen queue and permissive enum JSON converter; original StepDensity measure/count/stream methods; original RecomputeMeasureTiming and EditorRateAlteringEvent conversion; original NPS/sample formulas; and original UpdateDensityGraphBounds for all six positions. Source, adapter and assembly hashes are recorded in density.expected.reference.json.

Witnesses:4 preference/PostLoad/partial-vector/Undo cases,11 density/count/stream/NPS cases and6 geometry branches. Event fake flags and supplied initialized rates are explicit adapter inputs; source fake-dependency and reader algorithms remain covered by existing editor timing references. GPU rasterization is not executed by this oracle.

The source partial Vector4 JSON behavior is retained: once the vector object exists, omitted channels are0, rather than inherited palette components. Unknown enum values fall back to registered source defaults. Unknown fields, unknown mount-map entries and unknown vector members are preserved by native writes.

## Deliberate native safety and presentation adaptations

1. Source measure Steps is byte-cast:256 loaded steps become0. Native retains true u32 counts and exposes source_byte_overflows. Source count/NPS parity applies within255 steps per measure; native dense charts have mathematically correct extended counts. This is an explicit source bug avoidance, not silent RSSP substitution.
2. Source all-empty graphs can produce NaN percentages. Native renders a finite baseline. Zero/nonpositive-duration measures have zero graph rate; nonrepresentable positive rates refuse graph generation rather than entering the GPU.
3. Original sparse stream segmentation can bridge empty measures until a later non-step event breaks the stream. Executed cases produce '4' across a sparse gap and '1-1' when an intermediate non-step event is present. This observed source behavior is preserved; it is not replaced with conventional consecutive-measure stream counting.
4. Count/time generation is a cancellable background job. Limits are262144 measures,2million heads and2.1million events; exhaustion reports unavailable and never edits the document. Task::capture serializes the full session synchronously on the UI thread once per new chart/revision/stream-settings binding. This capture can stall on large charts; the pending binding avoids repeating it for cursor-only refreshes. Timeline construction/sorting run within the worker and are not interruptible at every instruction. Moving serialization off the UI thread needs an owned document snapshot or immutable shared representation; this increment does not provide one.
5. Native GPU actors retain one mesh. Both accumulation-mode plots are prepared in the background worker, so resizing or changing colors processes at most2048 retained points rather than rescanning the entire chart. Above2048 samples, each complete ordered bucket contributes its maximum sample, covering the entire chart with bounded geometry and preserving global peak. The label explicitly says aggregated. It is not a partial/truncated chart. Exact per-measure data remains in the Report. Native float RGBA/mesh primitives adapt the source graph; no claim of MonoGame pixel identity.
6. Six position formulas consume the existing native chart/header/lane/misc anchors. DPI conversion truncates like source UiScaled. The native logical canvas currently supplies scale1. Reverse flips vertical graphs only. A white rim, time marker, source peak and optional rotated stream text remain bounded. Mouse dragging is represented by existing keyboard chart navigation; no mouse dependency is introduced.
7. The five-row keyboard inspector chooses one of20 fields, edits its value, restores defaults, saves or cancels. Color channels and per-mount offsets are all reachable. Save commits one global preference history action; Cancel has no side effect. This staged native inspector adapts the source immediate-edit window.

Persistence Undo owns changed leaf paths, including individual color channels and mount offsets. Other preference sections and unrelated density fields survive intervening external changes. A changed owned leaf causes an atomic external-change refusal. Preference history never dirties the simfile.

## Application order

1. Copy durable sidecars:
   - editor-density-core.rs → editor src/density.rs
   - editor-density-task-core.rs → src/density/task.rs
   - editor-density-layout-core.rs → src/density/layout.rs
   - editor-density-preferences-core.rs → src/density_preferences.rs
   - editor-density-history-core.rs → src/density_preferences/history.rs
   - editor-density-clock-core.rs → src/generation/editor_timing/density.rs
   - editor-density-{graph,render,settings}-native.rs → theme screens/edition/density_{graph,render,settings}.rs
2. Regenerate `editor-density-build.py` and `editor-density-ui-build.py`, check then apply core.patch followed by native.patch. Do not apply old contexts after adjacent parent integrations.
3. Core tests sidecar → tests/density_graph_workflows.rs (11). Native tests → support/density_graph/mod.rs + usual EN/FR wrappers edition_density_graph[_fr]_workflow (3 per locale).
4. Parent runs targeted Cargo validation. Include source timing/fake flags, shared preferences/history, Routine and minimap layout regressions. No full-theme success is implied by these prepared tests.

Prepared tests cover source defaults/JSON/restore witnesses, all count/NPS/stream cases and overflow adaptation,6 source geometries, exact Stop/warp measure phases in real native documents, Routine participants/rows, fake exclusions, cancellation/identity/resource refusal, nested ownership/conflicts, UI persistence/Undo/Redo/defaults/Cancel and finite bounded read-only actors alongside accessible RSSP analysis.
