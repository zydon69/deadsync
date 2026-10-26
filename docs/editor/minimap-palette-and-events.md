# Minimap palette and event selection contracts

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
This preparation is not connected to the native minimap.

## Palette dispatch

`ArrowGraphicManager.CreateArrowGraphicManager` chooses the implementation from
chart type, not merely the number of players. In particular, Couple must not
silently use Routine colors.

| Chart family | Head/short | Hold/roll body | Mine |
| --- | --- | --- | --- |
| Dance Single, Double, Couple, Solo, ThreePanel; SMX ordinary | subdivision color at `GetStepColorRow()` | global hold / roll colors | global mine |
| Dance Routine; SMX Team | player-blended subdivision | player-blended hold/roll when `ColorMultiplayerHoldsAndRolls`, global otherwise | player-blended mine |
| PIU Single, Double, Couple, HalfDouble, normal PIU coloring | lane-based PIU color | lane-based PIU hold color for both hold and roll | global mine |
| PIU Routine, normal PIU coloring | player-blended lane-based color | player-blended PIU hold color for both | player-blended mine |
| PIU row-based variants | subdivision color (player blend in Routine) | global or player hold/roll according to multiplayer setting | global or player mine |

Source references: `ArrowGraphicManager.cs` factory around226, dance ordinary602,
routine681, solo816, SMX982/1126, PIU ordinary1352, Routine1421, row-based1474/1539.
PIU lane index is `(lane + StartArrowIndex) % 5`; HalfDouble uses a different
start index and must not reuse ordinary index0 mapping.

`PreferencesNoteColor.PerPlayerNoteColors` builds all cached variants:

- Normal UI multiplier1.5 and selected UI multiplier8.0. These differ from large
  chart texture multipliers (selection1.7, held1.5).
- It first packs RGB floats to bytes with truncation (`Utils.ToRGBA`), then
  multiplies/clamps each byte (`ColorRGBAMultiply`). Multiplying floats before
  byte quantization is not equivalent and can shift channels.
- Routine first blends base RGB with player RGB using alpha0.90 by default,
  then uses the same packing/multiplication. Player indices wrap over configured
  player colors. Their defaults are not the native game's existing player tint.
- PIU lane order is bottom/top/middle/top/bottom, repeated for10lanes. Hold
  colors desaturate HSV saturation by0.8 before packing. Roll shares this body.
- Row color subdivision uses reduced fraction against the source denominator,
  with unsupported subdivision falling back to192nd color. `GetStepColorRow`
  handles Stepmania row, Note row relative to measure, and Beat row relative to
  measure scaled by signature denominator/4. Source dependency constants and
  all three methods need differential fixtures before full parity claims.

Marker constants are in `MiniMap.cs`: background`0xFF1E1E1E`, rim white,
outside-content`0xFF020202`, editor region`0xFF303030`, hovered`0xFF373737`,
selected region`0xFF3E3E3E`, cursor`0xFFCCCCCC`. Label is the shared UI label
color multiplied1.5; pattern and preview use their shared UI colors with alpha255.
These marker/palette preferences must remain independent of visibility toggles.

Recommended typed adapter: immutable resolved palette snapshot keyed by chart
family, note-color preference revision and step-color method; lookup returns
`{head, body, mine}` for player/lane/color-row/selected. Pixel raster remains
independent and receives those packed colors. Do not replace these lookups with
an approximate theme tint while describing the result as original parity.

## Event enumeration

`Editor.UpdateMiniMap` around3410–3553:

1. Patterns (only when enabled): iterate by position and merge exact row-adjacent
   regions, extending end row and end chart time; stop when AddPattern says below.
2. Preview region (only when enabled): use its exact fractional positions/times.
3. Labels (only when enabled): skip row0; stop only on AddLabel BelowBottom.
4. Query crossing sustains once per lane and draw them before ordinary notes.
   Count each against the note budget, including those whose heads precede view.
5. Iterate remaining events in original chart order. Tap/Fake/Lift use short;
   Mine distinct; Hold/Roll use their full head-to-tail shape. Keysound/Attack
   do not become synthetic taps. Stop when actual Add result says BelowBottom.
6. Budget check is after adding: `numNotesAdded > maxNotesToAdd`, original
   default6144. Preserve this behavior deliberately and separately report an
   incomplete result. The raster and mesh also have independent resource guards.

New isolated `minimap_event_policy` target has3 tests for class dispatch,
post-add budget/crossing counts, label filtering and pattern adjacency; still
pending Cargo validation. It does not claim that native enumeration is complete.

## Required bounded grid API before native connection

`Grid.visible` already performs one predecessor lookup per occupied track to
include a crossing sustain, but returns track-major order. Taking its first6145
items would starve later lanes. `visible_overview_budgeted` samples heads and is
explicitly an approximation, so cannot masquerade as exact source enumeration.

Add a chronological bounded query inside `notes.rs`, where tracks are private:
one BTree range per track plus a small heap over current track heads. Crossings
are returned separately (deduplicated by player/lane/head), followed by heads in
(row, source event ordering, player/lane) order. Stop after the explicit source
budget and return an independent `complete` flag. Query count scales with tracks
and emitted heads, not all prior notes. Time-axis lower bound needs exact timing
conversion and warp-aware boundary handling; a naive inverse row can discard
simultaneous heads within a warp. Test those boundaries with real Clock data.

Native cache must include source Arc/content token, chart GUID, axis/projection,
lane geometry, palette revision, selection endpoints and player selection. Both
raster and mesh incompleteness must reach the localized loading/partial label.

## Prepared bounded query (not validated yet)

`notes/minimap_query.rs` now prepares `Grid::minimap_chronological` without a
`notes.rs` declaration until coordinated integration. It performs one predecessor
lookup and one bounded range per occupied track, then heap-merges heads. Results
separate `crossings` and `heads`, with `complete` and an explicit tree-query count.
Track-budget overflow returns an error; it does not hide later lanes behind an
apparently complete earlier-track subset. Same-row tie order is deterministic
(player, lane), with no claim to the unexamined SMEventComparer dependency.

`rows_for_chart_time` finds the first note-phase row at/after the lower time and
the last at/before the upper time, using two binary searches over the monotone
note-phase clock. Thus a collapsed warp includes all equal-time rows instead of
selecting one cursor inverse. Caller still expands raster bounds to account for
the source's extra tail pixel before querying. Three new pending tests cover
track fairness/crossings, bounded work after10,000 preceding heads, u64MAX, and
brute-force comparison against real clock windows including negative stops,
warps, delays and offsets. This does not prove GPU or palette behavior.

## Source dependencies now inspected

The restored dependency checkout at `/tmp/deadsync-grooveauthor-step-library`
is at the exact GA gitlink `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`; its
Fumen gitlink is `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`. The source confirms
48rows/beat, note-time monotonicity, and the note ordering: row, lane, underlying
Fumen event class, player. Query now applies this order for represented classes:
Tap/Fake/Lift (`LaneTapNote`), Hold/Roll (`LaneHoldStartNote`), Mine (`LaneNote`).
Keysound/Attack stay deterministic without an unsupported source-order claim.
The chronological query3, event-policy3 and raster2 tests have passed centrally.

`minimap_palette.rs` now implements immutable resolved caches for ordinary row,
Routine row and PIU lane families, including HalfDouble startindex2. The oracle
executed4,224 lookups across the3 original color presets plus modified UI
multipliers, blend and PIU saturation. It extracts the unchanged source cached
constructors, byte helpers and Fumen HSV conversions. XNA drawing `Color` is a
no-op stub (not used for asserted UI colors), fraction reduction is an exact
integer adapter, and `FloatEquals` copies the verified source epsilon expression.
Source bytes and dependency pins are checked. Palette Rust2tests are pending;
no native configuration or UI is connected yet.

## Preference/panel proposal (not implemented)

Retain original persistent groups and field names:

- `MiniMap`: visibility, ranges, axis for Variable, lane geometry and overlay
  visibility/width options.
- `NoteColor`: subdivision RGB colors, hold/roll/mine colors, UI multipliers,
  Routine blend/player colors, PIU colors/saturation, multiplayer-hold flag and
  `UseRowBasedColoringForPiu`.
- `Options`: `StepColorMethodValue` (`Stepmania`, `Note`, `Beat`).

The existing minimap panel may provide an **open colors** action leading to a
shared note-color inspector. It must not persist a private duplicate palette
inside MiniMap. Suggested keyboard pages: preset/general UI values, subdivision
colors, player colors, PIU colors. Draft/apply/cancel use the application file's
conflict-safe save and preserve unknown fields; no song dirty/history change.
The inspector should later be shared by the large partition instead of creating
a second independent definition. Clamp/validate finite RGB and numeric values,
and include palette revision in the native minimap raster cache key.

## Native foreground adapter preparation

`minimap_content_scene.rs` receives the validated viewport; it does not recalculate
it. The adapter combines caller-supplied overlays and palette with the bounded
chronological query, includes a one-pixel conservative query margin, and preserves
independent note/marker completeness flags. Native test3 is pending: crossing
hold+roll+markers, explicit partial budgets, and four equal-time warp heads.
Full original UpdateBegin background/rim/editor region and UpdateEnd fade remain
separate required composition work before connecting the entire source raster.
