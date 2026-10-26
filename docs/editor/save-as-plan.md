# Save As and editor file navigation

This is a source review and implementation plan, not an implemented feature.
Reference GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.

The preparatory media-rebasing oracle now executes the complete unchanged
Fumen.Path class and unchanged UpdateRelativePath method. It generated 21 cases
under .NET 10.0.403 on macOS, recording source/method hashes and platform.
The isolated Rust helper and its three workflows passed validation. This is not
a connected Save As operation or a Windows-runtime parity claim.

## Examined source

- `StepManiaEditor/Editor.cs:5162–5250`: Save uses the current path/format when
  available; otherwise it opens Save As. Save As asks for a destination,
  infers SM/SSC from its extension and forwards the advanced save preferences.
  Cancellation does not save. Successful save updates recent files and the
  saved action position.
- `EditorSong.cs:1372–1448`: changing file location rebases relative media
  references so they still identify the original assets. It covers music,
  preview music, lyrics, preview video and six song images. Absolute references
  become relative to the new song directory. Empty references remain empty.
- `EditorSong.cs:1696–1758`: the original queues path mutation before pre-save
  checks and output. This ordering does not establish a safe rollback contract
  for failures and must not be copied as an accidental data-loss behavior.
- Pinned Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`,
  `Fumen/Path.cs:95–134`: URI-based relative path calculation and lexical full
  path resolution. Missing assets do not require filesystem canonicalization.

## Existing reusable Rust boundaries

`storage::FileSession` already provides atomic existing-file replacement, exact
external-change detection, a cooperating-writer lock, durable writes and
recovery snapshots. A failed save does not mark the Session saved. Save As must
reuse those guarantees and retain live resident chart identities, pending grids
and history. Reopening a serialized copy as a new Session would lose those
contracts and is unsuitable.

## Next implementation increments

1. Execute the original path-rebasing methods with precise provenance for empty,
   absolute, parent-relative, Unicode, spaces, escaped characters and missing
   media cases. Distinguish platform-specific Windows namespace behavior.
2. Introduce a staged same-format Save As plan bound to source Session/content,
   the original disk baseline and a reviewed destination baseline. Prepare media
   property edits and serialization before any live state mutation. Refuse a
   newly appeared or externally changed destination instead of overwriting it
   using a stale confirmation. Keep the old source file unchanged.
3. Publish the new path, expected bytes, rebased properties and saved position
   only after durable output. Define failure behavior for rename versus directory
   sync separately; do not pretend a file was never written after a late sync
   error. Preserve or explicitly retire source recovery receipts safely.
4. Add keyboard destination selection and a concrete overwrite review. Verify
   cancel, same-file handling, pending peers/StepF2 attributes, Undo/Redo after
   relocation, stale source/destination, write/permission failures, recovery and
   subsequent ordinary Save. Verify that media still resolve to the same assets.
5. Review cross-format writer behavior and advanced options independently before
   exposing SM/SSC conversion through this path. A same-format Save As increment
   alone does not complete the original Save As or export functionality.

In-editor New/Open/Recent, pack persistence and advanced export options remain
separate required work, as recorded in `menu-comparison-next.md`.

An unresolved design boundary is media-property Undo after relocation: old
history values are relative to the old asset directory. Simply rebasing the
current document and keeping the new path makes those older values point to
different assets when undone. The storage/history adapter must preserve asset
meaning across that history, not just the current serialized output. Neither
the pure path helper nor its oracle resolves this ownership issue.

### Proposed history migration to investigate

Keep the live Session identity, chart GUIDs and both history branches. Prepare a
pure in-memory relocation patch before writing the destination. It would rebase
the current document and each stored DocumentSwap document, plus the before/after
payloads of media Value/AddedProperty changes. Value ranges must be classified in
their actual historical document context, not guessed from the current chart
indices. Walk cloned document contexts backwards/forwards; never replay external
profile or audio-preference actions merely to inspect history.

An absent Value replacement can restore a relative value from immutable source
bytes. Its rebased equivalent therefore needs an explicit replacement; changing
only existing replacement payloads is insufficient. Preserve source ranges and
unknown tags rather than reparsing and invalidating every stored range. Include
pending grids in output serialization without modifying them.

Relocation also invalidates content caches without destroying open-view identity.
A fresh consistent revision mapping for current/history/redo content tokens may
be needed. Reusing an old revision after Undo must not resurrect a clock/media
cache captured against the former asset base. Planning must also detect a new
edit followed by Undo: the visible revision can return while the redo branch has
changed, so a current revision token alone cannot bind a prepared history patch.

This approach is unimplemented and requires dedicated history proof. Same-format
Save As must remain unexposed until media edits, structural snapshots, pending
peers, both history branches and cache invalidation survive relocation. It must
also permit rescuing working edits to a different destination when the old disk
file changed, because that operation leaves the old file untouched; this is
distinct from authorizing overwrite of an externally changed destination.
