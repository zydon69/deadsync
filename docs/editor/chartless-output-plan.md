# Advanced chartless output

Isolated draft: `/tmp/deadsync-chartless/{base,work}`.
Apply `editor-chartless-output.patch` after external-watch and platform/startup. Apply-check succeeded against current primary with watcher143 and Density/Dark changes; no changes to their dispatch or reserved keys. No primary source applied, no Cargo/staging/commit by this agent.

## Contract

- Explicit `AdvancedChartless` policy permits zero native charts only; existing Advanced remains strict GUID selection. `Plan::timing_chart()` now returns `Option<usize>`; no index sentinel.
- Pinned source writer null fallback omits global timing, uses default 4/4, omits DISPLAYBPM, retains music/offset and SSC last-second hint. Every nonempty lost global timing field and SM hint appears in review before publication. Native session is never rewritten to fit the output.
- Adding a chart or removing the selected GUID requires an explicit new timing choice. Undo can make the policy valid again, but old reviews stay stale. Current Save opens keyboard settings when that choice is required. Cancellation preserves the published policy.
- Recovery V2 remains lossless native serialization and strict native identity/target extension validation. Optional `output_requires_review` defaults false for existing ready envelopes. Declared review state must exactly match actual timing resolution; malformed, false-ready or spurious-review claims are rejected. Coherent unresolved policies recover their complete native draft and remain blocked from Save until explicit selection. This does not alter publication epochs, witness or pending durability.

## Executed validation

Actual pinned Fumen .NET10 readers/writers ran 16 source/target/anonymize/omit combinations with zero charts and null fallback. Source hashes, assembly hash and input/output hashes are in durable fixtures `crates/deadsync-editor/tests/fixtures/chartless-export/` (delivered in patch). Generator: `scripts/editor-chartless-export-reference.py`.

All draft Rust files parsed through rustfmt stdout (no rewrite), and `git apply --check` passed. **No Cargo compilation or Rust test executed by this agent.** Source oracle proves the reader/writer configuration only, not native history/recovery/publication.

## Prepared Rust tests (parent executes)

- `chartless_output_workflows`: 8 tests, including the 16-case oracle, strict normal GUID path, creation/Undo review invalidation, cross-format source-codec/native recovery, required timing loss acknowledgement, bounds/cancel/offset refusal, plus two crash recovery flows (chart added under chartless policy and selected GUID deleted) ending with explicit GUID publication and native author preservation.
- Recovery codec: 3 additional tests for chartless identity envelopes, strict ready/review declaration, optional legacy-ready field and extension validation.
- Native output UI: 3 additional EN/FR tests for real chartless SaveAs, explicit chart selection after creation, and current Save opening review settings followed by cancellation.

Suggested parent sequence: core lib + chartless_output_workflows + output_policy + output_storage + auto_recovery; then native edition output_ui tests serial with existing edition tests.
