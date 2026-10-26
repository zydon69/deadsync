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
