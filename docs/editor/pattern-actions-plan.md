# Exact selected/all Pattern menu actions

Source GA `48e18f71655d0f0d86144c63239331833501fda2`, Editor.cs:7413–7467 and main menu:4193–4206.

## Source contract and executed witness

Selected Fixed/New obtains Selection.GetSelectedPatterns; All Fixed/New obtains Chart.GetPatterns. Notes, unchecked/intermediate markers and current cursor/range do not widen the selected set. Clear Selected/All supplies that same exact set to ActionDeletePatternNotes, which clears generated note regions and preserves the marker definitions.

New Seeds creates ActionMultiple, calls real ActionSetObjectFieldOrPropertyValue<int> for each marker with Random.Next, and enqueues that seed action before starting ActionAutoGeneratePatterns. Seeds form a separate history action. Undo generated notes leaves new seeds; a second Undo restores previous seeds. Redo reuses the stored seed values. Cancelling generation does not silently roll back the already committed seed action.

`scripts/editor-pattern-actions-reference.py` executes unchanged four source callbacks, RegeneratePatterns and whole Selection, with the actual pinned Fumen ActionQueue/ActionMultiple/ActionSetObjectFieldOrPropertyValue. The asynchronous generation action is an explicitly documented recording adapter. Six cases cover selected/all fixed/new and empty selection; unknown intervening note selection is ignored. Source and adapter/assembly hashes are recorded in pattern-actions.reference.json. This witness proves dispatch/reseed/history only. Existing pattern_batch and clear_patterns source references remain the witnesses for movement/deletion/generation internals.

The pinned source ActionAutoGeneratePatterns constructor itself appears to add its input twice (spread initialization then AddRange). This dispatcher does not alter or duplicate the established Rust generator to emulate that incidental issue. Generation engine parity is a separate established lot; this witness does not claim to execute that asynchronous body.

## Prepared implementation

`pattern_actions::Plan` captures chart GUID and IntentGuard, exact sorted marker rows, source action kind and fresh seed map. Prepare does not edit history. Selection.validate is made crate-visible without changing validation; checked Plan uses it directly without needlessly copying note clipboard payloads. Same existing identity/token gates apply.

The immutable Review includes exact rows/action/seeds. Begin refuses foreign/stale or mismatched review before any change. For New, it saves only the selected marker seeds as one metadata/history action, then captures the existing worker against that new revision. Unknown metadata and unchecked definitions remain intact. A setup failure exposes `seeds_committed`, allowing the UI to say that Undo restores the changed seeds. Dropping/cancelling the returned worker preserves the seed action.

Workers are existing pattern_batch::Task and clear_patterns::Task, with their existing profile directory behavior, cancellation, typed intervals, atomic Apply and file-format limits. No second generator or clear codec is introduced.

The simplified native UI is an explicit adaptation: workers build detached previews and generated notes/clear changes are committed only on preview Apply. This preserves existing native review controls. Compared with source eager deletion before asynchronous generation, failed or cancelled native work leaves note data intact; a New seed action still remains in history. The panel explains that distinction and cancellation/discard messages reiterate that seeds were kept.

Native F6 in the mixed checklist opens Checked; F8 in Regeneration opens All. Optional six-row panel: action, scope, prepare/count, seed history note, start, cancel. Changing configuration invalidates the prepared Plan. Start consumes the original checklist. Cancel before Start returns to the exact checklist without editing history. All/current marker tools remain available.

## Tests prepared, not executed by implementing agent

- core6: checked identities including notes and intermediate unchecked marker; source-shaped separate reseed/cancel; full generated-note Undo then seed Undo/Redo; exact clear with hold shortening and unchanged marker metadata; empty/stale/foreign/mismatched review; actual source random witness seed maps.
- native3 per EN/FR: New All cancellation/Undo/Redo, exact checked Clear preserving unchecked marker notes, prepared cancellation leaves seed history untouched.

## Durable application order

1. Copy `patches/editor-pattern-actions-core.rs` to editor `src/pattern_actions.rs` and native counterpart to theme `screens/edition/pattern_actions.rs`.
2. Regenerate both patches with `patches/editor-pattern-actions-build.py` against the current source; `git apply --check` each; apply core then native. The native patch adds Workspace wrappers and existing job/preview status hooks.
3. Copy core-tests.rs to editor `tests/pattern_actions_workflows.rs`.
4. Copy native-tests.rs to theme `tests/support/pattern_actions/mod.rs`, with usual `FRENCH` EN/FR wrappers `edition_pattern_actions[_fr]_workflow.rs`.
5. Parent runs targeted Cargo batches after the shared-source freeze clears. No tests or claims of full-suite success are implied by source-oracle execution.
