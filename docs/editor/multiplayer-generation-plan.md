# Generation capability: original contract and multiplayer extensions

## Scope correction

The previous delivery matrix called multiple-participant source generation a
remaining GrooveAuthor feature. Source inspection contradicts that claim.
GrooveAuthor at the fixed reference commit does not provide generation graphs
for Routine, Couple or SMX Team. The remaining parity work is to enforce and
prove the original capability boundary consistently. A future multiplayer
generation feature would extend the original editor; it cannot be described as
an unported original capability.

This correction does not reduce the requested port: all original generation,
configuration, pattern, batch and editor workflows remain required. Couple and
Routine chart editing/playback remain separate, already requested capabilities.

## Authoritative source evidence

Reference commits:

- GrooveAuthor: `48e18f71655d0f0d86144c63239331833501fda2`.
- StepManiaLibrary: `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`.
- Fumen: `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

The following are source observations, not a claim that the complete original
GUI has been executed:

| Source | Observed contract |
| --- | --- |
| `StepManiaEditor/Editor.cs:141–154` | `SupportedSinglePlayerChartTypes` contains exactly eleven layouts. |
| `Editor.cs:4934–4956` | Graph dictionaries and loading candidates are initialized only for those single-player types. Pad data has a broader inventory. |
| `Editor.cs:5087–5101` | `GetStepGraph` performs an exact chart-type dictionary lookup; it has no Routine-to-Double aliases. |
| `EditorChart.cs:1156–1159`, `3217–3220` | `IsMultiPlayer` depends on the layout's `DefaultNumPlayers`, and `SupportsAutogenFeatures` is its negation. Explicitly setting Routine to one player does not enable autogen. |
| `EditorChart.cs:3115` | A pattern marker cannot be inserted into a multiplayer chart. |
| `UI/UIAutogenChart.cs:107–111` | The destination selector lists only the eleven single-player types. |
| `UI/UIAutogenChartsForChartType.cs:113–127` | Batch source and destination selectors use the same single-player list. |
| `EditorActions/ActionAutoGenerateCharts.cs:85–99` | Chart generation requires exact input/output graphs and destination roots. |
| `ActionAutoGenerateCharts.cs:134–150`, `181` | One event list enters expressed/performed search; output `NumPlayers` is one. |
| `EditorActions/ActionAutoGeneratePatterns.cs:105–112` | Pattern generation also requires the exact chart-type graph. |
| `StepManiaLibrary/ExpressedChart/ExpressedChart.cs:1000–1035` | Events are grouped by row/lane into one performer's state, without selecting or partitioning `LaneNote.Player`. |
| `StepManiaLibrary/PerformedChart/PerformedChart.cs:1459–1512` | Generated note events explicitly use `Player = 0`. |
| `Fumen/Converters/SMPropertyParsers.cs:1135–1146` | Legacy Pump Double co-op is promoted to Pump Routine by the reader, including a source with only P1's exclusive `X/x` marker. |

The original layout inventory is Dance Single/Double/Solo/Threepanel, Pump
Single/Halfdouble/Double, and SMX Beginner/Single/Dual/Full. Routine/Team permit
variable participants, but the capability depends on their chart family rather
than the number of occupied participant streams.

`scripts/editor-generation-capabilities-reference.py` extracts the actual pinned
arrays, unchanged capability methods, graph registration condition and Fumen
layout defaults into `generation-capabilities.reference.json`, with source
hashes and commits. This is static reference evidence. It does not execute .NET,
load graph files, run generation, or pretend to supply a multiplayer-output
oracle. Existing expressed/performed differential fixtures separately exercise
the original eleven generation graphs.

## Rust API inventory and confirmed inconsistency

| Public API | Prior behavior | Current increment |
| --- | --- | --- |
| `job::TARGET_LAYOUTS` | Already exactly the original eleven targets. | Unchanged; canonical generation capability inventory. |
| `ChartTask::capture` / shared batch capture | Checked target inventory but not source layout. | Validate source against original graphs and Fumen's effective co-op layout before creating a task. |
| `source::read`, `prepare`, `prepare_editor` | Required one raw participant but accepted a one-stream Routine or P1-only legacy StepF2 chart. | Reject unsupported source families independently of raw stream count. |
| `SourceChart::prepare_grid`, `prepare_editor` | Public mutable layout/grid fields could bypass a source-read decision. | Revalidate layout, StepF2 notation and participant count at preparation. |
| `pattern_job::Task::capture` and `pattern_batch::Task::capture` | Deferred the source decision to a later worker; one-stream Routine could pass. | Apply the same original source capability boundary. |
| `batch_job::Task::capture` | Grouped by the raw persisted type. | Group by effective type, excluding legacy co-op Double from the ordinary Double batch while retaining genuine one-player Double charts. |
| `pattern::generate` | Could search an aliased multiplayer layout directly. | Reject unsupported generation layouts before graph search/progress. |
| `graph::StepGraph::builtin` | Maps Routine/Couple/Team to geometrically related single-player graphs. | Unchanged; geometry availability is not an autogen capability grant. |
| `expressed::frames(grid, player, ...)` | Explicitly filters one participant. | Unchanged; useful interpretation primitive, not a whole multiplayer generation contract. |
| `PatternPreview::apply(..., player, ...)` | Explicit player-scoped application primitive. | Unchanged; it does not imply that original graph generation supports that chart family. |
| `performed::convert`, `convert_steps`, `output::assemble` | Lower-level two-foot/performance APIs have no joint-player search state. | Unchanged by this increment; editor entry points keep the original target inventory. These must not be advertised as coordinated multiplayer generation. |

No user-validated Couple/Routine playback or chart-editing path is removed. The
closed gap was an unproven acceptance of multiplayer geometry aliases in the
generation path. Genuine single-player Dance/Pump Double remains accepted.

## Incremental proof plan

### 1. Original capability boundary — implemented and validated

Centralize the source layout decision; retain the editor geometry aliases.
Reject Routine/Team/Couple even with one serialized owner or explicit
one-player metadata. Reject legacy StepF2 Double by its effective Routine type.
Keep successful task capture for all eleven original layouts.

Six new workflows cover the extracted inventory, SM/SSC refusal without
document/revision/redo mutation, all eleven accepted single-player families,
public prepared-source bypass attempts, direct pattern refusal before search,
and mixed legacy-co-op/ordinary Double batch selection. The fixture is static
source evidence; the tests exercise actual Rust generation entry points.
The integrating agent confirmed all six workflows passed, together with
`generation_job_workflows` (10), `pattern_job_workflows` (8), and
`pattern_batch_workflows` (7), in terminal session `70524`. These results prove
the capability boundary and the exercised existing generation paths; they do
not prove a multiplayer generation extension or physical runtime behavior.

### 2. Original helper capability oracle — future stronger differential proof

Extract unchanged `IsMultiPlayer`, `SupportsAutogenFeatures` and `GetStepGraph`
bodies into the existing development oracle with a minimal property/dictionary
adapter. Populate graph registrations from the original single-player list and
Fumen's actual layout properties. Execute cases across all supported editor
types and one-player metadata overrides; record source and assembly hashes.
Document that graph dictionary plumbing is an adapter, while full graph search
continues to use the original graph/differential fixtures. Unsupported source
cases have a rejection result, not invented generated multiplayer output.

### 3. Simplified native capability presentation — pending

Expose the same supported inventory in chart/pattern/batch controls and give a
localized explanation for unsupported source layouts. A geometry alias should
never make a generation button imply that coordinated Routine generation
exists. Preserve chart editing, source data and existing previews on refusal.
Exercise the explanation and task creation with headless EN/FR key workflows.

## Optional multiplayer-generation extension: design boundary

This section is a design direction, not an implemented original feature or a
reason to delay proving the original eleven-layout pipeline.

If multiplayer generation is separately requested, choose explicit semantics
before relaxing guards:

1. **Per-owner projection:** retain the whole chart's timing/event clock and
   scheduled attacks, select one owner's performance notes, then interpret that
   projection on a verified single-player geometry. This can be compared with
   the original library processing the same explicitly projected event list.
   It does not prove coordinated multi-owner output.
2. **Destination ownership:** retain explicit source/destination owner mapping;
   never combine all owners into one two-foot state or silently output P1 only.
   Empty declared participants and `MaxPlayerIndex` must survive acceptance and
   persistence. Use one atomic history gesture for accepted multi-owner results.
3. **Shared-lane safety:** two independently generated owners can collide on a
   physical lane or cross a hold. A coupled collision policy or joint search is
   required before combining results. Independent successful searches cannot
   certify a valid Routine performance. Do not overwrite other owners silently.
4. **Timing fidelity:** pauses, warps, fake segments, hold release phases, attacks
   and non-performance events must retain the original full-chart event clock.
   Recomputing a partial-owner chart clock can change first-event row times.
5. **Notation:** StepF2 has shared lanes, exclusive ownership symbols, four-owner
   and kind restrictions, and retained decorations in Rust. Explicit conversion
   policies must precede any standard/StepF2 output merge.
6. **Validation:** deterministic owner projections against the unchanged original
   expressed/performed engines; collision/overlap cases; stale preview and
   cancellation; exact undo/redo; SM/SSC save/reopen; and real editor review.
   Joint multiplayer search has no original oracle at these reference pins and
   needs its own clearly stated behavioral contract.

No production multiplayer extension is introduced by this plan.
