# Editor save artifacts and undo history

## Save locks and song distribution

Saving an existing song or using Save As creates an empty hidden file named
`.<song filename>.deadsync-save-lock` beside the song. Concurrent DeadSync
writers use this stable file to coordinate the external-change check and atomic
publication. It is deliberately retained after unlocking. Unlinking it after a
save could let another process lock the old inode while a third locks a newly
created file, breaking that coordination.

These files contain no song data and are not read as charts by the song scanner.
Exclude `.*.deadsync-save-lock` files when preparing a pack for distribution.
Keep the stable lock files in working song directories while editor instances
may be running. Do not remove or relocate them as part of a save operation.
Recovery files and backups are separate artifacts and should be reviewed before
discarding them; they may contain unsaved work.

## Undo capacity

The history setting limits the combined number of undo and redo entries. The
default is 1,024, with a configurable range from 1 to 32,768. This is an action
count, not a memory budget. A structural edit or assistant proposal can retain
document and grid snapshots, whereas an ordinary note edit stores changed notes.

Large documents with many structural changes may therefore use substantial
memory. A smaller history setting reduces retained entries, prioritizing the
most recent past and then the nearest future. There is currently no guaranteed
byte limit for history. A memory budget would require separate sizing and
eviction rules for all history variants; the action-count setting does not
provide such a guarantee.
