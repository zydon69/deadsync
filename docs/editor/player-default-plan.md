# Participant defaults across native chart families

Prepared, not applied or compiled. Shared hooks are isolated in
`player-default-hooks.patch`; four public workflows are in
`crates/deadsync-editor/tests/player_default_workflows.rs`. Existing participant
workflows need two initial-default expectations updated by the patch.

## Original evidence

`editor-player-default-reference.py` has executed 432 real-reader cases across
Dance/Pump Routine, SMX Team, Dance/Pump Couple and four Single/Double controls.
The reference files record pinned GA/SML/Fumen blobs, SDK/assembly/program hashes
and the hash of the **unchanged original MaxPlayers property** extracted from
EditorChart.cs. A minimal property host supplies actual note owners from Fumen;
full EditorChart construction is reviewed, not executed. Original constructors
start from SMCommon family defaults, then SetUpEditorEvents clamps to owners;
DeserializeV1CustomData subsequently assigns MaxPlayerIndex+1 through that setter.

272 charts load and retain raw NumPlayers across real Fumen read→write→read.
160 invalid multistream combinations for fixed families are rejected by Fumen;
fixtures retain those rejections, without turning them into invented successful
source charts. This lot does not change the existing native editor's ability to
preserve extra raw streams in such files.

Family defaults: Dance/Pump Couple and Routine and SMX Team have two players;
ordinary Single/Double controls have one. A raw one-block chart or entirely empty
block must still have that family default. Explicit metadata can lower a variable
family to one player when no P2+ note exists. The exact original setter clamps to
highest actual owner+1, not the count of distinct owners; a P4 note still requires
four players when MaxPlayerIndex=0. Fixed families ignore that metadata setting.

### Preserved native extension

Three entirely empty streams without metadata have raw Fumen NumPlayers=3,
but GA's editor defaults to two because it counts owners with notes. Native
Session and initial metadata keep the raw three streams/participants (six cases),
preserving the existing tested empty-stream workflow. This is explicitly distinct
from original editor behavior. With explicit metadata, variable families follow
the source setter and may reduce empty trailing participants to one.

## Patch contract

- `notes::default_players` resolves through the canonical shared layout table.
- `Grid::parse_layout` applies the raw family minimum after standard parsing.
  StepF2 keeps its existing early source-layout-specific decoder: a legacy Pump
  Double with only P1 X stays one player; Pump Routine starts with two.
- Session applies metadata overrides to every variable family with actual-owner
  clamping. It no longer special-cases only Pump Routine.
- `ChartData::existing_or_initial` uses parsed native participants for known
  layouts, including empty streams; opaque notes use the family fallback without
  rewriting or rejecting the metadata-only operation.
- NewChart resolves aliases canonically and uses the common creation default.
  This fixes ordinal 42/SMX Team creating one player because its string lacks
  a `team` suffix.

Tests cover all oracle cases, new charts/aliases, initial metadata, explicit one
player, P4 clamping, byte-exact Undo/Redo, serialized reopen and StepF2 Double P1.
No Cargo or GUI validation has run for this isolated patch.
