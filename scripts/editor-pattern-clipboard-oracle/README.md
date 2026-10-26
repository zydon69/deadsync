# Original Pattern clipboard oracle

Generate its fixtures with:

```sh
python3 scripts/editor-pattern-clipboard-reference.py \
  --editor-root /path/to/grooveauthor \
  --reference-root /path/to/pinned/StepManiaLibrary \
  --dotnet /path/to/dotnet10 \
  --output crates/deadsync-editor/tests/fixtures/grooveauthor/pattern-clipboard
```

Build the pinned StepManiaLibrary Release project first. The script validates
the library and nested Fumen commits, then obtains original editor sources with
`git show` at the fixed editor pin.

Executed unchanged original code:

- The complete `ActionPasteEvents` and `ForceAddSideEffect` classes.
- `ForceAddEventInternal`, `CanEventExistAtRow`, `IsMultiPlayer`.
- `EditorPatternEvent.Definition`, its clone/match methods and the Pattern event
  clone override.
- Actual StepManiaLibrary/Fumen event ordering and layout defaults.

Explicit adapters supply the event list/container, row/player access, base event
cloning, input/output projection and inert selection/timing callbacks. Fixtures
use supported built-in profile GUIDs, but the adapter does not execute profile
observers, configuration managers, timing recomputation, the original GUI, or
file serialization. These limits are recorded in the provenance. Rust's separate
tests cover metadata persistence, unknown fields and atomic history behavior.

The current 15 cases exercise independent definitions, relative head positions,
same-head replacement, legitimate region overlap, multiplayer destination skips,
mixed ownership, valid/invalid destination lanes, replay/undo, and the actual
reference event order. Source marker seeds are modified after detached copying
to prove that the executed original clone keeps independent definitions.

Generated original code stays under ignored `obj/`. Fixture provenance records
source/helper/adapter/program/assembly/request/output hashes and SDK version.
