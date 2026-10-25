# GrooveAuthor functionality in DeadSync

## Delivery contract

The `edition-mode` branch starts at fork main `1408347b6`, including French
localization, partner game modes, and macOS application support.

The requested result is the same functionality as GrooveAuthor, implemented in
Rust within DeadSync, with a simpler interface. Chart generation, pattern
generation, their detailed configuration, and the other advanced tools are
required. Basic note editing is an intermediate milestone, not completion.
Every ported feature needs functional tests of its public workflow. A successful
build or a test of a helper formula alone is insufficient.

The user approved a central chart, a small permanent toolbar, and advanced tools
in panels opened on demand. Tools must stay accessible without opening all
panels at once. No upstream pull request is planned before the complete work is
reviewable and tested.

## Pinned reference

- GrooveAuthor: `48e18f71655d0f0d86144c63239331833501fda2`, local checkout
  `/Users/zydon/Sites/feetness/grooveauthor`.
- Its StepManiaLibrary submodule: `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`.
- `reference-inventory.json` records the source files and hashes. It is an
  inventory, not evidence that every file has been examined or ported.
- Fumen: `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`; its 83 source/project/documentation files are inventoried. Inventory does not imply review.
- The 14 pad definition JSON files are copied unchanged from the pinned library.
  `dance-couple` and `pump-couple` use the corresponding double geometry.
- The 11 original compressed FSG graphs and all 58 serialized step types are
  imported. The Rust decoder validates counts, states, destinations and size
  limits. Functional graph workflows check all reference node counts and root
  reachability, normal tap sequences, holds/releases, and bracket transitions.
- Source and data used from these projects retain their MIT license notices in
  `licenses/`. No MonoGame or FMOD binaries are included.

## Current implementation and verification

| Domain | Rust implementation | Evidence / remaining work |
| --- | --- | --- |
| SM/SSC source preservation | Byte spans and explicit replacements; unknown tags, unchanged chart note data, and legacy metadata bytes preserved | `editing_workflows`, `property_workflows`; broader format corpus still required |
| Notes | Sparse lanes, sustains, rolls, mines, lifts, fakes, keysound and attack attribute preservation; arbitrary existing participants | Round-trip and owner tests; attribute inspector, new participants, StepF2 conventions pending |
| Commands | Gesture undo/redo, selection, cut/copy/paste, automatic advance | Core command workflows; clipboard conflict behavior intentionally refuses overlapping sustains instead of silently removing them; GrooveAuthor ForceAdd parity pending |
| Layouts | Reference pad coordinates, starting tiers, ten relationship matrices, mirror/flip mappings | All imported layout definitions validated; original compressed graphs decoded and traversed |
| Transformations | Mirror, flip, move rows, reassign existing participant, convert note kinds | `selection_workflows`; native panel exposes mirror/flip/move; broader GrooveAuthor transformations pending |
| Native file safety | Atomic same-directory replacement, advisory editor save lock, byte-exact external change check, durable explicit recovery drafts | Save/reopen, concurrent editors, external change and recovery tests; background automatic drafts and conflict review UI pending |
| Interface | Editor opened from Practice menu or F2; central chart, compact toolbar, optional tools/help, close confirmation | `edition_workflow` tests real native key adapter and generated actors; hardware input, pointer, complete runtime rendering and navigation verification pending |
| Metadata | Existing and new scoped tags with escaping/history; F7 chart expression and audio-offset properties | Property and chart-property save/reopen/undo tests; remaining song/chart/pack inspectors pending |
| Timing/gimmicks | Original tags preserved | Typed editing, inheritance, conversion and timing tools pending |
| Playback and audio | Existing DeadSync Practice audio retained outside the editing workspace | Edited chart preview, waveform, scrub, loop, Song Sync and audio analysis pending |
| Multi-chart | Switch existing charts with Ctrl/Cmd+Tab; shared history | Generated chart creation is available; generic creation/clone/delete, multi-view comparison and library pending |
| Generation | Original graphs, typed configurations, seeded .NET random, performed-path scoring and pattern search with start/end footing and lookahead | Differential pattern fixtures across all 11 original graph layouts, timing changes, context and seeds; preview/apply/undo/save and cancellation tests. Expressed-chart lane-state preparation and graph candidate matching are compared to the original interpreter across all 11 layouts, including holds, rolls, fake/lift instances and jumps. Interpretation costs, automatic bracket policy selection and mine expression match original chart interpretation; replacement expansion and ranking match the original cache; performed-chart conversion and destination mines compared across Dance/Pump/SMX; automatic fallback selection, file assembly and asynchronous chart-conversion previews are connected; single-region native pattern previews are connected through F4; F8 converts all charts of a selected source type with per-result review and atomic acceptance; F9 regenerates selected/all saved patterns with one preview and history action; full configuration editing and broader runtime parity remain pending |
| Analysis | Existing Practice density graph outside the editing workspace | Editor density/NPS/streams/minimap and navigation pending |
| Customization | Initial native shortcuts | Rebinding, presets, colors, scroll/receptor/waveform preferences and command palette pending |
| Export and scale | Original SM/SSC format retained | Cross-format export checks, advanced save settings, long-file benchmarks and large-generation cancellation pending |

### RSSP reuse

The user's explicit instruction is to reuse the RSSP library already included in
DeadSync wherever it fits. The editor depends on the same Git dependency and
locked revision (`da616e3a`); no second revision is introduced. Metadata decoding
uses its CP1252/UTF8 and escape handling. Snapshot analysis uses its parser,
statistics, timing, NPS, streams, hashes and tech analysis. `rssp_workflows`
checks that unsaved edits affect analysis and that the saved/reopened output
has the same stats and hash. The native analysis panel is still pending.

RSSP's normalized summary writer is available for future reviewed exports.
Source-preserving editor saves retain unknown fields that are absent from an
analysis summary; they must not silently replace the original with a normalized
summary. RSSP layout/analysis limits remain explicit; the editor's participant
model and GrooveAuthor generation are not limited to RSSP's supported layouts.

## Commands to reproduce current checks

```sh
cargo test -p deadsync-editor --locked
cargo test -p deadsync-theme-simply-love --test edition_workflow --test edition_batch_workflow --test edition_pattern_set_workflow --test edition_pattern_set_fr_workflow --locked
CARGO_INCREMENTAL=0 cargo check -p deadsync --locked
```

Tests use synthetic temporary files. Native screen tests deliver synthetic
`RawKeyboardEvent` values to the actual editor adapter; they do not prove physical
keyboard/pad input or GPU output. Owner-color actor assertions prove that both
participants are represented in the generated scene, not that a GPU rendered it.

## Remaining generation path

GrooveAuthor `ActionAutoGenerateCharts` first builds an `ExpressedChart` from
the source notes and input step graph, then a `PerformedChart` on the destination
step graph, with its performed-chart configuration, fallback rules, roots and
seed. `ActionAutoGeneratePatterns` additionally constrains preceding/following
footing and pattern configuration. The Rust port must preserve these contracts.
A random lane picker or a basic difficulty filter would not meet the request.

Automatic fallback selection, generated-note assembly and reversible chart
creation, source preparation, session-bound previews and a native chart-generation
panel are implemented below. The surrounding-footing and region primitives below are also implemented. Next
work must extend integrated single-region differential coverage, connect native
controls, and complete configurations and batch generation.
Differential fixtures must extend the existing tech, bracket, crossover,
hold, mine, layout and seed cases to that complete conversion pipeline.
Generation results need preview and one reversible application gesture before
being exposed as complete. The other advanced editor domains remain required
as recorded in the delivery contract and matrix above.

## Pattern generation reference checks

`pattern_generation_workflows` compares generated lanes, foot assignments, step
kinds and timestamps against committed output from the unmodified, pinned C#
StepManiaLibrary. The development oracle runs separately from the product:

```sh
# Requires .NET 10 and the pinned recursive StepManiaLibrary checkout.
dotnet build /path/to/StepManiaLibrary/StepManiaLibrary/StepManiaLibrary.csproj -c Release
python3 scripts/editor-generate-fixtures.py --reference-root /path/to/StepManiaLibrary --editor-root /path/to/GrooveAuthor --dotnet /path/to/dotnet
```

Ordinary Cargo tests require neither .NET nor network access. Fixture provenance
records both library pins and the SDK. Fixed seeds cover negative and extreme
values; patterns cover all eleven original graphs, prior lane/foot context,
explicit footing, invalid roots, BPM changes, stops, delays, overlapping warps
and simultaneous timing events. They do not establish whole-chart generation
parity or complete editor parity.

Generation decisions use the reference Fumen row-based f64 clock. Replacing it
with beat-based or f32 timing can change strict density-threshold decisions.
Parsing and chart timing inheritance reuse RSSP; gameplay and analysis continue
using RSSP timing. Fumen's MIT notice is retained in `licenses/`.

Pattern generation returns an immutable preview. Applying it checks the document
revision and creates one reversible history gesture. Timing preparation and
search both support cancellation. Automatic preceding/following footing
interpretation and the native preview/configuration panels remain to be ported.

### Expressed-chart interpretation stage

`generation::expressed` prepares participant-specific transient lane states from
an editor grid, retaining held rolls and separating releases from steps at the
same position. Mine and last-release context follow the reference phase order.
Graph candidate enumeration retains edge/target order and foot-portion instance
types; its scratch buffer is reused across destinations. It supports cancellation
without returning partial candidates.

`expressed_workflows` compares complete candidate lists, destination foot states,
step types, actions and roll/fake/lift instance assignments with the original
private `GetLinkInstanceIfStateMatches` through the development oracle. Fixtures
cover all eleven original layouts. Grid workflows additionally cover participant
isolation and simultaneous releases, mines and heads. This verifies candidate
matching, not the final interpretation: performed-chart conversion is verified separately below.

### Interpretation path selection

`interpretation_path` ports `ChartSearchNode` history, cumulative costs, last-foot
tracking, release-skipping history queries, deterministic path comparison and
best-path pruning per destination. It preserves insertion order and the source
strict comparison on equal paths. Long unique histories are dropped iteratively.

The development oracle executes the original `GetCost`, `GetOrientationCost`,
`ChartSearchNode.CompareTo` and private `Prune` on branching graph interpretations
under Balanced, Aggressive and NoBrackets policies. Rust tests replay those
original move costs and verify retained paths, final choices, cumulative costs,
last-foot ownership and preceding-step identities. These tests now also compare every candidate move cost and orientation cost
calculated in Rust. A small-stack test drops 100,000 history nodes.

### Movement costs and full step search

`interpretation_cost` ports all source cost branches for single steps, swaps,
crossovers, inversions, jumps, brackets and three/four-arrow combinations, using
the original pad relationships. Source decision order is retained, including the
pinned two-arrow helper's duplicated Behind pairing check. `expressed::search`
combines matching, Rust costs and deterministic pruning, with progress and
cancellation. Source grids remain immutable; no partial path is returned on
failure or cancellation.

The original private `ExpressedChart.Search` is invoked on complete synthetic
charts by the development oracle. Thirty-three committed cases cover all eleven
original graph layouts and all three fixed bracket policies. They include holds,
rolls, releases simultaneous with heads, mine cues, lifts, fakes, jumps, brackets,
keysounds and impossible interpretations. Rust checks the chosen path's complete
foot/portion states, actions, instance types, per-move and cumulative costs,
orientation costs and ownership history. Thirty reference searches succeed;
three fail, and Rust matches both outcomes. Keysounds are ignored as movement
instructions like the reference's generic LaneNote; unsupported attack notes are
explicitly refused rather than converted into steps.

These checks establish fixed-policy step interpretation. The automatic policy
and mine-expression checks below extend this stage; performed-chart conversion is covered below; native generation panels remain pending.

### Complete expressed-chart assembly

`expressed::interpret` ports automatic bracket-method selection: configured
method, difficulty threshold, simultaneous notes requiring brackets, and a
balanced pass optionally repeated according to bracket frequency. The original
event time span is explicit, since timing events, mines and keysounds may extend
beyond movement frames. Zero-duration spans retain the source division behavior.

`mines` derives release and step actions from foot states, preserving the source
ordering and its treatment of simultaneous actions. It associates mines with
preceding releases first, otherwise following steps, otherwise no neighboring
arrow. Relative recency groups simultaneous events and tracks the associated
foot. Mine events are sorted by the source semantic keys. `build_chart` combines
the chosen path, step events (including releases) and mine expressions as the
input for performed-chart conversion, without changing a source document.

The C# oracle now invokes the original public `CreateFromSMEvents` for ten
policy cases, including zero brackets, zero duration, empty charts and mine-only
charts. Step paths, policies, release/step lists and mine associations are checked
against Rust. The existing thirty valid fixed-policy charts also compare their
mine results. A dedicated case covers AfterArrow, BeforeArrow and NoArrow,
recency ties and foot association. Cancellation and invalid-input checks return
no partial interpretation. Destination mine placement and fallback conversion are verified below.

### Performed-chart replacements

`fallbacks` imports the unchanged `default-steptype-fallbacks.json`, expands its
include lists and validates every step's replacements. It ports foot-portion
selection, hold/release priority, instance metadata, bracket expansion, jump
restrictions, blank links and the original ranking. Source-specific portion
behavior is retained rather than silently corrected. Fallbacks can be disabled,
which returns the source link unchanged like the reference.

Replacement order affects fallback indices. `reference_sort` ports the
comparer-based ArraySortHelper introsort from .NET runtime `v10.0.13`, retaining
the permutation of equal-ranked replacements. The source and MIT notice are
recorded in `licenses/dotnet-runtime-MIT.txt` and fixture provenance:
[ArraySortHelper.cs](https://github.com/dotnet/runtime/blob/v10.0.13/src/libraries/System.Private.CoreLib/src/System/Collections/Generic/ArraySortHelper.cs)

The original cache is invoked through the development oracle for 475 source
cases: all 58 step types, both feet, taps/holds/releases, mixed bracket actions,
roll/fake/lift metadata, jumps, four-arrow combinations, blanks and disabled
fallbacks. Rust compares complete ordered lists, dropped-foot detection and
removed-step counts. Forty sorting cases independently compare equal-key
permutations around partition thresholds and larger arrays. Configuration tests
cover comments, trailing commas, includes, missing/duplicate entries and cycles.
Cached replacement slices are immutable and protected by a short mutex;
concurrent first requests compute independently and share the stored result.
These checks establish replacement expansion and ordering. Their integration
with performed-chart search and mine placement is verified below; native file
assembly and editor application remain separate work.

### Performed-chart search and destination mines

`performed::convert_steps` ports root tiers and stable randomized root order,
replacement costs (including dropped feet/arrows), per-destination pruning and
same-position release collision checks. Incoming instance metadata is retained
along the performed scoring path. `convert` continues the same PRNG into the
original mine placement rules: relative recency, preferred foot, hold/step and
mine occupancy, random free-lane fallback, decreasing recency and unused lanes.
Results carry their destination layout and counts of skipped events/mines.
The original's blank-link continuation and empty-performance behavior at a
failed final depth are preserved for differential fidelity; previews must expose
these losses before application. A missing release for an AfterArrow mine
returns an error instead of indexing an empty release list.

Twenty-nine committed C# conversion cases compare complete destination foot
states, links, roll/fake/lift instances, starting tiers, steps and mines. They
cover eight Dance/Pump/SMX layout pairs, fixed positive/negative seeds, root
tier fallback, blanks, impossible paths, empty and mine-only charts, recency
fallback and simultaneous NoArrow mines. The last case retains the source's
raw duplicate mine behavior; file assembly merges identical duplicate mines and
validates the resulting note grid before applying a preview.

A recency-fallback fixture exposed a PRNG mismatch that ordinary step-path tests
had not detected: LINQ OrderBy().ToList/ToArray does not evaluate its key selector
for a singleton. `SeededRandom::order_indices` preserves this behavior. Eighteen
original-runtime cases compare both ordering and the next PRNG value, proving
that root-tier ordering does not shift subsequent mine placement. Conversion
cancellation, invalid roots and invalid mine associations return no partial
performance. These cases use synthetic lane events with explicit timestamps.
The source adapter tests below cover split timing and the interpreter's
first-event-of-row grouping; a wider full-conversion corpus remains needed.
The native conversion panel and save workflow are covered separately below.

### Generated notes and chart creation

`output::assemble` converts performed foot states and instance metadata to a
canonical Grid, including holds, rolls, lifts, fakes and mines. It rejects
orphan releases, unterminated holds and conflicting cells, and reports merged
identical mines. All twenty-nine conversion fixtures compare the assembled notes
with `CreateSMChartEvents` from the original C# library. Automatic fallback
selection is checked against the original fourteen-by-fourteen pad containment
matrix, with sparse and translated custom-pad cases.

`Session::append_chart` copies current source chart metadata and effective timing
into a new SM/SSC chart, preserving existing file bytes and unknown properties.
Structural history owns both document states, so chart creation, earlier note
edits and subsequent metadata changes remain independently undoable and redoable.
The final simfile is reparsed before history changes. Note attributes reject
unescaped simfile delimiters and comment openers, preserving valid escaped data.
Tests cover save/reopen, RSSP analysis, legacy metadata, multiple additions,
opaque unsupported charts, stale revisions and atomic rejection. One real-file
workflow reads source notes, timing and mines through FileSession before
interpretation, conversion, assembly, append and save/reopen. This does not yet
prove the full split-timing adapter behavior described above, and generation UI
remains pending.

### Immutable source preparation and interpretation clocks

`source::prepare` reads a serialized editor snapshot so unsaved notes, effective
chart timing and difficulty agree. It prepares movement frames, first-event-of-row
times, individually timed mines and the original first/last-event span. Timing-only
rows, keysounds and sustain tails contribute to that span. Miscellaneous timing
events before delays and scroll/speed events after notes retain their ordering.
The adapter does not invent a 60 BPM event when no BPM exists: Fumen leaves such
charts at zero event time, including SSC charts whose only local timing is OFFSET.

Five differential clock cases verify exact f64 timestamps for same-row tempo and
delay events, ordinary/negative stops and overlapping warps with tempo changes.
Seven real files are read by the unmodified Fumen SMReader/SSCReader, then passed
to the original ExpressedChart engine. Rust compares event spans, frame and mine
timestamps, complete interpreted movement links/instances and mine associations.
They include song timing, chart timing, offset-only timing, metadata beyond the
last note, and a final delay with/without a same-row scroll event. An integration
test uses this adapter for conversion, append, save and RSSP reopen. Snapshot,
invalid-request and cancellation checks cover unsaved edits as well.

`serde_json` enables `float_roundtrip`: the default parser can shift reference
values by one ULP, changing strict density/policy comparisons. Fixture numeric
comparisons retain exact f64 equality; whole-valued doubles normalize only their
JSON integer/float representation.

This is not yet complete input parity. Attack timing and generation of multiple
participants are explicitly rejected by source preparation. Fumen timing-entry
validation, conflicting/rounded beat positions, unusual encodings and broader
cross-format/legacy timing cases still require reader-level differential coverage.
Native conversion jobs and the first panel are described below; full input
parity and the complete set of GrooveAuthor generation tools remain unfinished.

### Background chart generation and native preview

`ChartTask::capture` snapshots unsaved document bytes and validated configuration.
`run` composes preparation, expressed interpretation, automatic replacements,
performed conversion and output validation. `spawn` runs it on a named worker;
progress uses one shared slot and completion uses a bounded single-result channel.
The worker never borrows the editor, mutates a session, or writes a file. Dropping
a job requests cancellation without blocking the UI; cancellation also discards
an already-computed result before publication. The screen retains a cancelled job
until terminal so repeated input cannot launch concurrent generation jobs there.
Initial graph loading and snapshot serialization are not yet interruptible.

Preview identity combines a live-session identity and history revision, so equal
revision numbers after reload/recovery cannot authorize application. The source
chart must still be selected. One explicit apply adds one undoable chart; no file
write occurs until Save. Structural undo/redo reloads the active grid and clamps
a selection whose generated chart was removed. Reports disclose source/output
note and mine counts, skipped events, dropped/merged mines, ignored keysounds and
empty output. They do not silently apply a lossy or empty result.

F3 or F1 > Generate opens the native panel: eleven official destination layouts,
Balanced/Stamina Single/Stamina Double profiles, and a reproducible signed seed.
Up/Down chooses a field; Left/Right changes it. Calculate produces an immutable
preview. Page Up/Down and Home/End inspect its notes; Apply adds it, Escape or
Discard abandons it. The normal Practice update loop polls the worker. Polling
does not occur inside a key handler, so an Enter intended to cancel cannot become
an Apply merely because a result arrived during that event. Controls and status
labels are provided in English and French, with the existing locale fallback.

`generation_job_workflows` covers worker completion/cancellation at every stage,
late cancellation, immutable snapshots, stale/reloaded/recovered sessions, source
selection, explicit mine merging, empty/keysound-only results, one-gesture history,
Save/reopen and unchanged source bytes. Seven real Fumen-reader fixtures now
continue through the original C# performed converter and CreateSMChartEvents;
the Rust task's final generated notes are compared with those outputs. Native
keyboard workflows cover preview actors, explicit apply, undo/redo, save/reopen,
discard, cancellation and unsupported multiplayer input without modifications.
These actor assertions still do not establish GPU rendering or physical input.

This first chart-conversion panel does not complete the generation feature set:
full expressed/performed configuration editing, presets management, batch
creation and advanced roots/fallback controls remain required. Single-region
native pattern generation is described below. The rest of the editor delivery matrix remains in scope.

### Pattern-region footing and replacement

`context::derive` ports the five static surrounding-footing helpers from
GrooveAuthor ActionAutoGeneratePatterns. Following and preceding regions are
interpreted independently, each with its original natural root. The port keeps
same-arrow history, per-foot preceding times, following-foot parity, jump/time
ambiguity, bracket heel/toe priority, root-lane collision correction, side
transitions and the late-chart distribution guess. A transition at absolute step
zero is discarded when building RootContext, like CreateWithPattern.

The development oracle extracts these five method bodies unchanged with `git
show` from the pinned GrooveAuthor commit. It compiles them with small editor
event/pattern adapters and the original StepManiaLibrary. The adapters provide
explicit event order and pattern bounds; they do not recreate the whole editor.
The generated source lives under ignored obj/, and provenance records its method
hash and editor commit. The fixture generator now needs the pinned editor checkout
(`--editor-root`, defaulting to sibling grooveauthor). Cargo tests remain fully
independent of .NET and reference checkouts.

One hundred fifty-four committed differential cases cover all eleven official
graph layouts: missing context, odd/even pattern lengths, preceding taps/jacks,
jumps, brackets, holds/rolls, releases sharing a step row, mines/keysounds,
fakes/lifts, equal-time events, late-chart context, real side transitions and
unsorted overlapping-hold event order. They compare all derived footing/history
values, then feed them into pattern generation and compare the final notes with
the original CreateWithPattern. Of these cases, 152 produce matching patterns;
one source interpretation and one generated pattern are rejected by both engines.
Invalid/cancelled inputs produce no context.

`pattern_region` implements the 32-step surrounding window (extended for minimum
transition spacing), complete boundary rows, and original region extraction
order. Holds overlapping the beginning are included as full head/tail pairs,
ordered by lane before normal row-ordered editor events. Hold tails do not count
as editor step heads. Window/extraction tests currently use explicit editor events;
reader-level verification of every event category is still required.

Pattern application now shares the original deletion rules: remove heads in the
region, shorten crossing holds to one pattern spacing before its start, fall back
to the immediately preceding row when needed, and remove a hold with no room to
remain a sustain. One note gesture performs trimming, removal and generated-note
insertion together. Tests preserve attributes and other participants, undo to the
exact original bytes, redo, save and reopen the resulting grid. The selected-owner
primitive is explicit; whole multiplayer generation remains unfinished.

These primitives feed the single-region source-to-pattern task described below.
Wider original-action differential coverage and batch pattern actions
remain required. Footing and density come from the source
contexts rather than fixed values chosen to match the existing tests.

### Editor timing and pattern-density decisions

`source::read` now creates an owned SourceChart (grid, effective timing,
layout and difficulty). Native chart conversion now uses `prepare_editor`,
which retains editor-assigned times like the original GenerateSmEvents action.
The standalone `prepare` entry point retains Fumen clock semantics. Alternate grids can reuse the same parsed source without changing the
original grid or live editor. Attack-note rejection and unsupported participant
checks remain explicit.

`GenerationTiming` now retains distinct time-signature, fake-region, tick, combo,
label and scroll event categories. Fumen generation preserves its clock semantics.
`EditorTimeline` implements the separate GrooveAuthor editor timing/dependency
pass: active warps/fakes, shorter nested fakes ending an earlier region, negative
stop clamping, and retroactive same-row fake flags with stop/delay exceptions.
Time signatures inside warps commit elapsed warp time only in the editor pass,
as the two original implementations differ here.

A second pass prepares rate-altering event states. Position-only time queries
use the preceding rate event and fall back to the first event; event-type queries
respect same-row order. The first tempo backfills earlier rate states. Hold ends
use hold-tail order. Positive stop/delay ends include their duration, fake/pattern
ends query their end position, and warp ends retain the original point time.
Chart end follows the final editor event, not the largest sustain tail. Source
step counts exclude explicit fakes and notes made fake by timing. Pattern NPS
combines that count and the supplied pattern-step count over the original end-time
rule. No fixed NPS or substitute lane-density formula is introduced.

The development oracle compiles nine unchanged method bodies from the pinned
editor: timing refresh, rate refresh, position/time queries, negative-stop end
refresh, chart start/end, and pattern GetNps. Minimal containers/accessors replace
UI/tree plumbing; the test does not run the whole GrooveAuthor editor or its file
loader. Provenance records the extracted method hashes. Thirty-nine committed
cases compare exact f64 head times, fake flags, step counts, chart ends, position
queries and NPS. Cases include simultaneous events, zero-delay guards, nested
warps/fakes, time signatures inside warps, negative-stop stacks, long holds before
later metadata, final stop/delay/fake/warp events, scroll changes, pattern-region
ends, missing tempo and zero duration. Tests also reject cancelled, invalid and
numerically unrepresentable inputs, and reuse chart-local timing without altering
the original source grid.

`EditorTimeline::project` now preserves surviving assigned times, fake flags and
rate states after lane-only deletion, validates shortened sustains, and inserts a
new pattern marker using the typed active-rate query. `prepare_region` extracts
complete hold pairs, global clock events and local metadata without recomputing
an isolated Fumen timeline. It retains the first included event's time per row
and the full included event span. The development oracle additionally compiles
unchanged original region extraction and deletion helpers. All 39 cases compare
these projected regions, including empty ranges and shortened tails. Mixed note
kinds on one row verify the original lane-before-type comparison.

A separate native ChartTask test serializes 36 of these cases as SSC and compares
the generated double-chart notes against the original library consuming the
editor-assigned events. Multiple loaded markers are now included; the raw
before-refresh Region cases and missing-tempo loader defaults remain excluded. This is
not a claim of complete original file-loader fidelity, especially for keysounds.

`pattern_job::Task` now connects this preparation for one region: marker-relative
inclusive/exclusive endpoints, hold-aware deletion, projected timing and density,
32-step bounds extended by transition settings, following then preceding
interpretation, full preceding lane counts and context derivation. It invokes the
pattern kernel and assembles an immutable final-grid preview through the same
Session edit path as acceptance. A private Session identity/revision token and
selected-chart check protect acceptance. The preview reports deletions, shortened
holds, average NPS and kernel warnings. Seven workflow tests cover empty contexts,
crossing holds, endpoint rules, cancellation, foreign/stale previews, one-gesture
undo/redo and save/reopen, including the Workspace command path.

Chart and pattern jobs share a generic bounded worker with a single progress
slot and one completion result. Cancelling an already ready result discards it;
dropping a worker never joins the compute thread. Pattern jobs are connected to
the native interface through the optional F4 panel. An integrated differential test serializes 36
cases as SSC and compares generated notes and exact average NPS with the original
editor region/deletion/density/range/context helpers chained to the original
StepManiaLibrary interpretation and pattern generation. The additional original
range helper is extracted unchanged and included in provenance. Cases cover
stops/delays, warps and signature changes, negative-stop clamping, crossing holds,
local metadata, dense rows, exclusive endpoints, single-step patterns, beginning
of chart and empty charts. All 36 produce matching notes. Four added cases load multiple markers after the
timing refresh, including dense warps, negative stops and mixed note kinds. Raw
before-refresh Region cases and missing-tempo cases remain excluded from this
integrated SSC test.

This harness composes original helper bodies with explicit container/accessor
adapters; it does not execute the complete editor action, file loader, UI or
multi-pattern scheduler. The integrated fixtures currently use dance-single,
balanced configs and seed 42. Other layouts/configs/seeds have component-level
coverage but still need broader integrated differential coverage.

Batch regeneration and overlap scheduling remain unfinished. Repeated live edits also need timeline
cache semantics: rebuilding after each edit can change floating-point clamp
history. These tests do not prove the complete editor port.

Reader normalization for invalid/conflicting timing entries, unsupported attack
timing, previews/last-second hints and wider
legacy input coverage remain unfinished. In particular the missing-tempo case
above verifies these extracted methods, not the complete editor loader's defaults.

### Native single-region pattern panel

F4 or F1 > Generate pattern opens a panel over the central chart. It captures the
current selection; without a selection it uses four beats starting at the cursor.
The panel exposes all nine supported subdivisions, four original pattern
performed profiles, signed seeds, inclusive/exclusive endpoints and ignoring
preceding lane distribution. The range is displayed in beats; select a different
range in the chart before reopening the panel. Other pattern/expression/performed
configuration fields still require a future advanced inspector.

Calculation shares the bounded, cancellable background worker and immutable
preview flow with chart conversion. The preview shows the full resulting grid,
fixed generated-region highlighting even while browsing, note deletions,
shortened holds, average NPS and translated footing warnings. Apply replaces the
region and stores its marker in one undoable gesture; it does not save the song.
Escape/Discard and cancellation retain the original document and history. The
Practice update loop polls both job kinds and input handlers never poll jobs.

The native workflow suite now covers F4 and menu entry, selected and default
ranges, subdivision/profile/signed-seed/endpoints/distribution controls, preview
actors, explicit application, undo/redo/save/reopen, discard/cancel, unsupported
multiplayer input, and explicit English/French strings. These tests execute the
real raw-key adapter and actor construction, not GPU rendering or hardware input.
The complete port, advanced marker configuration, multi-region generation and the
other delivery-matrix requirements remain unfinished.

### GrooveAuthor V1 pattern metadata storage

`generation::metadata` reads and writes the original `GAChartDataVersion` and
`GAChartData` fields. SSC scopes them to the chart; SM uses the original
`CHART0000` suffix convention (four-digit indices). Definitions retain original
GUIDs, lengths, signed seeds, endpoint flags and distribution flags, along with
chart music-offset settings, expressed-profile identity and player-index data.
Unknown chart/marker JSON fields survive edits. New custom tag names preserve
their requested spelling because Fumen extras and GrooveAuthor custom lookups
are case-sensitive; ordinary document lookups remain case-insensitive.

A development-only .NET oracle compiles the unchanged original CustomSaveDataV1
and EditorPatternEvent.Definition types. Seven fixtures compare their serialized
values/defaults, null patterns, comments, strict GUID JSON parsing and rejection
of trailing commas. This proves the JSON type contract, not the full editor
loader or every legacy file variant. Provenance includes hashes of the extracted
types. The Rust parser rejects invalid/overflowing rows and lengths instead of
silently deleting markers. Unsupported versions/incomplete data remain opaque
and cannot be overwritten through the V1 saver. Extra fields cannot shadow known
fields when writing.

The initial in-memory registry maps the original two pattern, seven performed
and three expressed built-in GUIDs to the ported configurations. Custom maps may
be supplied explicitly; an unresolved GUID is reported rather than silently
substituted. Configuration-file import/export and guarded custom-profile storage
are implemented below. Native profile management/editing and the original
missing-profile fallback/diagnostic behavior remain unfinished.

`Session::set_properties_atomic` validates all tag edits on an isolated snapshot
and records a single reversible property gesture without copying note data. Failed edits preserve redo and
pending note changes. Eleven workflow tests cover the original C# JSON fixtures,
SM/SSC save/reopen, unknown fields and legacy source bytes, profile resolution,
invalid inputs, atomic rollback, and custom-tag spelling through updates,
undo/redo and chart cloning.

Original constructor inspection confirms that custom markers are added after
SetUpEditorEvents, through typed rate queries, without refreshing surviving event
times. `EditorTimeline::with_markers` follows that sequence, retains heads/rates,
rejects conflicting duplicate rows and checks cancellation. SourceChart retains
its immutable document so editor consumers can read markers without reparsing the
file. Standalone Fumen preparation still ignores editor-specific metadata; native
editor generation reports unsupported metadata rather than guessing its timing.
Existing raw fixtures inserting Region before refresh represent a different state.

### Regenerating a loaded marker

F5 or F1 > Regenerate saved pattern finds the saved marker containing the cursor;
when ranges overlap it chooses the marker with the latest start. Its original
pattern/performed/expressed profiles, seed, endpoint and distribution settings
feed the same snapshot-bound background preview as F4. Applying the result changes
notes in one reversible gesture, preserves marker data/unknown fields and writes
no file until Save. Saved markers are drawn beside the central chart. Metadata
for drawing is cached by source identity, revision and active chart, rather than
parsed while constructing actors on each frame.

The native suite covers the F5/menu route, saved-marker actors, exact settings
against the core task, preview/apply/undo/redo/save/reopen and absent markers.
Core workflows cover SM and SSC reloads, overlapping marker selection, preserved
metadata, cancellation, invalid bounds and unresolved profiles. The integrated
36-case original-helper comparison includes four after-refresh loaded-marker
cases. It composes original helper bodies and the original library with container
adapters; it does not execute the full GrooveAuthor loader/runtime or scheduler.

F4 now persists new marker definitions on acceptance, as described below.
Existing marker bounds can be changed through F6. Creation still rejects an
existing marker at the same raw start rather than silently replacing it. Native
profile management, live timeline caching, multi-region/batch and
overlap actions, advanced marker configuration and the remaining editor delivery matrix
remain required for the complete port.


### GrooveAuthor profile files and native loading

`generation::profiles` imports the original `pc-<guid>.json`, `pcc-<guid>.json`
and `ecc-<guid>.json` wrappers, rather than treating them as bare library configs.
UTF-8 BOMs are accepted like the original stream-based loader.
The ordered root members are retained because PatternType, StartingFootSpecified
and the three travel-speed properties mutate Config during deserialization. Later
Config assignments replace earlier setter effects. Comments, trailing commas,
case-insensitive enum names, surrounding enum whitespace, numeric enum strings
and named enum combinations follow the reference JSON behavior. Unknown wrapper
and nested generation members remain as raw JSON for export, including numeric
literals outside normal integer/floating ranges, while the current
library's known fields determine generation. Renaming preserves that ordered body.

Twenty .NET fixtures compare effective generation values against five original
setter bodies, the original speed-update and subdivision helpers, and the original
subdivision enum extracted unchanged from the pinned editor. Explicit adapters
replace the generic wrapper/observer plumbing; the oracle does not run the whole
profile UI or reproduce its display caches. Cases cover before/after/interleaved
Config assignment, duplicate Config members, enum syntax, clamped speed indices,
unknown fields, invalid identities and invalid settings. The source-body hash is
recorded in provenance. Ordinary Rust tests consume committed results without
requiring .NET or reference checkouts.

New pattern profiles support all nine subdivisions and use random UUID v4
identities. Configurations resolve separately by kind, so the same GUID may occur
in pattern, performed and expressed namespaces. Directory loading retains built-in
profiles, reports duplicate identities deterministically (later filename wins),
skips invalid profile files without changing them, and preserves unrelated files.
Absent directories return built-ins and are created only on export. Export refuses
existing filenames; guarded updates compare exact prior bytes under a sidecar
lock and commit through an atomic temporary-file replacement. Failed saves retain
the caller's editable profile. Profile names cannot become filesystem paths.

The shell installs the editor's directory through AppDirs and editor::runtime:
`AutogenConfigs` under DeadSync's application data root, including portable and
isolated layouts. F5 loads that directory on the background worker, not in the raw
key handler. It captures the song revision first, resolves profile files when the
worker starts, clones the selected configurations and releases unused profile
bodies before pattern search. Unrelated loading issues are counted in the preview;
missing required profiles fail without changing the chart. Loading and generation
support cancellation, and previews retain the existing identity/revision guard.

Nine profile workflow tests cover the C# fixtures, ordered round trips, unknown
fields, guarded saves/external changes, all nine persisted subdivisions feeding
saved-marker generation, duplicate/built-in handling, independent namespaces,
missing directories, immutable song snapshots and cancelled workers. A native
workflow creates a custom profile, regenerates through F5, applies/saves/reopens,
regenerates again, and checks the visible ignored-file report and unchanged bad
file. A path test checks application data roots. Actor tests do not prove GPU
rendering or physical keyboard delivery.

The native advanced configuration inspector and profile library management remain
unfinished. Profile-file compatibility and loading do not complete those tasks,
batch generation, waveform/audio editing or the rest of the delivery matrix.


### Persistent F4 creation and grouped history

F4 prepares both generated notes and a new saved marker. Preparation and discarded
or cancelled previews create no profile files. Acceptance stages the note and
metadata edits on an isolated session, verifies custom profile dependencies against
their captured bytes, and exports any new subdivision profile without overwriting
existing files. It then publishes a single grouped undo gesture. Grouped history
recursively includes note edits during serialization and rejects cross-chart note
edits and structural chart changes in this transaction. Failed staging preserves
the source and redo history.

The chart's existing expressed-profile identity and unknown metadata fields are
preserved. When initially adding editor metadata to SSC, a distinct chart OFFSET
is retained using the original constructor's one-ULP comparison with song OFFSET.
All nine native subdivisions can be saved and regenerated through F5; the two
built-in subdivisions reuse their original identities. Custom performed and
expressed configurations must already be registered. Creating another marker at
the same raw start is rejected; use F6 to edit the existing marker explicitly.

Profile files are shared configuration assets and remain after undoing a chart
edit. Reaccepting the same preview after undo reuses only the exact same profile
bytes. External profile changes reject acceptance. A filesystem failure after a
profile has been committed may leave an unreferenced profile; no destructive
cleanup of shared profiles is attempted. This is not a multi-file filesystem
transaction.

Eight persistent workflow tests cover SM/SSC save and reopen, grouped undo/redo,
other pending charts, offset comparison, all subdivisions and exact regeneration,
custom expression inheritance, dependency changes, stale/foreign previews,
filesystem failures, duplicate markers, and discarded/cancelled workers. Native
F4 workflows additionally save, reopen and invoke F5 using the persisted settings.
These tests compose the previously compared metadata/profile/generation contracts;
they do not execute the original full editor action or prove the complete port.


### Saved marker inspector

F6 or F1 > Edit saved pattern opens a draft for the marker containing the cursor.
The latest start takes precedence when regions overlap, using the same selection
helper as F5. Start and length move by the current chart snap; the signed seed
and both inclusive endpoint flags and distribution flag are editable. Apply
commits the settings as one undoable metadata gesture without moving, removing
or regenerating notes. F5 then generates from the changed marker. Profile GUIDs
and unknown fields are retained. Pattern/performed profile selection uses the
optional selector described below; detailed configuration editing still requires
an advanced inspector. This panel does not expose all GrooveAuthor configuration.

Deleting a marker requires a separate confirmation and keeps its notes. Escape
discards the draft or cancels confirmation. Applying to another chart, another
session or a changed revision is rejected. Moving onto another marker's start
or beyond the original signed row limit fails without clearing redo history.
Zero-length metadata remains valid as in the reference; a subsequent generation
can still reject a region with no usable steps.

Five core workflows cover SM/SSC edit/save/reopen/regenerate, all basic fields,
unknown metadata and profile retention, untouched notes and pending other charts,
one-gesture undo/redo, no-op/discard, stale/foreign/wrong-chart drafts, collisions,
row overflow, overlapping regions and zero-length markers. Two native workflows
cover F6 and menu entry, all basic controls, actor labels, explicit save, F5 after
reload, deletion confirmation/cancellation, undo/redo and absence of marker
actors after deletion. These are adapter/actor tests, not physical input or GPU
proof. Original UIPatternEvent and EditorPatternEvent were inspected for the
separation of property edits and generation; no full original UI-action oracle
is claimed by these tests.

### Clearing pattern notes and navigating saved markers

The F6 inspector offers Clear notes from saved region, Previous pattern and Next
pattern. Clear uses a background task and an immutable full-grid preview with
removed-note/shortened-hold counts. Apply commits a single note gesture, leaving
all marker metadata and timing tags unchanged. Discard and cancellation leave
notes and history untouched. Unsaved inspector changes must be applied or
abandoned before clearing or navigating, so the visible draft cannot silently
select a different region from the saved marker.

`clear_patterns::Task` captures the document identity/revision and selected marker
starts. It sorts and deduplicates starts, resolves only the required pattern
configuration, and processes regions sequentially against the retained grid.
Performed/expressed profiles are unnecessary for this operation. Custom pattern
files load on the worker; invalid unrelated files are reported without being
changed. Exclusive endpoints and marker-relative subdivisions determine the first
and last possible step. Empty regions are skipped, unlike generation requests
which still reject empty step ranges. All note participants are cleared; holds
crossing the first step are shortened by one spacing, with the original one-row
fallback/deletion rule. Overlapping regions operate on prior alterations, avoiding
double deletion. Result application rejects foreign, stale and wrong-chart
previews. The native UI currently clears one saved marker; the core task accepts
multiple explicit marker starts.

Previous/next navigation uses strictly preceding/following starts and wraps to
the last/first marker. It moves the cursor, clears note selection and opens the
selected marker's inspector without adding history or changing song bytes. This
follows the inspected original Editor navigation methods; no original full UI
runtime oracle is claimed.

Twelve new differential fixtures run unchanged ActionDeletePatternNotes deletion
and alteration types plus EditorPatternEvent first/last/count method bodies.
They compare raw inclusive/exclusive/empty/off-grid regions, spacing 1/12/48,
unsorted overlapping markers, consecutive hold trims, taps, holds/rolls, mines,
fakes, lifts and keysounds.
Explicit chart/event adapters replace the original containers and supply spacing;
they do not execute the complete GrooveAuthor loader or UI. The timing harness's
39 existing cases were regenerated with these original bound methods: their
existing results are unchanged (the output now also includes a null Cleared field).
The extraction hash is recorded in provenance. Ordinary Rust tests use committed
fixtures and need no .NET installation.

Six clear/navigation workflows cover these fixtures, both SM/SSC save/reopen,
all participants, preserved metadata, one-gesture undo/redo, missing profiles,
no-op/redo retention, pending drafts, snapshot guards, cancellation before/during
work and wrapped navigation. Two additional native workflows cover preview
actors, explicit apply/discard, worker cancellation, save/reopen, retained marker
actors and navigation with pending edits. These do not prove physical input or
GPU rendering. Batch generation, advanced configuration, waveform/audio editing
and the rest of the delivery matrix remain required for the complete port.


### Choosing saved-marker generation profiles

F6 > Choose generation profiles loads a read-only catalog through the bounded
background worker. Left/Right cycles the pattern and performed profiles; Use
these profiles in the draft stages their GUIDs. Applying marker settings commits
those GUIDs with the other pending marker fields in one undoable gesture. Escape
from the selector returns without changing the existing draft; Escape from the
inspector abandons it. Cancelling catalog loading also retains the prior draft.
No profile files or directories are created by catalog loading or selection.

The catalog includes the original two pattern, seven performed and three
expressed built-ins and successfully loaded custom files. The native marker
selector exposes pattern/performed profiles and the chart-level expressed profile
as described below. Built-in labels are translated, custom display names
have whitespace collapsed and are shortened for the panel without changing their
file contents. Missing referenced identities are shown explicitly and must be
replaced deliberately; opening the selector never substitutes a default. Invalid
unrelated files are reported and preserved. Built-ins precede custom names, with
GUIDs breaking ties deterministically inside each kind's namespace.

Catalog results and staged selections are bound to chart/session identity and
revision. Selected custom files retain their byte snapshots; changed contents
reject marker application without altering metadata or redo history. The check
covers the opened dependency files, not a locked directory or filesystem-wide
transaction. No-op marker application does not rewrite the original metadata.
After staging, unused catalog/config bodies are released while the selected
dependency snapshots remain with the marker draft.

Five core workflows cover all twelve built-ins, custom/unnamed profiles, separate
kind namespaces, invalid files, missing directories, read-only loading, SM/SSC
profile selection/save/reopen/generation, preserved unknown fields and chart
expression, one-gesture undo/redo, identity/revision guards, external custom-file
changes, missing-profile replacement, no-op/redo retention and worker cancellation.
Three native workflows cover asynchronous loading, translated entry strings,
custom name display, explicit staging/application, back/discard/loading cancel,
retained prior drafts, undo/redo/save/reopen, F5 with the selected custom profiles
and explicit missing-profile replacement. They exercise adapters and actors, not
physical input or GPU rendering. They compose the previously compared original
profile codec and generation contracts; no complete original UI oracle is claimed.

Editing/cloning/creating/deleting profiles through the native library, all
advanced configuration controls, the remaining song/chart/pack properties,
batch generation,
waveform/audio editing and the remaining delivery matrix are still required for
the complete port.


### Chart-wide interpretation profile

The saved-marker profile selector now has a third row: Interpretation (whole
chart). This is the chart's ExpressedChartConfig, not another per-marker setting.
Original UIChartProperties and EditorChart inspection confirms that it is shared
by pattern and chart generation. Selecting all three rows stages them atomically;
an invalid or missing expressed identity cannot leave only the pattern/performed
choices staged. Applying marker settings writes the chart expression and marker
fields together in one undoable gesture. Other markers and notes remain unchanged.
The existing unknown fields and split music-offset settings are retained.

The expressed profile participates in custom-file byte guards and draft change
detection. Pending expression-only changes therefore block clear/navigation just
like pending marker edits. Deleting a marker continues to discard pending edits
and preserves the chart's currently committed expression. Missing referenced
expression profiles require an explicit replacement; unsupported identities are
not silently converted to Dynamic. The F6 inspector requires a saved marker; F7 now exposes chart-wide properties
without one, as described below. F3 also has per-request profile overrides,
described in the conversion-profile section below.

Native F3 now resolves the saved chart expression on its worker, using the same
AutogenConfigs root as F4/F5. Its immutable result records the resolved identity
and reports unrelated ignored profile files in the preview. Explicit core chart
tasks still accept caller-supplied expression configurations without this builder.
SourceChart now exposes its editor preparation directly so conversion reads the
snapshot once, resolves the profile, and prepares the retained editor clock from
the same parsed source. The existing source::prepare_editor wrapper delegates to
that method; standalone Fumen preparation is unchanged.

Four expression workflows cover SM/SSC shared scope, unchanged other markers,
notes/other charts, unknown metadata and offsets, undo/redo/save/reopen, custom
expressed profiles reaching F3/F4/F5, explicit-configuration output equality,
missing/stale selections, ignored files and changed custom-expression bytes.
Two additional native workflows exercise the chart-wide row, explicit staging,
undo/redo/save/reopen, both F5 and F3 output against configured core previews, the
resolved identity and ignored-file actor report, and missing-expression repair
with discard. These build on the original metadata/profile/generation fixtures;
they are not an original full UI/runtime oracle or GPU/hardware proof. All other
remaining delivery-matrix requirements stay in scope.


### Chart properties without pattern markers

F7 or F1 > Chart properties opens a chart-wide draft even when there are no saved
patterns or GA metadata. It exposes interpretation selection, whether to use a
chart music offset, the stored chart offset and its effective value. Left/Right
adjusts the numeric offset by 0.0001 seconds; Shift changes the step to 0.001.
Holding the numeric arrows repeats adjustment, while boolean toggles ignore
repeat events. Interpretation selection uses the existing background catalog
through a single-row subpanel. Back/discard/loading cancellation retain the
preceding property draft until it is explicitly applied or abandoned.

`chart_properties::Draft` captures chart/session identity and revision. No-op
application adds no metadata and preserves redo history. Expressed-profile
selection validates kind/identity and captures custom-file byte guards. Music
offset setters reject nonfinite values and use the original one-ULP DoubleEquals
comparison, including signed-zero equality. The effective offset follows the
original GetMusicOffset: chart value when enabled, song value otherwise.

Applying changes retains unknown GA fields and markers and records one atomic
property gesture without loading/copying note grids. For SSC, offset edits write
the effective chart OFFSET alongside GA music-offset fields in the same gesture;
changing interpretation alone does not rewrite existing OFFSET text. The song
offset and other charts remain unchanged. The source version is retained; legacy
SSC timing inheritance/version rules are not upgraded by this inspector. For SM,
per-chart music offsets remain in the original GA editor metadata; gameplay still
uses the song OFFSET. The native help explains the format distinction. This does
not implement live edited-audio preview, waveform or Song Sync.

Six new fixtures compile the unchanged original EditorChart.GetMusicOffset body
with minimal property containers and compare positive/negative/zero song/chart
values and both enable states. The original metadata fixtures were regenerated
without changing their outputs; provenance includes the extracted method. These
fixtures do not execute the original writer or audio runtime. Seven core workflows
cover that comparison, no-marker SM/SSC property selection/save/reopen, no-op and
unknown-field preservation, atomic offset/tag undo/redo, other charts/pending
notes, one-ULP/nonfinite inputs, source identity/revision guards, custom-file
changes and redo retention. Three additional native workflows cover F7 and menu
entry, custom interpretation through save/reopen/F3, numeric/Shift/held-arrow
controls, effective-offset actors, no-op/discard/subpanel-back/loading cancellation,
retained drafts and the SM format distinction. Actor/input-adapter tests do not
prove hardware input, GPU rendering or live audio alignment.

The rest of the chart/song/pack property controls, profile management,
batch generation, audio/waveform tools and the
remaining delivery matrix remain required for the complete port.


### Per-request chart-conversion profiles

F3 > Choose profiles for this conversion offers performed and expressed profiles
from the same background-loaded catalog. The first choice on each row explicitly
means Use the preset from the previous panel or Use the source chart interpretation.
Custom and built-in profiles can override either role independently. Selected
names are reviewed on the main conversion panel before generation. Changing its
preset clears only the performed override, retaining the independent expression
choice. Back and catalog-loading cancellation retain the previous generation
settings; opening the editor again resets these transient choices. Missing saved
request identities stay visibly missing until replaced or explicitly reset to
the appropriate default.

These selections affect only the conversion request. They do not edit source
chart properties, saved marker GUIDs or profile files, and generate no file until
the result is applied and the song is saved. `ProfileOverrides` carries optional
identities into ChartTask. A fresh worker directory load resolves requested
profiles, validates destination weights and reports ignored unrelated files.
The source document is captured when the task is created; profile contents are
resolved when the worker runs, not locked at catalog-selection time. Once
prepared, the note preview remains immutable and uses the normal source identity/
revision guard for application.

Without an expressed override, native conversion follows the source chart's
ExpressedChartConfig as before. An explicit expressed override can convert a
source whose persisted interpretation identity is unresolved, without rewriting
that identity. No default is silently substituted for a missing explicitly chosen
request profile. Default/partial/full overrides use the same original performed
and expressed engines as explicit concrete configurations.

Four core workflows compare full and partial override output with configured core
tasks, cover all-default preset/source behavior, both SM/SSC apply/undo/save/reopen,
same-GUID identities in independent namespaces, unchanged source/profile files,
unresolved-source expression, ignored files, stale/foreign/wrong-chart catalog
results, missing request profiles, cancellation and immutable-source previews.
Three native workflows cover asynchronous selection, visible reviewed names,
resolved worker identities, output/apply/undo/save/reopen, back/loading cancel,
preset reset with retained expression, explicit return to source interpretation,
transient-state reset and missing-profile replacement through the default choice.
They compose the previously compared original configuration and generation
contracts; they do not execute the original full UI/runtime or prove GPU/hardware
input. Native profile creation/editing/library management, batch generation,
remaining metadata/timing controls, audio/waveform tools and the rest of the
delivery matrix remain required for the complete port.

### Chart-set generation engine (native panel pending)

`generation::batch_job::Task` snapshots all charts of a chosen supported source
layout in document order. They share one serialized source, requested seed and
performed configuration; each follows its own saved expressed configuration
unless the request explicitly overrides it. Profile files are loaded once for
the whole task. A change to those files between children cannot mix configurations
within one result. Work runs through the existing cancellable background-job API.

The preview retains a separate success or error for every source. Ordinary child
failures leave successful results available, following the partial-success
contract in GrooveAuthor's `ActionAutoGenerateCharts`. Cancellation rejects the
whole task. Accepting successes appends them atomically as one structural history
action; an all-failed result cannot alter the document or discard redo. Source
identity and revision guards apply to the entire preview. Workspace undo restores
the previously focused chart; redo focuses the first generated result.

Nine functional tests cover SM/SSC output equality against independent conversion
tasks, preservation of pending edits, source difficulty/meter and SSC custom
metadata, serialization/reopening, single-step undo/redo, focus restoration,
foreign/stale previews, cancellation before/between/after processing, partial and
complete failure, failed final validation preserving redo, and one shared profile
snapshot despite removal of its backing file between source charts, plus
independent saved source interpretation profiles with a shared nonzero seed. Conversion
children use the previously compared original generation engine; these tests do
not execute the complete original batch action or UI. The native batch chooser,
per-result review panel and their input-adapter tests are still pending, as are
batch pattern regeneration and the remaining delivery matrix.


### Native chart-set generation (F8)

F8 or F1 > Generate chart set opens a compact optional panel. Source types are
limited to supported layouts actually present in the song, with the focused
chart's type preferred and an existing supported type as fallback. The source
row shows the number of charts to convert. Target layout, preset, seed and
performed/expressed profile choices share the single-conversion implementation;
profile staging, Back and loading cancellation return to the batch panel.
Settings remain transient and source metadata/profile files stay unchanged.

The worker reports processed charts. Left/Right reviews every immutable result,
including failures; Page Up/Down and Home/End inspect the selected note grid.
The summary identifies the source, difficulty, meter, note/mine counts, skipped
events, dropped mines and ignored profiles, or the complete error. Text wraps
and fits within the summary area. Accept adds every successful result, even when
the displayed result failed; discard/cancel changes no song data. An all-failed
preview has only the discard action. Undo removes all additions and restores
the original focused chart. Nothing writes the song until Save.

Five `edition_batch_workflow` tests use the production keyboard adapter and actor
tree. They compare previews with independently requested core batches, cover
source/target/preset/seed controls, result navigation and scrolling, apply/undo/
redo/save/reopen, custom profile routing and output, unchanged source metadata
and profile files, partial/all failure, redo retention, menu entry, loading and
worker cancellation, discard and unsupported sources. All 29 prior native editor
workflows also pass. This verifies input dispatch and actor construction, not GPU
rendering or physical input. Complete original batch-action differential replay,
batch pattern regeneration, configuration management and all remaining domains in
the delivery matrix are still required for the full GrooveAuthor port.

### Multi-pattern scheduling primitives

`pattern_schedule` ports the chronological helper contracts of
`ActionAutoGeneratePatterns`: stable marker order, immediate-next overlap clipping,
expected retained step totals, the last nonempty marker's final row, one global
NPS estimate and incremental preceding-lane counters. An empty immediate next
marker does not clip the current one, even if a later marker is nonempty. The
last marker's step row is used rather than the maximum end of every earlier
marker. The counter retains the original inclusive lower search bound, including
its observable recount of a step on the previously captured row. Timing/marker
events advance the captured row, while fake step heads participate in preceding
lane counts even though density excludes fake notes. Invalid/cancelled updates
leave counters unchanged. Overflow beyond the reference integer range is refused.

The existing single-region task now uses these same scheduling/density/counter
primitives. This preserves its result while providing the contracts required for
a shared multi-region engine. `EditorTimeline::patterns_nps` also distinguishes
an all-empty set of markers from a candidate final step at row zero.

Fifteen committed scheduling fixtures compare Rust with five unchanged C# helper
bodies extracted from the pinned editor: GetTotalStepsFromAllPatterns,
GetLastPatternStepTime, GetNps, RemoveEventsWhichOverlapNextPattern and
UpdatePrecedingLaneCounts. They cover disjoint/adjacent/nested/overlapping regions,
empty immediate/final/all regions, no selection, stable unsorted/repeated and
same-row inputs, exclusive/off-grid/single-row boundaries, spacing one, and
combined delay/tempo/negative-stop/warp timing. Input candidate note rows and the
editor containers are adapters; the expected counters, clipping and density come
from the original methods. Existing deletion and marker-bound helper bodies are
also used. A second Rust workflow covers invalid regions/events, counter rollback,
reference-range overflow and cancellation. Standard tests need neither .NET nor
reference checkouts.

The adapter supplies the marker list directly. The pinned action constructor
currently copies `allPatterns` and then adds it again; that constructor is not
executed by these fixtures. Explicit repetitions are covered independently, so
this evidence must not be described as complete action/runtime parity. Full
multi-region generation still needs incremental timeline insertion with preserved
assigned times, sequential shared context, failure handling and one preview/apply
workflow, followed by native controls and integrated differential checks. It is
not available through the native menu yet. The full delivery matrix remains open.

### Incremental insertion for multi-pattern context

`EditorTimeline::with_taps` inserts generated tap heads using typed rate queries
without rebuilding surviving events. Their assigned times and fake flags remain
unchanged. New fake flags follow GrooveAuthor's `IsEventInFake`: later fake
segments replace earlier ends, warps form a union with the simultaneous stop/
delay exception, and negative-stop regions use the original inverse-rate query
and exclusive end. Fake/warp lookup is indexed once per insertion rather than
rescanning all timing events for every tap. Negative-stop inverse arithmetic
retains the original multiply-by-reciprocal evaluation.

This immutable operation validates participants, lanes, row limits, duplicate
positions and collisions with existing heads or sustains. Invalid/cancelled
requests leave the original timeline unchanged. The new timeline can immediately
prepare independently timed preceding/following context regions for the next
pattern. It does not insert into a live document or claim complete batch
regeneration.

Forty-three differential fixtures compare two consecutive insertions with the
pinned editor's typed time query, unchanged IsEventInFake, deletion and region
extraction methods. The oracle now uses Fumen's actual pinned interval tree for
fake/warp/stop searches, with reference identity equality on adapter events.
Cases extend all 39 existing timing scenarios plus four negative-stop end-rounding
cases at fractional BPMs. They cover surviving and inserted head times, fake
flags, density counts, chart ends, context spans, frame times and editor-event
order. Input clear regions and tap positions are explicit adapters, not a replay
of the complete editor UI/action. The existing 39 timing, 12 clear-pattern and 15
scheduling baselines regenerate without changes. An additional Rust workflow
covers collisions, invalid positions, boundary holds and cancellation during
input collection. The multi-region generation worker, one preview/apply gesture
and native panel still remain to be connected and verified.

### Saved-pattern batch worker

`pattern_batch::Task` now captures selected saved marker rows or all markers in
the active chart. Selection is sorted and deduplicated. One worker loads profiles
once, validates the shared chart/interpretation metadata, clears all selected
step regions, computes one global density estimate, and generates regions in
chronological order. Each region keeps its own seed, subdivision, performed
profile and preceding-distribution flag. The individual pattern task and batch
worker share region interpretation, surrounding-footing derivation, search and
note extraction. Incremental lane/transition context and typed note insertion
are shared across successful results. Empty regions are reported without search;
ordinary per-region failures retain other results and the cleared zones for
explicit preview acceptance. Missing shared interpretation/pattern definitions
fail preparation rather than guess how to clear their zones.

The preview owns the final grid, per-marker output/error, removal/hold-shortening
counts, raw generated tap count, repeated tap count, clipping count, ignored
profile issues and common NPS. Foreign, stale or wrong-chart acceptance is
refused. Successful acceptance applies the entire note difference as one history
action, without rewriting marker metadata or profile files; Save remains explicit.
All-empty/no-change acceptance preserves redo. Cancellation at any stage, including
a ready result, changes no live song data. Workspace methods expose selected/all
requests and one acceptance path. The native panel is still pending.

A subtle original overlap behavior is preserved: an intervening empty marker can
leave repeated identical taps. The private working timeline retains those event
instances for subsequent lane counters and context; movement frames retain one
lane state. The exported grid coalesces identical taps, matching the pinned
Fumen writer's actual note serialization. The general public tap-insertion API
continues to reject collisions. Repetition behavior is compared through multiple
following patterns, not only the final overlapping region.

Seventeen pipeline fixtures compose unchanged original scheduling, deletion,
region/context and timing helpers with the original C# expressed/performed engines
and the original Fumen note writer. They compare per-region generated rows/lanes,
clipping, common density, repetition counts and the complete exported grid across
33 representable SM/SSC cases. The warp case uses SSC; SM's reader ignores WARPS,
so a separate SM-representable delay/negative-stop/tempo case is included. Cases
include neighboring/nested/overlapping markers, empty and exclusive ranges,
off-grid and one-row markers, spacing one, evolving contexts after repeated taps,
ignored preceding distribution and shared transitions. Seven Rust workflows cover
that differential pipeline plus actual atomic save/reopen, one undo/redo, metadata
and other-chart preservation, selected-chart guards, stale/foreign snapshots,
missing markers/configurations, partial/all failures, cancellation, no-op redo
retention and a profile file disappearing after the first region.

The oracle supplies the unique selected marker list directly and orchestrates
original components synchronously. It does not replay the full editor action
constructor (which currently duplicates its input list), dispatcher or UI. Broad
multi-layout batch differential coverage, native batch-pattern controls, full
configuration management, timing/audio editing and the remaining delivery matrix
remain required for the complete port.


### Native saved-pattern sets (F9)

F9 opens an optional seven-row selector. All saved markers are initially selected;
Left/Right on the marker row cycles them and the inclusion row toggles the current
one. Actions select all, clear the selection, or select markers whose start lies
inside the workspace's marked area. The count is reviewed before starting; an
empty selection and missing marked area produce localizable messages. Reopening
the selector starts with all markers selected. Only the active chart is affected.

F1 > Regenerate saved patterns now opens a small submenu for the existing cursor
pattern command (F5) or the set command (F9). The main menu retains fourteen rows.
Single-pattern keyboard behavior remains available directly through F5.

The batch worker remains cancellable. Its preview shows the complete resulting
grid, selected marker highlight and per-marker success/error/empty status.
Left/Right wraps results and seeks the reviewed marker; ordinary preview scrolling
is available. The summary shows removals, hold shortening, common NPS, generated
and repeated taps, clipped steps, ignored profile issues and end-footing warnings.
At widths of at least 800 logical pixels, a right panel leaves the note field
fully visible on the left; narrower views use the compact bottom preview.
Panel and note sizes are explicit actor dimensions so geometry checks do not
require a populated texture-dimension cache. The help overlay stays hidden while
reviewing a generation preview and returns afterwards if previously enabled.
Errors are displayed in full with wrapping/size bounds. If any marker failed,
the summary explicitly explains that applying also clears those selected regions.
This is the original action's clear-then-generate result, held read-only until
acceptance. All-failed sets with removals therefore still offer Apply. Empty/
unchanged sets report no changes and retain redo. Apply is one history action;
Save remains explicit. Selection, discard, calculation cancellation and opening
panels never write the song or profile files.

Five English native workflows exercise the production input adapter and actor
tree: full preview equality with core results, marker navigation, actual save/
reopen and one undo/redo, all/none/individual/area scopes, held-toggle filtering,
partial/all failures and visible clear warning, wide/narrow preview geometry,
no-op redo, worker cancellation,
discard, submenu routing and F5 preservation, missing markers and custom profiles.
A separate French test process verifies Oui/Non, selection errors, resolved
preview labels and acceptance without racing English locale state. All 34 prior
native workflows also pass. These checks prove dispatch and constructed actors,
not GPU rendering, physical input or full original-editor runtime parity. The
broader delivery matrix, including timing/audio tools, profile management,
additional layouts and scalability validation, remains required.

### Scoped BPM editing core and native inspector

`timing_edit::TempoDraft` stages song-shared or chart-local BPM maps against a
session identity/revision. It exposes the charts affected by a shared edit and
supports adding/updating, deleting and moving nodes. The initial row-zero BPM
cannot be removed or moved; collisions with another tempo node are refused,
matching the original miscellaneous-event movement guard. Finite values follow
the original 0.000001 BPM minimum and one-ULP setter equality. Values must also
fit DeadSync's RSSP gameplay timing representation. No invalid/discarded/stale/
foreign/wrong-chart draft can change the file or clear redo. Acceptance is one
property history action, and Save remains explicit. Workspace methods expose
capture and acceptance for the native timing panel.

The tempo loader mirrors Fumen's AddTempos row conflict resolution and invalid
value handling, plus GrooveAuthor's minimum clamp and missing initial-tempo
fallback. Later beats win when they quantize to one row, duplicate exact beats
keep the final value, and the original subdivision rounding is preserved.
Missing row-zero tempo uses earlier charts' starting-tempo histogram, enumerated
by first-seen chart-type groups as in EditorSong (including
last-key tie handling), then the song's explicit zero-beat tempo/display BPM,
then 120 BPM. A no-op draft keeps the original source bytes rather than rewriting
its normalized representation. A real BPM edit serializes the normalized map.
This is not a claim that all legacy loader normalization is finished.

The first local SSC timing tag switches the entire chart to its own timing set.
`inherited_fields_for_write` therefore copies the complete inherited timing set,
including raw labels/comments/legacy bytes and OFFSET, before replacing the
selected fields. An already-independent chart keeps its own timing without
pulling unrelated song fields into it. Song edits affect inheriting charts only.
SM has one shared timing set and rejects chart-local scope. SSC below 0.7 or with
an absent/unrecognized split-timing version currently rejects chart-local edits;
shared song edits remain available. Atomic legacy migration is still required.

This revealed and fixed the same detachment issue in F7: adding an audio OFFSET
to an inheriting chart now retains its BPMs, stops, delays, warps and other timing
rather than silently activating an incomplete local timing set. The shared raw
property transaction preserves copied values byte-for-byte and keeps all changes
under one undo. Structural and duplicate property guards still run before live
mutation. Expression-only chart-property edits do not detach timing.

Fifteen committed tempo fixtures exercise the pinned Fumen importer, unchanged
GrooveAuthor tempo setter/minimum and unchanged CanEventExistAtRow/
CanEventBeMovedToRow guards through explicit adapters. They compare loaded maps
and every resulting node value, including half-row/negative/duplicate positions,
invalid tempos, minimum clamps, fractional values, integer row bounds and ULP
behavior. Five additional file fixtures run the actual pinned SM/SSC reader
before and after explicit expected exports, comparing all charts' BPM maps,
per-note times and audio offsets. They cover inherited/local/shared BPM edits and
the F7 offset regression. Ten Rust workflows add actual atomic save/reopen,
one undo/redo, note/other-chart preservation, all inherited raw timing fields,
no-op redo, rejected edits, missing/legacy/offset-only timing, fallback behavior
and shared property-transaction validation. Ordinary tests need no .NET runtime.

The adapters do not replay the complete original editor action/notification loop.
Other typed timing/gimmick editors, legacy migration,
complete timing normalization, edited audio preview/cache invalidation and the
remaining delivery matrix are still required for the full port.


### Native BPM inspector (F10)

F10 opens the optional timing panel; F7 chart properties also has an entry for
it. Pending chart properties must be accepted or discarded first. A supported
SSC file starts with chart-local timing; SM and older SSC versions start with
shared song timing. The scope label reports the number of affected partitions
and whether a local timing set is inherited. Only the format/version determines
scope availability: malformed local BPM data produces an error instead of
silently selecting song-wide edits.

Up/Down selects the scope, beat, BPM value or an action. Numeric keys (including
the numeric keypad), period/comma, Backspace and Delete edit the numeric fields;
the first digit replaces the existing value. Left/Right moves a beat by the
current snap (Shift: four beats), or adjusts BPM by 1 (Shift: 0.1). Beat input
rounds to the nearest 1/48 beat. Enter on either numeric field or the stage
command stages a BPM. Previous/Next seeks and selects existing nodes, wrapping
at the ends. Unstaged input blocks navigation to prevent accidental loss.

Move uses the selected source node and the entered destination beat; value
changes must be staged at the original beat first. Initial-node moves/deletion
and occupied destinations are refused. Delete removes the node at the entered
beat. These operations only modify the private draft. Scope switching is blocked
while that draft or its input has pending changes. Apply stages any remaining
numeric input, accepts all BPM edits as one undoable gesture, and closes the
panel. Escape or Discard abandons the draft without clearing redo. No-op Apply
also preserves redo and original bytes. Saving to disk stays explicit.

Nine English native workflows and a separate French workflow exercise production
RawKeyboardEvent dispatch and the generated actor tree: F10/F7 access, numeric
controls and rounding, scope isolation, malformed local data, initial/collision
validation, navigation, move/delete acceptance, pending input, help visibility,
undo/redo, unchanged files before Save, and save/reopen. Expected timing edits
are compared with the shared core already checked against the original C#
references. These tests do not claim GPU rendering, physical keyboard input,
audio playback or complete GrooveAuthor runtime parity.
