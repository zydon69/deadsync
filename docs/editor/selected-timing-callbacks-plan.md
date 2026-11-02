# Selected Warp / negative Stop callbacks

Prepared in sidecars only. No Cargo or primary edits.

## Source evidence

Pinned GA48e18f71655d0f0d86144c63239331833501fda2. UIEditEvents.cs821–833 guards focused/editable chart and nonempty focused chart selection, then passes that **exact mixed selection** with allEvents=false to the action constructor. Constructors filter Warp events or strictly negative Stops, preserving enumeration order. They do not expand the selection to a row interval or to all events. Selected positive Stops, notes and other event types are not converted.

Original Selection.cs returns a HashSet enumeration. In the pinned .NET runtime, insertion slots and LIFO removal-slot reuse determine that enumeration. Conversion arithmetic iterates OriginalEvents directly, so this order changes overlapping-event results. Existing native BTreeSet membership lost this information. New selection/order.rs retains source slot enumeration independently from sorted UI choices and membership; presets Add/Remove/Replace and expansion update/preserve it. Native presentation order remains unchanged. select_all(bool) is native SetSelectedEvents/reset semantics; source additive SelectAll callbacks continue through apply_preset(Add).

Original actions finish Do/Redo with OnNoteTransformationEnd(NewEvents) and Undo with OnNoteTransformationEnd(OriginalEvents). Editor.cs5840ff deselects ineligible mixed events, and when the transformed chart is not focused clears selections rather than changing focus. New typed history receipts implement that behavior, including collision-deleted originals restored by Undo.

## Implementation

- Existing Selection::prepare_timing_conversion and Workspace validation retain exact identity/revision checks; only enumeration changes from BTreeSet sorting to selected_events().
- Existing timing_edit::conversion engine, clock arithmetic, Stop0 atomic refusal, tiny-Stop Warp min1, format refusal and collision acknowledgement remain authoritative. No new timing engine.
- History Entry gains optional SelectionHistory (before/after effects). Conversion records it immediately after its atomic timing target action. Session emits the typed effect on apply/Undo/Redo; no action-label matching or external revision maps. Effect binds session token and chart GUID. Unrelated history emits no selection receipt. A semantic no-op emits the source selection outcome but keeps existing native no-op history/Redo behavior.
- Native source callback requires a nonempty global mixed event_selection. Absence never falls back to Workspace's range/cursor/all. It binds the requested direction and checked scope, prepares immediately, then opens collision/review row. Explicit Apply commits. Cancel returns to the original picker/global context.
- After apply/Undo/Redo the native picker choices are rebuilt from the fresh document and only the exact effect events selected, in effect order. Another focused chart clears selection without focus changes.
- Both source Actions are explicitly configurable but unassigned by native default, matching their source empty shortcuts. Source profile import now recognizes them. Current base191 (navigation6+TogglePreview), resulting193.

## Oracle and tests

scripts/editor-selected-timing-reference.py executes the entire unchanged original Selection class, exact TryGetFocusedChartSelection and two selected callbacks, both conversion routing helpers, and all four original action constructors. Recording queue is an explicit adapter; arithmetic is not reimplemented here. Six cases cover mixed selection/filter/order, empty/readonly/missing chart guards, remove/reuse slot ordering and clear/duplicate selection. Source/adapter/assembly hashes accompany selected-timing.expected.json under patches. The previous full original timing-conversion oracle covers arithmetic23 cases including selected reversed order, collisions and overlaps; constructor oracle separately covers rounded-zero Warp min1. Source Undo/OnNoteTransformationEnd contracts are directly inspected in original actions/Editor.

Core4 workflows prepared: exact mixed reversed selections plus typed Do/Undo/Redo outcomes for both directions; source slot reuse preserved by expansion and additive presets (oracle witness [192,240,144]); explicit collision review and restored collision-original selection; session-bound receipts and unrelated history isolation. Native4 tests each loop EN/FR: configured F15/F16 actual dispatch, reviewed apply and UndoRedo selections; no-selection refuses despite row range; positive Stop filtering/collision acknowledgement; foreign focused chart clears selection on Undo. Tests have not run; parent owns Cargo.

## Apply mapping

1. editor-selected-timing-order-core.rs → editor/src/event_clipboard/selection/order.rs.
2. editor-selected-timing-history-core.rs → editor/src/history/selection_effect.rs.
3. Regenerate editor-selected-timing-core-build.py and editor-selected-timing-ui-build.py against current primary, then apply their core/native patches. The native patch includes its private unit tests in timing_conversion.rs. No separate native module copy.
4. editor-selected-timing-core-tests.rs → editor/tests/selected_timing_callback_workflows.rs.
5. editor-selected-timing.expected{,.reference}.json → editor/tests/fixtures/selected-timing.expected{,.reference}.json.

Both patch checks passed against the current191-action primary during preparation. No formatting of large primary files was performed.
