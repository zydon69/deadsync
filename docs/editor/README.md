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
| Metadata | Existing and new scoped tags with escaping/history; F7 chart properties; F11 song text/music, SSC preview file, sample region, end hint, global music offset and editor sync | Property and chart-property save/reopen/undo tests; remaining song/chart/pack inspectors pending |
| Timing/gimmicks | F10 scoped BPM, stop, delay, warp, fake, signature, tick, combo, scroll, speed and label editing; legacy SSC activation; shared revision/inheritance transactions; measure grid and label annotations | Original C# parser, setters, clocks and interpolation comparisons; complete visual scroll/speed simulation, broader legacy normalization and conversion remain pending |
| Playback and audio | Enter starts/stops native audio preview; F11/F6 edits/auditions samples; W loads and toggles a channel-separated waveform using the playback clock | Core clock, decoded WAV/MP3, native actor geometry/offset tests and basic OpenGL runtime review; other rendering backends, audible playback, decoder lifecycle/EOF, advanced waveform preferences, scrub, loop and audio analysis remain pending |
| Multi-chart | Switch, duplicate, create and delete charts; structural undo/redo; usable no-active-chart workspace | Lifecycle workflows cover final-chart deletion, recovery, metadata reindexing and persistence; original live chart ordering, complete properties, multi-view comparison and library remain pending |
| Generation | Original graphs, typed configurations, seeded .NET random, performed-path scoring and pattern search with start/end footing and lookahead | Differential pattern fixtures across all 11 original graph layouts, timing changes, context and seeds; preview/apply/undo/save and cancellation tests. Expressed-chart lane-state preparation and graph candidate matching are compared to the original interpreter across all 11 layouts, including holds, rolls, fake/lift instances and jumps. Interpretation costs, automatic bracket policy selection and mine expression match original chart interpretation; replacement expansion and ranking match the original cache; performed-chart conversion and destination mines compared across Dance/Pump/SMX; automatic fallback selection, file assembly and asynchronous chart-conversion previews are connected; single-region native pattern previews are connected through F4; F8 converts all charts of a selected source type with per-result review and atomic acceptance; F9 regenerates selected/all saved patterns with one preview and history action; full configuration editing and broader runtime parity remain pending |
| Analysis | F12/F1 snapshot RSSP inspector: counts, NPS, tech, stream breakdown, bounded density graph and interval navigation | RSSP differential/workflow and English/French native tests; original density algorithm/settings equivalence, complete tech/options UI, minimap, pointer interaction and physical rendering remain pending |
| Customization | Initial native shortcuts | Rebinding, presets, colors, scroll/receptor/waveform preferences and command palette pending |
| Export and scale | Original SM/SSC format retained | Cross-format export checks, advanced save settings, long-file benchmarks and large-generation cancellation pending |

### RSSP reuse

The user's explicit instruction is to reuse the RSSP library already included in
DeadSync wherever it fits. The editor depends on the same Git dependency and
locked revision (`da616e3a`); no second revision is introduced. Metadata decoding
uses its CP1252/UTF8 and escape handling. Snapshot analysis uses its parser,
statistics, timing, NPS, streams, hashes and tech analysis. `rssp_workflows`
checks that unsaved edits affect analysis and that the saved/reopened output
has the same stats and hash. The native snapshot analysis panel is described below.

RSSP's normalized summary writer is available for future reviewed exports.
Source-preserving editor saves retain unknown fields that are absent from an
analysis summary; they must not silently replace the original with a normalized
summary. RSSP layout/analysis limits remain explicit; the editor's participant
model and GrooveAuthor generation are not limited to RSSP's supported layouts.

## Commands to reproduce current checks

```sh
cargo test -p deadsync-editor --locked
cargo test -p deadsync-theme-simply-love --test edition_workflow --test edition_batch_workflow --test edition_pattern_set_workflow --test edition_pattern_set_fr_workflow --test edition_timing_workflow --test edition_timing_fr_workflow --test edition_legacy_timing_workflow --locked
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
SM has one shared timing set and rejects chart-local scope. SSC below 0.7 or with an absent/unrecognized version requires the reviewed
conversion described below before local editing. When legacy local tags exist,
timing edits and generation wait for this conversion so gameplay can honor them.

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
Other typed timing/gimmick editors, cross-format migration,
complete timing normalization, edited audio preview/cache invalidation and the
remaining delivery matrix are still required for the full port.


### Native timing inspector (F10)

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

Seventeen English native workflows and three separate French workflows exercise production
RawKeyboardEvent dispatch and the generated actor tree: F10/F7 access, numeric
controls and rounding, scope isolation, malformed local data, initial/collision
validation, navigation, move/delete acceptance, pending input, help visibility,
undo/redo, unchanged files before Save, and save/reopen. Expected timing edits
are compared with the shared core already checked against the original C#
references. These tests do not claim GPU rendering, physical keyboard input,
audio playback or complete GrooveAuthor runtime parity.


### Scoped stops and delays

`timing_edit::pause::{Draft, Pauses, Kind}` and the workspace pause APIs stage
both STOPS and DELAYS together. Stop lengths can be positive or negative, but
cannot be zero. Delays may be zero but cannot be negative. The importer uses the
same Fumen beat-to-row rounding and conflict resolution as BPM import; it removes
zero-equivalent stops and negative delays only after resolving same-row source
conflicts. Node value changes retain the original one-ULP equality. Each type has
its own position map: a stop and delay may coexist at one row. Both allow moving
or deleting a row-zero node, unlike the immutable initial BPM. A destination
already occupied by the same type is rejected.

The scoped transaction preserves the complete raw inherited timing set before
the first local SSC edit. Only the changed pause type is serialized; an untouched
delay map, labels with legacy bytes, BPMs, OFFSET and other properties retain
their original bytes. Removing every local pause writes an explicitly empty tag
so reloading cannot unexpectedly restore the song's pauses. Both event types
apply in one history action. Invalid/stale/foreign/wrong-chart drafts and no-op
acceptance preserve document bytes and redo. SM uses shared timing; legacy SSC local timing requires conversion, and modern
SSC can edit either scope. The shared core and native F10 inspector both
expose these operations.

Twenty-two new oracle fixtures use actual Fumen AddStops/AddDelays importers,
unchanged GrooveAuthor stop/delay setters and original movement guards through
explicit node/container adapters. They cover positive/negative/zero lengths,
duplicate beats, row collisions, invalid later events masking earlier valid
ones, half rows, negative source positions, fractional values, integer limits,
subnormal delays, signed zero and ULP equality. The generator preserves the
original C# JSON negative-zero token when committing expected data. The update
adapters assign the referenced length field; they do not replay the original
interval-tree notifications or editor action loop.

Five additional fixtures run the real original SM/SSC reader before and after
explicit expected exports. Eight Rust workflows compare imported maps, each
edit stage, actual note times, inheritance, sign changes, coexisting stop/delay
nodes, empty maps, save/reopen, pending notes, undo/redo and rejected operations.
Lengths outside finite f32 gameplay bounds are refused; DeadSync's existing
RSSP timing quantization remains unchanged. Full timing normalization,
audio/cache updates, other timing/gimmicks and complete runtime
parity are still required for the requested port.


F10 has an event-type heading. Tab cycles BPM → stop → delay → warp → fake, and
Shift+Tab cycles in reverse. This adds no top-level menu rows. Stops and delays retain a
single draft when switching between them, allowing one combined acceptance and
undo. Unstaged input must be staged first. Switching between BPM and pause
drafts requires applying or discarding changes; the panel refuses a switch that
would lose work. Scope switches follow the same rule. Duration keys accept a
minus sign (also numeric-keypad subtraction); negative delays and zero stops
are rejected. Left/Right adjusts durations by 0.01 seconds (Shift: 0.001).
Pause navigation uses only events of the selected type, and row-zero pauses can
be moved/deleted. Applying reports the combined number of stop/delay nodes.

The four added English native scenarios and one added French scenario cover
combined stop/delay acceptance and one undo, save/reopen, type switches with
pending input and staged edits, sign/zero validation, scope isolation, node
movement/deletion and translated headings/controls/errors. Earlier BPM native
workflows remain in the regression set. Actor and keyboard tests do not prove
physical input, final GPU layout or audio preview behavior.


### Typed warp and fake region editing

`timing_edit::region::{Regions, Draft, Kind}` and the workspace region APIs edit
WARPS and FAKES as independent maps of start row to integer row length. The
original Fumen importer rounds both starts and lengths, resolves same-row
conflicts before filtering invalid lengths, and discards nonpositive lengths.
GrooveAuthor's constructors clamp a positive length that rounded to zero to one
row. These rules are preserved. Set/update uses exact integer equality and a
one-row minimum. Moving/deleting an initial row-zero region is allowed. Same-type
start collisions are refused; nested and overlapping intervals and different
types at one start remain allowed.

The Rust operations additionally reject endpoints outside the supported signed
32-bit row range rather than reproducing unchecked C# GetEndRow overflow. Those
bounds are validated before mutation. SM cannot directly represent these tags:
region capture rejects SM export until explicit migration/conversion is
implemented. Legacy SSC without local timing can still use shared region edits in the core;
local timing requires conversion before editing. These limitations do not claim full export
parity with the original application.

BPM, pause and region drafts now share one private timing target implementation.
It captures scope, affected charts, session identity/revision and the complete
inherited raw timing set. It validates acceptance even for a no-op, filters only
the tags being replaced, and records a single property-history gesture. The
public typed BPM/pause contracts remain unchanged. This prevents new timing
editors from independently implementing timing ownership or dropping inherited
properties. F7's offset path still uses the same raw-copy helper.

A region draft may change warps and fakes together, preserving other tags and
unchanged region maps byte-for-byte (including comments, legacy label encoding,
OFFSET and pending note edits). Empty local sets remain explicitly empty after
save/reopen. Discard, no-op acceptance and rejected stale/foreign/wrong-chart
requests leave source bytes and redo intact. Eight new Rust workflows cover
these operations, overflow guards, inheritance, shared/local ownership, pending
notes, save/reopen and both history gestures.

Twenty-four new fixtures use actual Fumen AddWarps/AddFakeSegmentEvents,
unchanged GrooveAuthor region minima/setters and the original miscellaneous-node
movement guard inherited through the pause oracle's explicit containers. The
constructor's minimum clamp is supplied by an explicit adapter; notification
callbacks are stubs. This is not an execution of the original full UI/action or
interval-tree runtime.

Four additional fixtures read complete SSC files with the actual original
reader before and after explicit expected edits. Reader timing events and notes
then feed the unchanged original editor timing/dependency methods through the
existing oracle containers. The Rust workflows compare every chart's region
maps, file-clock note times, editor-clock times, per-note fake flags, eligible
step counts, end time and position queries before edits, after acceptance and
after reopening serialized bytes. Cases include nested fake regions that shorten
a previous region's effective coverage, overlapping warps, coincident stops or
delays, shared/owned timing and removal of every local region. No Rust output is
used to manufacture these C# expected results.

Region drawing/selection, SM conversion and broader legacy normalization, the remaining timing/gimmick types,
audio/cache integration and the full delivery matrix remain work for the port.


The region file-clock integration cases above use integral region lengths. The
fractional raw-source gap identified at that checkpoint is now addressed by the
normalized source timing path described below. This does not establish complete
legacy-format or whole-editor runtime equivalence.


### Native warp and fake region inspector

The F10 panel now includes warp and fake region types in its Tab/Shift+Tab cycle.
There are still ten action rows and no extra top-level menu rows. Lengths are
shown/entered in beats and rounded to integer 1/48-beat rows. Values that round
to zero or put the region end beyond the supported row range are refused. A
successful stage displays the canonical stored length. Left/Right changes length
by the current snap; Shift changes it by one row. This does not change the
reference importer rules for raw source values.

The panel preserves one combined warp/fake draft when switching between those
two types. Switching to BPM or pauses with unaccepted region edits is blocked;
the paired stop/delay draft follows the same rule. Scope switches and navigation
with unstaged input remain guarded. Navigation searches the selected type only,
wraps at both ends, and seeks the chart cursor. Initial regions can move or be
deleted. Move validates the entire destination interval and compares lengths in
integer rows, so equivalent beat input does not spuriously count as a different
length. Overlaps and simultaneous different types remain allowed. Apply is one
history gesture for both maps; no-op or discarded changes preserve redo.

A small presentation adapter dispatches to the typed BPM/pause/region drafts.
Region lengths remain u64 rows in the core, rather than being stored as floating
point seconds or beats. Navigation uses ordered-map lookups without allocating
an event list for each keypress. The interface handles unsupported SM region
editing with a localized error and retains the current panel/draft.

Four additional English native workflows exercise production keyboard dispatch,
source-preserving export, paired edits, validation/cancellation/redo, navigation,
move/delete, shared timing, fine length controls, SM refusal and save/reopen.
One scenario reproduces the edits of the actual original C# file/editor fixture
and compares native-produced output's head times, fake flags and eligible step
count with those original results. A new French workflow checks the translated
headings, duration units, invalid-length message and paired acceptance. These
join the existing editor regression workflows; physical input, GPU layout,
region interval drawing, audio preview and complete source normalization remain
separate requirements.


### Normalized source timing and separate file/editor event sets

`generation::timing::load` now constructs normalized source events for both the
standalone Fumen generation clock and the editor dependency clock. It uses the
same subdivision-to-row conversion as the typed timing importers. Valid records
are ordered by their exact source beat; later beats and final duplicate-beat
values win rounded-row conflicts before semantic filtering. Negative positions
are discarded after rounding. Malformed individual values are skipped as in the
original property parsers. Nonfinite positions/values and unsupported row/end
ranges produce errors instead of propagating unsafe clocks.

The loader handles first-invalid-tempo replacement, invalid later tempos,
zero-equivalent stops, negative delays, nonpositive warp/fake lengths, signature
validation/default insertion and same-row metadata conflicts. It respects the
SM reader's supported property set and FREEZES alias. Duplicate timing property
tags are rejected with an error: the actual original reader throws an
ArgumentException for the corresponding case. No source tags or notes are
rewritten by import.

The file event set retains Fumen's imported values, including a positive warp or
fake length that rounded to zero rows, subminimum tempos and missing initial
BPMs. A separate editor event set applies the original constructors' one-row
warp/fake minimum, tempo minimum and missing initial BPM fallback. Fallback uses
previous charts grouped by first-seen chart type, original histogram tie order,
then the song's exact zero-beat BPM, valid explicit display range and 120 BPM.
`EditorTimeline::build` consumes this editor set; standalone generation-clock
queries retain the file set. Explicit `GenerationTiming::new` events retain the
existing single-clock contract. Source preparation reuses its already parsed
Document instead of copying/parsing a second file snapshot for normalization.

Thirteen additional committed source fixtures use the real pinned SM/SSC reader,
unchanged original editor tempo-fallback and display parser, constructor clamp
conditions and editor timing/dependency bodies. The oracle runs with the same
invariant culture installed by GrooveAuthor's Editor constructor; it does not
inherit macOS French decimal parsing for editor helper calls. Twelve cases
produce reader/editor results; the repeated-tag case records the original
reader's failure. The adapters supply chart containers, typed nodes and fields;
this still does not execute the full original editor application.

Four new Rust workflows compare note times, first-event group times, editor
head times/fake flags, eligible step counts, end times, position queries and NPS,
plus missing-tempo histograms, source-preserving save/reopen and rejected
nonfinite inputs. Cases include half rows, tiny regions, subminimum/missing/
future tempos, invalid/masking/malformed records, metadata conflicts, mixed chart
types and SM-only semantics. Normalization does not mutate document bytes,
CP1252 metadata, pending note history or redo.

This removes the earlier fractional generation-clock/editor-constructor mismatch
for the compared modern source paths. Broader legacy/malformed/escape/format corpora, typed UI handling of all remaining
metadata, interval rendering, audio integration and scale/cancellation checks
remain requirements for the full port. Gameplay/analysis continue to use the
existing pinned RSSP implementation; its timing quantization is not replaced by
these editor-generation clocks.


Two of the source-normalization fixtures also run the actual original expressed
and performed chart pipeline after the editor dependency pass, using the pinned
configuration, seed 42 and dance-double target. A functional Rust workflow runs
ChartTask against the original fractional SSC bytes and compares every emitted
note with those C# results, then applies one new-chart gesture, saves, reopens
and undoes it. This covers the production generation path, in addition to the
clock and fake-flag comparisons above.


The normalized reader now follows the original Fumen ownership rule for legacy
SSC too. Working generation and timing edits require the reviewed conversion
below when old versions would cause gameplay to ignore those local fields.
Some typed inspectors also
still reject malformed raw maps that the generation reader can skip; the shared
record normalization must extend to those inspectors before full import/editor
parity is claimed. The display-tempo range fallback is already shared by the BPM
inspector and normalized loader, including rejection of partially valid ranges.
The SM FREEZES alias may occur multiple times in the original reader and is
merged in source order; this differs from duplicate ordinary timing tags, whose
raw-extra dictionaries cause the original reader to throw. Both behaviors are
included in the new source corpus.


### Reviewed legacy SSC timing activation

`timing_edit::migration::Draft` captures a file-wide immutable conversion plan.
The reader interprets local SSC timing like the original Fumen reader, independent
of VERSION. The conversion activates that timing for DeadSync/RSSP as well by
writing a supported version and materializing required editor BPM maps. Existing
local chart timing stays independent; missing/default/subminimum tempos use the
already validated editor constructor/fallback path. A formerly inherited chart
that needs its own default tempo copies the complete inherited raw timing set
before its first local BPM. Other tags, legacy encoding and notes stay intact.

Known old numeric versions upgrade to 0.70, the minimum split-timing version.
This deliberately stays below the separate 0.74 description/chart-name boundary.
Missing/unrecognized versions upgrade to 0.83, preserving their existing modern
name interpretation. Already supported versions are unchanged and never
downgraded. No-op, discarded, stale and foreign plans preserve source and redo.
The version and all planned chart properties apply in a single history gesture,
including when note edits are already pending. Disk writes still require Save.

F10 and F7 open a translated review panel for an old SSC: source/target version,
number of pre-existing local timing charts and number of BPM maps to write.
Enter accepts; Escape/Keep Original discards the plan. Generation/regeneration/
clear tools route legacy local timing to this review before starting a job.
The shared core also refuses working generation/timing edits that would write
ignored old local timing; offset export has the same guard. Read-only normalized
clock queries remain available for comparison before conversion. Conversion can
be undone independently of later BPM/property edits or new generated charts.

Three new actual-reader/original-editor fixtures cover mixed inherited/local/
offset-only charts, missing version with inherited tempo fallback, and fractional
or subminimum chart timing. Seven functional core workflows compare editor
head times/fake flags/step counts/end/query times before and after conversion,
save/reopen, the real RSSP ownership change, metadata name boundaries, pending
notes, CP1252/unknown tags, no-op history, invalid/stale/foreign plans and blocked
ignored offset/timing writes. The mixed legacy fixture also runs the original
chart-generation pipeline for all four charts; the Rust task outputs match its
notes after conversion, then apply, save, reopen and undo all generated charts
and the conversion. These tests do not imply equivalence for all historical
format dialects or unsafe malformed values.

Four native English workflows plus a French workflow cover review/accept/cancel,
undo/redo, Save/reopen, missing-version defaults, translated labels and routing
from generation and properties. Existing native editor workflows remain in the
regression set. SM conversion, broader malformed/escape compatibility, interval
rendering, remaining timing/property tools, audio preview/waveform integration
and scale/runtime verification remain required for the full editor port.

### Playback position clock

`EditorTimeline::time_position` ports the original rate event's inverse clock:
chart seconds to a fractional row. It keeps negative lead-in positions, freezes
through positive stops, advances through warps/negative stops, and treats an
exact event time as preceding every rate event with that time. A stable time
index makes playback queries logarithmic without changing row-based queries.
Projected and inserted-note timelines retain this index. Nonfinite inputs,
nonfinite results and clocks with no rate events return errors.

The 39 editor timing oracle cases now include playback positions at negative
and positive times and immediately before/at/after each timing rate event. The
oracle calls the unchanged pinned `GetChartPositionFromTime` method, using its
collection adapter to select the last strictly preceding rate (first rate as
fallback). Rust compares the resulting fractional rows exactly. This proves
these clock cases, not audio output or the native transport: music offset,
asset selection, play/pause/seek and snapshot-driven cursor wiring remain to
be integrated before audio preview is usable.

### Native audio preview

Enter in the central editor now starts playback at the cursor; Enter again
stops and retains the last audio-driven row. Any editing/navigation/menu key
stops first. Space retains its selection behavior. The English/French help and
status messages describe these controls. Playback never writes the simfile.

`playback::Preview` captures current document timing and effective chart/song
music offset, resolving a nonempty chart MUSIC before song MUSIC relative to
the simfile. It does not reuse the Practice chart's previously loaded clock.
Its empty note grid deliberately allows empty charts and multiple participants
without requiring single-player generation support. File existence is checked;
decoding is delegated to DeadSync's existing audio engine. Negative starts use
that engine's silent lead-in support. Editor sync offset, loop/scrub controls,
waveform rendering and complete decoder failure/EOF reporting remain pending.

The editor queues the actual `GameplayAudioCommand::PlayMusic`/`StopMusic`
commands. Practice forwards them to its gameplay command queue both after raw
input and during update; the existing shell drain executes them with the real
audio control. Close also stops the engine. The cursor uses mapped audio music
nanoseconds and the captured inverse clock, rejecting snapshots older than the
request. Unmapped snapshots do not simulate progress; losing mapping after
playback has mapped stops the preview. The current stream snapshot has no track
ID or decoder result: its seeded mapping is not proof of audible output, and
complete decoder lifecycle/error integration remains required. Cursor rendering
currently rounds down to a note row while the clock supports fractional rows.

Five core workflows cover SM and multiplayer preview, unsaved timing/offset capture without mutations,
independent snapshots, chart audio/offset overrides and song fallback, negative
lead-in, stops/warps and explicit missing-file/invalid-input failures. Four native
workflows cover commands and their cut positions, mapped cursor updates,
stop/resume, stale/unmapped snapshots, editing/close cleanup, missing audio and
preserved selection controls. A separate French workflow checks visible controls,
playing/stopped/error statuses. These use temporary fixture assets and synthetic
snapshots; they do not prove device output, audible alignment or physical input.

### Song preview synchronization (F11)

`GASongDataVersion:1` / `GASongData` now supplies the editor-only `SyncOffset`.
The fallback for a file without song metadata is GrooveAuthor's default open-song
preference, 0.009 seconds; a V1 object with no SyncOffset uses the original C#
field default, zero. Configurable new/open-song preferences remain pending.
The music-file clock now follows the original order of operations exactly:
`music = chart - effective_music_offset + sync_offset`, and
`chart = music + effective_music_offset - sync_offset`. This replaces the initial
preview implementation's implicit zero editor sync. Preview snapshots retain
both offsets independently when later document edits occur.

F11 opens a compact song sync panel. Left/Right changes by 1 ms, Shift by 0.1 ms,
0 sets zero and Home restores 9 ms. Apply records one undo gesture; Cancel/Escape
and unchanged Apply retain file bytes/history. Save remains explicit. Opening
the panel stops playback first. Root/individual chart OFFSET and all timing maps
are untouched; the whole V1 JSON object is retained so default chart selection
and unknown nested fields survive edits, including in SM. Malformed, incomplete,
nonfinite or future-version metadata returns an error rather than being replaced.
Other song fields and default chart selection are retained but not editable yet.

Five C# oracle fixtures extract the original song data schema and the unchanged
EditorPosition conversion methods at the pinned editor commit. Four core
workflows compare both directions exactly, plus default/no-op/foreign/stale
handling, finite validation, comments, unknown fields, save/reparse and undo/redo
in SM and SSC. Two native workflows exercise F11 cancellation, unchanged Apply,
playback cleanup, the changed actual audio cut, undo/redo and Save/reopen. A French
actor workflow verifies visible value, help and action/status labels. Existing
preview fixtures now declare an explicit zero song sync where zero is intended.
Device output, full decoder lifecycle, waveform/loop/scrub and the other editor
features listed in the delivery matrix remain required.

Validation for this checkpoint: 243 core tests, 73 native editor tests and
`cargo check -p deadsync --locked` pass. Enter on the numeric keypad is also
accepted for playback and covered by the F11 audio-cut workflow.

### Typed time signatures and measure grid

F10 now includes Time Signatures after Fake regions in the Tab cycle (Shift+Tab
from BPM also opens signatures). It shares beat/scope, stage, previous/next,
move/remove and explicit Apply/Save controls with the other timing tools.
The value is an unreduced fraction; 2/2 remains distinct from 4/4. Slash and
numeric-keypad division accept fraction input. Left/Right adjusts the numerator,
Shift adjusts the denominator through 1/2/4/8/16/32. Initial signatures can change
value but cannot move or disappear. Pending text and staged edits keep the same
cross-type/scope guards. An old source denominator can move unchanged even when
it is outside the interactive editor's validator.

The shared typed draft uses the existing ownership transaction: a first local
SSC signature copies the complete inherited timing set before detaching it.
SM supports shared time signatures directly. No-op/discard retain exact bytes
and redo; accept is one undo even with note edits already pending. Legacy SSC
still goes through the existing timing-conversion review.

The reader and interactive validator deliberately differ, as in Fumen and
GrooveAuthor. File signatures require positive integers and a denominator that
divides 192; invalid records are skipped after resolving same-row conflicts.
New input requires positive signed-int values and a power-of-two denominator
no greater than 48 (thus at most 32). Source imports inject 4/4 at row zero when
missing. Unsafe signed-int measure multiplication is rejected explicitly instead
of reproducing the reference's overflow; original document bytes stay intact.

Measure numbering rounds upward when a signature begins partway through a
measure. Position queries use a cached index; visible boundaries are a lazy
iterator, including for source maps with one row per measure. The central field
now renders these actual boundaries and measure numbers instead of assuming
four beats per measure. Its cache refreshes on document/chart/revision changes.
If unsafe source arithmetic prevents a map from loading, the field retains beat
lines and omits measure labels; opening the typed inspector reports the error.
Zoom-aware label density, signature-dependent arrow coloring and other display
preferences remain part of the unfinished editor scope.

Five oracle cases use actual Fumen loading, the unchanged GrooveAuthor signature
validator/setter and the existing movement adapter. They compare every stage's
unreduced values, measures and row-relative positions: default/multiple meters,
partial measures, legacy denominators, same-row conflicts, invalid input,
immutable initial nodes and collisions. Five core workflows also cover inherited
SSC timing, SM shared writes, pending notes, save/reopen, undo/redo, foreign/stale
plans, large coordinates, checked overflow and lazy visible boundaries. Four
English native workflows cover editing, cancellation/no-op/invalid/immutable
input, type guards, SM moves/deletes, legacy denominator moves and rendered
measure-label positions through undo/save/reopen. One French workflow checks
translated headings/actions/status and fraction entry. GPU/device verification
and the other features in the delivery matrix remain outstanding.

Validation for this checkpoint: 248 core tests, 78 native editor tests and
`cargo check -p deadsync --locked` pass. These include a final rerun of signature
controls, French UI and signed-zero source conflict handling.

### Instant scroll events in F10

F10's Tab order is now BPM, Stop, Delay, Warp, Fake, Scroll, Signature. The shared
inspector stages scroll multipliers, navigates/moves/deletes their nodes, switches
song/chart scope, applies one undo gesture and saves explicitly. Zero and negative
rates are valid. A node at row zero can move/delete, unlike an initial BPM or time
signature. Left/Right changes the rate by 0.01x; Shift uses 0.001x. English/French
labels, help, validation and statuses describe these rules.

`timing_edit::scroll` loads Fumen-compatible rounded row conflicts, keeps f64
values and uses the original one-ULP setter equality. Editor value lookup propagates
the first rate backward before its row, as GrooveAuthor's rate-dependency pass
does; a file without scroll events has an effective editor rate of 1. This lookup
does not insert an event into the document. New input and source values must fit
the finite gameplay f32 range; unsupported values return errors. The typed parser
currently shares the existing numeric-pair parser's strict malformed-input rules;
broader lexical compatibility with the lenient file reader remains pending.

The existing scoped timing transaction copies all inherited SSC timing on first
local edit. No-op/cancel retain exact bytes and redo. Other chart timing and unknown
properties survive apply/save/reopen. SM export is explicitly rejected until file
conversion is available, since SM does not implement these scroll events. These
controls edit the events used by saved gameplay/generation; preview spacing modes
and scroll-aware visual simulation in the central editor remain unfinished.

Four oracle cases run actual Fumen scroll loading and the unchanged GrooveAuthor
setter with the existing movement adapter: empty maps, zero/reverse, late first
rates, fractional-row collisions, identical/one-ULP input and move/delete sequences.
Four core workflows compare all node values/stages, round-trip serialization,
inherited timing, undo/redo/reparse, RSSP's actual gameplay scroll segments,
stale/foreign/no-op history guards, SM rejection and invalid row/rate limits.
Two English native workflows cover stage/type guards, zero/reverse, initial move/
delete, cancel/no-op, fine-control input, apply/undo/save/reopen and SM rejection.
A French workflow checks visible labels/status and negative/zero numeric
input. Broader runtime/device verification and the rest of the delivery matrix
remain required for the full editor port.

Validation for this checkpoint: 252 core tests, 81 native editor tests and
`cargo check -p deadsync --locked` pass, including the regenerated one-ULP scroll
setter fixtures and a final rerun of coarse/fine scroll controls and French UI.

### Hold tick counts and supported SM timing types

F10 now edits TICKCOUNTS with the same scoped stage/apply/save workflow. New
values are signed-int-range nonnegative integers, including zero; fractional,
negative and overflowing new input is rejected. Left/Right changes by one tick,
including with Shift. A missing node offers GrooveAuthor's new-event default of
four ticks per beat without inserting it until explicitly staged. Initial tick
nodes can move/delete. The value remains integral throughout the domain and
history; the native adapter parses integer text before acceptance.

Fumen imports signed tick counts, even though the original editor's setter only
accepts new nonnegative values. The port preserves those source values and allows
them to move unchanged. Its integer reader skips malformed/fractional/overflowing
integer records, resolves rounded-row conflicts in source-beat order and keeps
unknown surrounding document bytes. New invalid edits fail without mutation.
SSC inheritance uses the shared full-timing detachment transaction; SM uses a
shared song TICKCOUNTS tag and requires no conversion. RSSP rereads the canonical
saved/reparsed tick string including retained legacy signed values.

The full SSC Tab order is BPM, Stop, Delay, Warp, Fake, Tick, Scroll, Signature.
The native SM cycle now skips unsupported Warp/Fake/Scroll types, giving BPM,
Stop, Delay, Tick, Signature in both directions. This replaces the prior UI's
unsupported-type error dead ends. Typed domain export guards remain active for
SM-incompatible data; existing SM bytes stay intact while cycling.

Four C# oracle fixtures use actual Fumen loading and the unchanged tick setter
with the existing movement adapter: empty maps, normal/zero counts, signed source,
rounded collisions and signed-int limits. Four core workflows compare all stages,
round-trip serialization, inherited timing, RSSP, shared SM writes, undo/redo,
foreign/stale/no-op history, lexical filtering and unchanged legacy negatives.
Three English native workflows verify zero input, mutable initial nodes, type
guards, integer validation/controls, no-op/cancel, signed SM moves/deletes and
apply/undo/save/reopen. One French workflow checks labels, validation and staged
acceptance. Existing format-cycle tests now verify the SM supported-type list.
These tests cover event editing and persistence; complete gameplay-like hold tick
preview, other timing properties, waveform/loop controls and the remaining
features in the delivery matrix are still outstanding.

Validation for this checkpoint: 256 core tests, 85 native editor tests and
`cargo check -p deadsync --locked` pass, including the supported SM type cycle,
integer tick controls and French tick validation/acceptance.

### Combo multiplier pairs in F10

F10 now stages and edits COMBOS pairs in the original `2x/3x` display syntax:
independent hit and miss integers from zero through i32::MAX. Left/Right changes
the hit value; Shift changes the miss value. Physical X/slash and numeric-keypad
multiply/divide keys enter the pair. A missing node offers the original new-event
default of 1x/1x without adding an event until explicitly staged. Initial combo
nodes can move/delete, and a changed pair must be staged before moving it.

The full SSC cycle is now BPM, Stop, Delay, Warp, Fake, Combo, Tick, Scroll,
Signature. SM skips Combo in addition to Warp/Fake/Scroll; its supported cycle
remains BPM, Stop, Delay, Tick, Signature. Domain export guards reject COMBOS in
SM and retain the original bytes. SSC local edits use the shared full inherited
timing transaction; one acceptance is one undo, Save remains explicit and
no-op/discard retain bytes/redo.

The source reader accepts `beat=hit` (miss defaults to hit) or `beat=hit=miss`,
including signed source integers, whitespace and rounded-row conflicts. Lexical
integer errors and malformed records are skipped as in Fumen's property parser.
New pair input follows the original regex/integer validator, including leading
zero normalization and its final-newline behavior; it requires nonnegative values.
Existing signed nodes remain representable and movable unchanged. Serialization
writes both integer components; it never treats a pair as a fraction or a float.
Unsafe coordinates/nonfinite beats fail explicitly. Other timing, unknown tags
and independent chart properties survive inherited timing detachment.

Four reference fixtures invoke the actual Fumen ComboPropertyParser and event
loader plus unchanged GrooveAuthor validator/getter/setter methods. They cover
empty/normal maps, optional miss values, malformed records, signed legacy values,
rounded conflicts, zero/max values, exact formatting/whitespace/newline rules,
no-op setters and move/delete/collision sequences. Four core workflows compare
all node values and displayed pairs, serialization/reparse, inherited timing,
RSSP's chart_combos string, undo/redo, foreign/stale/no-op guards, SM rejection
and signed-source moves. Three English native workflows cover pair entry,
independent adjustment, keypad input, validation, mutable initial events,
no-op/cancel/type guards, apply/undo/save/reopen and SM filtering. One French
workflow checks visible labels, validation, pair entry and acceptance.

These changes prove editing and persistence for the tested cases. Complete
combo/scoring simulation during editor playback, the remaining timing events,
waveforms/loops, chart lifecycle and other delivery-matrix features remain pending.

Validation for this checkpoint: 260 core tests, 89 native editor tests and
`cargo check -p deadsync --locked` pass. French UI tests were rerun after the
final wording adjustment to the combo help/validation messages.

### Interpolated scroll speeds in F10

F10 now edits SPEEDS independently of instant SCROLLS. Values use the original
`2x/48rows` or `2x/0.5s` grammar: a target rate, a duration and an explicit unit.
Zero duration changes instantly. Left/Right adjusts the target by 0.01x; Shift
adjusts the duration by one row or 0.001 seconds. Initial events can move/delete.
The inspector handles a decimal in both rate and duration, unit suffix keys,
keypad multiply/divide, full precision text beyond the old 24-character limit,
and physical-key aliases for W and shifted-period slash. Modifier shortcuts do
not insert suffix letters into the input. French/English controls and validation
are included. Text uses a round-trip f64 representation rather than truncating
seconds to the original getter's G9 display precision.

The SSC cycle is now BPM, Stop, Delay, Warp, Fake, Speed, Combo, Tick, Scroll,
Signature. SM skips Speed as well as Warp/Fake/Combo/Scroll and retains its existing
five supported types. The typed export guard also rejects SM. New values require
unsigned decimals, integer row durations and supported finite float ranges;
exponent notation is accepted by the file reader but not by the original editor's
new-input grammar. Existing signed targets/periods remain movable and serializable
unchanged. All nonzero source unit flags mean seconds; row-mode file durations
are quantized from beats using the original Fumen conversion. Unsafe end-row
addition and nonfinite data fail explicitly without changing document/history.

`speed::Speeds` supplies logarithmic row selection and the unchanged clamped
linear interpolation formula. The first event uses its own target as the prior
rate. Each later event starts from the previous event's target, including when
that earlier ramp was interrupted; it does not sample the old ramp's in-flight
value. Row periods and second periods query their respective coordinates, and
instant/legacy negative periods retain the original formula's behavior. The
row-selected query accepts a caller-provided event timestamp for seconds-based
events, so a future notefield integration must supply correctly ordered chart
event times. Row-only queries do not perform a clock lookup. Full integration
with preview spacing and time-based event-tree lookup remains pending.

Five oracle fixtures run actual Fumen speed property parsing/loading and unchanged
GrooveAuthor validator/setter and row/time interpolation methods. Every edit stage
compares target, duration/unit and interpolated queries: empty maps, interrupted
row ramps, second ramps, zero-length changes, signed source, fractional row
quantization, malformed records, decimal grammar/newline rules and movement.
Four core workflows also cover inherited timing, RSSP readback, undo/redo,
foreign/stale/no-op guards, SM rejection, checked end limits and precise text
round trips. Three English native workflows verify both units, two decimal
components, adjustments, invalid input/endpoints, initial moves/deletes,
no-op/cancel/type guards, modifier-key safety and Save/reopen. One French workflow
checks labels, validation, suffix entry/aliases and acceptance. These tests do not
prove hardware keyboard layouts, GPU output or complete visual speed simulation.
Those and the remaining delivery-matrix features are still required.

Validation for this checkpoint: 264 core tests, 93 native editor tests and
`cargo check -p deadsync --locked` pass. This includes the original interpolation
queries, both duration units, precise native fields and French input aliases.

### Section labels and committed Unicode text

F10 now edits SSC LABELS with translated controls, staging, row movement,
removal, scope selection and exact document undo/redo. The SSC cycle is BPM,
Stop, Delay, Warp, Fake, Label, Speed, Combo, Tick, Scroll, Signature. SM skips
labels and preserves any unknown source LABELS property without rewriting it.
Accepted labels appear alongside their rows in the central field.

The label value accepts committed Unicode text separately from physical editor
shortcuts. Winit text and IME commit/preedit events are routed only to the focused
label value; composition remains tentative until committed. Ctrl/Cmd shortcuts
cannot insert text, Backspace removes one extended grapheme (including combining
accents and joined emoji), and Delete clears the field. Focus loss or IME disable
clears tentative composition. IME activation follows editor field focus. The
candidate window uses an approximate inspector position; this is not a complete
caret, selection, clipboard or pointer text widget.

The sanitizer follows the original label setter for newlines, tabs and SSC
separator characters. Equals signs are explicitly rejected: the original setter
accepts them, but its CSV reader drops that record on reopen. Existing escaped
slashes remain literal text when moving a legacy label. The normalized timing
loader no longer treats decoded `//` label text as a second comment, which could
previously erase subsequent label records and affect chart-end timing. A final
empty CSV item preserves meaningful whitespace in the last label through MSD
trimming; the reference parser ignores that empty item.

Four C# oracle fixtures execute the actual Fumen label parser and unchanged
GrooveAuthor sanitizer/setter methods, with stage-by-stage edit comparisons,
Unicode, signed-zero/row collisions, malformed records and whitespace round
trips. An additional original-reader timing fixture covers escaped-slash labels.
Core workflows cover inherited timing, RSSP readback, exact undo/redo, document
identity/revision guards and SM rejection. English/French native workflows cover
composition, grapheme deletion, safe shortcuts, staging/movement, rendered actor
text and Save/reopen. Automated event/actor checks do not prove physical IME,
keyboard layout or font/GPU rendering on the user's machine. Full editor parity
and the remaining delivery-matrix features remain pending.

Validation for this checkpoint: 268 core tests, 97 native editor tests and
`cargo check -p deadsync --locked` pass. The last-label whitespace Save/reopen
workflow and all four label core workflows were rerun after their final changes.
The additional shell text-routing unit test could not compile because seven
`include_str!`/`include_bytes!` references require absent song-Lua fixture files
(`full_song_lua`, `song-inputs`, `song_lua`, `itgmania-song-lua-micro`). This is
not recorded as a passing shell test; the production shell compilation passed.

### Previous/next section label navigation

Ctrl/Cmd+L moves to the next label; Shift reverses direction. Like GrooveAuthor's
`EventTree.FindPreviousEventWithLooping` and `FindNextEventWithLooping`, the
search is strictly before/after the cursor, wraps at the ends, returns the same
label for a singleton, and leaves the cursor unchanged when there are no labels.
Repeated keys are supported in the chart view. Inspectors retain keyboard focus
and do not navigate the underlying chart. A jump stops audio preview before
moving, retains the workspace selection anchor and does not create an undo step.

Workspace commands read the current chart's effective LABELS, including unsaved
edits and undo/redo. Chart timing ownership determines whether song labels are
inherited. SM's unsupported LABELS property is preserved without using it for
navigation. Invalid label maps return an error before modifying the cursor.

The label oracle now extracts both original looping methods unchanged and runs
them against the actual Fumen red-black tree. Row-only event and strict-search
adapters replace GrooveAuthor's full editor-event comparison machinery; this is
not execution of the full editor. Four fixtures compare previous/next results
at 18 cursor positions initially and after every edit stage. Separate workspace
tests cover scope, empty/singleton maps, integer limits, selection, unsaved edits,
undo/redo, Save/reopen and error atomicity. Native tests cover Ctrl/Cmd, Shift,
repeat, panel focus, audio stop and preservation of document bytes/history. Help
includes these shortcuts in English and French. Hardware shortcut handling and
physical IME/GPU validation remain pending, along with the delivery matrix.

Validation for label navigation: 272 core tests and 99 native editor tests pass,
including 792 differential navigation queries (both directions) against the
original extracted looping methods. `cargo check -p deadsync --locked` passes.
The label core workflows were also rerun after extending their oracle assertions.
This does not remove the previously documented missing shell-test fixtures.

### Measure navigation follows the departure time signature

In the chart view, Page Up/Down now moves by one measure instead of sixteen snap
increments. Snap still controls single
Up/Down movement. A measure jump uses the time signature active at the departure
row; it does not snap to a measure boundary, split the movement at an intervening
signature, or undo the previous jump when the signature changes between jumps.
The workspace reads effective chart timing, including unsaved F10 changes and
undo/redo. Key repeat, selection anchors and stopping native audio are retained.
Inspectors keep keyboard focus. English/French help explains the shortcut.

`Signatures::measure_destination` returns the signed destination, including
negative positions exactly as the original commands request. The current
workspace still stores nonnegative integral note cursor rows and clamps its
visible destination to 0..i32::MAX. Negative/fractional editor cursor navigation
therefore remains an explicit parity gap, not a completed feature. Future signed
cursor work must preserve safe note placement and selection rules rather than
casting negative positions into note rows.

The signature oracle now extracts `OnMoveToPreviousMeasure` and
`OnMoveToNextMeasure` unchanged from the pinned Editor.cs. Position and active
signature adapters replace the full UI/rate-event tree; original Fumen signature
loading remains in use. Five fixtures compare both signed destinations at 1,040
positions initially and after edit stages, alongside the existing signature
setter and measure-grid checks. Four workspace workflows cover exact-boundary
behavior, crossing signatures, reader-only denominators, snap independence,
selection, local timing, unsaved changes, undo/redo, save/reopen, bounds and
invalid-map atomicity. Two native workflows cover page shortcuts, snap, repeat,
audio stop and inspector focus. Physical input/GPU behavior and the full delivery
matrix remain unproven by these tests.

Validation for measure navigation: all 276 core tests and 101 native editor
tests pass; `cargo check -p deadsync --locked` passes. The oracle comparisons
include signed destinations before zero; the workspace tests separately verify
its documented nonnegative boundary. Missing shell unit-test fixtures documented
above remain unresolved and are not included as passing tests.

### Global saved-pattern navigation and inspector selection

Ctrl/Cmd+P selects the next saved pattern and opens its properties; Shift selects
the previous pattern. This reuses the existing marker navigation and F6
inspector, with strict before/after start-row queries and wraparound. Holding the
shortcut repeats through markers, including within the marker inspector. Other
inspectors keep keyboard focus. Pending marker edits block navigation with the
existing Apply/discard message. No markers means no movement or panel opening.
An active audio preview stops before navigation. Note selection anchors now
survive marker navigation: choosing a pattern is separate from selecting notes.
Help documents the shortcuts in English and French.

Four differential fixtures extract `OnMoveToPreviousPattern(double)` and
`OnMoveToNextPattern(double)` unchanged from pinned Editor.cs. An actual Fumen
red-black tree with row-only pattern events adapts the original interval-tree
API; explicit cursor/selection callbacks replace the full UI. The fixtures
compare cursor and selected pattern for 52 positions in empty, singleton,
ordinary and reference-limit maps (104 directional command results). Core
workflows also cover overlapping regions, unsaved marker moves/deletions,
undo/redo, per-chart data, Save/reopen, retained note selection and atomic errors
for invalid metadata. Native tests cover Ctrl/Cmd, Shift, repeat, inspector
selection/focus, pending edits, audio stop and empty charts.

The simplified UI exposes the chosen pattern through its open inspector. A
persistent per-chart last-selected pattern across closing inspectors and moving
the cursor remains to be implemented before claiming full original selection
state parity. These tests do not prove hardware input or full rendered output;
the remaining delivery matrix still applies.

Validation for global pattern navigation: all 279 core tests and 103 native
editor tests pass; `cargo check -p deadsync --locked` passes. Four differential
fixtures compare the cursor and selected start for 104 original directional
commands. The previously documented missing shell unit-test fixtures and
remaining editor parity requirements still apply.


### Native RSSP analysis of the working chart

F12 or the Analysis entry in F1 opens a read-only inspector using the same pinned
RSSP dependency as DeadSync. It reports arrow/step counts, sustains, mines, lifts,
fakes, peak/median NPS, duration, hash, crossovers, footswitches, brackets, jacks
and stream intervals/breakdown. The density graph aggregates to at most 80
columns, preserving each group's maximum. Left/Right selects a density interval,
Home/End selects the first/last, and Enter returns to that interval's chart row.
RSSP intervals span four quarter-note beats, independent of time signatures.
Density counts note rows with steps, not individual arrows; the UI labels this
explicitly. It does not claim original GrooveAuthor density-algorithm parity.

A captured task owns serialized snapshot bytes, session identity, revision and
chart index. Capture serializes synchronously; RSSP computation and graph
aggregation run off the UI thread. Result publication requires that identity,
revision and chart still match. A current report is reused when reopening and
selects the interval containing the current cursor. Unsaved edits, undo/redo,
recovery/session replacement and chart changes invalidate it. F12 explicitly
refreshes. Analysis never writes the song or changes document history.

There is at most one spawned editor-analysis worker per process, including while
an editor closes and another opens. A cancelled job remains bounded until it
finishes; Escape closes the panel and rejects its eventual result. RSSP has no
cooperative cancellation hook inside the analysis call, so this does not promise
immediate CPU interruption. A permit is released on success, error, panic or
failed thread spawn. Unsupported layouts produce a translated explanation
instead of silently showing another chart's stats. Complete Couple/Routine and
other unsupported RSSP-layout analysis remain pending.

RSSP's row scanner expects measure separators on separate lines. For valid
inline commas, the analysis adapter uses the editor's existing note-attribute
lexer to normalize only the selected note field in a temporary copy. Attribute
commas and escapes remain inside tokens; note comments cannot become measure
separators. Other source bytes and the actual song remain intact. Canonical note
fields reuse the original analysis snapshot without an additional whole-file
copy. Tests compare inline separators to canonical RSSP stats, density, NPS and
hash. This normalization is not an export or a rewrite of the working document.

F1 displays at most eight actions with a translated page indicator, keeping the
expanded optional tools menu within the standard 854x480 scene. Existing action
indices are retained. Inspector strings, empty-stream text and unsupported-layout
messages are translated into English/French. Summary text and graph construction
are bounded; very long stream breakdowns are truncated only for display.

Core tests cover snapshot fidelity, current/foreign/stale identity, chart-local
timing, cancellation, unsupported layouts, density-bin row bounds, peak-preserving
aggregation, the global worker permit and note-separator/attribute safety. Native
English/French workflows cover F12, menu pagination, unsaved notes, Save/reopen,
revision-aware cache/refresh/undo, density actor data/navigation, cancellation and
translated unsupported-layout handling. These are synthetic input/actor tests,
not proof of audible output, physical rendering, device input or responsiveness
on long files. Source-capture benchmarks, full options/tech inspection, original
analysis equivalence, pointer navigation and the rest of the delivery matrix
remain required for the full port.

Validation for this checkpoint: all 287 core tests and 107 native editor tests
pass, including the new analysis unit/workflow and separate French UI suites.
`cargo check -p deadsync --locked` passes. The two native analysis suites were
rerun after the final report-status display adjustment. The previously documented
missing shell unit-test fixtures remain outside these passing test claims.


### Duplicate the complete working chart

Ctrl/Cmd+D or the final F1 action duplicates the active chart and focuses the
copy. This follows the clone action's document/history contract: the working
notes and chart properties are copied, undo restores the previously focused
chart, and redo restores the same history state rather than recalculating a
clone. Subsequent edits to the clone remain separate history gestures. Original
GrooveAuthor source inspected for this contract: ActionCloneChart.cs and the
EditorChart copy constructor at the pinned reference revision. This checkpoint
uses functional tests, not execution of the original full editor constructor.

The session serializes a snapshot before taking the chart block span, so pending
note/property edits are included and source spans cannot refer to stale byte
positions. SSC cloning copies the entire current chart block, including local
timing, labels, expression/pattern data, unknown properties and original note
encoding/attributes. Existing source charts and global metadata are retained.
The same block-span helper is shared with generated-chart assembly. Loaded grids
are checked against the resulting descriptor/notes before the history swap.

SM NOTES/NOTES2 headers and note bodies are copied exactly. GrooveAuthor's global
GAChartDataVersionCHARTnnnn and GAChartDataCHARTnnnn properties are copied as raw
values to the new index, preserving future JSON members and avoiding metadata
loss on Save/reopen. Existing destination-index metadata is rejected rather than
silently attached or overwritten. The four-digit custom metadata index limit is
enforced when copying metadata. Unknown global/vendor indexing conventions are
not guessed. Failed validation preserves the current document and redo branch.

One structural DocumentSwap stores the complete document/grid state; focus is
bound to its before/after revisions using the existing generated-chart history
mechanism. The clone keeps the cursor row and begins with participant zero. Notes
and participant separators remain intact. English/French menu, help and success
messages are included. The shortcut does not repeat while held and inspectors
retain keyboard focus. The F1 menu now contains sixteen actions over two pages.

Four core workflows cover pending notes/tags, scoped timing/labels, unknown
properties, pattern data, SM raw metadata and CP1252 source preservation,
NOTES2, invalid/stale/index-collision guards, source/clone edit independence,
Routine participants, undo/redo, focus restoration and Save/reopen. Native
English/French workflows cover the shortcut, menu, repeat suppression, inspector
focus, participants, history and persistence. This does not prove hardware
keyboard handling or physical rendering. Generic empty-chart creation and
chart deletion, including deleting the last chart while keeping undo available
in an editor with no active chart, remain required for full lifecycle parity.

Validation for duplication: all 291 core tests and 110 native editor tests pass;
`cargo check -p deadsync --locked` passes. The four duplication core workflows
were additionally rerun after strengthening SM metadata reparse assertions.
Missing shell unit-test fixtures documented above remain outside these claims.


### Create an empty chart

Ctrl/Cmd+N opens a compact layout/Create/Cancel inspector. Sixteen layouts are
available: the original fourteen editing layouts plus Dance/Pump Couple.
Creation adds a Beginner chart with meter 1, empty description and notes, dynamic
expression config and no saved patterns. Default participant counts follow Fumen
(including two for Couple/Routine/Team). The original chart's credit, music,
labels, pauses, scroll maps, custom fields and pattern payload are not cloned.

SSC receives its own initial tempo/signature, scroll/speed 1x, four hold ticks
and hit/miss multipliers 1/1. Song audio offset is retained for playback/export;
editor metadata starts with chart offset disabled and the correct max participant
index. SM creation explains shared song timing, retains global timing maps and
adds indexed metadata for the new chart. This reflects SM persistence limits;
the original editor's transient per-chart SM timing state is not reproduced here.
Existing orphan metadata at that index is rejected instead of overwritten.

The defaults use the original most-common tempo/signature helpers with grouped
chart-type enumeration, exact zero-beat/display/default-tempo fallbacks and the
reference dictionary's last-tie behavior. The signature histogram preserves
Fumen Fraction's written hash followed by rational equality (including equivalent
fractions which share that hash), without changing the general typed-signature
map's equality. The oracle executes unchanged GetBestChartStartingTempo and
GetBestChartStartingTimeSignature bodies with actual Fumen file loading. Seven
fixtures cover no charts, song/display fallbacks, majority, grouped ties,
fraction hashing and missing initial timing. Each fixture also compares all
sixteen Fumen layout column/player properties against actual created charts.
These 112 creation cases run in one differential workflow.

The oracle supplies chart groups in file-load order. It does not instantiate the
full editor, apply its live ChartComparer sorting or assign original runtime
GUIDs. Exact tied-default choices after live chart sorting therefore remain a
parity gap; the port currently resolves those ties deterministically in grouped
source order. Full live chart ordering/identity work must revisit this context.

A creation draft binds defaults to session identity/revision. Apply prepares and
validates notes, metadata and the new descriptor before one structural history
swap. Undo restores the prior active chart; redo restores the created chart and
its later independent edits. Existing source bytes and pending edits are kept.
Legacy SSC charts must pass the existing timing migration review first. The core
also supports bootstrapping a chartless SSC document, adding/updating VERSION to
0.83; this does not yet expose a chartless native editor.

Five core workflows cover original default/layout comparisons, current timing,
non-inheritance, offset flags, empty-song bootstrap, SM reparse, invalid/stale/
foreign/legacy/orphan guards, notes/participants, focus, history and Save/reopen.
English/French native tests cover layout selection, create/cancel, modal focus,
Routine participants, SM shared-timing wording, migration review and persistence.
Physical input/rendering, full chart property editing, empty workspace UI and
deletion (including the last chart with undo available) remain in scope.

Validation for empty-chart creation: all 296 core tests and 115 native editor
tests pass; `cargo check -p deadsync --locked` passes. The original generated-chart
creation tests remain in their existing suite; empty-chart workflows live in a
separate file. The original-method oracle adapters and remaining live-ordering,
SM transient timing, empty-workspace and missing shell-fixture limits above are
not included as claims of completed parity.


### Delete charts and retain a usable empty workspace

Ctrl/Cmd+Shift+Backspace opens a confirmation for the working chart. Cancel is
selected initially. The dialog captures current serialized descriptors, identity
and revision, so confirmation cannot delete another chart or apply stale data.
Opening it stops audio. Successful deletion removes the SSC chart block (local
notes/timing/properties) or SM NOTES/NOTES2 block in one structural history swap.
Pending edits in surviving charts are retained. Undo restores exact pre-deletion
bytes/grids and the original active chart; redo restores the post-deletion state.

SM GAChartDataVersionCHARTnnnn/GAChartDataCHARTnnnn fields for the removed chart
are removed and subsequent real-chart indices are renamed. Values, unknown JSON
members and unrelated global/vendor fields remain intact. Unbound orphan indices
are retained; unknown vendor indexing conventions are not guessed. Known V1 song
timing-chart references are updated within their chart type, including numeric
enum values, case-insensitive enum names and a missing index's default zero.
When their owner is deleted, the explicit type is cleared to request normal
selection fallback. Other song fields, including SyncOffset and future members,
are retained. Unknown/incomplete song-metadata schemas reject deletion before any
history/document mutation. As in original EditorSong loading, V1 timing-chart
references resolve from type-group file-load order before UpdateChartSortInternal;
this differs from the still-pending live UI sort/tie context.

Active-chart replacement follows the original SelectBestChart priorities: same
type and difficulty, highest preferred difficulty, Single/Double counterpart,
then supported-type order. It uses file order for ties; original ChartComparer
sorting/runtime GUID tie behavior remains unported. Source inspected at the
pinned revision: ActionDeleteChart.cs, Editor.DeleteChart and EditorSong's
DeleteChart/SelectBestChart/custom-song loading. These functional tests do not
execute the full original editor or prove live sorted-tab equivalence.

Deleting the final editable chart keeps the editor open with a no-active-chart
view. It supports Undo/Redo, Save, explicit recovery drafts, song sync properties,
Close and Ctrl/Cmd+N creation. F1 offers the applicable file actions. Note,
transport, generation and timing commands are blocked without a chart. Existing
opaque unsupported charts remain in the file and do not become a fabricated
editable grid. Empty/opaque-only files can reopen, including through a cached
song selection after the last editable chart was saved away. Native empty-file
creation and history transitions are now exercised rather than closing the editor
and losing access to undo.

Workspace exposes active_chart()/has_chart()/try_grid() for this state. Its legacy
chart()/grid() accessors retain an active-chart precondition; UI/render/input paths
check presence before using them. Recovery can replace an empty document without
loading a nonexistent grid. Creation, undo and redo preserve the distinct state
of no active chart even when opaque chart blocks still exist.

Seven core workflows cover SSC middle removal, pending edits and unknown fields,
SM indices/raw values/reparse, timing references and future metadata, stale/
foreign/wrong-chart/schema guards, final deletion, empty recovery/save/reopen,
new chart after deletion, participant restoration, replacement priority/focus,
and opaque survivors. Native English/French workflows cover confirmation/cancel,
modal focus, audio stop, safe empty-state inputs, song properties, Undo/Redo,
file menu, creation, cached reopening and persistence. No physical keyboard,
rendered GPU output, original live-order equivalence or full file-browser UX is
claimed by these synthetic tests. The remaining delivery matrix is still required.

Validation for chart deletion: all 303 core tests and 119 native editor tests
pass; `cargo check -p deadsync --locked` passes. The native deletion suite was
rerun after adding an explicit empty-state song-properties check. Original live
chart ordering/ties, physical runtime validation and the previously documented
missing shell unit-test fixtures remain outside these passing claims.


### Chart difficulty and rating

F7 now includes the six original difficulty choices and an integer rating. Use
Left/Right to cycle difficulty; type a rating or adjust it by one with arrows
(Shift: 100). Enter stages the numeric field; Apply accepts the whole chart draft.
The original UI minimum is one; overflow and fractional/negative input are
rejected. Legacy zero/signed or malformed source values are preserved until the
rating itself is changed. Unknown difficulty spellings are also preserved until
an explicit selection. Pending numeric input blocks opening the timing panel;
Escape discards the complete draft without modifying history or the file.

SSC writes DIFFICULTY/METER; SM edits only the corresponding positional fields
in NOTES/NOTES2, preserving escaped descriptions, radar values and note bytes.
Difficulty/rating, expression metadata and offset changes share one atomic undo
gesture. Identity-only edits do not introduce unrelated GrooveAuthor metadata.
The displayed chart heading reads current values rather than stale parsed
source descriptors. Save/reopen, undo/redo, no-op redo retention and session/
revision/chart guards are covered by workflows.

Seven differential fixtures use the actual pinned Fumen SM/SSC readers and a
small adapter reproducing the pinned EditorChart constructor's enum/integer
projection. They cover named/numeric/unknown difficulties, SSC fractional and
thousands ratings, invalid/NaN values and the distinct SM integer parser. This
is reader/projection evidence, not execution of the full original editor UI.
The UI minimum comes from UIChartProperties.DrawRowInputInt (min 1). Exotic
numeric syntax and every .NET enum spelling are not exhaustively compared.
Native English/French tests drive real key dispatch and inspect generated actors;
these do not establish physical keyboard or GPU-rendered layout equivalence.
Full chart properties, including names/credits and assets, remain in the delivery
matrix alongside the other outstanding editor features.

Validation for difficulty/rating: all 307 core tests and 123 native editor tests
pass; `cargo check -p deadsync --locked` passes. The four new core workflows cover
the reader fixtures, atomic combined edits, identity-only edits, persistence and
invalid/stale guards. Four additional native tests cover both file formats,
invalid input, clamping/Shift increments and French labels. The previously
recorded missing shell unit-test fixtures and physical runtime checks remain
outside this validation.


### Chart names, descriptions, credits and styles

F7 > Name, description and credit opens an optional text panel. SSC exposes all
four original chart text fields (CHARTNAME, DESCRIPTION, CREDIT and CHARTSTYLE).
SM exposes only its positional chart description; other fields are marked
unavailable and skipped during keyboard navigation. This reflects the pinned
Fumen SM writer, which does not serialize the other three per-chart fields. Full
transient SM property editing and a guided format-conversion flow remain pending;
we do not claim this persistence guard implements that original transient UI.

Up/Down chooses a field, committed Unicode input replaces its initial value and
then appends, Backspace removes a scalar, and Delete clears it. IME preedit is
shown without accepting it or triggering editor shortcuts. Escape/Back returns
to chart properties with staged changes; Apply commits all text, difficulty,
rating, expression and offset changes in one undo gesture. Escape from the main
properties panel discards them. Single-line input preserves punctuation through
MSD escaping instead of applying the stricter timing-label sanitizer. Control
characters are rejected by the core setter and ignored by native text entry.

The reader follows Fumen's chart credit inheritance: an absent/empty SSC CREDIT
uses the song CREDIT. Explicitly clearing it writes an empty local tag; reopening
then restores the inherited song credit, as in the original reader. Unchanged
inherited values do not create local tags or GrooveAuthor metadata. Unknown tags,
other charts, existing CP1252 bytes, escaped SM headers and note payloads remain
intact. Four actual-reader fixtures compare the pinned EditorChart text-field
projection, including Unicode, reserved punctuation and inherited/missing values.
These fixtures use Fumen plus a constructor adapter, not the full original UI.

Four core workflows cover the reader fixtures, combined atomic edits, Unicode
save/reopen, SM persistence restrictions and stale/foreign/wrong-chart guards.
Native workflows cover committed text/IME, shortcut suppression, cancel/back,
empty credit, both file formats and French labels. Full text selection, cursor
movement within a field, very-long-field horizontal scrolling, original live
chart sorting and physical rendering remain outstanding.

Validation for chart text: all 311 core tests and 127 native editor tests pass;
`cargo check -p deadsync --locked` passes. The native suite was rerun against the
final panel navigation. These results retain the prior limits concerning
physical/GPU validation and the unrelated missing shell unit-test fixtures.


### Unicode text cursor, selection and field history

The optional chart text panel now has an editable cursor rather than append-only
input. Left/Right moves by extended grapheme, Home/End goes to a boundary,
Ctrl/Cmd+Left/Right moves by Unicode word, and Shift extends the selection.
Ctrl/Cmd+A selects all; Backspace/Delete erase backward/forward (the selected
range takes priority), with Ctrl/Cmd erasing by word. Tab/Shift+Tab navigates
supported fields. Focus initially selects the whole field for easy replacement.
Ctrl/Cmd+Z, Shift+Z and Y operate on the current field's local undo/redo history;
focus changes reset that field history. These operations never enter document
history before Apply, which still produces a single chart-properties gesture.

`text_buffer::Buffer` owns UTF-8 byte positions constrained to grapheme
boundaries, selection replacement, word movement, local snapshots (256 maximum)
and a caret-following viewport. The editor reuses the already pinned
unicode-segmentation version; no new remote library version was introduced.
IME preedit replaces only the visible selected range and cannot mutate history
or trigger navigation; committed text inserts at the selected cursor position.
No-op edits retain redo. Long labels show at most 48 graphemes plus markers,
with an actor width limit, while full values persist. Literal trailing pipes are
preserved, including when switching field focus.

The current compact renderer uses `|` for the cursor, brackets for selection,
and `...` for hidden text. These are presentation markers and are never saved.
They are provisional: native graphical caret/selection overlays, pointer text
selection, system clipboard integration and pixel-based horizontal scrolling
remain to be ported. The grapheme/word behavior is a specified Rust interaction
contract tested against invariants, not a claim of exact ImGui cursor semantics.

Legacy control characters are escaped in field previews. They remain untouched
in source bytes; restoring the original field via local undo is allowed. A new
partial edit that retains a forbidden control character is reported and blocks
field switching. Escape can discard that invalid field buffer while preserving
other staged fields. Selecting all and typing a valid single line resolves it.
An invalid buffer is never silently presented as a committed draft value.

Four buffer workflows cover grapheme boundaries (combining accents, family
emoji, flags and Japanese), bidirectional selection, word erasure, mid-string
insertion, history/no-op retention and bounded viewport/preedit. A separate core
workflow verifies exact restoration of legacy control text. Four additional
native workflows cover cursor/IME/local undo, long-field persistence, modifier/
repeat dispatch, actor width limits and invalid legacy-buffer discard. Existing
French labels are checked with the new cursor rendering. These synthetic tests
do not prove physical keyboard, font shaping or GPU rendering behavior.

Keyboard accessibility remains a delivery requirement: every editor operation
must be reachable without a mouse. Pointer gestures may complement keyboard
commands; they must not gate a feature. The current inspector, note selection,
generation and timing flows are keyboard-driven. Future file/library, waveform,
loop and graphical editing work must preserve that same access path.

Validation for the text cursor: all 316 core tests and 131 native editor tests
pass; `cargo check -p deadsync --locked` passes. Physical/GPU validation and the
unrelated missing shell unit-test fixtures remain outside these results.

### System clipboard in chart text fields

Ctrl/Cmd+C, X and V now use the system text clipboard while a supported chart
text field is focused. Copy exports only the selection, without rendering
markers. Cut writes it successfully before deleting it from the draft. Paste
replaces the selection, strips single-line control characters and is one local
undo action. An empty/control-only paste preserves selection and redo. Empty
copy/cut selections do not access or clear the system clipboard. Legacy
selections containing NUL are conservatively rejected before IO to avoid native
text-termination differences; their original chart bytes remain intact. Outside this
panel the existing note clipboard remains independent. Timing-label text and
other inspectors do not yet share this system clipboard path.

The renderer queues a single text request tied to the field edit serial and
field identity. The shell services it immediately after Practice raw input.
A focus change, text change, cursor action or IME preedit makes the request
stale; stale requests never access the clipboard. Autorepeated copy/cut/paste
keys are ignored, and IME composition prevents shortcut dispatch. Clipboard
errors and unsupported/non-text contents preserve field data and are reported
in English/French. Later successful operations replace an old failure message.
Clipboard contents are neither logged nor accessed on entry, redraw or typing.

`clipboard::TextClipboard` is an IO boundary exercised with deterministic test
transports. The shell uses arboard 3.7.0 (text only; image support disabled), with
Wayland data-control enabled. Its native clipboard is lazy, owned on the main
thread and retained across editor closure for the application lifetime. Exiting
explicitly drops it. Retention matters for Linux selection ownership; Wayland
availability still depends on the compositor's data-control/XWayland support.
See the [upstream platform and ownership documentation](https://github.com/1Password/arboard#gnulinux).
Operations are synchronous and triggered only by these user commands; clipboard
backend latency and live cross-platform behavior require runtime validation.

Three native workflows cover selected Unicode, copy/cut/paste ordering, modifier
handling, local/document undo, save/reopen, failure/no-text/empty behavior, stale
focus/text/IME requests, repeats and independence from note commands. A French
workflow checks failures without changing chart bytes. One shell integration
target compiles the actual adapter and runs its lazy-startup/repeated-shutdown
contract without inspecting or overwriting the host clipboard. These tests prove
the editor protocol and adapter lifecycle, not physical OS clipboard transfers.

Validation for system text clipboard: all 316 core tests, 135 native editor tests
and the one shell adapter lifetime test pass. `cargo check -p deadsync --locked`
passes. Actual OS transfers, backend latency, physical/GPU behavior and the
previously documented unrelated shell unit-test fixture gap are not certified
by these synthetic tests.

### Per-chart music files

F7 > Chart text and music now includes the SSC chart MUSIC override. Type or
paste a path, press F2 in that field to choose a file, or F3 to clear it and
inherit the song music. The empty field displays that inheritance explicitly.
Like the existing text fields, music supports cursor/selection, clipboard and
local undo. It is staged with the other chart properties and applies in their
single history gesture. SM cannot persist per-chart music and marks this field
unavailable; song-level music editing and transient SM chart overrides remain
separate outstanding work.

The original EditorChart constructor reads the raw TagMusic extra, rather than
the reader's inherited Chart.MusicFile. Four actual Fumen reader fixtures prove
this distinction in SSC override/empty/missing fields and SM. Six fixtures call
the pinned Fumen.Path.GetRelativePath helper directly and compare same-folder,
subfolder, outside-folder, dot components, spaces/hash, Unicode and literal
percent paths with the Rust lexical conversion. Other platform-specific URI,
root/case and symlink edge cases are not certified by those six fixtures.

The native picker uses the existing rfd dependency and the original five audio
extensions (mp3/oga/ogg/opus/wav), plus All files. It starts in the song folder
when the current override is empty, or the current audio directory when valid.
It selects one existing file and stores a relative path, including ../ for an
outside file; it never copies or rewrites the audio. Different filesystem roots
retain an absolute path. Selected paths must be Unicode without controls; Unix
literal backslashes are rejected because playback treats them as separators.
Typed paths may reference a missing asset, matching the original permissive
property setter; playback reports that condition before issuing an audio command.

A music picker request is bound to the field edit serial. Changes of field,
text, IME state or clearing the override invalidate a pending request before any
dialog opens. Cancel/failure leaves the draft and its undo history intact. The
shell services requests after Practice raw keys through a testable Picker
boundary. The actual platform dialog is modal and runs on the UI thread; live
focus/input behavior, keyboard interaction in that dialog and audible playback
still require runtime validation.

The existing playback snapshot already resolves an explicit working chart MUSIC
before song MUSIC. Tests now exercise that path through the F7 draft, application,
undo/redo, clear, save/reopen and real GameplayAudioCommand::PlayMusic payloads.
Opening properties stops transport before changing files. Command tests use
small file fixtures and do not claim decoder or speaker output validation.
Four core workflows cover reader inheritance, combined atomic edits, path
fixtures, filesystem validation and unsupported SM persistence. Three native
workflows cover picker/clear/audio commands, cancel/invalid/stale requests and
keyboard path entry. A French workflow verifies labels and cancel/clear without
saving. One shell integration target tests the actual adapter's initial-directory
calculation without opening an OS dialog.

Validation for per-chart music: all 320 core tests, 139 native editor tests and
both shell adapter contract/lifetime tests pass. `cargo check -p deadsync --locked`
passes. Native modal focus/input, physical rendering, actual decoding/audibility,
other platform path edge cases and the existing unrelated shell unit-test fixture
gap remain outside those passing claims.


### Song text, global music and shared property editing

F11 now exposes an optional Song text and music panel. It edits TITLE, SUBTITLE,
ARTIST, CREDIT, GENRE, ORIGIN, TITLETRANSLIT, SUBTITLETRANSLIT, ARTISTTRANSLIT and
MUSIC in both SM and SSC, including chartless files. Fields are split into two
pages (eight visible rows maximum); Up/Down or Tab selects a field and Page
Up/Down changes page. Escape returns to the F11 property draft, where Apply
commits the complete draft and editor sync offset as one undo gesture. Escape
from F11 discards it. The sync-only history label and preview-only sync semantics
are retained. This does not add an editor for the song's gameplay OFFSET.

`text_fields` is now the single native implementation for song and chart text:
cursor/selection, grapheme and word movement, local undo, IME, bounded preview,
clipboard, music picker and clearing share the same code. A typed target carries
both scope and field, so a pending chart request cannot write a song field (or
the reverse). Existing F7 chart workflows remain regression targets. Song music
uses the same F2 picker and F3 clear commands. It has no inheritance placeholder
when empty; clearing it removes the fallback song audio while explicit chart
MUSIC overrides remain usable. Picker labels are scope-neutral and localized.

The song draft combines text and sync edits with the existing atomic scoped
property helper, preserving pending grids, unknown tags, raw legacy bytes and
future JSON fields. Text-only edits do not introduce GASongData. No-op drafts
preserve redo, and stale/foreign drafts cannot apply. Legacy original control
text can be restored without rewriting source; new control-containing values
are rejected consistently with chart fields.

Four fixtures use the actual pinned Fumen readers and the EditorSong constructor
field projection for all ten fields, escaped Unicode and initial chart authors.
The original chart holds its inherited author from loading; the song Credit
setter does not reassign chart Credit. For supported SSC charts with missing or
empty CREDIT and a nonempty previous file credit, applying a new file credit
therefore materializes that previous author locally in the same undo gesture.
Explicit chart credits and opaque chart payloads are left intact. SM has no
persistent per-chart credit and follows the changed file credit on reload.
This is a file projection contract; full original live author lifetime (notably
explicit empty-author states across several unsaved UI operations), sorted live
chart identity and unsupported layouts still require separate fidelity work.

Four core workflows cover the reader fixtures, atomic text/sync changes, author
preservation, pending notes, JSON/vendor bytes, SM/SSC/chartless files, exact
undo/redo/save/reopen and invalid/stale/foreign/no-op behavior. Four native
workflows cover Unicode clipboard editing, global audio selection/clear with
explicit chart overrides, scope-switch request invalidation, cancel and empty
workspace persistence. A French workflow checks fields, paging and clear/cancel;
the previous sync translation test retains its preview-only explanation. Audio
assertions inspect real command payloads using fixtures; actual decoding,
speakers, OS dialogs and GPU rendering are not proven by those assertions.
Song image/video assets, gameplay offset, display tempo, remaining inspectors,
validation dialogs and original live model semantics remain on the full delivery
matrix.

Validation for song text/global music: all 324 core tests, 144 native editor tests
and the two shell adapter contracts pass. `cargo check -p deadsync --locked`
passes. The documented original live-model, native OS/GPU/audio and unrelated
shell unit-test fixture gaps remain outside these results.

### Native keyboard runtime observation (2026-10-25)

Built commit `ac1cd77702` with `cargo build -p deadsync --locked` and
packaged a separately identified `org.deadsync.editorqa` macOS app. Its portable
Resources directory contains only a synthetic SSC and two generated WAVs; the
user song library/configuration was not used. Windowed OpenGL, 1280x720, French.
The standalone startup log confirms focused-window keyboard fallback when raw
Input Monitoring is unavailable. No permission was granted.

Using native keyboard input only, observed title -> guest profile -> colour ->
single -> standard -> song selection -> P practice -> F2 editor. The editor
renders the eight synthetic notes. F11, Up, Enter opens song text; PageDown, Down,
F2 opens the native audio chooser in the fixture song directory. Keyboard
selection and Return close it with a successful music-selection status; Tab then
moves to Back, proving focused keyboard navigation resumes. The selected music
remained `song.wav`; this observation does not prove switching tracks, persistence,
clipboard transfer, decoding, audible output, or complete keyboard coverage.

Observed UI defect: the French text-field help is a single long bottom line
which clips at the right edge. It needs wrapping or shorter contextual help and
a rendered regression check. These runtime observations supplement the tests;
they do not establish GrooveAuthor feature parity.

The footer now reserves 42 logical pixels for wrapped status/help, bounded to
the viewport width with 18-pixel side margins. Chart rendering ends above the
shortcut/footer area. A native actor contract checks the wrap width, position
and height at 854x480. This addresses the observed single-line overflow in code;
rendering the rebuilt application is still required to assess final readability.

Footer validation: `cargo test -p deadsync-theme-simply-love --test edition_song_text_workflow --locked` passes all five tests, including the bounded-footer actor contract. The contract checks layout properties, not glyph readability or GPU output.

Rendered follow-up: the rebuilt `org.deadsync.editorqa.footer` portable app was
opened at 1280x720 using only the synthetic song. The French SongText help and
SSC ChartText help both render fully on two lines, above the bottom margin,
with the shortcut line separate. This confirms the observed overflow is fixed
for these two panels at this resolution; other viewport/language combinations
remain unobserved.

A subsequent native automation paste of `Édition clavier QA` displayed only `v`
in the empty chart name. The next Command shortcut could not reach the app;
its process was no longer present and the game log contains no failure report.
Therefore OS clipboard and Command modifier behavior are NOT validated.
Investigate window fallback modifier events / automation routing and reproduce
with a single running QA instance before claiming clipboard parity or a crash.
No property was applied and no fixture song was saved during this observation.

### Focused-window modifier reconciliation

The shell now handles `WindowEvent::ModifiersChanged` when native raw keyboard
input is unavailable and the window accepts live input. It reconciles Shift,
Control, Alt and Super against held shell controls and routes only changes
through the existing raw shortcut adapter, without synthesizing pad presses.
The bundled winit 0.30.13 macOS `view.rs::update_modifiers` explicitly documents
that some flag-change events have no usable physical key code; only the modifier
snapshot is reliable in that case. Raw-backend mode remains gated out.

`cargo test -p deadsync-shell --test window_modifier_fallback --locked` passes
three tests: missing Command press/release recovery, duplicate suppression and
mixed modifier reconciliation, and a public editor workflow that routes the
Command snapshot into chart-text paste and observes accented text in actors.
The workflow uses an in-memory clipboard, so it does not validate macOS pasteboard
IO or establish the cause of the earlier automation/process-lifetime failure.
A rebuilt native runtime observation remains required.

Native follow-up on `77dcf0f1c`: built and packaged a fresh portable application
`org.deadsync.editorqa.modifiers` with the same synthetic fixture. Stopped the
previous QA process before launch, leaving a single QA instance. With raw input
unavailable, keyboard navigation reached chart text. Native automation paste
inserted the exact string `Édition clavier QA` (not a literal v), and displayed
the localized successful-paste status. Escape, five Up presses, Return applied
chart properties; Command+S displayed the saved status. A filesystem read then
confirmed `#CHARTNAME:Édition clavier QA;` in this application's isolated SSC.
The original synthetic source copy still has no CHARTNAME. This proves one
macOS paste -> apply -> keyboard save path after modifier reconciliation.
Copy/cut, mixed held modifiers, focus recovery across every panel, physical
hardware combinations and other operating systems remain separate checks.

### Simultaneously held editor modifiers

Editor shortcut state now tracks the four Control/Super physical keys and two
Shift keys independently. Releasing one no longer disables a modifier while
another is still held. `edition_clipboard_workflow` passes five tests, including
three Control/Super hold/release combinations driving text paste, final release
stopping clipboard IO, and both Shift keys selecting an accented grapheme after
left Shift is released. These checks exercise the public native editor adapter;
they do not prove physical OS delivery. The later native copy/cut follow-up could
not proceed because the QA process had exited before input; no new clipboard
claim is made from that attempt.

### Song music offset draft (core; native controls pending)

The song draft now captures/validates the global `OFFSET` (missing means zero),
exposes a finite music-offset setter with the reference one-ULP equality, and
includes changed OFFSET in the existing atomic song-properties transaction.
Changing only music offset does not inject GASongData.
`apply_itg_offset` follows pinned UISongProperties.ApplyItgSongOffset: add 0.009
to MusicOffset and set SyncOffset to 0.009 in the same draft. It is deliberately
non-idempotent, as described by the original tooltip. The native F11 panel has
not yet exposed these new controls.

All six `song_properties_workflows` tests pass. New cases exercise SM and SSC
apply/undo/redo/reopen, source-tag preservation, unsaved preview time conversion,
repeated ITG migration, one-ULP no-op, invalid-number rejection and malformed
OFFSET rejection. No audio decoder or audible playback is exercised. These are
workflow checks against inspected original methods, not newly generated C#
differential fixtures for the ITG action.

Native F11 follow-up: the song-properties panel now exposes global music OFFSET
and an explicit Add ITG 9 ms action. Left/Right edit by 1 ms (Shift: 0.1 ms),
0 and Home reset the music offset to zero. Editor sync retains its own 9 ms
Home default. The ITG action stages both fields; Apply accepts them together,
Escape discards them. English/French help distinguishes preview-only sync from
gameplay OFFSET and warns that repeating the ITG action adds another 9 ms.
The text-panel wraparound tests were adapted for the two added menu entries.
Native rendered/audio validation of these new controls remains pending.

Validation of the native song-offset controls: all 326 core tests pass. The
selected native playback, French timing, French chart-properties and song-text
workflows pass; a final rerun includes the newly added French OFFSET/ITG test.
The new English workflow stages ITG + an additional 1 ms, proves the document
unchanged before Apply, then applies/undoes/redoes/saves SM and SSC. The French
workflow verifies offset/action labels, repeated-action help and Escape discard.
No real decoder/speaker check is implied by these native adapter workflows.

### Preview sample region draft (core; native controls pending)

Song properties now capture SAMPLESTART and SAMPLELENGTH (missing defaults zero,
as in Fumen Song), expose finite setters with original one-ULP equality and
include changed values in the same reversible song transaction. Unchanged values
retain their source bytes. Reference: pinned EditorSong.SampleStart/SampleLength,
UISongProperties preview rows, Fumen SM/SSC readers and writers. Original model
setters accept signed values; the original length UI clamps its drag minimum to
zero. The Rust core preserves signed legacy data; the forthcoming native length
control must enforce that UI minimum on edits.

All eight song_properties_workflows tests pass, including SM/SSC atomic offset +
sample changes, undo/redo/reopen, invalid-number rejection and no-op preservation
of original decimal formatting. These tests do not cover an actual original
reader comparison for sample regions. Native fields, current-time shortcuts,
jump-to-region actions, preview-region rendering and audio audition remain
required; the new draft API alone does not deliver that feature.

Native sample-region fields: F6 from F11 song properties opens a compact optional
panel for sample start/length and Back. Arrows edit milliseconds (Shift: 0.1 ms),
Home/0 reset, Tab/Shift+Tab navigate, Escape returns to the existing song draft.
Length edits clamp to zero as in the original UI; start may be negative. Apply
in the parent accepts fields together. English and French help exposes F6.
Current-time setters, boundary navigation, visual region and audition remain
unimplemented; this numeric panel does not stand in for those original actions.
`cargo check -p deadsync --locked` passes after this integration.

All seven native song-text workflows pass after sample panel integration. The
new public-keyboard workflow stages start/length, checks the zero minimum and
Shift fine adjustment, confirms unchanged source before Apply, then verifies
SM/SSC tags, one-step undo, redo and disk save without adding OFFSET. This is
an adapter/actor test, not a rendered or audible runtime validation.

### Sample boundary position actions

Playback now shares an audio-independent Clock with sample-region commands.
Preview still validates its audio file before playback; position conversion
alone requires no MUSIC path or decoder. A song draft can capture a clock bound
to its session revision, using staged song offset/sync while retaining a chart
offset override. F2 in the optional sample panel sets start/end from cursor
song time; F3 jumps to the selected boundary. Core setters reproduce pinned
UISongProperties.SetPreviewStartFromCurrentTime / SetPreviewEndFromCurrentTime:
clamp current time at zero, and clamp resulting length at zero. End overflow
is rejected. Start/end jumping uses the shared inverse clock.

The native workspace still has a nonnegative integral row cursor, so boundary
jumps floor fractional rows and clamp negative/out-of-range positions. Original
GrooveAuthor has a richer continuous position model; this remains a fidelity
gap, not full boundary-navigation parity. Visual region and audition are pending.
All 14 targeted core playback/song-properties tests pass, including no-audio
draft time conversion, boundaries, invalid values and revision ownership.

All eight native song-text workflows also pass, including F2/F3 boundary navigation after deleting the fixture audio file, source unchanged before Apply, and discarding the parent draft. Runtime rendering/audio validation remains separate.

### Preview region positions and native actors

Song drafts expose fractional sample_positions using the shared clock, matching
the inspected EditorPreviewRegionEvent start/end time relationship. Ten targeted
core song-property tests pass, including a region across a tempo change,
fractional rows, unchanged pending document and stale-draft rejection. No new
C# differential fixture was generated for this region.

The native chart adds a tinted sample region and boundary lines, clipped to
the visible chart area, with English/French label. A cache keys source identity,
revision, chart and staged sample/offset/sync values; navigation/redraw does not
rebuild the clock. Nonpositive sample lengths hide the region. Parent Apply,
discard and draft edits refresh it. This is a positional region for the current
editor row scroll model; original visual-speed/scroll-gimmick parity, direct
pointer manipulation and audio audition remain outstanding. The rendered
appearance has not yet been observed in the rebuilt app.

All nine native song-text workflows pass, including staged region-label appearance and disappearance after parent discard with no source mutation. `cargo check -p deadsync --locked` passes. These actor checks do not validate GPU clipping or final text placement.

### Sample audition command pipeline

F4 in the optional sample panel toggles looped audition. A dedicated SSC PREVIEW
file takes precedence and plays from zero for its full duration. Otherwise the
current chart MUSIC override or staged song MUSIC is used with staged
SAMPLESTART/SAMPLELENGTH in raw file seconds, independent of chart offset/sync.
A nonpositive length or missing selected file rejects playback. The command uses
the original default preview fades (0 s in, 1.5 s out), looping and rate 1.
Reference: pinned MusicManager.StartPreviewPlayback, RenderPreview sample wrap
and PreferencesAudio defaults. Fade preferences editing remains pending.

Audition shares the existing audio command adapter, but does not advance the
chart cursor. F4 stops it; another sample-panel action stops it before editing
or leaving. This is a command integration, not evidence of actual decoded audio
or audible loop/fade equivalence. Dedicated preview-file editing, decoder/error
feedback, and rendered/audible QA remain required. Sixteen targeted core tests
pass, including cut times unaffected by staged offsets and PREVIEW precedence
even when no sample length is set.

The selected native song-text and playback workflows pass, including looped-cut payload, toggle stop, edit stop, zero-length refusal and unchanged document. `cargo check -p deadsync --locked` passes. OS decoder/speaker output is not covered by these fixtures.

### Editor focus input cleanup

The shell's existing focus-clear path now forwards release_editor_input through
Practice to the editor. This clears every tracked Control/Super/Shift key, IME
preedit, and pending clipboard/picker request without changing draft text,
selection history, saved document or audio policy. Previously shell controls
were reset but the editor's separate held-key state could remain active after
missing release events. The native workflow covers held Command/Shift, queued
paste cancellation without IO, subsequent ordinary text, composition cleanup,
and no leftover Shift selection. Physical OS focus transitions still require
a runtime observation.

All six edition_clipboard_workflow tests pass after focus cleanup integration.

Additional sample verification: all 12 song-properties workflows pass. The new
case establishes chart-specific offset priority over a staged global offset,
then disables the override and verifies inheritance plus stale-draft rejection.
This is a Rust public-workflow assertion; it is not an additional original
reader differential fixture. The previously running focus-cleanup
`cargo check -p deadsync --locked` has completed successfully.

All 11 native song-text workflows pass with an added sample-region geometry
check: it resolves solid-sprite effective dimensions (unit native size times
scale), verifies positive fill bounded by chart top/bottom, then scrolls beyond
the region and checks both fill and label disappear without document changes.
The first test assertion incorrectly treated the actor's zero explicit size as
its effective size; it was corrected using the actual solid-sprite sizing
contract. This is actor geometry evidence, not a GPU screenshot check.

### Rendered sample-region runtime observation (2026-10-25)

Built the current branch and packaged a fresh portable QA application identified
as `org.deadsync.editorqa.sample`. It has an independent configuration and a
synthetic SSC/WAV fixture (sample start 0.5 s, length 1 s). Windowed OpenGL at
1280x720, French; startup confirms focused-window keyboard fallback. No Input
Monitoring permission was granted.

Keyboard-only navigation reached Practice -> F2 editor. Observed the green
preview region, both boundary lines, readable French label and notes on top of
the fill. F11 -> F6 displayed 0.500000 s start and 1.000000 s length with the full
French multiline help inside the window. F4 displayed the looped-preview status;
F4 again stopped it, and F3 moved the cursor/chart viewport to the start boundary
(the toolbar shows the expected integral-row approximation). No error was found
in the QA game log, and the synthetic simfile remains byte-identical to its
fixture. This proves rendered layout and keyboard command responsiveness for
this case. It does NOT establish speaker output, actual loop/fade audio,
decoder error handling or waveform parity.

### Dedicated preview-file editing

Song text now includes the SSC PREVIEW field in the shared Unicode text/file
picker controller. F2 chooses audio, F3 clears it, clipboard/local undo follow
the same field path. Empty means audition the song's sample region. Native
labels/status distinguish this file from song MUSIC. SM marks the field
unsupported and rejects changes rather than writing a tag its original reader
does not use (pinned Fumen SSCReader TagPreview vs SMReader). Unchanged legacy
source stays preserved. Audition resolves the staged field before Apply.

The preexisting C# song-text oracle fixtures contain only the original ten
fields; their comparison explicitly remains limited to those fields. They do
not prove PREVIEW parsing/writing. New Rust workflows cover staged audition,
SSC tag persistence/undo/redo and SM refusal; an actual original-reader fixture
for PREVIEW, picker runtime and broader native text-field regressions remain
separate validation requirements.

Validation: 17 targeted core song-property/text tests and 21 native song-text/
French chart-property tests pass. The new native workflow drives the public
editor picker workflow using a fake picker, stages PREVIEW, auditions its full-file cut before
Apply, saves the SSC tag and verifies SM cannot open that picker. The picker
fixture uses the canonical macOS path, as the native chooser does, to avoid
asserting a lexical alias spelling as a distinct media file. Existing Fumen path
semantics are unchanged. Actual OS picker/decoder output is not proven here.

### Actual Fumen preview/sample reader comparisons

Extended the C# SongText oracle projection with Song.PreviewMusicFile,
PreviewSampleStart and PreviewSampleLength. Regenerated all six song-text
fixtures with pinned Fumen/StepManiaLibrary and the .NET 10 development runtime.
The two new SSC/SM inputs contain an escaped Unicode PREVIEW path, a negative
sample start and a fractional length. Expected values are emitted by the actual
original readers, not calculated from Rust. The Rust workflow now compares all
eleven fields and both sample numbers, retains chart-author checks and verifies
that a no-op draft leaves each source byte-identical.

This exposed that SM text projection displayed an ignored PREVIEW tag. Rust now
projects it as empty, matching Fumen SMReader, while retaining its opaque source
bytes and refusing unsupported edits. SSC continues to decode it. All four
song_text_workflows pass against the six updated original-reader cases. This
supersedes the earlier note excluding PREVIEW/sample fields from those fixtures;
it still does not prove the entire original editor UI or audio implementation.
The broader core suite was launched before fixture regeneration; its result
must not be reported as a clean current-state full-suite run without a rerun.

### Last-second hint property

SSC song drafts now read/write LASTSECONDHINT, with finite setters and original
one-ULP equality. Missing defaults to zero. Existing negative legacy values
remain byte-preserved if untouched. SM rejects edits and retains any unknown
source tag. The optional F11/F6 audio panel exposes the hint after sample length;
numeric edits clamp to zero, F2 uses nonnegative current song time, F3 jumps
through the same draft clock. Apply groups it with other song properties.
Reference: pinned EditorSong.LastSecondHint, SSCReader TagLastSecondHint and
UISongProperties.SetLastSecondHintFromCurrentTime/JumpToLastSecondHint. The hint
means an optional song end time extending play after the last note, per the
original event description. Original event-widget rendering and gameplay
consumption of the hint remain unverified.

All 14 core song-properties tests pass, including finite validation, untouched
legacy value, SSC apply/undo/redo/reopen and SM refusal. The previous broad core
run completed successfully but preceded final source/fixture edits; a fresh
full core run is active at the current state and must be reported separately.

The end-hint native label now explicitly displays unavailable in SM instead of
a numeric value. Selecting that row explains its optional song-end purpose in
English/French and lists the current-time/jump/Apply controls. This is a field
explanation, not proof that gameplay consumes LASTSECONDHINT. A French public
keyboard workflow covers SSC value formatting, SM unavailable state and help.
A broader `edition_*` integration run is also launched at this source state.

All 23 selected French chart-property and song-text workflows pass after the end-hint label/help update. Full core and editor integration results are pending.

### Broad validation checkpoint after audio properties

Both fresh runs completed with exit code zero:

- `cargo test -p deadsync-editor --locked`: 334 tests passed, zero failed.
- `cargo test -p deadsync-theme-simply-love --test 'edition_*' --locked`: 158 integration tests passed, zero failed.

The core implementation/fixtures remained unchanged during the fresh core run;
the subsequent end-hint label/help change is covered by the full editor run.
These results supersede the earlier pending status. The native integration run
covers all editor integration targets selected by edition_*, not every theme or
shell unit test. Missing unrelated shell fixture limitations remain as noted.
The suites still do not establish full GrooveAuthor parity, actual speaker
output, decoder/loop/fade equivalence, pointer workflows or waveform behavior.

Next audio-preference work: expose the original fade-in/fade-out settings
(defaults 0 / 1.5 seconds) through an optional keyboard panel, persist preferences
in the application data directory independently of songs, reject invalid
values, and prove load/edit/save/reopen plus audition-command application.
Malformed/future preference files must not be silently overwritten. This is
planned work, not an implemented preference feature at this checkpoint.

### Audio preference storage foundation (not connected to native UI yet)

Added audio_preferences::Audio with original fade defaults 0 / 1.5 seconds and
finite nonnegative setters, matching the original UI's zero minimum. A separate
versioned JSON File adapter loads defaults when absent, rejects malformed or
unknown-version files, retains unknown root/audio fields, and leaves source bytes
untouched on no-op. Explicit saves use a same-directory temporary file, sync,
atomic replacement, advisory lock and exact expected-byte conflict detection.
The adapter tracks post-replacement expected bytes before directory sync so a
sync failure does not make a retry conflict with its own successful replacement.
It does not write songs or mark preference changes saved before successful sync.

All four audio_preferences_workflows pass: defaults without creating a file,
edit/save/reopen independent of a song, unknown-field/no-op preservation,
competing-session refusal, and invalid values/files unchanged. The application
data path, shell loading, keyboard preference panel and use of these settings
in audition are still pending. This module alone is not the complete preference
feature; original full PreferencesAudio import/export and actual fade DSP
equivalence remain outside these tests.

### Application preference path and audition bridge

AppDirs now resolves editor-preferences.json directly under application data.
The editor runtime installs both generation-config and preference paths in one
immutable, absolute-path bundle. The shell supplies those paths explicitly, so
portable and installed layouts do not infer preference location from songs.
Native editor creation loads that file, or uses defaults if absent. Malformed
or future-version files leave the editor open with an error and fallback audio
settings; no automatic save occurs. An explicit reload failure retains the last
usable audio settings and invalidates its writable file adapter.

Sample audition commands now use the loaded fade values instead of literals.
The keyboard preference editor and saving from that panel are still pending.
Runtime path unit test and application-directory unit test pass. All 14 native
song-text workflows pass, including explicit preference reload, custom fade
command payload, reopen and invalid-file retention without song edits.
`cargo check -p deadsync --locked` passes. These checks cover values/commands,
not actual fade samples produced by a decoder.

The dedicated edition_preferences_runtime_workflow also passes: the real State constructor reads the installed runtime path, then opening with a future-version preference file succeeds with default audio and a visible error while both preference bytes and song source remain untouched.


### Keyboard audio preferences panel

The native editor now opens audio preferences with F11 → F6 → F5 (song
properties, sample, audio settings). Tab/Shift+Tab and arrows navigate. Left/right
adjust fades by 1 ms, Shift by 0.1 ms; four decimal places keep fine edits visible.
Home restores the selected default and 0 clears it. A separate action restores
both original defaults (0 / 1.5 seconds). Changes remain staged until Save;
Escape or Discard returns without changing active settings or song history.

Saving writes editor-preferences.json through the application preference adapter.
Malformed/future files and externally changed bytes are refused; the draft stays
open and active audition settings remain unchanged on failure. Even a no-op save
checks for external changes. The adapter separately remembers bytes written before
a directory-sync failure, so reverting a draft cannot silently leave a different
on-disk value treated as clean. Unknown fields remain preserved.

Native tests cover keyboard save/reopen, cancellation, defaults, fine-step display,
audition command values and conflict/future-version refusal without song edits.
These tests exercise native state/actors and commands, not physical audio output
or GPU layout. Full original audio preference coverage and audible DSP comparison
remain incomplete.

Validation for this checkpoint: four core preference workflows and 18 native
workflows (17 song-text/settings plus one runtime bootstrap) pass.


### Waveform multiresolution foundation

waveform::Builder accepts frame-aligned interleaved i16 PCM packets and preserves
channel separation and sample-to-sample history across packet boundaries. Its
packed 32-bit nodes follow SoundMipMap's 9-bit extrema / 14-bit mean variation
layout and its first previous value (0.5). Coordinates and per-sample distances
truncate as in FillMipMapWorker. Dyadic levels permit arbitrary half-open frame
queries without rescanning the underlying PCM. All levels retain both child
extrema, including an odd final frame. Invalid packets fail before changing the
builder; allocation failures return errors.

Four workflows pass: source-formula quantization/distance examples, exhaustive
interval extrema against direct PCM scans for a 137-frame stereo signal,
packet-boundary independence, and empty/invalid/clipped inputs. These are Rust
reference-formula tests, not differential execution of the original C# class.
Higher-level mean distances use rounded child averages; GrooveAuthor initially
accumulates several levels directly, so dense-region numerical parity is not yet
proven. The inspected original coarse merge also reads GetMin for its second
child's maximum; this port retains the actual maximum instead of copying that
extrema-loss defect.

Decoder integration, cancellable background loading, bounded resource policy,
timeline mapping, native waveform rendering and waveform preferences remain
pending. Waveform decoding must use DeadSync's music timeline options
(mp3_gapless=false, mp3_info_silence=true), rather than decoder defaults, to align
with playback. Memory currently scales with all packed sample levels (less than
approximately 8 bytes per input sample, excluding vector overhead); do not start
unbounded decoding on the UI thread.


### Background waveform decoding and first native view

editor_audio::Loader runs actual deadlib-audio-decode readers on a named worker,
without opening a sound device. Its per-editor permit remains held until work
ends, including cancellation; replacing requests cannot accumulate workers on
that loader. Dropping/cancelling a Job is nonblocking. Poll consumes one result,
rejects even already-completed results after cancellation, and reports unexpected
worker disconnects. Decode and pyramid construction check cancellation regularly;
codec file-open/read operations themselves are not interruptible.

Builder::with_memory_limit accounts for packed samples plus all pyramid levels.
It bounds geometric base-vector growth and rejects a packet before mutating the
builder if the packed-data budget would be exceeded. Native loading uses 256 MiB;
codec buffers, vector headers and other editor state are outside that budget.
This is an explicit failure for an oversized full-resolution waveform, not a
silent reduction of source resolution. Automatic downsampling/disk caching for
very large audio remains future work.

DecodeOptions::STEP_MANIA_TIMELINE centralizes the existing playback policy;
both audio-stream and waveform loading use it. MP3 encoder delay/padding and the
Info-header silent frame remain present, so using decoder defaults cannot shift
the waveform relative to music playback. Audio-stream's option values are
unchanged.

W in the main keyboard editor toggles loading/display without stopping music.
The initial view is opt-in; original default visibility and configurable waveform
preferences still need porting. The view uses chart MUSIC overrides, otherwise
the staged song music or source song music; PREVIEW is not the chart waveform.
It reuses decoded data when timing/offsets change, rebuilding only its playback
clock. Changing the selected audio cancels the old request, clears old data, and
retries when the old worker actually releases its permit. Errors are localized;
hide/show explicitly retries loading, including after replacing a file at the
same path. There is no filesystem watcher.

The view draws separate sparse envelopes and dense regions behind the chart,
clipped to the note field, using the existing row-based scroll and shared music
clock. Rendering work is bounded by viewport height (at most 4096 bands) and
channel count. Clock errors clear pending/old data rather than publishing a stale
waveform. The current rendering uses integer-row time intervals; continuous
sub-row zoom, scroll/speed visual semantics, original dense-region numerical
parity, channel scaling/colors/preferences, GPU observation and original C#
differential rendering remain incomplete.

New native workflows decode a real generated stereo WAV across packets, compare
it to direct PCM envelopes without changing source bytes, test cancellation,
budget failure and subsequent retry, and decode the generated MP3 fixture using
the playback timeline (including its silent Info frame). Native keyboard/actor
workflows cover chart audio priority, view clipping/hide, preservation of playback
when toggling, offset alignment of a timed pulse and recovery after a missing
file. Core tests add packed memory budget and cancellation contracts. These are
CPU state/actor and real decoder tests; they do not prove actual GPU output or
physical audio fade equivalence.


Validation checkpoint for the native waveform implementation:

- `cargo test -p deadsync-editor --jobs 2 --locked`: 344 tests pass.
- `cargo test -p deadsync-theme-simply-love --test 'edition_*' --jobs 2 --locked`:
  169 tests pass across the 19 targets present at that run.
- The subsequently added `edition_waveform_worker_contracts` target passes all
  three source-level unit contracts (permit exclusion/release, cancellation after
  success is queued, and one-shot unexpected-disconnect reporting). It compiles
  the worker module directly: the complete theme unit target depends on the
  unrelated missing `native-out-clocks.json` SongLua fixture in this checkout.
  This isolated target is not evidence that the entire theme unit suite passes.
- `cargo check -p deadsync --locked`: passes. Existing dead-code/platform warnings
  and the upstream `block` future-compatibility notice remain.

Use a limited Cargo job count for broad local suites; unrestricted parallel
linking of the native integration binaries caused substantial host pressure and
was explicitly stopped before rerunning successfully with two jobs.


### Native OpenGL waveform and audio-preference runtime review

Reviewed source commit `12a42c78d` in a newly built macOS application:
`cargo build -p deadsync --jobs 2 --locked` completed successfully, followed by
`scripts/build-macos-app.py --binary target/debug/deadsync` packaging with a
separate `org.deadsync.editorqa.waveform` identifier. The bundle uses portable
application data, a generated 40-second stereo WAV and one synthetic SSC chart.
French, windowed 1280 × 720, OpenGL; sound was muted in this isolated QA config.
No personal songs/configuration or input permissions were changed.

Observed with native UI screenshots, using only keyboard navigation:

1. Title → guest → colour → Single → Standard → song wheel → P → F2 opens the
   native editor. W loads the waveform and shows the translated ready status.
2. The two channel envelopes differ as generated (smooth amplitude modulation
   on the left, stepped amplitude on the right). They stay behind readable notes
   and coexist with the highlighted music preview region.
3. W removes both envelopes while retaining notes, cursor and preview region;
   W reloads them. Down advances to beat 1 and moves notes, waveform and region
   together inside the chart view. No GPU geometry change was needed.
4. F11 → F6 → F5 opens the French audio preferences panel. Shift+Right visibly
   changes fade-in from 0.0000 to 0.0001 seconds. Escape discards that draft.
5. Reopening that panel, Right then Save writes fade-in 0.001 / fade-out 1.5 to
   the bundle's editor-preferences.json and shows the translated saved status.
   Disk JSON was independently checked.
6. Closing the panels and editor with F2, then reopening the editor and panel,
   visibly reloads 0.0010 / 1.5000 seconds from application preferences.
7. Restore defaults then Save writes 0 / 1.5, verified on disk. The editor was
   left open on the waveform view with a clean song and sound muted.

Before/after SHA-256 checks confirm both synthetic audio and SSC are unchanged.
The run log identifies OpenGL and contains no ERROR lines. Built executable
SHA-256: `7bba61d3bd018307a80adbe75ff0ed9e8620f15789d1c7d3a0808697d303f43b`.
Local screenshots are `/tmp/deadsync-waveform-visible.png`,
`/tmp/deadsync-waveform-hidden.png`, `/tmp/deadsync-waveform-scrolled.png`,
`/tmp/deadsync-audio-fades-french.png`, `/tmp/deadsync-audio-fades-saved.png`,
`/tmp/deadsync-audio-fades-reloaded.png` and `/tmp/deadsync-waveform-final.png`.
They are local QA artifacts, not portable repository fixtures.

This establishes the basic native OpenGL view, keyboard access and real preference
save/reopen on that isolated run. It does not establish audible fade equivalence,
Vulkan rendering, every resolution/layout, long-file performance, configurable
waveform preferences or full GrooveAuthor parity. Earlier sections describe the
historical state before this runtime review.
