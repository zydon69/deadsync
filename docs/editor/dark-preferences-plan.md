# Dark native review and test delivery

Prepared only; no Cargo or primary source application.

## Confirmed corrections

- The core generator's plural replacement produced `File::darks()` and `set_darks`, neither exists. Corrected history generation and the Session snapshot guard to `dark()` / fallible `set_dark(target)?`.
- The native generator originally assumed the F1 menu had 26 items. It now detects the Density integration, appends Dark at index27 (28 items) and preserves Density index26. Source callbacks remain explicit Actions, no simulated keys.
- `editor-dark-after-density-check.py` builds a temporary tree, applies Density hooks there, regenerates Dark and checks both patches. This passed; `editor-dark-{core,native}-after-density.patch` are durable outputs for that order. Regenerate once primary changes.

## Source and native contracts

Source `StepManiaEditor/Preferences/PreferencesDark.cs` defines hidden by default, Window, AfterBackground and RGBA [0,0,0,.8]. Its RestoreDefaults is a non-file Undo action. Source Editor.cs2574–2579 draws overlay before or after Waveform;2750–2774 draws full viewport height and window width or active chart bounds. The native z8000/8001 order follows existing background/waveform layers; the compositor sorts by (z, insertion order), preserving overlay insertion after waveform at equal z. Chart and UI layers remain above it. Native chart bounds reuse visible lane geometry with existing side margins, an adaptation rather than MonoGame pixel parity.

Source toggles ShowDarkBg directly (Editor.cs2317), ordinary preference controls are immediate; native stages controls with preview/Save/Cancel and records one global preference action on Save/toggle. This preserves simfile bytes and makes changes undoable, but does not claim source immediate-toggle history parity. The owned-history contract currently owns a changed Color object as a whole, not individual RGBA channels; conflicting external palette edits refuse atomically.

## Tests

`editor-dark-native-tests.rs` -> theme tests/support/dark/mod.rs. Create EN/FR wrappers with `const FRENCH:bool=false/true; #[path="support/dark/mod.rs"] mod workflow;`.

Three workflows per locale: keyboard Save/persistence/Undo/Redo/reopen, staged Cancel/default restore/Undo, and overlay z/order/window/chart width/clamped RGBA with unchanged simfile. Await parent Cargo validation; these tests have not run.

Application: integrate Density first, regenerate Dark core/native, copy preferences/history/native modules, apply hooks then copy tests. Both baseline and isolated after-Density patch checks passed during preparation. No runtime or GPU result is claimed.
