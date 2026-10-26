# Short note rendering

Prepared isolated adapter, not connected to State yet. Source files and the root
1280px atlas were checked byte-for-byte against the pinned Git blobs; hashes are
in `short-note-reference.json`. This is source inspection, not an executed C#
draw oracle or a GPU comparison.

`EditorEvent.DrawTap` emits fill then rim, rotates both using the layout manager,
quantizes fill alpha to a byte, and retains float alpha for the white rim.
`DrawFakeMarker` centers an unrotated 40px marker without selected artwork.
`EditorLiftNoteEvent.Draw` uses that fake marker instead of its lift marker when
the note is fake. `EditorMineNoteEvent.Draw` uses unrotated mine fill/rim and
overwrites fill alpha with the note alpha, then optionally draws the fake marker.

`short_note_render.rs` reuses the existing source layout/arrow resolver and atlas
rectangles; the five mine/marker rectangles come from the same verified atlas.
Three isolated tests cover composition, all supported layouts, marker precedence,
and invalid/hidden projection. All three passed central Cargo validation in run 36375; the native bridge remains unapplied.

Native integration must use the shared XNA palette and source fake classification
for fake regions, warps and negative stops, including while stopped. Explicit
fake-note kind alone is insufficient. Viewport clipping must retain marker
centering and the existing bounded work policy. Pending lane gestures need the
same graphics/colors as committed notes, at source edit alpha 0.8. Keysounds and
attacks remain separate unsupported short-note rendering contracts.
