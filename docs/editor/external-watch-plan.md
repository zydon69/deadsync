# External simfile notifications — isolated preparation

Depends on the OutputPolicy publication epoch and the preceding
`editor-file-platform-startup.patch`. Production sources are untouched by this
lot. Native Rust tests are prepared, not executed; the source .NET oracle below
has been executed successfully.

## Contract and threading

`FileSession::external_snapshot()` captures an opaque path, immutable baseline
Arc and publication identity/epoch without IO. Snapshot `is_current` is a cheap
identity/epoch check. `probe` reads only in a worker, reuses the existing
`advanced_export::Limits::default().bytes` budget, checks cancellation between
bounded read chunks, and rejects unsupported/oversized files before allocating
large payloads when metadata makes that possible. Missing is an observation;
access failures remain IO errors, never deletion notifications.

Native Runtime schedules at most once per second while the editor can show a
notification. One process-wide permit belongs to the worker until completion,
including cancellation/unwind; dropping a State does not release a running
worker's slot early. Threads are not joined on the UI path. Standard filesystem
reads cannot promise immediate interruption of a blocked OS/network read; the
single retained permit prevents repeated spawning in that case.

Frame updates only poll a channel and compare snapshot identities/epochs. They
perform no stat/read, parse, join, or large byte comparisons. Ignore-version
comparison runs in the worker. Pending notices wait for existing panels/jobs to
finish. Own publication, Recovery install, or document replacement invalidates
old notice/ignore/worker results, even when target bytes are unchanged.

## Keyboard behavior and integrity

- Modal starts at Ignore; Up/Down/Tab selects Ignore / Reload / Disable
  notifications. Escape means Ignore this observed version.
- Ignore remembers the exact external bytes (or Missing) only for notification
  deduplication. It never installs a new save baseline or edits/deletes recovery.
  Ordinary Save still refuses the ignored external version.
- Reload targets the current snapshot path, not arbitrary MRU0. It reuses the
  File controller, captures the current source-history guard, and enters the
  existing Save/Discard/Cancel review for unsaved work. Explicit Discard has its
  separate confirmation. Failed/cancelled loads retain the old document.
- Dirty Pack remains protected by the existing Pack guard; this lot does not
  silently save or discard Pack.ini.
- Source suppression option is a staged field of the shared preference owner;
  its write is merged once into Options, preserving unrelated owners.
- Existing external-write conflict protections stay active when notifications
  are disabled or monitoring hits its size/type limit.

The source uses OS Changed events plus a three-second own-save heuristic. Native
notification instead compares exact baseline bytes and publication epoch. A
real external change during those three seconds is therefore still reported.
Deletion/recreation is explicitly observed; the source Changed-only callback
ignores Created events. The native dirty Reload review is intentionally explicit
rather than treating a rendered source warning as permission to force Reload.

## Source evidence actually executed

`editor-external-watch-reference.py` extracts unchanged complete
ShowFileChangedModal, OnSongFileChangedNotification and
CheckForShowingSongFileChangedNotification from pinned GrooveAuthor commit
48e18f71655d0f0d86144c63239331833501fda2. A .NET10 harness supplies property-only
song/preferences, immediate dispatcher and captured UI delegates. Eleven cases
cover normal/no song/saving/grace/already showing/suppressed/Ignore/dirty Reload/
Pack warning/suppression checkbox/Created event. No full EditorSong reload,
actual OS watcher, rendering, native Rust or platform thread behavior is claimed.

The generated request, actual expected output and provenance (source/method/
program/assembly hashes, SDK) are in `editor-external-watch-reference/`.

## Ten prepared Rust tests

Three core tests: ignored bytes preserve dirty state and recovery while Save
rejects conflict; own publication/another session revoke snapshots; Missing,
access/type failure, cancellation and oversize stay distinct.

Seven native tests: Ignore; dirty Reload review/cancel; stale callback after
publication; current path versus another MRU0; suppression persistence preserving
other Options owners; retained permit and unwind release; pinned source oracle
contract with the documented native timing/dirty-review adaptations. Filesystem
publication is real where tested; no publication witness is toggled.

## Following work

Standalone editor launch without an explicit Practice song is still a product
entry decision; the startup preference model alone does not implement that UI.
Pack save coupling and advanced chartless format conversion remain later lots.
