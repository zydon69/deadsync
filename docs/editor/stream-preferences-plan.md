# Shared Stream preference port — isolated delivery

## Exact source inventory

Pinned GrooveAuthor48e18f71655d0f0d86144c63239331833501fda2. Read the complete Preferences/PreferencesStream.cs and UI/UIStreamPreferences.cs, plus SubdivisionType.cs, StepDensity.cs comparisons and ImGui/ImGuiLayoutUtils.cs character input. There is **no PostLoad method** in PreferencesStream.

Seven business fields:

| JSON/source field | Constructor default | Native keyboard control |
| --- | --- | --- |
| NoteType | SixteenthNotes | all9 source subdivisions, arrows |
| AccumulationType | Step | Step/Row |
| ShowBreakLengths | false | boolean |
| MinimumLengthToConsiderStream | 1 | source UI0..8 |
| ShortBreakCutoff | 4 | source UI0..64, Shift increments8 |
| ShortBreakCharacter | '-' | committed BMP text, Backspace/Delete space, Home default |
| LongBreakCharacter | '\|' | committed BMP text, Backspace/Delete space, Home default |

ShowStreamPreferencesWindow is an eighth persisted source field controlling only the source window; it is preserved as opaque data and never included in business RestoreDefaults/history. Native panel opening uses native state. RestoreDefaults resets all seven fields. Save/Cancel is the native staged inspector adaptation, while source controls commit immediately. Save produces one global non-file history action; the three footer actions are defaults/save/cancel. Panel has10 rows at480px. F1 final item; Density and Dark previous menu slots retained.

## Model, converter and persistence

`stream_preferences::Preferences` is authoritative for the shared Stream group. `File::streams()/set_streams()` load/save its fields, preserving unknown Stream fields and other preference owners. The existing `File::density_stream()` and `density::StreamSettings::read()` now derive from the same model. The source NoteType converter has a subtle distinction: missing value keeps constructor Sixteenth, invalid strings/nonstring tokens use enumzero Quarter. Case-insensitive names and defined numeric strings are supported. Numeric JSON tokens, even2, default to Quarter; the real Fumen converter does not parse them. The Step/Row converter behaves similarly. Undefined numeric strings would create invalid enum indices and crash the source subdivision array; native refuses those imports explicitly.

Source integer imports are signed i32 and can exceed the UI drag bounds; model retains them verbatim. Density uses nonnegative lengths, so negative minimum/cutoff map to0: all actual stream lengths and interstream gaps are nonnegative, preserving the source comparisons. Source char imports include controls; those remain stored, with escaped display in the inspector. Native committed text accepts one noncontrol BMP scalar, matching the representable source UTF-16 char domain without accepting lone surrogates. Supplementary characters refuse. Multi-character committed text uses its first representable character, as source DrawCharInput takes s[0].

Undo merges only changed leaves into a freshly opened preference file. Unrelated Stream fields, unknown JSON, Audio/Density/Dark groups survive; a changed owned leaf refuses atomically. Existing session history keeps this change out of chart dirty state. The update merges back into native File and shared source-density runtime, invalidating an outdated analysis binding. The independent RSSP panel remains unchanged. Source notifications are adapted by explicit refresh after configuration/Save/Undo/Redo; no observer protocol is claimed.

Sibling ordering agent confirmed source MoveToStreamBoundary does **not** consume these preferences; it follows consecutive step-row spacing. No navigation/density equivalence was introduced.

## Evidence and tests

`scripts/editor-stream-reference.py` executes whole original PreferencesStream/SubdivisionType with real Fumen Notifier, PermissiveEnumJsonConverterFactory and Undo action/queue. Ten cases cover defaults, all seven changed fields, invalid strings/numeric tokens, case-insensitive and numeric-string enums, signed/maxi32 values, UTF-16 BMP/control marks, and source window-state preservation through defaults/Undo/Redo. Fixtures and source/adapter/assembly/Fumen hashes are durable under patches/editor-stream.expected{,.reference}.json. Source UI is inspected, not rendered in this oracle.

Prepared core6 tests cover those10 cases, per-field external merges, owned conflict atomicity, no-op/Redo/non-dirty behavior, signed density adaptation and malformed/unrepresentable imports. Native4 per locale cover Save/density recomputation/UndoRedo/reopen, all7 keyboard fields and BMP committed text, Cancel/Restore/per-leaf Undo preserving another field, plus empty-editor Density/Dark/Stream menu routes. They have **not run**; parent owns Cargo.

## Application order and mapping

First integrate Density, then Dark. `editor-stream-check.py` copies required sources to a temporary tree, optionally applies missing Density/Dark hooks, generates/checks Stream patches; no primary edits. Last check passed on current primary containing both. Regenerate after other source changes.

1. `editor-stream-preferences-core.rs` → editor/src/stream_preferences.rs.
2. `editor-stream-history-core.rs` → editor/src/stream_preferences/history.rs.
3. `editor-stream-native.rs` → theme/src/screens/edition/stream_settings.rs.
4. Apply editor-stream-core.patch then editor-stream-native.patch (generators accept --base).
5. `editor-stream-core-tests.rs` → editor/tests/stream_preferences_workflows.rs.
6. `editor-stream.expected{,.reference}.json` → editor/tests/fixtures/stream.expected{,.reference}.json.
7. `editor-stream-native-tests.rs` → theme/tests/support/stream_preferences/mod.rs; wrappers edition_stream_preferences[_fr]_workflow.rs declare const FRENCH=false/true and #[path="support/stream_preferences/mod.rs"] mod workflow.

No primary changes, Cargo or staging were performed for this delivery.
