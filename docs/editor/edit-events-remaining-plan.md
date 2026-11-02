# Edit event menus: remaining work after timing, presets and reviewed note scopes

> Historical implementation plan. The sections below describe earlier gaps,
> not current missing features. The current reviewed implementations are
> `note_operation_review.rs`, `selected_transform_review.rs`,
> `event_selection/presets.rs` and the checked timing conversion route.
> See `completion-audit-2026-11-02.json`, flow 08, for exact current source
> checks and previously executed results whose hashes still match. Physical
> keyboard/runtime validation remains distinct from this engine evidence.

Target: GA `48e18f71655d0f0d86144c63239331833501fda2`.
This is a focused dispatch/interaction review, not certification of every source
file or every native test. `reference-inventory.json` lists file hashes and is
explicitly not implementation coverage. The older menu-comparison-next.md table
is a historical checkpoint; its blanket Pack/SaveAs/SelectAll/swaps absence labels
must not be read as current state.

## Three increments now available or prepared

1. Pairwise note-kind swaps and Warp↔negative-Stop actions: models/UI implemented,
   source action/constructor references executed, exact refusal messages prepared.
2. All eleven SelectAll preset families with all/current/explicit players on note
   families: additive source behavior, whole-chart expansion, paged checklist,
   explicit native Replace/Remove. Original .NET membership/lastSelected54 cases.
3. Reviewed ConvertAll/SwapAll and exact checked note heads: isolated prepared
   model/UI/patches, all fourteen original one-way Convert methods executed56cases,
   real Fumen112format writer/readbacks. Parent Cargo validation still required.

All existing scalar note conversions are represented. Adding another independent
Tap→Mine/etc pipeline would increase drift, not close an absent source feature.

## A. Exact checked timing conversion — isolated patch prepared

**Gap:** F7 timing inspector originally supports current row range or all events;
source Convert Selected receives the exact mixed selected event identities.
A checked Warp subset must not expand to every Warp between its first/last row.

**Prepared fix:** `patches/editor-checked-timing-conversion.patch` adds a Selection
bridge extracting only checked Warp/Stop rows, validates source token/focus and
reuses the existing timing conversion Plan unchanged. Positive Stops are filtered
by the existing negative-stop eligibility. F7 in the mixed checklist opens the
same review with checked scope, returns to that checklist on Cancel and discards
its consumed stale snapshot on successful Apply. Current range/all keyboard
indices stay stable; checked scope is a third explicit option.

Prepared tests outside Cargo discovery while cross-build runs:
`/tmp/deadsync-checked-timing-core-tests.rs` (3),
`/tmp/deadsync-checked-timing-native-tests.rs` (2 per EN/FR).
They check unchecked peer preservation, positive Stop exclusion, no-op/stale/
foreign guards, save/native reopen/Undo and exact Cancel return. No Rust run yet.

**Compatibility limit:** Warp→negative Stop inside an SSC native Document is a
source editor action, but original Fumen SSC reading ignores negative Stops.
Native tokenizer save/reopen tests are not proof of target gameplay interpretation.
Advanced output preflight already blocks those Stops for SSC. Durable export UI
must use that policy, or explicitly review compatibility before publishing.

## B. Exact checked spatial transforms and player conversion

**Remaining:** the Transformations and player panels still operate on Workspace
row ranges. Source selected-head Mirror/Flip/ShiftLanes and Notes→Player/SwapPlayer
must accept the exact checked set produced by the new presets. Same-row unchecked
lanes and other players cannot be included. Existing sparse Grid, Pad mappings,
ForceAdd behavior and player models already exist; this is scope/dispatch work.

**Concrete sequence:**

1. Factor transform::apply into one internal engine with optional explicit head
   predicate. Expose apply_positions beside the existing range entry point;
   both execute the same staged Grid/remove-all/ForceInsert and owner rules.
   Avoid cloning a second independent lane-shift or mirror algorithm.
2. Provide a detached review for scoped positional/owner changes. Its report must
   distinguish selected heads moved/deleted and *unselected* notes overwritten or
   sustains trimmed by ForceAdd. Require acknowledgement of destructive effects
   before the new exact-selection UI applies. A generic "changed count" is not
   sufficient to explain those side effects.
3. Extend original action oracle coverage with sparse same-row unchecked peers,
   Routine third owner, mine ownership, hold trims, unsupported symmetry lanes,
   nonwrapping boundary deletion, row-zero deletion and zero-offset no-op.
   Prove Undo/Redo restores every side effect; existing range/force tests remain
   regression targets.
4. Route from the mixed checklist to that optional scope review. Preserve the
   current range toolbar behavior; add source commands to configurable binding
   dispatch only after the actual native callback exists.

**Do not invent behavior:** ActionChangeNotePlayer and ActionSwapNotePlayer call
plain DeleteEvents/AddEvents, not ForceAdd. Current native owner tools refuse
colliding target addresses. Their collision policy needs a targeted original
oracle before claiming exact collision equivalence; it must not be silently
changed to ForceAdd based on lane-shift behavior.

## C. Mixed Shift All — model present, interaction can be shorter

SelectAll NotesAndMiscellaneous followed by the existing Ctrl+Left/Right mixed
shift already expresses whole-chart row shifting with immutable initial BPM/
signature protection. A direct ShiftAll review shortcut can reuse that capture;
there is no absent timing/attack/pattern shift codec to build. Earlier/later
source action deletes invalid negative destinations and ForceAdd may replace
other events, so the existing moved/deleted/immutable report should be shown
before a new global shortcut confirms destructive outcomes.

The original UIEditEvents::DrawShiftEventsMenuItems passes all events to lane/
row shifts but its Mirror/Flip/MirrorAndFlip callbacks call current Selection.
That source dispatch quirk is not evidence for a source MirrorAll operation.

## D. Quick Add Event — optional shortcuts/defaults, not missing event families

Source UIEditEvents:1152–1559 lists Tempo, Speed interpolation, Scroll, Stop,
Delay, Warp, Fake region, Ticks, Combo, Signature, Label, Attack, Pattern, plus
Move Preview/End Hint. The native typed inspectors already represent these.
F2 in native sample properties already stages current-position Preview/EndHint.

Remaining quick-add work can be one small typed dispatcher over those inspectors:
source row is Math.Max(0, nearest row); same concrete type already at that row
blocks creation with a localized conflict notice; Pattern additionally requires
supported autogen layout. Default Tempo inherits active BPM. Stop/Delay/Attack
length is active seconds-per-row ×48. Other default fields must be taken from
original EventConfig factories, not guessed from native inspector defaults.
Execute original factory predicates/defaults in a small .NET reference, then
prepare an atomic snapshot-bound action and connect a compact optional menu plus
actual shortcut callbacks. Test same-row distinct types, collision refusal,
negative/fractional positions, active BPM/offset, Pattern unsupported Routine,
Undo/Redo and SM/SSC restrictions.

Preview/EndHint are deliberately **not** SelectAll miscellaneous events:
original IsSelectableWithModifiers and WithoutModifiers both return false.
Search/selected-region visuals and abstract rate nodes likewise are not selected.
Their absence from native Event identities is correct for this menu scope.

## Finish criteria for this focused menu scope

- Apply and execute all prepared model/UI tests; repair failures before marking
  any lot delivered. Keep source reference execution distinct from Rust validation.
- Connect A and B, choose the shortest existing mixed ShiftAll route or add one
  reviewed shortcut, then quick-add/default/conflict dispatch D.
- Expand configurable callbacks as they become executable; a catalog entry alone
  is not proof that a shortcut invokes a native command.
- Document deliberate native safety extensions (zero sustain/Stop refusal,
  explicit Replace/Remove, owner collision refusal) beside source-proven behavior.
- Do not call full editor or full theme suite green from these focused results.
