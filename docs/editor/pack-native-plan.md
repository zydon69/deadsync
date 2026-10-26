# Native Pack.ini panel: proposed isolated bridge

## Scope and status

The native implementation is integrated in
`crates/deadsync-theme-simply-love/src/screens/edition/pack_properties.rs`, with
State dispatch/close guards in `edition.rs`, shared Unicode fields in
`edition/text_fields.rs`, chartless menu access in `edition/chart_deletion.rs`,
and help in `edition/help.rs`. EN/FR keys live in `assets/languages/en.ini` and
`assets/languages/fr.ini`.

Central validation passed all 17 Pack native tests: eight EN workflows, eight FR
workflows and one grouped EN/FR unit covering pending/release/source identity.
The 39 native regressions also passed (help two + general editor 37). The shell
`cargo check --lib` remains in progress at this update. Metadata's six tests and
storage's 13 filesystem workflows and six fault/race units passed separately
(the six units in session `84624`). All filesystem fixtures
use temporary `Pack/Song/file.ssc` trees, never user Pack.ini files.

This increment provides the seven metadata properties and explicit create/save,
with keyboard editing, cancellation, external-conflict and pending-durability
states. It does not imply pack browsing/deletion, banner scanning/decoding,
watchers, shared Save As or preference-history support.

## Integrated State hooks

In `screens/edition.rs`:

1. `mod pack_properties;`.
2. `Panel::PackProperties` (the isolated controller owns its cursor and phase).
3. `pack_properties: Option<pack_properties::Settings>`, initialized `None`.
4. One F1 menu entry, `PackPropertiesOpen`, calling `open_pack_properties()`.
   Expose it in the chartless menu too: metadata does not require a chart.
   No global shortcut is reserved or changed in this increment.
5. Panel key dispatch delegates to `pack_properties_key(code)`; panel actors
   delegate early to `pack_properties_actors(actors)`. The generic labels branch
   returns an empty list, like ChartEventCopy, because this panel draws itself.
6. Include this panel in repeated text cursor routing. The IME/text-input bridge
   uses the existing shared `text_buffer`, preedit and clipboard pipeline.
7. Main close/discard/save guards must query `pack_has_unsaved_changes()` and
   `pack_has_pending_durability()` independently from the simfile. A closed panel
   retaining a pending receipt must remain visible to those guards. Do not let
   Escape or a successful song save silently discard that Pack state. Parent's
   Save As/exit work owns this shared policy; agree the hook before application.

In `edition/text_fields.rs`, integrated after ownership coordination:

- Add `Target::Pack(pack_properties::Field)`.
- `field_at` and `text_target` query the controller's current editable field;
  `key` uses that field's Pack-specific localized label.
- `field_value` returns the loaded value or displayed empty string without
  converting absent optional metadata to `Some("")` merely by focusing a field.
- `field_supported` checks the bound controller is editable and not pending.
- `set_field_value` delegates to the controller; no song Session command runs.
- `Target::Pack` is not a song media target: Banner remains editable path text,
  without accidentally entering the song asset picker or changing song artwork.
- New native field navigation calls `sync_text` before moving/saving, then
  `focus_text`; unchanged display text does not mark null fields edited.

No Workspace changes are required. The Pack draft is independent of Session's
song/chart dirty state. No shared storage.rs mutation is required.

## Discovery, identity and defaults

The source path is `workspace.file().path()` (canonical simfile path). The song
folder is its immediate parent; the pack root is that folder's immediate parent,
matching original EditorPack.Refresh. There is no nearest-ancestor search or
fallback to the working directory. Missing parents produce a localized error.
`FileSession::open(root)` performs read-only discovery: `None` presents Create;
malformed/oversized files present an error and never become a create operation.

Settings captures `chart_order::SessionBinding` and the canonical source path.
Before editing, saving, reloading or creating, require `belongs_to(session)` and
the same file path. Chart changes and ordinary song edits remain allowed while
a different song session/path is refused. A pending receipt is retained rather
than dropped if its source is no longer current; report the identity conflict.

Explicit Create calls `FileSession::create`, which still writes no file. Values
come from `Data::new`: folder basename, banner empty (no unreviewed banner scan),
`chrono::Utc::now().year()` supplied by the native boundary, active committed
song sync from `song_properties::sync_offset(document).ok()`, and an injected
preferred-sync input. Existing native State exposes no original GA preferred
sync setting; use the existing ITG default `.009` as a documented fallback
until a real preference bridge exists, not an invented preference value.

## Controller and keyboard contract

The isolated `pack_properties.rs` owns `Settings`, `Field`, presentation state
and native methods. A separate pure controller can be tested without State.
Settings holds the source binding/path, root, optional storage FileSession,
creation defaults, current row and phase. No rendering function performs I/O.
Discovery/reload/save occur only from explicit open/key commands, never per frame.

Normal loaded/new-draft rows, in order:

1. Display title (nullable text).
2. Transliterated title (nullable text).
3. Sort title (nullable text).
4. Series (nullable text).
5. Year (integer, nonnegative edits; loaded signed values preserved untouched).
6. Banner path (nullable text).
7. Sync (NULL / ITG; unknown numeric source values displayed and preserved until
   an explicit left/right change chooses a named value).
8. Save Pack.ini.
9. Cancel changes.

Up/Down or Tab/Shift+Tab selects rows; Home/End remains text editing within a
field and navigates first/last row otherwise. Left/Right changes Sync only when
that row is focused. Enter on Save validates active text before saving. Enter
on the explicit discard row drops unpersisted edits and closes without writing.
Escape closes and retains all Pack edits, including invalid text being edited;
opening the panel again restores them. A close request for the editor opens this
panel and blocks exit until save or explicit discard resolves the draft.
Ctrl/Cmd+S saves Pack only while this panel is active. Shared Unicode selection,
clipboard and IME handling apply to the text rows. Text input is disabled for
Sync/actions and all conflict/pending modes.

Missing mode has Create Pack.ini / Cancel. Create enters the seven-field draft;
only Save publishes it. The footer states explicitly that creation is staged.
Dirty marker is driven by the Pack draft, never by the song dirty flag.

External change is checked at Save by the storage adapter, with F5 offered as
an explicit check/refresh command. A conflict never forces an overwrite. The
conflict view offers Keep editing / Reload and discard Pack edits / Cancel.
Reload has a second explicit confirmation; only a successful read replaces the
old controller. Invalid external content leaves the old draft intact. If an
explicitly confirmed reload finds absence, transition to Missing without writing.

If publication succeeded but directory sync failed, show a distinct pending
view: Retry durability / Close and keep pending edits / review explicit release. Keep the FileSession and
its receipt in State even when the panel closes. Retry calls `retry_durability`,
not `save`, and cannot overwrite an external edit. A successful receipt updates
only its saved snapshot, retaining later live edits. The release action opens a separate confirmation that defaults to keeping
local state. Confirming keeps the current disk state exactly, drops only the
local draft/receipt, and explicitly leaves durability unconfirmed. Its status
distinguishes external changes from an unchanged published file; failed external
inspection is reported as unknown. Neither save nor retry nor rollback occurs.
Enter on the keep row and Escape both cancel confirmation without closing the
panel. This provides an exit after permanent sync failure or external conflict
without pretending to undo bytes already published.

On successful save, stay in the panel with the saved fields and localized status.
Return `Durability::FileAndDirectory` versus `FileOnly` visibly in the status;
Windows is not certified by macOS execution. Pack changes must not modify song
sync or chart offsets. Pack title/banner asset refresh is a later explicit app
hook once the owning cache API is identified; do not call a song asset mutation.

## EN/FR keys to add after localization ownership is available

All keys live in `[Edition]`; proposed labels/text:

| Key | English | French |
| --- | --- | --- |
| PackPropertiesOpen | Pack properties... | Propriétés du pack... |
| PackPropertiesTitle | Pack.ini properties | Propriétés de Pack.ini |
| PackTitle | Display title | Titre affiché |
| PackTranslitTitle | Transliterated title | Titre translittéré |
| PackSortTitle | Sort title | Titre de tri |
| PackSeries | Series | Série |
| PackYear | Year | Année |
| PackBanner | Banner path | Chemin de la bannière |
| PackSync | Sync reference | Référence de synchronisation |
| PackCreate | Create Pack.ini | Créer Pack.ini |
| PackMissing | This pack has no Pack.ini. | Ce pack ne possède pas de Pack.ini. |
| PackCreationStaged | New Pack.ini: save to create the file. | Nouveau Pack.ini : enregistrez pour créer le fichier. |
| PackSave | Save Pack.ini | Enregistrer Pack.ini |
| PackCancel | Cancel changes | Annuler les modifications |
| PackSaved | Pack.ini saved; file and directory synchronized. | Pack.ini enregistré ; fichier et dossier synchronisés. |
| PackSavedFileOnly | Pack.ini saved; directory synchronization unavailable on this platform. | Pack.ini enregistré ; synchronisation du dossier indisponible sur cette plateforme. |
| PackFailed | Pack.ini could not be saved. Your edits are retained. | Impossible d’enregistrer Pack.ini. Vos modifications sont conservées. |
| PackInvalidYear | Enter a nonnegative whole year. | Saisissez une année entière positive ou nulle. |
| PackInvalidText | Values must occupy one line without NUL. | Les valeurs doivent tenir sur une ligne sans caractère nul. |
| PackSourceChanged | The active song changed. Reopen its pack properties. | La chanson active a changé. Rouvrez les propriétés de son pack. |
| PackReadFailed | Pack.ini could not be read. It has not been changed. | Impossible de lire Pack.ini. Le fichier n’a pas été modifié. |
| PackConflict | Pack.ini changed outside this editor. No overwrite was performed. | Pack.ini a changé hors de l’éditeur. Aucun écrasement n’a été effectué. |
| PackKeepEditing | Keep editing | Continuer la modification |
| PackReload | Reload and discard Pack edits... | Recharger et abandonner les modifications du pack... |
| PackReloadConfirm | Discard Pack edits and reload? | Abandonner les modifications du pack et recharger ? |
| PackPending | Pack.ini was published; durability is still pending. | Pack.ini a été publié ; sa durabilité reste à confirmer. |
| PackRetry | Retry durability | Réessayer la synchronisation |
| PackClosePending | Close and keep pending edits | Fermer en conservant les modifications en attente |
| PackHelp | Up/Down: choose, Enter: act, Ctrl+S: save, F5: check, Escape: cancel | Haut/Bas : choisir, Entrée : agir, Ctrl+S : enregistrer, F5 : vérifier, Échap : annuler |

No English domain error string should be inserted into a French user status.
Detailed errors may be logged separately through the existing diagnostic API.

## Validated native tests

EN and FR wrappers exercise the same temporary-tree scenarios:

- Discovery from a real temporary Pack/Song/file.ssc path, no incidental files.
- Explicit create then Cancel writes nothing; create/edit/save/reopen writes
  exactly Pack.ini at the pack root, preserves song bytes and sync offsets.
- All seven fields, Unicode editing, nullable unchanged focus, invalid year,
  unknown source sync preserved and explicit named-choice replacement.
- Existing metadata Cancel restores disk semantics, song dirty independent.
- External modification/deletion/appeared file, localized conflict, confirmed
  reload and invalid external content retaining the old draft.
- Pending publication state/close/reopen/retry via deterministic isolated fault
  injection; no second publication, no false clean, same snapshot acknowledged.
- Another song/session/path cannot apply a stale panel, even with similar chart
  metadata; ordinary edits of the same song do not spuriously invalidate it.
- Keyboard-only navigation and shared IME/clipboard binding after integration.

The parent applies shared State/text routing hooks and schedules central Cargo
validation. The hooks described here are applied, not outstanding work.


## Validated native targets

- `edition_pack_properties_workflow`: eight English workflows.
- `edition_pack_properties_fr_workflow`: the same eight French workflows.
- Library filter `screens::edition::pack_properties::tests`: one test grouping
  English/French pending-save success/conflict and stale source binding cases.

The native fault test injects a small private `PackFile` interface to verify
presentation and retry dispatch without creating a real I/O failure. It proves
that closing/requesting editor close retains a published receipt, retry invokes
no second publication, external conflict retains pending edits, text/save reject
a different source session, and an unpublished stale draft can still be
explicitly discarded. The real disk receipt and directory-sync fault behavior
are independently covered by the six core storage unit tests; these are not
represented as end-to-end native filesystem fault tests.

Eight workflows per locale cover both SM/SSC creation and all seven properties,
Escape/exit guard and explicit discard, optional-null focus and unknown sync,
confirmed conflict reload and malformed reload retention, invalid year across
close, appeared/deleted targets, and chartless Unicode/IME/clipboard editing.
The footer status is drawn above the opaque modal background, not hidden behind
it. Invalid text is retained separately from valid stored data until corrected
or explicitly discarded, and blocks Save while remaining visible on reopen.

When a source binding becomes stale, a read-only state offers an explicit discard
only for unpublished edits and always allows closing while keeping them. A
published pending receipt has a distinct explicit local-release confirmation,
without disk write or rollback. Shared song replacement
and app-exit paths must respect the same pending guard; they may not silently
replace this retained state.


## Session replacement and close review

The first native compile succeeded. Six of seven initial French workflows
passed; the remaining fixture expected ITG after toggling an initial ITG choice.
It is corrected to assert the actual source default `.009`, the ITG label before
Right, NULL after Right, and saved enum zero. Production sync selection was
unchanged. The corrected/new targets passed the subsequent central rerun (16 workflows).

`State::command` now rejects Recover before any execution if Pack edits are dirty
or a published receipt is pending. It opens Pack properties with EN/FR status
`PackRecoveryBlocked`. A real Ctrl+R test supplies a valid song recovery snapshot
and verifies the Session identity remains unchanged for a dirty Pack; the native
pending unit exercises the same raw key path. An additional workflow proves
clean Pack metadata still permits song recovery and remains byte-identical.
The source identity guard is retained for other programmatic replacements.

Shell bypasses found by static source review were reported to the parent:
WindowEvent::CloseRequested exits directly; Alt+F4 exits before the editor; and
Ctrl+Shift+R reloads Practice before its editor route, rebuilding Practice State.
The parent owns shell guards; this native module does not modify shell code.
The pending-after-external-change case has a validated explicit local-release
confirmation: the current disk state stays untouched, late edits/local receipt
are discarded only on confirmation, and durability remains unconfirmed.


## Explicit release validation increment

The 16 EN/FR workflows passed centrally after the Recover guard. The pending
native unit is expanded with four EN/FR cases using a real temporary Pack.ini
and the real storage external-change check, with only the pending presentation
boundary injected. It verifies cancellation through both the default keep row
and Escape, then explicit confirmation. Published/externally modified disk bytes
and modification time must remain identical; save/retry counters remain zero;
local dirty/pending state is released and a subsequent editor close is permitted.
The expected localized status differs for preserved external modifications.
This expanded grouped unit passed central execution, alongside all 16 EN/FR
workflows and 39 general/help regressions.

The temporary patch artifacts used for coordination are not implementation
files and will not be included in the final commit. The real code and test
files named above and below are the maintained sources.

The maintained tests are `tests/edition_pack_properties_workflow.rs`,
`tests/edition_pack_properties_fr_workflow.rs`, shared
`tests/support/pack_properties.rs`, and the private unit in
`src/screens/edition/pack_properties.rs`.

## Shell transition guard validation

`practice::request_editor_close` delegates to the same editor close decision.
The shell now calls it before WindowEvent::CloseRequested, Alt+F4 and practice
Ctrl+Shift+R reload. Refused transitions request a redraw and retain the editor.
`cargo check --locked --jobs 2 -p deadsync-shell --lib` passed after these changes.
The seventeen Pack native tests and thirty-nine help/general regressions passed;
OS events and physical key input were not simulated by that check.
