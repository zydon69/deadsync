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
| Metadata | Existing and new scoped tags with escaping and history | Property add/undo/reopen tests; full song/chart/pack property panels pending |
| Timing/gimmicks | Original tags preserved | Typed editing, inheritance, conversion and timing tools pending |
| Playback and audio | Existing DeadSync Practice audio retained outside the editing workspace | Edited chart preview, waveform, scrub, loop, Song Sync and audio analysis pending |
| Multi-chart | Switch existing charts with Ctrl/Cmd+Tab; shared history | Generated chart creation is available; generic creation/clone/delete, multi-view comparison and library pending |
| Generation | Original graphs, typed configurations, seeded .NET random, performed-path scoring and pattern search with start/end footing and lookahead | Differential pattern fixtures across all 11 original graph layouts, timing changes, context and seeds; preview/apply/undo/save and cancellation tests. Expressed-chart lane-state preparation and graph candidate matching are compared to the original interpreter across all 11 layouts, including holds, rolls, fake/lift instances and jumps. Interpretation costs, automatic bracket policy selection and mine expression match original chart interpretation; replacement expansion and ranking match the original cache; performed-chart conversion and destination mines compared across Dance/Pump/SMX; automatic fallback selection, file assembly and asynchronous chart-conversion previews are connected; single-region native pattern previews are connected through F4; full configurations and batch generation remain pending |
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
cargo test -p deadsync-theme-simply-love --test edition_workflow --locked
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
Wider original-action differential coverage and persistent/batch pattern actions
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
loader. Provenance records the extracted method hashes. Thirty-five committed
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
unchanged original region extraction and deletion helpers. All 35 cases compare
these projected regions, including empty ranges and shortened tails. Mixed note
kinds on one row verify the original lane-before-type comparison.

A separate native ChartTask test serializes 32 of these cases as SSC and compares
the generated double-chart notes against the original library consuming the
editor-assigned events. Loaded pattern markers and missing-tempo loader defaults
are excluded from this test and remain separate unfinished contracts. This is
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
the native interface through the optional F4 panel. An integrated differential test serializes 32
cases as SSC and compares generated notes and exact average NPS with the original
editor region/deletion/density/range/context helpers chained to the original
StepManiaLibrary interpretation and pattern generation. The additional original
range helper is extracted unchanged and included in provenance. Cases cover
stops/delays, warps and signature changes, negative-stop clamping, crossing holds,
local metadata, dense rows, exclusive endpoints, single-step patterns, beginning
of chart and empty charts. All 32 produce matching notes; loaded pattern-marker
and missing-tempo cases remain excluded from this integrated SSC test.

This harness composes original helper bodies with explicit container/accessor
adapters; it does not execute the complete editor action, file loader, UI or
multi-pattern scheduler. The integrated fixtures currently use dance-single,
balanced configs and seed 42. Other layouts/configs/seeds have component-level
coverage but still need broader integrated differential coverage.

Multiple overlapping pattern markers, persisted editable pattern metadata and
batch regeneration remain unfinished. Repeated live edits also need timeline
cache semantics: rebuilding after each edit can change floating-point clamp
history. These tests do not prove the complete editor port.

Reader normalization for invalid/conflicting timing entries, unsupported attack
timing, existing persisted pattern metadata, previews/last-second hints and wider
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
region in one undoable gesture; it does not create a chart or save the song.
Escape/Discard and cancellation retain the original document and history. The
Practice update loop polls both job kinds and input handlers never poll jobs.

The native workflow suite now covers F4 and menu entry, selected and default
ranges, subdivision/profile/signed-seed/endpoints/distribution controls, preview
actors, explicit application, undo/redo/save/reopen, discard/cancel, unsupported
multiplayer input, and explicit English/French strings. These tests execute the
real raw-key adapter and actor construction, not GPU rendering or hardware input.
The complete port, persistent pattern markers, multi-region generation and the
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
substituted. Configuration-file import/export, persistent custom-profile storage
and original missing-profile fallback/diagnostic behavior remain unfinished.

`Session::set_properties_atomic` validates all tag edits on an isolated snapshot
and records a single reversible property gesture without copying note data. Failed edits preserve redo and
pending note changes. Seven workflow tests cover the original C# JSON fixtures,
SM/SSC save/reopen, unknown fields and legacy source bytes, profile resolution,
invalid inputs, atomic rollback, and custom-tag spelling through updates,
undo/redo and chart cloning.

This storage layer is not yet wired to automatic F4 persistence or regeneration
of loaded markers. Original constructor inspection confirms that custom markers
are added after SetUpEditorEvents, through typed rate queries, without refreshing
surviving event times. Existing raw timing fixtures with a Region inserted before
refresh model a different state. Loading multiple saved markers must preserve
that distinction, and needs original-loader/action differential coverage before
connecting persistent generation. Live timeline caching, batch/overlap actions,
marker editing/deletion and the remaining editor delivery matrix stay in scope.
