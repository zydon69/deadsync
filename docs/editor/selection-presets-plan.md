# Whole-chart selection presets

## Source and real reference execution

GA pinned `48e18f71655d0f0d86144c63239331833501fda2`:
`UI/UIEditEvents.cs:909–1094`, `ActiveEditorChart.cs:1607–1644`, complete
`Selection.cs`, nine event classes' two IsSelectable methods.

Original Select All ADDS matching events, preserving unrelated selected events.
The last matching event becomes lastSelected. No matching event leaves the prior
lastSelected unchanged. Patterns belong to miscellaneous events and the combined
notes/misc preset. Holds and Rolls use the same source class with an IsRoll
predicate. Player-specific/current-player variants apply to the eight note
families. Explicit source menu Notes, Taps, Mines, Fakes, Lifts, Holds, Rolls,
HoldsAndRolls, Miscellaneous, NotesAndMiscellaneous and Patterns are all covered.

`scripts/editor-selection-presets-reference.py` executed 54 actual .NET cases:
eleven families, supported player filters (all/P1/P2), empty/preexisting selection.
It compiles five unchanged whole SelectAll methods, complete original Selection,
and original selectable predicates. Explicit inert event/tree adapters provide
event identity and ownership, but do not establish GUI or Fumen round-trip parity.
Fixtures `selection-presets.*` record source/method/adapter/program/assembly hashes.

## Native model and interaction prepared

`event_clipboard/selection/presets.rs` implements the predicates over exact Event
identities, additive source defaults and last matching event. Replace and Remove
are explicit native extensions. Notes includes native Keysound/Attack note kinds
which GA does not represent; their preservation remains a native extension.
Player filters on miscellaneous/pattern presets are rejected before any selection
change. `expand_to_chart` validates the old selection token, captures global
choices and retains exactly the previously checked identities. It selects no
additional events until the chosen preset is applied. No file/history is edited.

F3 within the mixed event checklist opens an optional five-row panel. It defaults
to whole-chart scope and Add; no implicit row-range limit is invented for source
Select All. Family/player/mode are adjusted by Left/Right, Apply returns to the
existing checklist with last matching event focused. Player controls support all,
current and explicit P1..Pn. Unsupported families constrain player selection to
all. Escape preserves previous choices. Copy/Cut/Delete still operate only on
checked identities, including any preexisting source-additive choices. Existing
checklist paging remains bounded to eight displayed rows and labels never list
all matching events.

Patches `editor-selection-presets-core.patch` and
`editor-selection-presets-native.patch` remain unapplied; new modules undeclared.
Four core tests include all 54 original membership/last-selection outcomes,
expand/Add/Replace/Remove readonly behavior, exact kind Cut/Undo, stale/foreign
refusal. Three actor/keyboard workflows per EN/FR cover global-kind Cut/Undo,
source additive plus Cancel, and Routine exact P2 ownership. Rust tests have NOT
run yet. Real .NET execution is evidence for source behavior, not Rust validation.
