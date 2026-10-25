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
| Multi-chart | Switch existing charts with Ctrl/Cmd+Tab; shared history | Multi-view comparison, chart creation/clone/delete and library pending |
| Generation | Original graphs, typed configurations, seeded .NET random, performed-path scoring and pattern search with start/end footing and lookahead | Differential pattern fixtures across all 11 original graph layouts, timing changes, context and seeds; preview/apply/undo/save and cancellation tests. Expressed-chart lane-state preparation and graph candidate matching are compared to the original interpreter across all 11 layouts, including holds, rolls, fake/lift instances and jumps. Interpretation costs, automatic bracket policy selection and mine expression match original chart interpretation; replacement expansion and ranking match the original cache; performed-chart conversion, fallback cost integration, destination mine placement and native asynchronous generation panels pending |
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

Next implementation work must finish performed-chart conversion with fallback
costs and mine placement, using the replacement rules now ported below. Differential fixtures must extend the existing tech, bracket, crossover,
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
python3 scripts/editor-generate-fixtures.py --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet
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
matching, not the final interpretation: performed-chart fallback conversion and mine placement remain separate work.

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
and mine-expression checks below extend this stage; performed-chart fallback
search and native generation panels remain pending.

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
no partial interpretation. Mine placement on the destination graph still needs
porting along with fallback conversion.

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
These checks establish replacement expansion and ordering, not complete
performed-chart conversion. Its search, fallback cost integration and destination
mine placement still need to be connected and verified.
