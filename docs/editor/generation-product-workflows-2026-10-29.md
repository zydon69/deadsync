# Generation product workflows and standalone configuration manager

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`, StepManiaLibrary `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`, Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

This checkpoint reads the paths below. It does not certify all generation algorithms, every profile field, or unopened source windows. Existing executed comparator/search/worker/metadata oracles remain separate evidence; a test count is not a coverage argument.

| Product requirement | Original source path | Native consumer and status |
|---|---|---|
| Choose a source chart independently of focused view, target type, performed config, then generate | `UI/UIAutogenChart.cs` SourceChart field27, SetChart63, Draw66; `Editor.cs4507–4508`; `EditorActions/ActionAutoGenerateCharts.cs` | `edition/autogen_source.rs`, `edition.rs` Generation panel, `generation/job.rs` ChartTask133, apply410. Actual engine, review/Apply, resident append/history, Save. Independent source catalogue ownership was repaired in the preceding lot. |
| Generate all charts of a source type | `UI/UIAutogenChartsForChartType.cs96–198`, actual action192 | `edition/batch.rs`, `generation/batch_job.rs55/104/218`: captures each eligible source, actual ChartTask, batch preview/append. Atomic review is the native presentation adaptation. Initial default type and asynchronous source completion/focus ordering are not independently reproduced here. |
| Regenerate all/selected markers and clear generated notes | `Editor.cs4175–4209`, `7467`, `EditorActions/ActionAutoGeneratePatterns.cs` | `edition/pattern_actions.rs`, `pattern_job.rs98–178`, region generator full worker; preview/Apply/Undo and persisted marker owner. Original worker oracle separately executes the unchanged action and real libraries. Clear retains the legacy-compatible read inventory contract. |
| Navigate/inspect an existing marker, generate that marker without seeking cursor | `UI/UIPatternEvent.cs` Generate callback99–105; `Editor.cs3787/6701` | `edition/marker_generation.rs`, MarkerEdit Draft admission, inspector Generate row. Captures marker identity/draft row, explicit pending edit refusal, full worker, Cancel/Apply and history. Already integrated before this lot. |
| Preserve live metadata while creating output charts | `ActionAutoGenerateCharts.cs` ignores SaveToChart custom dictionary198–202; constructor re-reads OriginalChartExtras | `generation/metadata.rs`, common history append/duplicate owner, SM and SSC. Native deliberately repairs demonstrated source stale-data loss. Three executed original action cases and provenance are separate committed oracle artifacts. |
| Sort/select/edit profiles from chart-bound selectors | `UI*ConfigTable.cs` comparers, chart/pattern/autogen config callbacks | `profile_catalog.rs`, `edition/profile_order.rs`, `profile_sort.rs`, `profile_edit.rs`: typed configs, custom-file byte CAS, shared global Session history, protected presets, source comparers, import/export and durability receipts. Initial cultural selector order is an explicit adaptation; initial ImGui sortspec is not proven. |
| Manage all three profile kinds before opening a song | `Editor.cs4220–4221` opens `UIAutogenConfigs` without chart guard; `UI/UIAutogenConfigs.cs42–54` draws Performed, Pattern and Expressed tables | **Confirmed missing before this lot.** Previously every catalogue Task required a valid chart. Added explicit Application context, two menu entries, three lists, New/default/Edit/Copy/Delete/Restore/import/export through existing inspectors. Global editing never stages a chart selection. |

## Ownership and context

`profile_catalog::Context` distinguishes `Application` from `Chart(usize)`. Application capture binds the current real Session revision token without reading or inventing a chart. Existing chart APIs retain their chart guard; an application catalogue cannot be used by generation/marker/expression selection. Both contexts reuse the same custom profile snapshots, dependency CAS, ordering and preset protections.

The native application manager uses `ProfileTarget::Application`, sharing the existing three-family catalogue panel. Left/Right changes the inspected profile only. E edits; N creates a genuine editor-default profile via Session ProfileCreation and opens its inspector. Existing inspector actions save, duplicate, restore defaults, import/export and delete. Selecting or editing a global profile does not stage generation overrides or a marker/chart expression. Profile deletion still invalidates genuine references through the existing shared deletion action.

Source New uses `EditorPatternConfig.CreateNewConfigAndShowEditUI`436–445, `EditorPerformedChartConfig`581–590 and `EditorExpressedChartConfig`102–107: an AffectsFile=false add action then show its inspector. Native N likewise publishes one ProfileCreation entry, then reloads the catalogue and opens the created identity. Source persists its manager files later; native retains the previously accepted immediate file/CAS publication model and typed post-publication durability warnings. New never inherits the selected custom profile's unknown metadata; Copy continues to preserve it.

Global history is the current Session's history, not an independent profile stack. Source `Editor.CloseSong` clears `ActionQueue.Instance` at5782; it is not an application-lifetime undo log. Native `file_menu::publish_file_candidate` replaces State/Workspace at371–414, therefore opening/replacing/closing a song resets the history while profile files persist. No history is transferred into another document. Application preference snapshots transfer profile directory/options, not history or chart catalogue identities.

Before replacement `publish_file_candidate`373 calls `flush_wheel_settings`; `edition/scroll_wheel.rs164–165` retries owned profile publication durability. A refused flush retains the old State and receipts; successful publication warnings never pretend rollback. This lot reuses that barrier and does not create a second persistence owner.

No-song support uses the existing chartless workspace; the manager neither adds a chart nor feeds `workspace.chart()` to a chart-bound Task. A catalogue for another Session/revision or changed custom file is rejected before mutation. A NoSong global Undo/Redo uses the existing application-change admission.

## Validation supplied, not run by this agent

- Core: chartless application catalogue; all three families; chart selectors refuse Application context; another Session rejected.
- Native: real Song/NoSong entry menus, New defaults for all types, Copy/Edit, shared F7/F8, Delete/Undo/Redo exact bytes, no song bytes/content dirty/focus/cursor/override changes, actual CloseSong history reset and manager reload after close; stale Session and external byte-CAS refusal retain pending draft.
- Parse-only rustfmt and `git apply --check` are reported separately at delivery. No Cargo or primary edits by this agent.

Remaining audit limits: complete configuration normalization across all source managers, full conversion search/output parity for every layout/config, batch asynchronous completion/focus ordering, and exact initial ImGui table sort state are not closed by this manager lot. These are explicitly unproven contracts, not confirmed missing product functions.
