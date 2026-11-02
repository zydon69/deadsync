# Remaining editor review — 2 November 2026

## Current checkpoint

The authoritative current index is `final-validation-2026-11-02.json`, together
with `completion-audit-2026-11-02.json`. All 164 source callback rows now have
bounded workflow evidence. The fresh full native editor gate is **629 passed,
0 failed, 0 ignored**. The 241 core and 1346 public workflow passes are reused or
separately rerun as explicitly recorded in the final index, not a fresh combined
run. The historical tables and intermediate notes below retain their original
execution scopes.

The current build is packaged and visibly open in
`dist/DeadSync-Editor-Final-20261102.app`, including the KeepChartOpen correction.
Strict signature verification passed before launch. See
`/tmp/deadsync-editor-final-app-receipt-20261102.json` for the scoped runtime
receipt. Physical user review remains pending. The full port is not yet declared
complete. Broader theme failures remain open outside the editor-only test gate.


This replaces the old 517-native-test matrix as the current review index. It
preserves all 21 source flows. Module test counts indicate available evidence,
not complete requirement coverage. Historical runtime checklists must be
reconciled against actual receipts before they can be called missing work.

| Source flow | Current executed native tests in mapped modules |
| --- | ---: |
| File: New/Open/Recent/Reload/Close/Exit | 38 |
| File: direct media / best assets | 38 |
| File: Save/SaveAs/advanced output | 10 |
| Recovery/autosave | 10 |
| Song and chart metadata | 3 |
| Pack properties / song list | 14 |
| Chart list / focus / tabs | 2 |
| Note entry and edit actions | 69 |
| Mixed event/region selection | 5 |
| Timing events / conversions | 4 |
| Advanced event copy | 1 |
| Attacks inspector | 3 |
| Single chart autogen | 9 |
| Batch autogen | 3 |
| Pattern marker inspector | 7 |
| Pattern Set / source regenerate commands | 1 |
| Global three-family config manager | 10 |
| Profile fields / gates / weights | 10 |
| Help / FTUE / About | 9 |
| Log / Performance | 12 |
| Layout / resolutions | 1 |

## Next work

- Reconcile all historical runtime checklist items against previous runtime receipts; checklist age is not proof of missing code
- Perform physical held-key validation with the user; automation cannot hold two ordinary keys
- Preserve broader theme suite failures in delivery report; scoped tests alone do not certify whole workspace

## Current limits

The previous combined frozen gate was 241 core / 617 native / 1345 public tests.
The latest native-only execution passed 618 tests after adding physical
Control/Command clipboard coverage. The advanced exporter module was then
strengthened and rerun separately: 5 passed, including exact admission
expectations for all 60 reference cases. These separate executions are not
a new combined frozen gate.
The broader theme suite is not green (15 failures in its recorded unfiltered
run). See the incremental audit for the scope of each later targeted execution.
See the JSON companion for named tests, requirements and exact receipt paths.
No editor commit, push or pull request has been made.

Current review application: `dist/DeadSync-Editor-Review-20261102.app`. Exact
gated binary, strict signature verified, actual French two-chart launch observed.
The older review application is preserved.

All eleven F10 timing types now have actual Stage/Apply/Save/Undo/Save
evidence at beat 4, with exact restored bytes, preserved notes and peer charts.
Performed speed and dance-single weight inheritance also have saved/reopened
runtime evidence. See parity-review-2026-11-02.md for scopes and limits.

## Incremental source comparison

`completion-audit-2026-11-02.json` now records requirements and bounded evidence
for file lifecycle, direct media, advanced exports, attacks, song/chart metadata,
packs, chart navigation, note operations and mixed selection, timing conversions,
autogen/patterns and generation profiles. It distinguishes reused hash-matched
results from newly executed tests.

Recovery/autosave, event-copy presets, Help/FTUE, diagnostic tools and
layout/resolution now have bounded source/evidence entries in the completion
audit. Remaining runtime limits and deliberate format-safety differences are
retained per flow.
The physical held-lane/Down check remains pending the user's trial. This is not
a declaration that the full port is finished.

## Chart-copy investigation resolved against loader preconditions

The apparent nonzero-initial-node difference came from an action-only fixture
that bypasses source loading. GrooveAuthor inserts missing initial tempo and
signature nodes at row zero before this action is available. The experimental
raw-position preservation change has been withdrawn; production fingerprints
match the previously validated source again.

A new regression starts with raw late-first timing and compares the native
normalized result with the unchanged original action on loader-normalized nodes.
Copy and live-copy modules pass 16 tests, zero failures. The source normalization
code and fixture limitations are recorded in completion audit flow 13. The review
app's production behavior is still current; the withdrawn experiment needs no
repackaging. The full port's other pending review items remain open.

## Historical pending-work checkpoint (superseded above)

- Link the 164 source callbacks to functional workflow evidence; presence alone
  is not implementation proof.
- Consolidate the final source/test manifest, including the five strengthened
  integration modules and the restored source-loading preconditions.
- Keep the user's held-lane/Down/Undo trial pending; raw Control/Command tests
  already pass, while Command-key OS automation remains inconclusive.

Help/FTUE/About has nine matching executed native tests; log/performance has
twelve; resolution requests have a native host-mode test and an inspected shell
consumer. Twelve public FTUE/performance tests also match the current sources.
These are revalidated historical executions, not newly run tests or a claim that
the whole theme suite is green.

## Advanced export admission review completed for the reference corpus

All 60 cases are classified: 28 accepted; 20 correspond to inspected source
editor pre-save guards; six demonstrate writer note/chart loss on readback;
six are excluded by the original reader before writer validation. The test
requires exact errors and category counts, verifies loss/exclusion witnesses,
and retains accepted note/layout/player comparisons. Five targeted tests pass.
Source pre-save guard matching is static evidence, distinct from the executed
low-level writer fixture. This does not certify every possible export input.

## Latest complete native editor gate

After strengthening Hold/Roll selection coverage, all 618 native editor tests
passed again with zero failures or ignored tests. All recorded input hashes were
unchanged after execution; all prior test names are retained. The startup wait
was diagnosed as a macOS dyld/fcntl wait when loading a proc-macro; normal
signature verification passed and the original process resumed without restart
or security changes.

The per-command review now covers 79 source callbacks with explicitly layered
or native-exact workflow evidence; 85 rows remain inventory-only pending review.
See command-coverage-2026-11-02.json. These counts are not a port percentage.

## Navigation and marker command review

The per-command review now records 103 reviewed callbacks and 61 inventory-only
rows. A new native test passed ten literal navigation cases in each of EN/FR:
fractional and negative measure movement, strict label neighbors and wrapping,
chart start/end, unchanged bytes/revision/history. Six indexed navigation
commands, two vertical commands, six pattern commands, two song-marker commands
and two auto-asset commands have layered or exact native evidence recorded per
row. The new test is additional to the last full618-test gate; a full619-test run
has not yet been claimed. Production is unchanged.
