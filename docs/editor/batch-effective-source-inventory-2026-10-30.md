# Batch source inventory: effective StepF2 eligibility

Reference GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`; Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

## Confirmed product defect

Native `edition/batch.rs` previously enumerated/counts charts by raw `steps_type`. BatchTask already selected by `generation/source.rs::effective_layout`. A StepF2 cooperative chart encoded as raw `pump-double` was therefore offered in the UI but omitted/refused by the actual worker. All-coop songs advertised a batch which could not start; mixed songs displayed a larger count than the real batch.

Source `UIAutogenChartsForChartType.cs62–91` picks only charts supporting autogen; `EditorChart.cs3217–3220` excludes multiplayer; Fumen `SMPropertyParsers.cs1137–1142` promotes StepF2 multiplayer notation to pump-routine. This delta fixes native admission/presentation and retains the valid engine refusal. Original file types/notation are preserved.

## Shared owner and cache

`batch_job::source_charts` provides the effective, supported `(source_index, layout)` inventory used by both Task capture and native display. The existing `effective_layout` interpretation is unchanged. Native caches layouts/counts using `chart_order::SessionBinding`, which checks content identity and revision: a new Session with equal bytes cannot reuse the cache, content edits/Undo invalidate it, and application-only preferences do not require redecoding unchanged notes.

The inventory is built at batch opening, before a batch key/start operation, and during refresh while the batch panel is active. Labels only read the current cache; there is no note scan per rendered label/frame. Hidden batch panels do not trigger new work. A stale cache cannot advertise stale counts. Focused StepF2 coop no longer masquerades as an eligible ordinary double source; normal fallback behavior for eligible source families remains otherwise unchanged. No chart is added, normalized, focused or edited by inventory capture.

## Supplied checks

Core inventory test: raw co-op double excluded, ordinary double and dance single retained, bytes/types unchanged.

Native tests: all-coop refuses before worker/migration admission; mixed UI count=1 matches the actual worker's source index1, source bytes/revision/focus unchanged; repeated labels/refresh do not rebuild; content edit/Undo and another equal-byte Session each invalidate the cache.

No Cargo executed by the sidecar author. Parse-only and patch applicability checks are separate delivery evidence. This delta does not claim batch completion/focus ordering parity, initial source type ordering parity, or whole-engine equivalence.
