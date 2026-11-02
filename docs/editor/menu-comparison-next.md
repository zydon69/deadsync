# Original menu comparison and next independent increment

> Historical checkpoint. The absence claims in the table below predate the File, Pack, Save As, advanced export, selection/conversion, keybinding, Help and layout ports. They must not be used as the current completion checklist. See `resume-validation-2026-10-28.json` for scoped current evidence; remaining features must be checked against the source and current implementation.

## Scope and method

Comparison at GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
This is a bounded review of the main menu's dispatch, selected delegated menus,
the actual action for the completed mixed-shift increment, and current Rust command/API
paths. It is not a claim that every original window, shortcut or callback was
fully reviewed. Source references below are relative to `StepManiaEditor/`.
Rendering/autoplay, minimap and audio remain assigned to the other active lots.

## Menu groups: concrete evidence and current status

| Original dispatch / implementation examined | Current Rust evidence | Remaining work established by this comparison |
| --- | --- | --- |
| File, `Editor.cs:3813–3895`: New/Open/Recent/Reload/Save/SaveAs/SavePack | `storage::FileSession` exposes open/save/external-change/recovery; native editor opens from song selection and has save/close/recovery | In-editor New Song/Open/Recent and Save As have no corresponding Workspace command/native panel. Core disk safety already exists; it must be reused, not described as missing. Pack save belongs to the separate absent pack model. |
| Advanced Save Options, `Editor.cs:3897–4005`: SM identical-event policy, omit chart timing/custom data, compatibility options | Source-preserving original-format saves; SSC migration and explicit Pump co-op normalization exist | A reviewed export/options plan and Save As path are still absent. Omitting editor metadata is explicitly destructive in the original help; it must not be silently introduced as a default. The original SM policy also confirms that chart-varying timing/attacks cannot be represented independently in SM. |
| Edit Shift Selected/All, `Editor.cs:4018–4036`, `UI/UIEditEvents.cs:46–119`, `ActiveEditorChart.cs:2544–2563`, whole `ActionShiftSelectionRow.cs` | `transform::apply` selects only `session.grid(chart).iter()` note heads; the mixed picker now offers snap-sized earlier/later shifts through `event_shift` | Mixed shifting is implemented and validated: six core workflows, 19 SM/SSC comparisons from 12 original action cases, and ten native EN/FR workflows. Existing notes-only transforms remain separate. |
| Edit Convert/Swap, `UI/UIEditEvents.cs:126–409` | `Transform::Convert` and participant assignment/swap; native note conversion/owner panels | Warp ↔ negative-stop conversion is exposed in the original (`:295–305`) but not in Rust timing APIs/panels. General pairwise note-kind swaps are also absent from the native conversion choices; do not confuse existing participant swaps with kind swaps. More action-level review is required before implementing either. |
| Select All, `UI/UIEditEvents.cs:913–1094` | Mixed picker individual choices and all/none in a row interval; separate participant-aware note tools | Original current-player/by-kind/all-chart selection presets have no equivalent picker commands yet. This is an interaction/preset gap, not missing note types or clipboard codecs. |
| Add Event, `UI/UIEditEvents.cs:1154–1253` | F10 typed timing/attack inspectors, F4/F6 Pattern creation/inspection | Concrete event families are represented; keyboard quick-add shortcuts and collision notices are not certified equivalent. Do not list the whole Add Event menu as absent. |
| View/Song, `Editor.cs:4054–4084,4111–4128`; whole `UI/UIPackProperties.cs` | Song/chart text, properties, media discovery and preview controls exist | Pack properties are absent as a model and native panel. Original Pack UI also lists songs and ITGmania Pack.ini metadata; these are distinct from already implemented song media/property controls. Preferences windows vary in coverage and require their own field-by-field review. |
| Pack persistence, whole `EditorItgManiaPack.cs` and Fumen `Converters/ItgManiaPack.cs` | No editor pack model; song/profile file safety is implemented | Pack.ini creation/edit/save, dirty state and external-change workflow are genuinely absent. Metadata includes title/transliteration/sort/series/year/banner/sync. This is a separate larger filesystem/history lot, not chosen next. |
| Chart, `Editor.cs:4132–4158`; whole `ActionCopyEventsBetweenCharts.cs` | Create/duplicate/delete/list/properties; new chart-wide copy core passes nine workflows and nine original oracle cases | Native chart-wide copy validation now passed eight EN/FR workflows, plus nine core workflows and the property-helper unit. The old blanket “advanced event copy missing” label no longer describes this command. Other chart properties need field-level comparison before claiming completeness. |
| Autogen, `Editor.cs:4162–4227` | Chart conversion, source-type batch, saved-pattern fixed/new seeds, clear/previous/next and profile panels already exist | This dispatch review establishes no newly missing generation feature. Broader original behavioral coverage remains a validation task. Exactly 11 original solo graph layouts are supported; multiplayer generation would be an extension, not an original menu feature. |
| Layout/Help, `Editor.cs:4236–4270`, key-command registration `:700–800` | Simplified panel layout approved by the user; fixed keyboard adapter and help | Original docking/layout presets are not automatically required as literal copies of MonoGame UI. Configurable shortcut rebinding remains absent; its conflict/modifier/persistence behavior needs a dedicated reference review. Debug/ImGui demo/FMOD splash entries do not create user-facing parity requirements for Rust tooling. |

## Completed increment: shift the mixed event selection earlier/later

### Original gap and implementation boundary

The menu passes its selected `IEnumerable<EditorEvent>` to the editor, which
forwards it to `ActiveEditorChart::OnShiftNotesEarlier/Later` and constructs
`ActionShiftSelectionRow`. That action accepts every repositionable selected
event, not just lane notes. Rust `transform::apply` only visits the sparse Grid;
`Event::Timing`, `Event::Attack` and `Event::Pattern` are never included. The new
mixed clipboard gives a reliable typed selection boundary, but cursor-relative
cut/paste is not sufficient proof of the original shift contract.

### Original action contract examined

- Constructor (`ActionShiftSelectionRow.cs:71–95`) copies the selection,
  filters `CanBeRepositioned` and sorts it. `EditorEvent.cs:324–327` ties this to
  immutable-position protection; initial BPM/signature are protected.
- Do (`:113–164`) removes all movable originals, then computes each destination
  row. An event failing `EditorChart::CanEventExistAtRow` is **deleted by the
  shift**, not retained at the old row and not clamped to zero.
- `EditorChart.cs:3108–3119` checks negative rows, lane range and unsupported
  multiplayer Patterns. Valid moved events use the real ForceAdd path, including
  overwritten events/hold-trim side effects, and preserve relative rows.
- Undo (`:167–206`) removes moved events, undoes ForceAdd side effects, restores
  original rows, and restores events removed for invalid destination positions.
- End-of-transformation selection only includes successfully moved events on Do;
  Undo restores the complete transformable selection. Editor selection widgets
  are separate from persisted file data.

### Implementation sequence used

1. Extend the real development oracle in a **new isolated project** to execute
   the unchanged whole ActionShiftSelectionRow plus real ForceAddSideEffect,
   original CanEventExistAtRow/Pattern clone and explicit immutable-position
   adapter. Record selection-after/action/undo/redo and unchanged unsupported
   events. Do not reproduce the nontransitive Fumen comparer in Rust sorting.
2. Introduce a new domain plan bound to Session/chart identity and revision,
   operating on explicit mixed selection choices. Reuse typed note/timing/attack/
   Pattern mutation and the existing one-gesture atomicity; preserve notes not
   selected by owner/kind and keep chart-wide metadata independent.
3. Source-proven fixtures: mixed positive/negative shifts, different earliest
   heads, immutable first tempo/signature, negative-row deletion, occupied
   same-kind misc rows, Pattern replacement/overlaps, note/sustain ForceAdd
   conflicts, attack rows across rate changes, participants and StepF2 attributes,
   source stale/foreign identity, late failure, zero-shift/no-op, and exact
   undo/redo/save/reopen. Keep integer-overflow safety distinct from original
   unchecked C# arithmetic, and format limits explicit.
4. Add earlier/later controls to the existing mixed picker or a small optional
   keyboard panel, using the chosen snap interval. Show counts of moved/deleted/
   immutable events in EN/FR and verify real native input/actors. Preserve the
   established notes-only transform command rather than silently widening it.

This increment is implemented in `event_shift`, the Workspace bridge and the
native mixed picker (Ctrl/Cmd+Left/Right). Six core workflows and ten native
EN/FR workflows passed. The pinned original action supplied 12 cases and
19 serialized SM/SSC comparisons. These results concern mixed shifting only.
Pack metadata and its preserving codec separately passed six workflows; pack
filesystem storage and its native panel are still being implemented. Save As,
New/Open/Recent and advanced export remain unfinished as described above.
