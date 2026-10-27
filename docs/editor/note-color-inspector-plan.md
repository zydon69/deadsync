# Shared note-color inspector

Reference: GrooveAuthor48e18f71655d0f0d86144c63239331833501fda2.

## Source scope and controls

`UI/UINoteColorPreferences.cs:41–145` exposes nine subdivision RGBs,
hold/roll/mine RGBs, three Pump RGBs, ten player RGBs, six scalar multipliers,
two booleans and the Options time-signature method. The three source presets
(`PreferencesNoteColor.cs:105–153`) update only the nine subdivisions and
hold/roll/mine (`ApplyColorSet:1246–1260`). The extracted preset bytes were
compared against `git show` of the pinned source. NoteColor Defaults preserves
Options.StepColorMethodValue, whose own component reset uses the Options
default, Stepmania.

The connected keyboard inspector has35 fields and seven visible rows: field,
three RGB components (or one scalar/enum/boolean), Defaults, Save and Cancel.
PageUp/PageDown change fields from any row; arrows edit values, Shift permits
fine increments, Home restores one component. Ctrl+Shift+U and an appended F1
item open it. Boolean/enum repeats are ignored; only numeric arrows may repeat.
Its two preview swatches read the already prepared XNA and UI caches, with the
selected field's family/owner/type. Chart selection and document bytes are not
changed to create a preview.

## One DTO and truthful persistence

No second color DTO or preference file is added. The existing
note_color_preferences::Preferences is retained behind Arc. Draft captures
baseline/staged Arc values, installs a staged Arc into both presentation caches
for preview, restores baseline on Cancel, and creates one non-documentary
global history action on Save. No-op Save adds no history action.

File.Settings becomes Clone rather than Copy and owns the typed color Arc.
original/written/settings snapshots clone Arc values; PartialEq compares the
DTO values. Both NoteColor and Options therefore participate in has_changes.
File.save writes both groups from that typed setting through the existing
conflict-safe atomic persistence path, preserving unknown root/group/RGB
members. No mutation of root JSON circumvents dirty tracking. File.note_colors
keeps its existing Result-returning API, while note_colors_arc supplies the
shared immutable value and set_note_colors validates before replacing it.

History owns changed leaves: HoldColor.X and HoldColor.Y are separate, as are
player RGB components, scalar flags and Options.StepColorMethodValue. Undo/Redo
reopens the current file, checks only owned leaves and preserves unrelated
external edits. An owned conflict leaves disk/history unchanged. The effect is
explicitly non-documentary and must be excluded from Save As document-history
walkers like AudioPreferences/ReceptorPreferences.

The Pack Open Snapshot retains committed color Arc values. If a live draft
exists, the baseline is captured instead, so an unsaved preview cannot escape
into a replacement song. The target preference cache receives the same Arc
before the existing final refresh.

## Prepared files and validation boundaries

The following ordered patches are now applied:

1. `/tmp/deadsync-note-color-core.patch`: typed File snapshots, global effect,
   shared Arc install and Pack Open Snapshot.
2. `/tmp/deadsync-note-color-ui.patch`: keyboard panel, dispatch, translations
   and protection against copying unsaved preview into a new song. This patch
   was generated and checked on a temporary copy with patch1 applied.

New modules: note_color_inspector.rs (presets.rs and merge.rs),
note_color_preferences/history.rs, and edition/note_color_settings.rs.

Validated targets:

- note_color_inspector_workflows:5 (Arc ownership, fields/bounds, source presets,
  leaf merge and conflict).
- note_color_storage_workflows:2 (typed dirty tracking across both groups,
  snapshot isolation, unknown JSON, invalid DTO and external conflict).
- note_color_history_workflows:3 (one global action, sibling-component merge,
  owned conflict).
- edition_note_color_settings_workflow:3; the first two exercise EN and FR
  (live partition preview/cancel, RGB+UI+Options Save/Undo/Redo, external conflict).
- One private Pack Open Snapshot unit checks unsaved preview isolation.

Rustfmt parsing was performed on new files and temporary patch snapshots;
this is not a compilation or runtime claim. Root centralizes Cargo validation.

## Validation checkpoint
All ten core workflows and all three native settings workflows passed. Four
existing color workflows, the left/right Shift lane workflow and all 27 editor
units passed. Read-only Content::palette exposes the existing UI cache to the
swatch without exposing mutable fields. Both color DTOs derive PartialEq for
value-based Arc snapshots.

A broader theme-unit run is not green: parallel execution gave 41 failures;
serialized execution gave 1463 passed, 14 failed and 54 ignored. The remaining
failures are in content-browser/download/input tests, with no editor unit
failure. Full-suite success is not claimed. Detailed local diagnostic log:
`/tmp/deadsync-theme-lib-serial-20261027.log`.
