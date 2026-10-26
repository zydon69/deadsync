# Minimap projection and gesture reference

Reference commit: `48e18f71655d0f0d86144c63239331833501fda2`.

`scripts/editor-minimap-oracle.py` extracts five method bodies unchanged from
`StepManiaEditor/MiniMap.cs`, verifies the checkout commit, compiles those bodies
with .NET 10, and evaluates 60 fixture configurations. The generated fixture
records the entire source-file SHA256 and individual extracted-method SHA256s.

Extracted methods:

- `ReflectScreenYIfNeeded`: reflect input around the visible reverse-view area.
- `GetYPixelRelativeToBounds`: editor rectangle projection remains unquantized.
- `GetPositionRelativeToPixel`: inverse projection uses the quantized viewport
  start when that option is enabled.
- `SetMiniMapAreaStart`: preserve the original quantization arithmetic exactly,
  including truncation and remainder behavior; do not substitute generic rounding.
- `UpdateBasedOnSelectedPixel`: preserve the grabbed percentage, clamp the minimap
  viewport inside its full range, and permit the editor region to overscroll.
  Disable dragging when the editor range is at least as wide as the minimap range.

The oracle uses fixture constructor glue for numeric fields and a Bounds.Y stub.
It does **not** instantiate the graphics-dependent MiniMap, reproduce hit testing,
select modes, event drawing, timing conversion or validate mouse/keyboard input.
The new Rust module adds finite/range guards and checked integer reflection for
malformed geometry; those protective guards are not claimed as original C# code.

Generation executed successfully with `/tmp/deadsync-dotnet10/dotnet`. The Rust
comparison target `cargo test -p deadsync-editor --locked --jobs 2 --test
minimap_projection` is **pending parent validation**. It compares six output
values per case plus reflected coordinates. The separate `minimap_navigation`
gesture target also remains pending here.

## Keyboard and pad adaptation

Original `Editor.cs::ProcessInputForMiniMap` pauses a playing transport when the
pointer captures the scrollbar, moves exact chart focus during capture, and
resumes at release unless `StopPlaybackWhenDraggingScrollBars` was enabled at
capture. Density-graph dragging uses the same policy.

The Rust Begin/Move/End model preserves that lifecycle and exact positions. The
choice of keyboard/pad focus, movement step and release binding is a simplified
interface adaptation. It must be labeled and tested as such; the C# arithmetic
oracle cannot prove those bindings. Current new modules are isolated preparation,
not connected, operable minimap functionality.

The source's repeating audio path is preview audition. No general chart A/B-loop
requirement is introduced by this minimap or preview reference.
