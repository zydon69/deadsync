# Keyboard bindings: source contract and port checkpoint

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
`keybind-source-inventory.json` inventories 170 binding properties: 164 command
callbacks and six mouse/scroll modifiers. Inventory is not implementation coverage.

## Current integration checkpoint

The first core and native patches are applied in the working branch. Central
core validation passed **16 tests**: registry 7, preferences/inspector/reservations
6, persistence/history 3. Native validation is pending at this checkpoint;
prepared coverage is `edition_key_bindings_workflow` 5, dispatcher units 2 and
Pack Snapshot preservation unit 1. Do not infer physical-input or rendering
certification from model/actor workflows.

The executable native subset currently has **13 actions**: Save, Undo, Redo,
Copy, Cut, Paste, Delete, previous/next measure, row-range selection, next player,
PlayPause and chart start. Defaults retain the existing native keyboard profile.
`key_commands::source` retains the separate 164-callback source inventory.

## Source contracts

- `Preferences/PreferencesKeyBinds.cs:167`: the default Ctrl alias is Command on
  macOS and LeftControl elsewhere. Bindings are lists of alternatives, each an
  ordered array of keys. PostLoad (2230–2275) removes invalid keys and retains
  an empty alternative when the list is empty.
- `KeyCommandManager.cs:230–330`: the implementation compares the final key timestamp with the
  first held-key timestamp. The comment describes final-key ordering, but the
  executable source also activates S followed by Ctrl for a Ctrl+S binding
  (both timestamps are equal to S's initial frame). Releases invoke a separate callback. Repeat starts after 0.5s
  and continues every 0.02s. A blocked-activation latch prevents releasing Ctrl
  from Ctrl+S from accidentally activating plain S while S remains held.
- `KeyCommandManager.cs:383–430`: longer chords are considered first; active
  blocking subsets suppress other commands. Rebinding suspends command input.
- `KeyCommandManager.cs:464–481`: both physical sides of Ctrl, Shift, Alt and
  Windows are aliases. In particular, ArrowModification's LeftShift default
  does **not** make it physically left-only. The native lane gesture now recognizes either physical Shift side, following
  the executed modifier-alias contract.
- `KeyCommandManager.cs:150–208,512–547`: conflict detection considers both
  directions of the ordered-prefix/subset predicate, excludes alternatives of
  the same command ID and ignores nonblocking commands. Warnings do not reject
  a binding automatically.
- `UI/UIKeyRebindModal.cs:161–210`: capture canonicalizes right modifiers to
  left and collects distinct keys in first-observed order until reset. Empty
  chords can be confirmed to disable an alternative.
- `UI/UIControls.cs:184–194,285–315`: add, remove, reset and replace operate on
  deep copies. Removing the last alternative leaves an empty one.
- `EditorActions/ActionUpdateKeyBinding.cs:32–37`: edits are global undoable
  actions and do not affect the simfile.
- `Editor.cs:1962–1979`: unfocused windows do not process input. Capturing UI
  cancels commands. Native cancellation must additionally release held visual
  and lane-gesture state; source Cancel itself does not invoke release callbacks.
- Fumen `PermissiveEnumJsonConverterFactory.cs:85–108` serializes enum keys by
  name, reads names case-insensitively and falls back to the enum default for
  unknown values. A native import needs an explicit MonoGame Keys mapping.

## Native compatibility boundary

Current native routing uses `winit::keyboard::KeyCode`, hardcoded semantic
handlers and physical digit lanes. Several source defaults conflict with native
ones: Space is PlayPause in the source but selection in native; Tab selects a
row range in the source but cycles the native tool; native playback uses Enter.
A source-default import must therefore be explicit. Preserve the current native
profile during initial migration and offer a source profile separately.

Do not replay synthetic raw keyboard events: that would alter typed text,
physical release identities and modal routing. Dispatch typed actions into the
existing semantic handlers, with context checks before activation and cleanup
before all modal guards.

## Integrated persistence and keyboard inspector

The typed application `KeyBinds` group uses immutable Arc snapshots, tracks
settings changes, preserves unknown source command fields, and merges global
Undo/Redo by owned command ID. Global preferences do not dirty simfiles and are
excluded from Save As document-history relocation. Pack Open carries the current
committed Arc; it does not silently replace unsaved settings with disk defaults.

The inspector edits detached drafts: alternatives, add, remove, unbind, reset,
capture, review, confirm, cancel, persistence and global Undo/Redo. Capture waits
for opening keys to release before recording. A separate review step permits
recording Enter; Escape is still reserved by legacy Close in this increment.
Configured actions produce localized dynamic help. Unmigrated shortcuts retain
separate help and explicit reservations.

## Runtime and source oracle

The unmodified complete `KeyCommandManager.cs`, verified against the pinned Git
blob, runs under .NET with host-only input/preferences/observer stubs through
`scripts/editor-key-command-oracle.py`. Fixtures and source hashes are in
`tests/fixtures/key_commands/source-clock.json`. S followed by Ctrl can activate
Ctrl+S: enforce executable source behavior rather than its stricter comment.

Activation suppression uses a **strict subset** relation: equal chords can
activate and repeat both actions, while conflict warnings report the duplicate.
The fixture covers seven frames for this equality case. Conflict comparison
allows equality and remains distinct from activation suppression.

Native update_with_time drives repeat timers; OS repeats only maintain physical
ownership. Modal/focus/binding changes cancel active commands and quarantine held
keys until release. Existing Control/Super/Shift masks remain authoritative.
Semantic callbacks invoke the existing operations without synthesized key events.

## Explicit limitations and next increment

Only modifiers followed by one ordinary final key are executable in this native
profile. Ordinary-key prefixes and standalone modifiers are rejected visibly.
The generic source evaluator can represent more; native routes must migrate
before their prefixes are safe. Ctrl/Cmd intentionally remains one native
accelerator; source physical Control-versus-Command customization and source-default
preset/import remain unfinished.

Legacy shortcuts have positive and negative modifier reservations. A strict
shorter subset may coexist because the longer reserved chord wins. Ambiguous
assignments which could shadow an unmigrated callback are rejected. F12,
Ctrl+F12, Ctrl+N and Escape remain analysis, Save As, New Chart and Close;
file-level New/Reload IDs are never aliases for New Chart/Recover Draft.

The prepared second increment is isolated in `patches/editor-keybind-lot2.patch` and is
**not applied or validated**: 13 additional native operations for snap, chart end,
labels, chart order, focused tabs, waveform and rate. Its next tests must validate
removed old routes, legacy sibling arbitration, source repeats and localized help.
Further categories and missing source callbacks still require implementation.


Central native validation: edition_key_bindings_workflow passed all 5 tests after explicit Arc<str>/String display conversions. Native unit and Pack Snapshot tests remain pending.
