# File menu: New / Open / Recent / Reload

## Source inventory (GrooveAuthor 48e18f71655d0f0d86144c63239331833501fda2)

- `Preferences/SavedSongInformation.cs`: SavedChartInformation exact match compares type, difficulty, rating, name and description; permissive match compares type+difficulty. Names from deprecated records can be null, distinct from empty strings.
- `GetChartToUseForFocusedChart`: focused exact, focused type+difficulty, other active exact (saved order), other active type+difficulty, then SelectBestChart. `GetChartsToUseForActiveCharts` preserves saved order, treats focused record through that fallback chain, and deduplicates chart object identity. It may return an empty active list while a focused fallback exists.
- `PostLoad`: migrate deprecated LastChartType/LastChartDifficultyType only when active records are absent/empty, focus zero, clear deprecated fields. Persistence/migration is a separate next increment; the first native internal descriptor DTO is not advertised as the source JSON schema.
- `Editor.cs:5433–5474`: lookup file path by exact string equality; remove every equal MRU entry, insert current song first, cap Options.RecentFilesHistorySize (default20). Capture active chart descriptors, focused active-list index, spacing zoom and fractional chart position.
- `Editor.cs:5512–5621`: Open and Recent perform unsaved-song/pack review, respect SaveLoadCloseEarlyOut, use preferred defaults for direct Open and saved descriptors for Recent. Reload uses recent index0, not a newly sorted filesystem entry.
- `Editor.cs:5299–5371`: asynchronous load; current position/zoom refreshed into history before close; new active views get dedicated tabs, then focus and exact saved chart position/spacing zoom. New history entry moves to front after successful loading. The native adapter should prepare the detached candidate before publication so a read/parse error retains the old document, an explicit robustness adaptation to the original close-before-load flow.
- `Editor.cs:5623–5700`: New has unsaved-change review and creates an EditorSong without charts, reset position and zoom1. Pending dragged media fields are optional separate flows. EditorSong constructor uses Options.NewSongSyncOffset, default0.009; it does not invent a starter chart.
- `Editor.cs:1664–1675`: file I/O locking derives from CanLoadSongs. Native guards must reuse its existing bounded loader/job state rather than merely checking a dirty flag.

## Prepared isolated models

`recent_songs.rs` provides descriptor matching using the existing cultural ChartComparer order and preferred-open fallback. Only source indexes are returned for installation into the newly created typed ActiveChartViews; indexes/GUIDs are not persisted as stable cross-load identities. The MRU compares raw OsStr values: Path equality would normalize `a/./song.ssc`, contradicting source exact string equality. Existing native cultural sorting and the approved Couple extension remain authoritative.

`file_origin.rs` provides Origin::{Existing,Untitled}, an explicit resource directory, and NewSong::prepare. It constructs an SSC in-memory document with no charts and explicit GA sync metadata, never a user-visible placeholder/temp file. SSC is the initial native internal format, an adaptation until cross-format Save As is integrated. An Untitled origin requires Save As even when the initial content history is clean. Undo/Redo remains owned by the same Session during first publication, not recreated after output.

`patches/editor-file-menu-models.patch` declares only those two new modules. No shared implementation has been changed. Five initial Rust tests are prepared, not compiled or run: recent3 and New2. A separate extension adds recent JSON preferences3 and row-preserving registry restoration2; neither layer is declared.

## Executed source oracle

`scripts/editor-recent-song-reference.py` executes the entire original SavedSongInformation class with unchanged SelectBestChart and ChartComparer, original supported-type table and real Fumen permissive enum converter. Song/chart/Preferences singleton adapters expose properties only; they do not claim full editor loading or MRU persistence. Source blobs are read with git show at pinned commits; provenance records hashes, .NET SDK and assembly/request/output digests.

174 inputs × en-US/fr-FR =348 successful outputs were produced by the actual .NET10 program. Cases include empty/invalid focus, exact metadata, metadata drift, deprecated null strings and duplicate active records, as well as cultural tie order and preferred fallback cases inherited from the existing source oracle. Rust differential assertions are still pending. The oracle now additionally executes original PostLoad and produces 348 migration outputs beside the matching outputs. These are outputs from the same 174 cases in two cultures, not 696 independent cases. Filesystem load, keyboard/IME, GPU and native publication have not been validated by that oracle.

## Typed provenance migration required before New is connected

FileSession currently assumes an existing canonical path. Do not represent Untitled by an empty path, a suggested filename treated as existing, or a hidden temporary user song.

Prepare a separate adapter layer with:

1. `existing_path()->Option<&Path>` and `resource_base()->&Path`; existing persisted-only operations require a checked accessor returning an error. The former `path()` contract stays available only on a typed persisted view, never a panic on Untitled.
2. Same Session remains resident; persistence state is optional. Ordinary Save routes Untitled to Save As. First Save As uses the resource directory as relocation base and publishes persistence state only after the durable/pending outcome, retaining the same GUIDs, grids, Undo/Redo and relocation receipt.
3. Media use resource_base directly. Existing path-based playback/music helpers can remain compatibility wrappers around new resource-directory helpers. Audit native playback, waveform, image previews, music chooser, song properties and Pack integrations; visual labels use localized Untitled.
4. Untitled has no original disk baseline, source recovery sidecar or recent entry. AutoRecovery needs a distinct application-owned untitled-draft format/location before it may run; until that increment, explicitly disable sidecar scheduling for Untitled and communicate the missing recovery behavior. Never call sidecar_path/recovery_path with a fabricated filename.
5. Pack properties/list operations needing a persisted song are gated explicitly. A dirty Pack, pending save receipt, active recovery worker or file load must not be bypassed by New.

No first-save adapter patch is applied yet; the currently prepared Save As writer remains the only storage writer to extend.

## Native keyboard/menu plan

Use an explicit File panel with New, editable Open path (existing text/IME Buffer), Recent list, Reload, Save and Save As. Reuse the simplified menu presentation. No OS file picker or mouse dependency is required for the initial keyboard route.

- Existing dirty-song/dirty-Pack review and pending-durability/recovery guards form a shared transition boundary. Save/Discard/Cancel are explicit; pending receipt cannot be silently discarded. Recover/open jobs must drain/complete before publication.
- Prepare a bounded FileSession candidate with cancellation, run existing preferred open or saved-descriptor resolution in the cultural order service, then install through common State::from_workspace and the application-preference Snapshot. Retain preferred-open diagnostics and all global settings; do not copy old chart caches/transport state into the new Session.
- The old document stays editable after failure/cancel. Stop transport explicitly only when publishing or when the modal captures it per existing policy; stale decoder generations and queued source-bound work cannot survive publication.
- Reload first captures current descriptors/fractional focus/spacing zoom, then reopens the current persisted path using those descriptors; MRU index0 should denote that file. Untitled Reload is unavailable, not a read of a placeholder path.
- MRU records update only at successful publication/close state capture. Missing/unreadable recent paths remain reviewable errors and never replace the old session. Cancellation does not reorder history.

Next implementation: source-compatible recent JSON read/write/migration in the existing application-preference file, shared guarded native replacement helper extracted from Pack open, then Open/Recent/Reload before the larger Untitled persistence migration. Every shared change remains a nonapplied patch until the parent checkpoint.

## Additional isolated implementation and validation targets

- `recent_song_preferences.rs`: source RecentFiles shape, enum conversion reusing open_preferences, PostLoad migration, raw unknown fields retained. MRU mutation stages fallible payloads first, preserves Options owners and exact file-name equality. Three tests prepared (348 PostLoad comparisons, shared-options MRU, invalid-shape errors).
- `chart_views/recent.rs`: restore_recent creates dedicated active tabs in supplied order, then focused fallback as temporary if absent. All views map their own clocks at one shared music time. The focused row is then assigned from position_at_row so a fractional row inside a warp is not inverted from its ambiguous music time. Two tests prepared, including 160.5 within warp and negative fractional focus.
- `pack_song_list/file_open.rs` plus `pack_song_preference/recent.rs`: detached bounded16MiB candidate preparation, cultural comparison via existing Orderer/Prepared, common State::from_workspace and application-preference Snapshot. Three native unit tests prepared (old Undo/GUID/session ownership retained while detached candidate restores settings/warp; cancellation/external change; explicit cancelled-job slot retention). This is a candidate helper, not an installed File menu.
- `file_open_permit.rs`: process-wide parser permit used by FileOpen and PackSongLoad. Worker owns it until parsing exits, even if receiver/UI is dropped. No blocking lock; overlap returns an explicit error. Two local-gate tests prepare drop/cancel repeated-spawn refusal and unwind/release proof, without claiming actual Windows/GPU execution.

Patch order: editor-file-menu-models.patch; editor-file-menu-recent-models.patch; editor-file-open-permit.patch; editor-file-open-candidate.patch. The last two apply checks pass against the current source. Core model tests total10; native candidate/permit tests total5. All Rust execution remains pending parent validation. Only the .NET source oracle has executed.

The remaining required UI work is still explicit: durable MRU integration in the existing preference transaction (without overwriting staged NoteColor/MiniMap Options), New/Open/Recent/Reload panels and text/IME target, shared dirty/Pack/pending/recovery review and publication, then the typed Untitled first-save storage adaptation. None of these are claimed completed by the isolated helpers.

## Shared MRU preference transaction patch

`patches/editor-recent-song-preferences.patch` adds the typed recent list to existing Clone Settings, with getters, validated replacement and record_recent_song. It writes RecentFiles into the same root transaction and RecentFilesHistorySize into the existing shared Options map before its single insertion; staged NoteColor, MiniMap and unknown fields remain present. This is still a patch, not an applied second preference file. `patches/editor-recent-song-preferences-tests.rs` prepares three transaction checks for concurrent typed settings, external modification refusal and invalid maximum with no mutation/output. Source maximum is an int32, so larger native usize values are rejected before storage.

`Preferences::default` explicitly uses an empty list and maximum20. Nonobject Options and an out-of-range history maximum are errors, not silent defaulting. Missing/unusable recent paths can remain in records for an explicit open error; they never become fake source files.

## Untitled resource relocation patch

`patches/editor-untitled-media-directory.patch` declares a new directory adapter in media_relocation/directory.rs and extracts the existing relative-path calculation into one shared helper. rebase_from_directory uses a real absolute resource directory and a new destination file. It never passes a fabricated old file, and original rebase(None) deliberately continues to retain relative strings. Two isolated tests are prepared; neither this directory API nor first-save relocation is applied. The eventual Plan source provenance must distinguish ExistingFile from ResourceDirectory and preserve all histories through the same typed relocation receipt.

## Untitled migration draft (not ready to apply)

The parent approved the explicit API migration: FileSession no longer has a total path() on arbitrary documents. storage/origin.rs supplies existing_path Option, require_existing_path Result, resource_base and a checked PersistedFile view with path(). The proposed FileSession stores typed Origin plus optional saved bytes/recovery ownership. Workspace::new_song consumes NewSong without creating a session replacement after save.

The current /tmp/deadsync-untitled-core tree and editor-untitled-core-draft.patch are incomplete integration artifacts, not runnable deliverables. Their base includes both Windows identity/publication layers on temporary copies. The Untitled delta does not duplicate those Windows changes. Directory relocation is integrated into the same current/Undo/Redo walker via a Copy SourceBase distinguishing ExistingFile and ResourceDirectory. First output changes Origin only after the existing publication barrier and returns no previous_recovery receipt for Untitled. New baseline and recovery ownership are prebuilt before output; no original placeholder exists.

Four first-save tests are prepared in storage/save_as/untitled_tests.rs: clean New still requires Save As and creates no sidecars; song/chart media across original/redo contexts with resident grid/GUID preservation; edit→Undo invalidates prewrite review; late-sync first output has a real published path but remains dirty and cannot clean later edits. Only the first of these is platform neutral; actual directory durability fixtures are Unix. All remain uncompiled.

Pending before this draft may be applied: complete all native/resource/test path callers, File panels/text targets/guards, typed Close/Open/New intentions and their explicit cancellation, native first-save publication/status checks, and validation. A pending Close intent must be bound to the source session/history and rebound only by exact relocation proof; Cancel/KeepEditing destroys it. A later unrelated save must never unexpectedly close the editor.

## Reprise du 28 octobre : provenance et appelants

`patches/editor-untitled-core-api.patch` reprend le draft sur la base courante avec les deux couches Windows préparées. Il comprend déjà `directory` et `SourceBase` ; ne pas appliquer en plus `editor-untitled-media-directory.patch`. Quatre références privées de tests à l'ancien champ `file.path` sont corrigées. Les modèles file_origin/recent restent des prérequis de déclaration. Les fichiers nouveaux `storage/origin.rs`, `media_relocation/source.rs`, `media_relocation/directory.rs` et `storage/save_as/untitled_tests.rs` doivent accompagner le patch.

`patches/editor-untitled-native-resources.patch` migre les ressources audio, formes d'onde, images et sélecteurs vers la vraie resource_base. Les deux opérations Pack refusent une provenance sans fichier existant ; aucun faux chemin n'est créé. SaveAs commence par une destination vide pour Untitled. Trois nouveaux tests de frontière native sont préparés. Une clé Edition/FileRequiresSaveAs doit accompagner cette couche (EN: Save the new song before editing its pack. / FR: Enregistrez la nouvelle musique avant de modifier son pack.).

`patches/editor-untitled-external-tests.patch` migre les appels persistés de huit fichiers de tests du thème, incluant les helpers partagés et appels multilignes : 25 `path()` et un `recovery_path()`. Les trois fichiers de tests core concernés sont déjà dans core-api. Les `path()` des ProfileFile, PackFile, DirEntry, TempDir et des vues typées persistées sont volontairement conservés.

`patches/editor-file-intent-guard.patch` déclare le guard de contenu avec compteur monotone et ajoute les bornes before_next/after_next à la preuve de relocation. `file_controller.rs` est un modèle natif séparé (déclaration proposée dans editor-file-controller-model.patch) : Review, Saving, Ready ; Cancel détruit l'autorisation ; une modification suivie d'Undo révoque l'action ; une première sauvegarde ne reprend l'action qu'avec la preuve exacte et un document devenu propre. Sept tests de contrôleur sont préparés, dont un avec publication Unix réelle. Il ne remplace pas encore State et ne constitue pas un menu Fichier utilisable.

Les apply checks de ces patches ont été effectués sur des copies de base contrôlées. Aucun de ces nouveaux tests Rust n'a encore été compilé ou exécuté lors de cette reprise. Les panneaux Fichier et le raccordement effectif du contrôleur restent à intégrer. Le futur OutputPolicy reste séparé de la provenance/sourcecodec : recovery doit conserver Session.serialize sans perte et les informations de politique séparées ; aucune conversion, anonymisation ou suppression ne doit être appliquée au snapshot de récupération.

## Panneaux raccordés en copie isolée

`patches/editor-file-menu-ui.patch` raccorde maintenant réellement le contrôleur et les panneaux au code natif. Il est préparé dans `/tmp/deadsync-file-ui/work`, toujours non appliqué à l'arbre compilé. Préconditions : core Untitled/API/resources/tests, modèles File/MRU, transaction RecentFiles, candidat Open/permis, guard d'intention, déclarations file_controller/file_recent et Snapshot MRU. Le patch UI élargit seulement la visibilité des adaptateurs Snapshot/FileOpen à l'intérieur d'edition ; il n'ajoute pas de seconde boucle de rendu ou de chargement.

- Entrée Fichier ajoutée à la fin du menu F1 normal et sans partition. New crée une vraie musique SSC en mémoire, sans partition initiale ; le titre sans partition indique la provenance sans titre.
- Open possède un champ natif Buffer avec texte Unicode, IME, édition et presse-papiers existants. Un chemin absolu SM/SSC est requis. Recent utilise les descripteurs enregistrés et Reload capture d'abord le rang fractionnaire/zoom actuels.
- Le callback culturel des morceaux Pack, déjà exécuté à chaque frame par le shell, draine aussi le FileOpen borné. La sélection préférée et la restauration Recent restent sur le thread principal. L'ancien State n'est remplacé qu'après préparation, contrôles du document source, du chemin logique et de la destination.
- Review/Save/Discard/Cancel utilisent le vrai contrôleur. Discard a une confirmation distincte. L'annulation détruit l'intention ; le worker annulé reste dans son slot jusqu'à sa terminaison, sans résultat publiable. Les gardes Pack, génération, prévisualisation, récupération et entrées de notes restent nécessaires à la publication.
- Save sur Untitled ouvre SaveAs. La preuve de relocation rebinde l'intention dès la publication ; un résultat pending ne continue pas l'action. Seule la confirmation durable d'un contenu toujours propre peut reprendre New/Open/Close. Cancel/KeepEditing et une erreur de rebind révoquent l'action. La fermeture revient par le booléen existant de raw_key après l'action synchrone de sauvegarde/réessai.
- La liste MRU est enregistrée dans la transaction de préférences existante ; elle conserve ses inconnues et le même objet File/baseline lors d'un remplacement de document. La fermeture confirme son écriture avant de sortir. Un document Untitled n'a aucune entrée fictive.

Six tests natifs ajoutés au module file_menu couvrent chacun anglais et français : accès F1/New ; revue sale/Cancel/Undo ; champ Open/IME/erreur ; Open/Recent/Reload avec rang160.5 dans un warp et zoom2.5 ; révocation SaveAs puis premier SaveAndClose ; annulation avec worker retenu et MRU inchangé. Ce sont 12 parcours de modèle natif préparés, pas des exécutions GPU/entrées physiques. Le patch `editor-file-new-options.patch` lit le NewSongSyncOffset source dans Options et ajoute deux tests sans mutation. Apply checks réussis sur les bases isolées ; aucun Cargo exécuté par cet agent.
