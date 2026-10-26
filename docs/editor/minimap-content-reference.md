# Minimap content raster increment

Source: GrooveAuthor commit `48e18f71655d0f0d86144c63239331833501fda2`,
`StepManiaEditor/MiniMap.cs` and `Utils.cs`. The generator verifies that source
bytes equal `git show <pin>:<path>` before extraction. Fixture hashes record
both complete files and unchanged extracted methods.

## Current evidence

The preceding viewport increment passed 20 native tests (10 English, 10 French)
and 2 minimap preference tests, with 22 shared preference regressions passing at
checkpoint `543e3d106`. The integer JSON compatibility fixture lives at
`crates/deadsync-editor/tests/fixtures/grooveauthor/minimap-preferences.reference.json`.
Its isolated .NET DTO accepts integer `uint` values and rejects decimal tokens;
it does not execute the original full preference loader.

Core work is declared and validated; native composition is still isolated and
not connected to the editor screen:

- `minimap_content.rs`: bounded pixel buffer, lane layout, horizontal short
  notes/cursors/labels, rectangles for pattern/preview markers, hold/roll head
  and body colors. Quantized and unquantized blending follow the source.
- `editor-minimap-content-oracle.py`: actually executed .NET 10 and generated
  168 cases with unchanged helper bodies, injected palette and a buffer stub.
  Each case records visibility classification, lane positions and final pixels.
- Core `--test minimap_content`: both tests passed; compares all 168 cases, crossing
  sustain body, invalid input and zero width.
- Native `--test edition_minimap_raster_mesh`: pending; same-color horizontal
  runs merge vertically into rectangles in one `Actor::Mesh`, with retained
  `Arc` vertex buffers for both orientations. The rectangle budget yields an
  explicit incomplete result. No texture service is introduced.

## Integration still required

The raster receives an injected palette. The separate declared
`minimap_palette` now matches 4,224 original C# lookups in two passing workflows;
its native preference/cache binding is still being implemented.
Roll and hold use distinct injected body colors; player and selected colors
must be resolved by the caller. C# oracle agreement proves raster arithmetic,
not event enumeration, palette, timing conversion, native draw pixels or GPU.

The native cache key must include source/content identity, focused chart GUID,
axis, mini-start (quantized and unquantized), visible range/height, lane geometry,
palette revision, selected note range and player selection. Reverse reuses the
retained reflected mesh. Do not reuse complete status from another cache key.

Use bounded head queries plus crossing sustains per lane; do not scan the whole
chart on every frame. The source counts crossing sustains against its note budget.
Expose partial/loading status separately from an actual complete map.

Source `Editor.UpdateMiniMap` also skips labels at row zero, merges immediately
adjacent pattern regions, and includes preview/pattern/label overlays only when
their independent preferences are enabled. The declared `minimap_event_policy` passed three tests covering these
enumeration rules; their native preference binding still needs integration. Keysound/attack rendering is not
silently treated as source-supported tap rendering. Preserve aggregate density
until the full lane raster/cache is connected and validated.

## Chronological queries and palette validation (2026-10-26)

`notes::minimap_query` passed three workflows: heap merging across participants
and lanes in source lane/type/player order, bounded work after dense history,
and exact inclusive note-phase windows through warps, negative stops, delays and
offsets. The query returns crossing sustains separately and reports incomplete
head budgets. It does not claim the full mixed-event comparer for nonlane events.

All ten core workflows passed: raster two, event policy three, chronological
queries three, palette two. One palette test initially assumed selected colors
would differ despite the source multiplier of eight saturating both to white.
It now explicitly checks that clamp and uses a nonsaturating multiplier for
dispatch differences; the production algorithm did not change.

## Central validation update

The following preparations have now passed centrally:

- Raster2 (including all168 original C# pixel cases), event-policy3 and bounded
  chronological query3:8coretests.
- Palette2, including4,224 original cached color lookups. The dispatch test
  explicitly proves selected saturation to white at multiplier8, then chooses
  multiplier2 to observe the underlying family difference.
- Native isolated scene3 and retained mesh2:5tests.
- Background2, including96 UpdateBegin/UpdateEnd C# cases with recorded marker
  positions. Clear buffers use the original rim/interior policy; marker calls
  are recorded separately so this oracle does not claim marker pixel drawing.

Full native State wiring remains unapplied while shared UI builds are running.
`patches/editor-minimap-native-content.patch` currently proposes the narrow
Raster buffer replacement API and reuse of the existing scene/raster, without
changing the validated viewport model. Native cache2 and shared NoteColor/Options
DTO2 are additional pending targets. No new Cargo execution was performed by
the media agent; these results are from parent-managed validation.

## Native hook review before application

The isolated native adapter now derives its diagnostic from a cache entry matching
session/content/chart and the complete current key. It returns no certification
while a panel hides the map, ShowMiniMap is false, geometry suppresses rendering,
or selection/player/palette/projection/content changed before the next draw.
Pending cache2 and State6 tests explicitly cover these transitions. The reverse
orientation clones a retained reflected vertex Arc; map boundsY are actor
translation, independent of local raster cache identity.

A source timing review found Pattern end conversion differs from note phase:
`EditorPatternEvent.GetEndChartTime` calls the pre-rate chart-position method.
The new `minimap_pattern_region` helper retains start after the Tap/Delay phase
(Fumen.Pattern is ordered immediately after LaneTapNote), and converts the end
with `Clock.fractional_chart_time`. One pending isolated test covers endpoints
on equal-row Delay, Stop and Tempo, start after Delay, overflow and nonmutation.
This correction is included in the unapplied native hook patch.

Current hooks read a single shared `NoteColor`/`Options` DTO during configuration;
rendering performs no filesystem reads. An Arc on State is the sole raw settings
snapshot. The minimap keeps resolved UI colors, and the main renderer can derive
its distinct XNA held/selected variants from the same DTO. The dedicated color
inspector/save transition remains a later required UI increment; read-only File
getters preserve existing groups without changing the audio Settings storage.

Content width reads original MiniMapWidth (90default), with existing native
right-edge mounting retained for this increment. The four original mounting
positions and density-graph-dependent offsets remain a separately required
geometry increment; the validated vertical viewport is unchanged.

### Latest pre-integration validation

Central validation has now also passed shared NoteColor/Options DTO2 and Pattern
phase1. Their core modules are declared by the parent. The unapplied integration
patch has been refreshed to omit those existing declarations; it adds only the
still-missing `minimap_content_preferences` declaration. `git apply --check`
passes against the current tree. Cache2 and State6 remain pending, and the patch
has not been applied by the media agent. Shared sources were left untouched
during the parent's holds build83823.
