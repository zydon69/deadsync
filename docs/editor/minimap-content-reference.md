# Native minimap content: implementation and evidence

## Reference and scope

GrooveAuthor pin `48e18f71655d0f0d86144c63239331833501fda2` supplies
`MiniMap.cs`, `Editor.UpdateMiniMap`, note-color caches and UI byte helpers.
Its StepManiaLibrary gitlink is `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`,
with Fumen at `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89` for HSV math and
SMEventComparer. Oracle generators verify source bytes against `git show` at
the relevant pin, and record complete-file/method hashes and adapter scope.

The native content patch is applied. It preserves the previously validated
vertical viewport and navigation. Aggregate density drawing has been replaced
by a lane raster and one retained mesh. The current mounting remains at the
native right edge; original four-position mounting is a subsequent increment.
No GPU screenshot comparison or manual game session is claimed here.

## Validated centrally

All executions below were managed by the parent, with no parallel Cargo process.

| Target | Passed | Evidence |
| --- | ---: | --- |
| `minimap_content` | 2 | 168 C# visibility/lane/pixel cases; crossing sustain, invalid bounds |
| `minimap_event_policy` | 3 | supported classes, source post-add budget, labels/pattern adjacency |
| `minimap_chronological` | 3 | heap order/crossings, bounded dense history, real-clock warp boundaries |
| `minimap_palette` | 2 | 4,224 C# UI color lookups plus dispatch/selected clamp |
| `minimap_background` | 2 | 96 C# UpdateBegin/UpdateEnd background/fade cases |
| `note_color_preferences` | 2 | distinct source groups, RGB objects, preservation/validation |
| `minimap_pattern_region` | 1 | start after Tap/Delay; end before equal-row rates |
| `edition_minimap_content_scene` | 3 | crossing hold/roll, overlays, partial budgets, warp heads |
| `edition_minimap_raster_mesh` | 2 | merged runs, retained orientations, explicit rectangle limit |
| `edition_minimap_content_cache` | 2 | exact current key, binding, failures, source/selection invalidation |
| `edition_minimap_content_workflow` | 3 | native State English paths |
| `edition_minimap_content_fr_workflow` | 3 | same native State French paths |

The latest run27299 passed the cache2 and State6 tests. The preceding
viewport/navigation regression run7744 passed9/10 French cases and stopped on
an obsolete density-quad assertion; English was not executed in that run.
The assertion now checks one bounded retained mesh, all4 fixture notes and
complete query/marker/mesh flags. The20-case rerun is pending. The original integer-preference DTO oracle and its
fixture remain at
`crates/deadsync-editor/tests/fixtures/grooveauthor/minimap-preferences.reference.json`.
That oracle accepts integer uint tokens and rejects decimal tokens; it does not
execute the original complete preference loader.

## Runtime contracts

- The raster uses source short-note, horizontal-line, rectangle and hold math,
  including quantization, edge blending, clipped crossing bodies and distinct
  hold/roll/mine colors. Geometry/resource guards are explicit adaptations.
- Original background, rim, content markers, unquantized cursor/editor region
  and fade are composed before/after foreground in source order. The background
  oracle records marker positions separately; marker pixels are covered by the
  raster oracle and native composition tests, not by that background oracle.
- Exact chronological head queries replace track-major sampling. Crossings are
  separate. The source budget is checked after adding (`count > maximum`), with
  crossing sustains counted first. Query, marker and mesh completeness remain
  independent; any partial result gets a localized indicator.
- Time bounds binary-search note-phase time directly, retaining every equal-time
  warp head. Pattern starts use the after-Tap phase; Pattern ends use fractional
  pre-rate chart-position conversion, including equal-row Delay/Stop boundaries.
- Raw NoteColor/Options settings are one Arc snapshot on State. The minimap
  resolves its UI cache from that snapshot; the main chart can derive XNA
  held/selected variants from the same RGB/player/blend fields. UI byte packing
  and multipliers1.5/8 differ from XNA variants1.5/1.7.
- File getters are pure configuration reads. Rendering performs no filesystem
  reads. NoteColor and Options remain distinct application groups with original
  field names; direct import of an entire original GA preference file is not
  claimed (GA root names have the Preferences prefix).
- Frame identity includes session/content/chart binding, axis, quantized and
  unquantized projection, dimensions, selection/player, focus/capture and palette/
  options revision. Both forward and reflected meshes retain Arc buffers.
- Diagnostics return no certification for hidden/non-drawable content, a panel,
  failed build, or a frame whose source/key no longer matches current state.

## Oracle adapters and limits

The content oracle runs unchanged C# helpers on an isolated buffer with injected
colors. The palette oracle runs unchanged cached constructors, byte helpers and
HSV functions; XNA drawing Color is a no-op adapter, fraction reduction is an
exact integer adapter, and FloatEquals copies its verified epsilon expression.
Neither oracle proves GPU output, event loading as a whole, or preference UI.

Keysound/Attack are not invented as source-supported taps. Their deterministic
query ordering does not claim parity with unsupported original editor classes.
Large input can produce an explicit partial result under resource limits; this
is never presented as complete merely because a mesh was emitted.

## Remaining required work

- Original four mounting positions, per-position offsets and DPI behavior.
- Original MoveToSelectedArea capture semantics and an explicit keyboard/pad
  equivalent, with provenance separate from the original mouse input.
- Advanced geometry/overlay controls and the shared note-color inspector with
  conflict-safe apply/cancel/save; pure getters do not constitute this UI.
- Original full chart-content bounds including metadata/end hints, beyond the
  already validated initial viewport/content-bound increment.
- Runtime measurements and real native visual/input verification.

New mounting/preferences preparations are isolated and unvalidated until their
specific tests and oracle are run. Shared source changes are coordinated with the parent; mounting preparation
remains isolated until explicitly integrated.
