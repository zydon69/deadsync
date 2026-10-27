# Short note rendering

Connected to committed chart views and detached lane drafts. Source files and the root
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
and invalid/hidden projection. All three isolated tests passed. Five native workflow tests also passed, including stopped Fake classification, draft alpha and clipping.

Native integration uses the shared XNA palette and source fake classification
for fake regions, warps and negative stops, including while stopped. Explicit
fake-note kind alone is insufficient. Viewport clipping must retain marker
centering and the existing bounded work policy. Pending lane gestures use the
same graphics/colors as committed notes, at source edit alpha 0.8. Keysounds and
attacks remain separate unsupported short-note rendering contracts.

The row projection now honors padded viewport query bounds. Ten projection tests,
37 general editor workflows, six hold/clipping workflows and 26 editor unit tests
passed after integration. These are automated actor/model checks; no live GPU
or physical keyboard validation is claimed.
