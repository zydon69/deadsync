# Required next minimap increment

This plan follows the first ConstantTime keyboard navigation increment. That
increment is not complete GrooveAuthor minimap parity. The native tests for it
are being validated by the parent; no additional source wiring is made here.

## Source and available contracts

Pinned source: `48e18f71655d0f0d86144c63239331833501fda2`.

- `Editor.cs::GetMiniMapSpacingMode`: use the chart's ConstantTime/ConstantRow
  spacing directly; when the main chart is Variable, choose the independent
  `MiniMapSpacingModeForVariable` preference (time by default).
- `Editor.cs::UpdateMiniMap`, around 3340–3410: editor area comes from the actual
  visible viewport, focal position and pixels-per-unit after zoom. The full area
  extends chart content by one editor range at each end. The minimap's visible
  range is a separate configured value, not automatically the entire chart.
- `MiniMap.cs::SetMiniMapAreaFromEditorArea`: position that minimap viewport
  relative to the full and editor ranges. This method still needs to be added
  unchanged to the C# arithmetic oracle before claiming its port is proven.
- `MiniMap.cs::GetYPixelRelativeToBounds` and `UpdateBasedOnSelectedPixel`: the
  existing oracle proves their arithmetic on 60 configurations. Native keyboard
  binding and viewport assembly are separate adaptations.
- The parent has validated `Clock::fractional_chart_time(row:f64)` and
  `fractional_music_time(row:f64)` against original rate-event boundaries.
  ConstantRow dragging must call those functions, never floor or cast a row
  before converting it to time. A row inside a warp is not interchangeable with
  an arbitrary inverse time-to-row round trip.

## Implementation order

1. Share the exact native `view_render::Viewport` bounds/focal position with
   minimap assembly. Remove the first increment's simplified viewport height and
   quarter-height focal approximation. Extend tests for reversed display and
   short/tall windows; assert the rectangle corresponds to the rendered chart.
2. Add an explicit axis to minimap projection/cache. ConstantRow uses fractional
   focus rows and row spacing after zoom; ConstantTime uses exact chart times.
   For Variable charts apply the dedicated minimap axis preference.
3. Add configured visible ranges and `SetMiniMapAreaFromEditorArea`, preserving
   its clamp/overscroll and quantization rules. Tests must cover minimap wider
   than content, editor wider than minimap, negative lead-in and long charts.
4. On movement, retain an exact requested row/chart time/music time. In a warp,
   presentation focus must preserve the chosen row rather than replacing it
   with a different row from time inversion. This may require a source-backed
   `Position::at_fractional_row` constructor with a cached validated clock.
5. Replace the current aggregate density thumbnail with the original minimap's
   lane-specific notes and overlays. Keep bounded chunk iteration and cached
   clocks; distinguish aggregate density from note-map parity in documentation.

## Original preferences to preserve

`PreferencesMiniMap.cs` specifies:

| Property | Original default |
| --- | --- |
| ShowMiniMap | true |
| MiniMapSelectMode | MoveToCursor |
| MiniMapWidth | 90 |
| MiniMapNoteWidth / MiniMapNoteSpacing | 2 / 3 |
| MiniMapPosition | LeftSideOfWindow |
| MiniMapSpacingModeForVariable | ConstantTime |
| MiniMapVisibleTimeRange / MiniMapVisibleRowRange | 240 / 24576 |
| ShowPatterns / PatternsWidth | true / 8 |
| ShowPreview / PreviewWidth | true / 8 |
| ShowLabels | true |
| QuantizePositions | false |

Position offsets vary with mounting position and density-graph geometry. Preserve
the enum's four positions and each stored offset, unknown preference fields and
invalid-enum default handling. A simplified panel can group advanced geometry,
overlays and axis settings behind expandable sections; simplification must not
remove the settings.

SelectMode.MoveToCursor and MoveToSelectedArea share projection arithmetic but
have distinct capture/hit-test behavior. Keyboard and pad focus must express the
same destination choice explicitly. Mouse geometry tests alone do not prove
keyboard equivalence.

## Proof required

Extend the pinned C# oracle with unchanged viewport-positioning methods and the
input setup fields they actually read. Record method/source hashes and adapter
scope. Compare ConstantRow and ConstantTime native focus through delays, positive
stops, negative stops and warps; retain selected note endpoints through capture,
playback resume and subsequent Copy/Transform. Validate preferences, reverse,
multi-chart focus switches, loading progress and cache invalidation separately.

The source's audio loop is preview audition. No unrelated general chart A/B loop
is added by this plan.

## Isolated preparation status

The viewport oracle is now prepared in a separate new script,
`scripts/editor-minimap-viewport-oracle.py`. It extracts the unchanged
SetMiniMapAreaFromEditorArea and SetMiniMapAreaStart bodies and executed
successfully with .NET 10, generating 160 cases. Constructor fields are fixture
glue; this does not validate native viewport assembly or keyboard interaction.

`deadsync-editor/src/minimap_viewport.rs` and the isolated `--test
minimap_viewport` target are prepared but not wired or Cargo-validated yet.
Three tests compare the C# positioning cases, actual viewport origin/focal/reverse
assembly, and ConstantRow fractional/negative/warp conversion through the new
Clock API. Native Position must still gain a validated row-preserving constructor
before this model can be connected without losing a selected row inside a warp.
