# Minimap mounting: source and native bridge

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.

## Evidence

`scripts/editor-minimap-mount-oracle.py` executed the unchanged original
`Editor.UpdateMiniMapBounds`, UI scaling and padding helpers under .NET10.
It verifies each source file against `git show` at the pin before extraction.
The fixture contains576 cases: four mounts, four DPI scales, three chart areas,
focus present/absent, three signed offsets and two widths.

The original chart object is replaced with supplied normal/scaled misc-event
edges. This proves bounds arithmetic, not the original chart/waveform geometry.
Rust target `minimap_mount` passed all four tests in central run91121,
including the576 differential comparisons. The native position hook is now connected; its workflow checks are recorded below.
`edition_minimap_mesh_budget` passed its CPU test in central run57250 for16384 and65536
rectangles, exactly six vertices per rectangle and explicit truncated results.
It makes no GPU claim.

## Preferences

The isolated DTO preserves original keys `MiniMapPosition`,
`MiniMapSelectMode` and `PositionOffsets`, signed offsets and unknown dictionary
entries. Defaults are LeftSideOfWindow, MoveToCursor and offsets110/110/10/10.
Those offsets are fixed source defaults; changing density-graph width does not
implicitly rewrite them. The source select modes are distinct; storing the
second mode does not implement its gesture.

## Required bridge

Source focused mounting uses `ActiveEditorChart.GetScreenSpaceXOfMiscEventsEnd`
and its scaled variant. Both add the misc-event padding and MiscEventAreaWidth
to the lane/waveform right edge. Unscaled lanes use size cap; scaled lanes use
current size zoom. Waveform width participates under its own visibility and
scale-width conditions. Multiplayer uses a wider right padding. In this pin,
`ShouldDrawMiscEvents()` returns true for every chart.

Native `view_layout::Geometry::right()` only identifies the lane field edge.
Its note centers retain lane spacing while sprite widths change with size zoom.
It cannot be substituted for either source misc-event edge. The layout bridge
must expose two explicit painted/reserved right bounds, shared by the renderer
and mounting. Supplying guessed widths here would hide a missing layout contract.

The model therefore takes `FocusEdges { without_scale, with_scale }` explicitly.
Native screen/chart insets must also be supplied explicitly: original ImGui
menu20/header21/padding10 are not the native editor's header dimensions. Existing
validated viewport geometry must remain consistent between drawing and keyboard
navigation when new mounting is applied.

Recommended integration order:

1. Declare and validate the isolated preference/bounds modules and four tests.
2. Expose a shared native chart-area/right-bounds layout contract with tests for
   zoom, multiplayer padding and waveform enabled/disabled; then use it for all
   four mounts. Window mounts do not need a focused chart edge.
3. Store preferences in the existing Settings transaction so dirty detection,
   cancellation, external-change detection and save remain coherent. A direct
   mutation of File.minimap_fields would bypass current Settings dirty checks.
4. Add the native settings fields and adapt keyboard navigation for
   MoveToSelectedArea against the source gesture, with EN/FR State tests.

No shared hooks for this increment have been applied.

## Native layout adaptation

`edition/minimap_mount_layout.rs` and its three isolated tests define the native
allocation explicitly. Waveform samples are currently constrained to each lane
channel interval in `waveform.rs`, so they add no extra right-hand reservation.
Native lane centers remain fixed while note/receptor/hold widths change with
size zoom. The scaled bound includes the last lane center plus half its scaled
width; the unscaled bound retains the original field width.

Focused labels currently receive the whole remaining screen in `edition.rs`,
whereas inactive labels have a separate smaller allocation. A finite shared misc
allocation must replace that discrepancy when mounting is connected: the same
`misc_start` and `misc_end` must control label drawing and the mount. The new
model clamps the allocation against actual available screen/panel space and
allows zero width. It does not create GA's absent independent waveform region,
nor assume multiplayer introduces an unpainted reservation.

The proposed native misc width96 is an explicit simplified-editor allocation,
not GA's MiscEventAreaWidth. It keeps the centered partition and moves advanced
controls into an optional panel, consistent with the user's approved design.
Changing that allocation is a product-layout choice, not a correction to the
original bounds oracle. The actual allocation must be tested through actors
before the integration can claim non-overlap.

Selection mode implementation and its native adaptation are documented
in `minimap-selection-reference.md`; the latest patch exposes and uses both modes.

## Integration checkpoint — 27 October 2026
The four-position mounting, two selection modes and 13-field keyboard options
are connected. Four preference transaction tests, three layout tests, one draft
test and eight native EN/FR workflows passed. Position and content share the
existing File transaction; Options is merged once with staged NoteColor values.
The native label allocation is 96px when space permits, and reserves the actual
rendered player indicator only for supported multiplayer families. No live GPU
or physical-input validation is claimed.

Twenty-six existing navigation/content EN/FR workflows also passed after
connection (run49487). Small implementation files were formatted separately;
no whole-file formatting was applied to edition.rs/history.rs/workspace.rs.
