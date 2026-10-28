# Source quick-add defaults

Pinned GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.

The optional timing F5 panel exposes all thirteen source Add Event types. It uses existing typed drafts, timing ownership, attack placement and pattern metadata transactions. Prepare is detached; Apply checks the chart GUID and monotone edit lifetime, then commits one history gesture. Existing same-type events at the target row are refused, different types may coexist. Pending timing inspector edits prevent opening the panel. Cancel preserves that inspector.

Defaults: active tempo; stop/delay/attack one beat; warp/fake48 rows; ticks4; combo1/1; signature4/4; label literal source content `New Label`; scroll1; interpolated scroll1 over48 rows. The inspector default interpolated length0 remains unchanged. Pattern uses builtin Sixteenths/Balanced GUIDs,192 rows, start inclusive/end exclusive, preceding distribution retained, and a newly drawn nonnegative seed on every creation. The detached plan retains that seed; Undo/Redo preserves the exact saved definition. A seed can be supplied to deterministic tests; it is not a permanent content hash.

`editor-quick-add-reference.py` executes unchanged original thirteen UI factory methods, thirteen EventConfig factory methods plus CreateDefaultModifier, original EditorPatternEvent constructor, exact-type collision predicate and original nearest-row method. Concrete Fumen event models are loaded from the pinned library. Chart identity/tempo/render host methods are explicit adapters. Result:52 factory cases (four tempos),8 nearest-row cases, collision probes, sixteen random-seed witnesses. Source hashes and adapter hash recorded. No writer interoperability claim is made by this oracle. Existing typed writer tests remain separate.

Source rounds positive exact half rows down, then Add clamps negatives to0. Native workspace presently exposes integral positions; the core adapter preserves source fractional semantics for future direct callbacks. Invalid/nonfinite/overflow positions are refused atomically.

Pattern uses the canonical eleven generation layouts. Unsupported multiplayer layouts refuse clearly. Existing typed codecs may refuse legacy versions, unsupported formats or invalid time-signature placement; this panel does not bypass those guards. Such refusals are localized, never a silent overwrite or serialization downgrade.

## Integration files

- copy `patches/editor-quick-add-core.rs` to `crates/deadsync-editor/src/quick_add.rs`; apply core.patch
- copy `patches/editor-quick-add-native.rs` to `crates/deadsync-theme-simply-love/src/screens/edition/quick_add.rs`; apply native.patch
- core-tests.rs to `tests/quick_add_workflows.rs` (five tests,52 source factory assertions)
- native-tests.rs to shared support module; EN/FR wrappers (three tests per locale)
- run targets centrally only after shared-source freeze clears

## Remaining ShiftAll routing

Do not claim source MirrorAll. UIEditEvents.DrawShiftEventsMenuItems accepts a supplied all-events list for lane/row shifts, but mirror/flip callbacks read the current checked selection. Existing global NotesAndMiscellaneous preset + mixed picker shifting already supplies all-event row motion. A dedicated ShiftAll route should reuse that exact event-shift engine and its reviewed delete/immutable counts; no new serializer or timing codec is needed.
