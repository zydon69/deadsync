# Native pack song list

## Status

Parent applied the shared theme/Practice/shell/EN/FR hooks and bounded storage
API. Central native workflows pass: Pack song list11EN+11FR (22 total), Pack
properties16, help2 and layout3. The list fixture corrections preserve the
existing mandatory Version1 contracts. Typed application-setting inheritance and
custom preference re-read/retained-path behavior are included in the22 passes.
Shell ICU wrapper four checks passed in52997, also compiling the production
shell frame/Practice bridge. All three pack-list units passed in52424. Checkpoint3fafbc3ba records this
validated list. A subsequent final-refresh/projection assertion refinement of
the Snapshot module passed in the 26 editor units in run46215. These are automated
checks, not a GUI QA claim. Parent owns Cargo/staging/commit. No user files were
written during development. Core metadata validation is in pack-song-list-plan.md.
Old applied hook patches are historical integration aids, not pending changes.

## Files and integration

- Theme `edition/pack_song_list.rs`: modal state, readonly discovery job,
  cultural ordering request, selection, bounded visible cells, dirty confirmations,
  async strict candidate and atomic publication.
- Theme `edition/pack_song_load.rs`: background strict Workspace loader. Uses
  `FileSession::open_bounded` from the separate storage patch, not the readonly
  Metadata type. Selects the first supported source chart, or chartless workspace.
- Shell `editor_pack_collation.rs`: reuses existing ICU Collator, precomputing
  title/folder sort keys on its owning main thread. Equal keys preserve discovery
  order. No native handles move to workers and no quadratic comparison matrix is
  used in production.
- Shared patch adds State/Panel/default/update/key/actors/F1-menu hooks and
  no-chart menu entry; existing entries keep their indices. List is entry19 in
  normal F1 menu and entry8 without a chart. F1 remains the menu key.
- Practice receives `process_editor_pack_songs`; shell calls it on each Practice
  actor frame, then flushes editor audio commands. A worker may complete without
  another keypress, so raw-key-only polling is insufficient.
- Collator `sort_key` visibility becomes crate-private; no ICU behavior changes.

## Keyboard and ownership contracts

Up/Down moves a bounded selection. PageUp/PageDown moves by visible page size.
Enter/NumpadEnter prepares the selected song. Escape cancels and preserves the
current editor; F5 refreshes when no scan/open is active. A refreshed/sorted list
reconciles the selected canonical source path. Initial selection prefers the
current song when it exists in the list. No mouse or text field is required.

Every result belongs to the original SessionBinding and canonical active path.
Changing content/session drops deferred state. Language changes invalidate the
order only. Invalid cultural permutations clear the visible order; F5 retries.
Only one scan and one strict load are retained, and input cannot launch repeated
F5 work while scanning. Dropped workers receive cancellation; a synchronous
bounded parser may finish before observing it.

The native display renders at most eight rows and clips visible metadata cells
to 256 Unicode scalar values, preserving original Catalog data. Discovery issues
are currently summarized by count, not an expandable per-issue browser. There
are no search filters or deletion commands. Source GA itself has neither search
nor interactive sort headers in this table.

## Safe song transition

Viewing metadata is readonly. Enter prepares a strict bounded Workspace off the
UI thread; the current State remains live. Loading failures, cancellation or
stale row/file guards leave it unchanged. The source may have no chart.
Header-only charts can be listed but fail strict Open with the existing error.

Pack draft or pending durability blocks opening and returns to Pack properties,
using the same predicates as the existing validated close/recovery guards.
Neither a Pack receipt nor an unpublished draft is silently dropped.

For dirty song data, Keep cancels opening and stays in the old editor. Save
calls the existing Command::Save path and publishes only if the old session is
clean afterward; failed/uncertain persistence leaves it dirty and prevents
transition. Discard requires a second explicit confirmation, defaulting to Keep.
No implicit stash is created. The candidate is already fully prepared before
any discard becomes effective. File bytes and the selected row's logical target,
identity, size and modification time are rechecked before publication, including
while the dirty confirmation is displayed.

On publication, State::from_workspace has already succeeded on the main thread.
The profile directory is preserved; old transport is stopped and its pending
audio commands transferred before replacing the editor. Session-specific charts,
workspaces, waveforms and caches are initialized by the common constructor.
The surrounding Practice song remains its original gameplay context after the
editor is later closed; this increment changes the active editor document, not
the wheel/Practice gameplay selection. A later explicit shell navigation feature
would need a separate gameplay transition and its guards.

## Bounded strict opening

The existing FileSession::open path remains unchanged semantically. A shared
private finalizer handles canonical path, format and strict Document::parse.
Applied open_bounded reads chunks with an explicit 16 MiB cap, checks metadata before
reading and checks growth/cancellation before accumulating each chunk. It never
parses a partial or over-limit buffer. Cancellation cannot interrupt the existing
synchronous parser internally. The native loader checks row guards before and
after loading and exact FileSession baseline before publication.

Filesystem identity/fingerprint checks are not an OS transaction: another process
can still modify a path after the final check. The candidate retains its exact
baseline, so normal later saves retain existing external-change detection. No
claim of locking arbitrary third-party editors is made.

## Prepared validation targets

- `bounded_open_workflows`:4 (exact cap/original parity, sparse-too-large,
  cancellation, strict header-only refusal).
- `storage::bounded_open_tests`:1 deterministic chunk growth/cancellation unit.
- `edition_pack_song_list_workflow`:11 EN and
  `edition_pack_song_list_fr_workflow`:11 FR. Keyboard paths cover readonly Open,
  cancellation, Keep/Save, separate discard, strict header failure, target change
  during confirmation, Pack draft guard, chartless Open, failed save/external
  conflict, invalid cultural result/refresh.
- `screens::edition::pack_song_list` units:3 (Unicode cell work cap; stale
  source content rejects publication; typed application settings survive while
  cursor/chart caches reset).
- Shell `editor_pack_collation::tests`:1 compares both complete en-US/fr-FR
  orders against the unchanged original C# fixture, recording ICU version.

The22 native workflows above have passed; four shell-wrapper checks passed. all three units passed in52424; the later Snapshot refresh/projection refinement passed in run46215. The Pack pending
predicate is reused and reviewed; these new list workflows do not inject an
actual pending Pack receipt. Existing Pack storage and native pending tests are
separate proof and must not be described as an injected list-path test.

## Source Open comparison

At GA pin 48e18f71, UIPackProperties271 delegates to Editor.OnOpenFile5528,
which guards unsaved song/Pack changes before OnOpenFileNoSave5545.
DefaultChartListProvider uses global preferred layout/difficulty and
EditorSong.SelectBestChart. The current native loader instead chooses the first
supported source chart (or none); those original global preferences are not
implemented here. This is an explicit selection-policy difference, not proof of
full Open parity. The deferred native candidate also preserves the old editor
on failure rather than closing it before the asynchronous load as original
Editor.OpenSongFileAsync does.

The current integration additionally preserves custom preference paths and
memory-only application settings via pack_song_list/application_preferences.rs.
A candidate validates any explicit path before installation; malformed external
preferences block publication. The original preferences File is transferred only
at publication, retaining conflict baseline rather than silently rebasing on a
fresh disk read. Native EN/FR workflows verify a subsequent rate change writes
the same custom file, and malformed external preferences preserve the old editor.
This fix's EN/FR workflows passed in run7306; all three units passed in52424. The later final-refresh/projection refinement passed in run46215.

The source preferred-chart difference has a separate prepared follow-up in
open-preferences-plan.md, with an executed original oracle. It is now connected: all 14 preferred-opening EN/FR workflows and the 22
existing list workflows passed in run5269; four shell ICU checks passed in67976.

Application-setting groups added by subsequent ports must extend the typed
application Snapshot and its memory-only tests; document caches, chart IDs and
transport must continue to belong to the newly prepared session.
