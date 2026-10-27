# Save As and editor file navigation

Same-format storage/history relocation is connected and validated. The native
Save As panels and typed view-rebinding extension are the next increments.
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

## Prepared first increment: in-place history migration

New, not yet declared or Cargo-validated modules:

- `document/media_relocation.rs` clones only Document replacement maps, preserving
  source Arc, byte ranges, unknown tags, spelling maps and BOM. It rebases the ten
  direct song-media fields represented by EditorSong.UpdateRelativePaths and
  native chart-level MUSIC overrides read before song-level MUSIC.
- `history/media_relocation.rs` prepares current/snapshot Documents and typed
  before/after slot patches. Undo is walked backwards, Redo from its next entry;
  nested groups use their actual direction. DocumentSwap changes the cloned
  inspection context at exactly its history position. Notes/player counts do not
  change the inspection document; external effects are exhaustively skipped.
- `patches/editor-media-history-relocation.patch` declares these modules and
  extracts the existing serialization body into serialize_document, retaining
  its note-grid inclusion logic. No shared hooks have been applied.

Value None is resolved against immutable source[range] in the corresponding
historical document. When its media meaning changes it becomes an explicit
replacement. AddedProperty None remains absence; rebased underlying tags continue
covering fallback behavior. Unknown/non-media payloads are not normalized.

The plan binds Session identity, next_revision, current/content/saved revisions
and both stack lengths. next_revision detects a new edit followed by Undo even
when visible content/revision return. It allocates an injective mapping for all
current/history/content/saved stamps strictly beyond the largest existing stamp
and next_revision, then reserves a later next_revision. The recovered unsaved
sentinel is preserved. Old cache and revision tokens become stale while the live
Session identity and chart GUIDs remain unchanged.

Plan::publication validates before taking an exclusive mutable Session borrow.
That borrow prevents edits during the later write/publication interval. Staged
bytes already include live pending note grids. Publication changes only Document
slots and revision numbers: external Change objects and their receipts, loaded
active/alternate grids, GUID vectors and both branch entries remain in place.
The commit has no I/O, path work, serialization or fresh allocation. It does not
mark the Session saved; the future durable storage adapter owns that decision.

Ten history unit tests are prepared, not executed: original None restoration and token
invalidation; structural redo contexts/GUIDs/grids; added-media absence;
edit+Undo stale plan; unchanged external audio receipts/files; unchanged profile
deletion receipt with later Undo/Redo; StepF2 bytes plus pending peer grid;
pre-publication overflow/path failure; chart-MUSIC current/None restoration and
added override/structural peer histories. Original pure relocation3 workflows and
21case .NET helper evidence remain previously validated, separate evidence.

### Still required before Save As is exposed

The durable adapter described below is prepared but not connected. UI
confirmation and cache refresh hooks remain required. Structured BGCHANGES/FGCHANGES,
other chart-scoped/custom media references and cross-format conversion are not
covered by the direct song-media fields plus chart MUSIC. They need explicit policies and tests before claiming complete
media relocation. A new ReceptorPreferences history variant is currently pending
in another patch; the exhaustive walker must explicitly skip it when declared.


## Prepared durable storage adapter (unvalidated)

`storage/save_as.rs` and `patches/editor-save-as-storage.patch` are isolated.
No shared declaration or Session hook is applied. APIs:

- FileSession::prepare_save_as(destination) captures source ownership, staged
  relocation, canonical destination parent identity and exact destination
  baseline. SaveAsReview exposes its destination, overwrite flag and output bytes
  for a concrete confirmation. Destination reads are capped at128MiB.
- FileSession::save_as(review) validates the source/session guard, locks the
  destination sidecar and compares exact bytes/inode/parent before and after the
  temporary durable write. Absent targets use persist_noclobber. The old source
  is never rewritten or re-adopted; externally changed old bytes do not prevent
  rescuing the working state into another destination.
- A successful rename publishes the live relocated history, new path and exact
  expected bytes. Directory-sync success marks the current content saved.
  A late sync failure returns WrittenPendingDurability, explicitly invalidates
  saved_revision even for an initially clean source and retains a retry receipt.
- PendingDurability::retry borrows the receipt, verifies destination bytes/inode,
  parent identity and exact published ContentToken, syncs without rewriting,
  and marks saved only for that content. New current edits remain dirty. Undo
  back to exact written content and unrelated preference actions do not change
  the bytes certified; future song Redo still becomes dirty appropriately.
- The old recovery receipt/path are returned as PreviousRecovery; a separately
  allocated receipt owns the new path. No draft is blindly deleted.

Eleven storage tests are prepared, not run: durable SSC/history/subsequentSave;
stale plan/edit+Undo and changed destination; rescue external old source;
clean-source lateSync/retry without rewrite; edits/Undo/global preference token;
identical-byte inode or parent replacement; separate recovery ownership;
symlink/same-file/parent failure; same-format SM; reviewed existing overwrite; injected atomic-publication failure
for absent/existing targets with live state and original bytes unchanged.

The first adapter supports Unix identity guards; other platforms explicitly
refuse before writing pending an equivalent file-identity implementation.
Cooperating writers are serialized. As in the existing storage adapter, a
noncooperating writer can still race between the last baseline check and an
existing-file rename. This draft does not claim a filesystem compare-and-swap
primitive or protection against adversarial directory replacement in that gap.
It is not yet a user-facing complete Save As operation.

## Storage/history integration checkpoint — 27 October 2026
Layers A/B and explicit ignores for global Audio/Receptor/NoteColor effects
are applied. All 42 editor core units passed: 21 existing and 21 new Save As
history/storage tests. The original media helper also passed its three public
workflows. Tests cover SM/SSC same-format output, current/pending grids, chart
MUSIC overrides, both history branches, external target changes, prepublication
errors and post-rename sync failure/retry without rewriting.

Fixture corrections use canonical temporary roots on macOS, matching the
existing storage contract; the structural-undo test explicitly loads the
restored lazy grid before asserting its allocation survives relocation. No
production fallback was introduced for either fixture issue.

The adapter currently requires Unix file/directory identity. Windows support
is prepared in separate patches and not part of these executed proofs. The
UI/rebinding extensions remain pending, so no end-user Save As completion is
claimed yet. Unknown/custom media paths and cross-format export remain separate
contracts.
