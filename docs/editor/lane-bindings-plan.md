# Configurable lane entry: indivisible eleven-callback migration

## Delivery

- `editor-lane-bindings11.patch`, isolated copies `/tmp/deadsync-lane11-nosong/{base,work}`.
- Rebased onto primary Action196 (navigation6, TogglePreview, two selected timing conversions, SavePackFile/Exit and NoSong/Close). Action207 after this patch. No primary implementation edit, Cargo, staging or commit by this agent.
- `git apply --check` passed against primary. All changed Rust files parsed by rustfmt. **Rust tests have not been executed.** Parent owns compilation/validation.
- Three core tests in new auto-discovered `lane_binding_workflows`; twelve private native tests in the existing edition command-bindings test module. Existing key preference/source-profile expectations updated specifically for real callbacks and removed reservations. Unavailable-command fixtures now use still-unavailable ToggleNoteEntryMode; the shorter-callback test explicitly assigns its original test chord rather than pretending Close is unavailable.
- Existing executable reference `scripts/editor-lane-entry-oracle.py` and `lane_entry_workflows` continue to cover the unchanged note-entry engine. This lot changes its native ownership and key configuration boundary, not its transformation rules.

## Original contracts

Pinned GrooveAuthor 48e18f71655d0f0d86144c63239331833501fda2:

- Editor.cs820–830 registers Arrow0..9 with distinct OnLaneInputDown/OnLaneInputUp and ArrowModification with real OnArrowModificationKeyDown/Up; all nonrepeat.
- PreferencesKeyBinds.cs266–276 defaults lanes D1..D9,D0 and ArrowModification LeftShift. Existing key normalization equates left/right within a family, while physical Control and Super remain distinct families.
- PreferencesKeyBinds.BlocksInput explicitly returns **false for ArrowModification**. It is a callback, unlike the retained raw-only static mouse/scroll modifier preferences.
- Existing Inputs engine retains original nearest/snap rounding, current participant, preview/delete behavior, hold/roll and mine/tap transformations, playback and auto-advance immediate behavior. No synthesized physical events.

## Implementation boundaries

All eleven actions have real native dispatch. Press and Release pass through the existing native alias coalescer: duplicate alternatives cause one logical press; release happens only after the last active alternative ends. Release dispatch occurs before context filters, with cancellation having removed the draft ownership. Neither an OS repeat nor a source timer repeats these actions.

ArrowModification accepts a modifier-only chord (including a modifier sequence) as well as an ordinary final key; other actions keep existing validation. Inspector capture already records and reviews modifier-only sequences before explicit confirmation. Source imports use the same action-aware validation, preserve exact raw JSONC and normalize physical families. Registry remains unchanged; only ArrowModification bindings become nonblocking, matching the original source.

Native runtime separately tracks all supported physical keys, including modifier sides. Focus loss, suspension and reconfiguration cancel detached drafts and quarantine held inputs. A newly pressed key while quarantine is active joins that quarantine; releasing an older key must not manufacture a new activation for the newly held key. Modal cancellation includes checked-selection panels even when other configurable shortcuts remain available there.

The lane layer stores logical lane ownership independently of Inputs. This closes release ownership for immediate playback/auto-advance and explicit native tools too. Explicit tools still place on press, once. Normal default Tap mode uses the original detached engine. Owned release first reconciles current exact cursor position, then releases through the engine. Cancel/focus/modal/rebind never manufacture a commit; already completed immediate source/tool actions are not silently undone.

Native defaults retain digits and Shift modification. Native Ctrl/Alt plus an unmodified digit remains a no-op; explicitly assigned modifier chords are allowed only when that actual alternative is down. Source profile follows the original callback evaluator. Existing native Ctrl+D0 zoom and other longer configured chords keep evaluator precedence.

Only after this complete migration are the ten digit reservations removed. No raw digit Place fallback or hardcoded lane release survives. Manual receptor feedback now follows the semantic lane callbacks rather than unrelated physical digits. General help points to the existing dynamic per-action binding list and explains hold/release/cancellation. EN/FR labels cover all eleven actions. F1 rescue metadata remains protected by the existing importer.

## Test coverage pending execution

Core:

1. All eleven source registrations, exact defaults/nonrepeat, nonblocking modifier press/release and absence of legacy reservations.
2. Modifier-only action validation, physical source import normalization/raw archive and no unavailable Arrow callbacks.
3. Native Ctrl alias coalescence across physical Control/Super alternatives, no early release.

Native (twelve):

1. Configured F13 lane and F14 modifier, hold extension, roll transformation, release commit; EN/FR labels and dynamic binding help.
2. Both Shift sides: releasing one preserves modification until last side releases.
3. Two Control and two Super sides: no early lane commit before last family alternative ends.
4. Modal and focus cancellation, late releases and OS repeats cannot commit.
5. Held-key rebind cancels; unassigned digits have no legacy route.
6. Explicit native tools preserve immediate placement; repeat/release cannot toggle twice.
7. Two distinct ordinary alternatives own one lane until the final release.
8. Native default digit Tap, Shift Mine and Ctrl+plain-digit no-op.
9. New held key during quarantine cannot start when the older key releases.
10. Auto-advance immediate tap stays a tap on subsequent release.
11. Limited checked-selection modal cancels ownership while unrelated configured shortcuts remain available.
12. F1 rescue opens on its first press while cancelling a held lane draft; releases cannot commit afterward.

Run parent-owned core targets `lane_binding_workflows`, `lane_entry_workflows`, `key_binding_preferences_workflows`, `key_binding_source_profile_workflows`, existing physical/registry tests; then complete native edition tests serially in EN/FR. A manual test should cover hardware held keys during window focus loss, a modifier-only custom binding, holding lanes during playback, and all explicit note tools.

## Honest compatibility limits

Existing source imports that archived unavailable callbacks retain their raw archive; explicitly reimport/preset to apply previously unsupported custom lane chords. This lot supports modifier prefixes plus one ordinary final, and modifier-only ArrowModification, as stated by the configuration validator; arbitrary multiple ordinary-key sequences remain explicitly rejected/archived. No fake fallback drops part of a source chord.
