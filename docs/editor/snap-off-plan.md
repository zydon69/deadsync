# Snap Off: prepared native migration

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.

## Source contracts

- `SnapManager.cs:17–20`: source index 0 is Off (rows=0), followed by
  48,24,16,12,8,6,4,3,1. `SnapData.cs:21–29` displays None for Off and
  1/(192/rows) otherwise. None should be translated in the native toolbar.
- `SnapManager.cs:24–27`: invalid saved SnapIndex/SnapLockIndex normalize to Off.
  Index changes do not move the cursor (`SetSnapToLevel:79–84`).
- `SnapManager.cs:45–77`: source cycling wraps; locks allow Off regardless of
  denominator, otherwise the locked subdivision must divide by the candidate.
- `Editor.cs:6362–6404`: navigation with Off delegates to previous/next measure.
  `Editor.cs:6574–6595` adds/subtracts the measure length of the signature at
  departure. It preserves the fractional chart position, including negative
  lead-in. It does not seek a measure boundary or round to a row first.
- Lane-entry already accepts zero snap rows: press chooses the nearest row
  using the source half-down rule. Off must reach this API as zero.

## Prepared isolated model

`src/snap.rs` has explicit source/native index conversion, display denominator,
lock-aware wrapping, departure-signature navigation and pure preference map
read/write. Five `snap_workflows` tests are prepared, not executed. This module
is intentionally not declared until the native migration is ready.

Native legacy indices 0..8 retain their existing meanings; append native index9
for Off. Source preference indices use conversion, never raw native indices.
This avoids interpreting every existing SetSnap(0) caller as Off. The source
preference default is Off; changing the current native initial quarter-note
setting must be an explicit integration decision, not an incidental array edit.
No preference File storage or UI change is applied by this isolated model.

## Required coordinated changes

1. Workspace: append rows0/nameOff and guard SetSnap against modulo0. Preserve
   cursor when changing subdivision, per source. `Command::Move` must use a
   chart signature for Off; do not multiply steps by0 and silently stop moving.
2. Explicit Hold/Roll tools: the fallback tail cannot be head+0. Use an explicit
   nonzero duration policy (one row while unsnapped) and test it separately;
   source press/release gesture uses actual cursor movement instead.
3. Native playback navigation must branch before `scroll_preferences::snap_destination`,
   which rejects rows0. Off uses the focused chart Signatures and retains its
   fractional position via the existing exact focus/seek transport contract.
   Global Workspace u64 cursors cannot themselves retain that fraction.
4. Transform shift, mixed-event Shift and timing-panel increments currently
   index SNAP_ROWS directly. Define their unsnapped edit increment as one row,
   separately from the measure navigation increment; avoid zero no-op edits.
5. Toolbar displays translated Off; Left/Right cycles in source order via
   conversion rather than clamping native legacy indices. Preferences persist
   source SnapIndex/SnapLockIndex in the shared application file and preserve
   unknown fields. No second settings file.
6. Tests: native Off entry nearest-row, held gesture extension, negative and
   fractional playback navigation, chart-specific signatures, snap changes
   preserving position, invalid saved indices and old native index compatibility.

## Remaining rendering scope

The current main/draft Tap and Mine paths still use the existing highlight-arrow
texture and M placeholder. Shared held-color integration only covers sustain
sprites. It does not complete source Tap/Mine/Fake/Lift rendering; root owns
that next renderer lot.

## Prepared integration patch

`/tmp/deadsync-snap-off-native.patch` now contains the coordinated shared-file
migration. `edition/snap_controls.rs` contains keyboard cycling/persistence;
`edition_snap_off_workflow` prepares seven native tests, including precise
playback seek and negative-anchor round trips. They have not been executed.

The conversion dialog deliberately excludes Off from sustain durations, while
unsnapped discrete edit tools use one row. Missing preference keys preserve the
existing native quarter-note default; explicitly saved source index0 selects
Off. Existing files therefore retain their cursor controls until Off is chosen.
The pure source preference parser itself still normalizes invalid indices to0.

Validation update: the five core `snap_workflows` tests passed centrally in
session 61352. Checkpoint 8210ee68c declares the core module. The seven native
workflows and the shared native migration remain pending; the patch now omits
the already committed module declaration.

## Native integration checkpoint
The coordinated native bridge is applied. All seven keyboard workflows passed:
measure navigation, fractional playback seek, negative anchor, detached entry,
nonzero explicit sustain duration, persistence and conflicting preference save.
No live audio/device validation is implied.
