# Missing generation profiles at song load: confirmed source/native gap

## Normal user path

Open a shared SM/SSC whose GA metadata references custom profile GUIDs not installed locally. Source automatically resolves the chart expression to Dynamic and marker Pattern/Performed to Sixteenths/PatternBalanced. Native currently loads the raw GUIDs, shows an absent selector value, then fails actual chart/pattern generation with a missing-profile error. Manual repair through the new manager/selectors exists; automatic source loading behavior remains missing.

## Executed original evidence

Run:

```
python3 scripts/editor_profile_load_source_oracle.py \
 --editor-root /path/to/pinned/grooveauthor \
 --reference-root /path/to/pinned/StepManiaLibrary \
 --dotnet /path/to/dotnet \
 --output /path/to/source.expected.json
```

The script reuses the existing generated-metadata model template/plumbing from repo `scripts/`. It reads source by `git show` and checks exact pinned checkouts. Three cases execute the entire original `EditorChart(EditorSong,Chart)` constructor, original `ExpressedChartConfig` property, original `CanBeEdited` (real empty Fumen WorkQueue), original custom deserialization methods and original `ConfigManager.GetConfig`/`ConfigData.GetConfig` method bodies. Declared containers seed known GUIDs explicitly; full config-manager filesystem initialization/default construction is not executed. Density/event factory/notifications remain plumbing. This is loading normalization evidence, not generation-output parity.

Results, independently asserted:

| Input | Effective source result | Source dirty/warnings |
|---|---|---|
| Known expression and marker profiles | Original profiles unchanged | false / none |
| Unknown expression only | Dynamic; marker profiles unchanged | false / none |
| Unknown expression, marker Pattern and marker Performed | Dynamic, Sixteenths, PatternBalanced | true / two warnings |

Fixture and SHA/pin provenance: `tests/fixtures/profile-load/source.expected*.json`. All three original cases executed successfully by sidecar author. No Cargo run.

Original lines: `EditorChart.cs520/583` initialize Dynamic; `460–466` property only accepts known config; `3995–4063` deserialization validates marker references and sets source unsaved changes when substituted. Actual original managers return null for unknown GUIDs through their dictionary lookup, unlike the earlier generated-metadata oracle's deliberately all-valid manager plumbing. That earlier oracle did not cover this contract.

## Current native consumers

- `generation/source.rs236`: `initial_editor_metadata` delegates to raw `ChartData::existing_or_initial`.
- `generation/job.rs241–263`: chart generation uses raw expression GUID and fails if registry lookup is absent.
- `generation/pattern_job.rs385` invokes `metadata.options` for saved marker generation; unknown pattern/performed references also fail.
- `generation/pattern_batch.rs142` takes the same raw metadata owner.
- `generation/metadata.rs::ChartData::read/save` is shared by inspectors, copy/duplicate, resident serialization and actual workers. A worker-only fallback would leave these consumers inconsistent and omit source dirty/save behavior.

## Required coherent follow-up (not implemented here)

1. Read the actual configured profile directory/default registry before publishing the loaded song, with captured custom-file byte guards and load cancellation. Profiles must be loaded before resolving chart metadata, as in source.
2. Derive a typed load-resolution plan bound to the exact Session/document and profile snapshot. Apply original fallback rules to known GA fields only; preserve unknown metadata, unrelated chart fields and raw ignored SM tags. Missing expression alone does not mark the source document dirty; missing marker Pattern/Performed does.
3. Establish one effective metadata owner consumed by inspectors, all generation actions, analysis/snapshots, Save/SaveAs/export/recovery and chart copy. Avoid a second private map or silent per-worker replacement. Raw source bytes for external-file CAS remain the originally read bytes.
4. Loading normalization is source baseline work, not a user Undo entry. The follow-up must explicitly model dirty baseline/history reset, rather than inserting a hidden undoable command or calling mark_saved and losing the marker-repair dirty state.
5. Catalogue/file changes during review must reject/reprepare instead of applying stale GUID resolution. User edits after opening remain ordinary existing history; do not re-normalize on every frame or overwrite a later intentional edit because a profile file disappears.
6. Tests should cover the normal share/open/generate/save/reopen path, both expression-only and marker repairs; exact unknown-field retention; zero load Undo; dirty distinction; profile files present vs missing; and stale profile-directory/CAS refusal. The original oracle is the expected semantic witness, not native assertions copied into a fixture.

Only oracle/documentation delivered in this lot. No production correction or completeness claim.
