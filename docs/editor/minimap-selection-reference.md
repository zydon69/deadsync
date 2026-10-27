# Minimap selection modes and keyboard adaptation

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
`StepManiaEditor/MiniMap.cs`, MouseDownInternal/MouseMoveInternal and helpers.

## Original behavior

Both modes grab an existing editor rectangle at the position clicked inside it.
For an outside click, MoveToCursor grabs using the cursor's fraction of the
editor rectangle and applies scrollbar projection. MoveToSelectedArea first
moves the editor cursor to the chart position under the clicked pixel, rebuilds
the minimap window, then captures only if the editor rectangle now contains that
pixel. Subsequent motion only drags a captured rectangle. Reflection happens
once; rectangle hit testing always uses unquantized geometry.

The grabbed fraction and the cursor's fraction are different concepts. Native
Geometry uses its fraction to recover the final focused row/time, so the new
Capture model keeps the grabbed fraction separately and restores the cursor
fraction after each calculation. Conflating them would change the chart focus.

## Evidence

`scripts/editor-minimap-select-oracle.py` executed11 unchanged original methods
under .NET10, verifying the entire source file against the pinned Git bytes.
The384 cases cover both modes, reverse, quantization, three viewport/full/editor
range relationships, inside/outside map pixels, two cursor fractions and a
follow-up15pixel motion. The fixture is
`crates/deadsync-editor/tests/fixtures/grooveauthor/minimap-select.reference.json`.

`minimap_select` passed three isolated Rust tests in central run 1010:
384 differential cases, distinct outside-click destinations and capture cleanup,
and malformed/outside input. The oracle proves the original vertical
selection arithmetic with a supplied horizontal point inside the map; it does
not prove native keyboard, chart timing inversion, or GPU behavior.

## Explicit keyboard/pad adaptation

The pending native patch retains the current MoveToCursor keyboard behavior.
For MoveToSelectedArea it uses a virtual pointer for one held navigation gesture:

- A small step whose target remains in the editor rectangle simulates grabbing
  at the current cursor, then moving to that target. One tap therefore still
  navigates; it is not lost as a mouse-down with no motion.
- A distant step (Page/Home/End, or a sufficiently large step) outside the editor
  rectangle simulates a map click at that destination using the original mode.
- Repeated/held navigation moves the same virtual pointer and retains the
  original grabbed fraction. Release ends capture and clears the virtual pointer.
- Existing Begin/Move/End audio transport policy still owns pause/resume.
  Focus/document/panel teardown clears capture without delayed resume.
- Row mode retains the fractional row through warp conversion. Note selection
  remains preserved by the existing seek_preserving_selection path.

This is a documented keyboard adaptation of mouse behavior, not an assertion
that the original application had these same keyboard gestures. Both source
modes appear in the optional advanced panel; selecting one must affect the route,
not merely save an otherwise ignored preference.

Prepared native EN/FR tests compare distant destinations on a scrolling minimap,
check a useful single small step, and retain no-song-dirty assertions. These and
the preference/options tests remain unexecuted until the patches are applied at
a coordinated checkpoint. No shared source has been changed for this increment.
