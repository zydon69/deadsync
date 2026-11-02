# Source actions39 callback bridge

## Delivery / validation state

`editor-source-actions39.patch` depends on contextual/platform Action143 plus the existing PatternActions model/UI. Baseline/work: `/tmp/deadsync-source-actions39/{base,work}`. It adds exactly39 registered source callback IDs, all unbound in native defaults, retaining the original settings action order. Native total182, registered source callbacks130/164 before the separate Dark lot. This count describes callbacks, not editor completion or certification of every source edge case.

`git apply --check` passed directly against primary at delivery. No Cargo, shared implementation edits, staging or commits by this child. Small Rust files formatted; large workspace/event_selection sources parsed via rustfmt stdout without reformatting their unrelated code. Tests are prepared and not executed here. The durable patch is authoritative; mapping is `editor-source-actions39-handlers.json`. Initial scaffold scripts were removed because subsequent review corrections superseded them.

## Nineteen additive selection callbacks

Eleven general presets (notes, each supported note kind, holds+rolls, miscellaneous, notes+miscellaneous, Patterns) and eight current-player note presets use exact `Preset` + optional current owner + `Mode::Add`.

The wrapper expands a valid existing checked selection to the whole chart while preserving its exact choices. With a row-range selection, that actual selection is retained. With neither, the default cursor-row capture is explicitly unchecked before expansion, so SelectAllMines cannot accidentally select unrelated taps at the cursor. Expansion/apply builds a detached candidate and only replaces UI choices on success. A stale capture refuses without dropping the old choices. Selection changes never dirty the document or enter edit history.

These callbacks work globally and from EventSelection. Local native Ctrl+A behavior remains unchanged in the native profile because the new actions start unbound; explicit source preset Ctrl+A now means original additive notes-only selection, Ctrl+Alt+A additive miscellaneous, Ctrl+Shift+A additive all. The dispatcher consumes the triggering chord once before local panel handling.

## Ten snap setters / four explicit player setters

Source snap names map to existing native indices explicitly: None->9, Quarter->0, Eighth->1, Twelfth->2, Sixteenth->3, TwentyFourth->4, ThirtySecond->5, FortyEighth->6, SixtyFourth->7, OneHundredNinetySecond->8. SetSnap changes quantization without seeking, resetting fractional focus or unnecessarily stopping playback.

SetPlayer1..4 map to exact owner0..3 through shared typed Command::SelectPlayer. An owner absent from the chart refuses; no modulo redirects player3/4 to another owner. These view-setting callbacks also work from EventSelection without replacing checked choices. Snap setters retain no-chart availability; player setters require a chart.

## Six Pattern callbacks

Four Regenerate All/Selected x Fixed/NewSeeds callbacks select exact `pattern_actions::Action` + checked scope, then invoke the same extracted preparation helper as the panel. They display a prepared review and require explicit Start. Seeds are not committed and no worker starts merely by invoking the shortcut. Missing checked selection and pending marker edits refuse explicitly. Existing model review/seed history/worker/preview safety stays in one implementation. Marker and EventSelection contexts permit these callbacks.

Two source MoveToNextPattern/MoveToPreviousPattern callbacks use the original fractional ChartPosition, strict starts and wraparound. `ChartData::adjacent_marker_at` performs bounded BTreeMap queries; nonfinite positions refuse at the Workspace API. Previous from48.5 selects48, whereas previous from48 selects the preceding marker. Existing native integer navigation remains unchanged under its separate Navigate* extension IDs. Source preset assigns original physical Ctrl/Command+P and Shift variant, neutralizing the native extensions as already specified by profile import.

Navigation preserves existing row-range endpoints and global checked choices. It opens the exact marker inspector, matching source ActiveEditorChart.SelectPattern's LastSelectedPatternEvent (not replacing global selected events). Presentation focus/receptors reset to the selected exact row. Current chart transport uses its current fractional row; sample preview does not masquerade as chart position.

Clear Selected/All have no source registered callback IDs in Editor.cs. Their existing native panel paths remain available; this patch invents no source Clear identity.

## Prepared tests

New core target `source_actions39_workflows`: **3 tests** covering all39 IDs/defaults/report/marker removal on both platforms, source physical chords, fractional marker boundaries/wrap/NaN/empty data and unchanged native integer semantics; Workspace source navigation keeps selected row range and document bytes.

Native private command_bindings module: **7 tests**:

1. All19 presets target exact kinds/current player across the whole chart; unrelated cursor notes stay unchecked.
2. Additive checked choices survive current-player preset application.
3. All10 snaps keep row and fractional focus; all4 player setters preserve exact owners and refuse absent owners.
4. Fractional pattern navigation selects the correct marker inspector and wraps without editing.
5. All4 regeneration shortcuts prepare exact checked/all scope and fixed/new mode; exact review marker counts; no seeds/jobs before explicit Apply.
6. Missing Pattern selection and stale preset expansion refuse without replacing choices or editing.
7. Actual source Ctrl/Command+A then Alt variant dispatch once from global/checked contexts in English and French.

Existing preference count143->182. Parent should include the new explicit core target and run native units serially, plus existing EN/FR PatternActions and selection-presets workflows. No claim of test execution by this child.

## Remaining source callbacks

`editor-source-actions39-remaining.json` lists34 registered IDs still unavailable at this checkpoint: 11 lane input/modifier, 6 note/event/stream navigation, 2 mixed selected row shifts, 2 timing conversions, 3 sound toggles, 2 event-position moves, 2 song asset actions, 2 file/application actions and4 general callbacks. Root's separate Dark implementation removes one of the latter once applied. Existing approximate native gestures or menus are not counted as faithful source handlers.
