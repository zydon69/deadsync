> Historical checkpoint, superseded by the current implementation (31 October).
> `edition/command_bindings.rs:797–824` now routes selected/all fixed/new seed
> regeneration through `edition/pattern_actions.rs`; keyboard region policies
> are consumed in `edition/region_selection.rs`, and source stream preferences
> have a native `edition/stream_settings.rs` consumer. The missing-work statements
> below describe the earlier checkpoint, not current absent functionality.
> These consumers alone do not certify source parity or complete execution;
> current evidence and open gates are in `delivery-audit-2026-10-31.md`.

# Concrete follow-up after Edit menu increments A–D

Read-only comparison at source GA `48e18f71655d0f0d86144c63239331833501fda2`.
Implementation status below describes current code paths, not test certification.
The old menu-comparison-next.md table is historical and now overstates absent work.

## Confirmed: selected/all Pattern actions and random reseeding

Source Editor.cs:7413–7467 and main menu:4193–4206:

- Selected Fixed/New uses Selection.GetSelectedPatterns, not interval or cursor.
- All Fixed/New uses FocusedChart.GetPatterns.
- New seeds iterates only that exact set, random.Next per marker, commits one ActionMultiple before starting asynchronous ActionAutoGeneratePatterns.
- New seed action is separate from asynchronous generated-note application. Cancellation does not silently roll back that committed seed history action.
- Clear Selected and Clear All both call ActionDeletePatternNotes over the supplied exact markers. Markers remain; notes inside their source deletion regions are cleared.

Current code:

- edition/pattern_set.rs has a separate exact manual marker checklist, all/none/range controls and fixed batch generation. Its initial set is all markers; it does not read the mixed event checklist's checked Pattern identities.
- generation/pattern_batch.rs accepts explicit rows and uses saved seeds. No new-seed generation/transaction mode is present.
- generation/clear_patterns.rs already accepts arbitrary explicit rows; native clear_marker_notes only starts from the single marker inspector draft.
- edition.rs Panel::Regeneration has current marker regeneration and opens the separate set picker. It has no source Selected/All fixed/new/clear dispatch.

Next coherent increment: a typed exact Pattern selection adapter + atomic seed plan, optional compact action review reusing pattern_batch/clear_patterns workers, and actual native callbacks for all six source menu actions. Preserve source two-history-gesture new-seed semantics; never regenerate seeds inside the worker or derive permanent deterministic hashes. Validate current Session/chart/revision and original marker definitions before any edit. Use the existing profile registry/directory behavior and generation cancellation. Tests need unchecked markers between selected ones, notes-only checked set, foreign/stale, seed Undo/Redo, cancellation after reseed, clearing sustains at region boundaries and native EN/FR input/review.

This is missing dispatch/seed history, not a missing generation engine or clear codec.

## Confirmed preference surface differences, pending interaction decision

Source UISelectionPreferences exposes OverlapAny/Center/All drag overlap and TimeOrPosition versus TimeOrPositionAndLane shift-click selection. Current native mixed picker intentionally selects discrete keyboard choices and lacks these pointer-region policies. Literal drag/click parity is not automatically required by the user's keyboard-only design; a keyboard endpoint/lane-bounded range adapter could preserve useful semantics. Do not implement inert preference toggles without a selection operation consuming them.

UIStreamPreferences exposes seven stream thresholds/notation fields. Current `analysis` uses RSSP summaries; no standalone source Stream preferences module/UI has been located. This needs comparison with the active rendering/analysis owner before assigning work; source stream-notation counting must not be confused with gameplay RSSP chart metrics.

## Already handled elsewhere or not newly missing

A/B exact conversions/transforms, presets, C ShiftAll and D Quick-add are implemented/being validated. File menu, advanced output, pack, media publication, key profiles, rendering/preferences are active independent lots. The source inventory of hashes is not implementation coverage. Debug/ImGui tooling and literal docking layout replication remain outside approved simplified presentation.
