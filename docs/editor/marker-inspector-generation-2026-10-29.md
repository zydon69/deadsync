# Generate the inspected pattern

Original GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
UI/UIPatternEvent.cs:99–105 directly queues ActionAutoGeneratePatterns with the
single displayed pattern event. It does not resolve the event from Position.
The native inspector previously had13 rows without this button. Its separate
Regenerate Pattern action resolves a marker at workspace.row, so closing an
inspector opened by PatternAdded without seeking could generate another marker.

A real Generate button is appended at row13. Existing row indices stay intact.
A core Draft task validates its captured chart/revision and original marker row,
then delegates to the existing single-pattern job (which now executes the
complete original action's repeated pass). Directory profile loading remains on
the worker. Builtin capture follows the existing no-directory Task path.

The native staged-draft adaptation matches Prev/Next and Clear: pending edits
are explicitly refused with MarkerPending; no implicit Apply occurs. The
captured marker, not the cursor, chooses the generated region. Preview capture,
Cancel and Apply do not seek or alter the precise cursor. The ordinary
GenerationPreview controls still permit intentional user navigation.

Cancel and successful Apply return to the same inspector. The return owner is
bound to Session identity, chart GUID, row and content revision; only an
actual successful token-validated Apply permits the expected revision advance.
Cancel after stale/foreign content cannot resurrect a replaced/deleted marker.
Failed admission preserves the draft. Source generation performs direct async
mutation; native Preview/Apply remains its explicit review adaptation.

Tests author real raw keyboard input for the new button and Preview controls:
PatternAdded opens B at96 while cursor is0.25; preview generates96–144, Cancel
changes no bytes/history, Apply changes only B notes and keeps the peer chart,
Undo/Redo restore exact bytes and fractional cursor. Pending fields, deleted
marker, changed revision, foreign document and stale-preview Apply/Cancel are
covered. Core tests separately exercise task admission. Tests are not run by
this sidecar agent; root owns Cargo. No new source engine oracle is invented:
the shared worker's original-class54-case oracle was delivered separately.
