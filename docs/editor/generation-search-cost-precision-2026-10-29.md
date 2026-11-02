# Generation search comparison precision

Confirmed correction against StepManiaLibrary `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`.

`PerformedChart/SearchNode.cs:1477–1478,1494–1495` subtracts the two `float`
fields before `Math.Abs` and comparison against the double literal `0.00001`.
Native `generation/search.rs:75` previously converted each operand to double
before subtraction. With nonnegative inputs `x=f32::from_bits(925353389)` and
`y=f32::from_bits(722255052)`, the original difference rounds to
`9.999999747378752e-6`; the native difference was `1.0000000156873455e-5`.
The original therefore reaches the random tie-break, whereas native selected
by this cost. This comparator is used by both conversion and pattern search;
changing its branch can change the winning generated sequence.

The fix keeps subtraction/absolute value in each declared field type and only
then converts to double for the source tolerance comparison. Double-valued
costs and comparison priority remain unchanged.

## Reproduce the original oracle

```
python3 scripts/editor_generation_cost_comparer_oracle.py \
  --reference-root /path/to/pinned/StepManiaLibrary \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/generation-cost-comparison/source.expected.json
```

The script reads the immutable commit through `git show`; it executes the
original complete `CompareTo` method unchanged, with field-only plumbing using
the original declared types. The manifest hashes the original file, method,
harness and fixture. The 144 cases use both f32 fields, six nonnegative finite
bit patterns, every pair and both random tie-break directions. The Rust test
consumes those original results. Construction, scoring and an entire generated
chart are not executed by this oracle. In particular, it does not prove a
configuration reaching this exact floating-point boundary.

## Audit checkpoint (not exhaustive certification)

| Contract inspected | Original evidence | Native evidence / limit |
| --- | --- | --- |
| Independent source / previous focused chart | ActionAutoGenerateCharts.cs:36–40,220–244 | generation/job.rs task snapshot and history.rs append_charts; independent picker already delivered |
| Expression, target performed search, output metadata | ActionAutoGenerateCharts.cs:130–196 | generation/job.rs:225–357; document.rs:394–474 clones current local metadata; timing/attack owners copied by history.rs:919–1024; effective inherited timing needs further coverage review |
| Preserved non-performance events | Fumen SMCommon.cs:1341–1358 | resident timing and chart attack cloning exist; not a claim every save/export permutation was executed in this audit |
| NPS and fallback/prune ties | PerformedChart.cs:459–495,1212–1250 | performed.rs average_nps/prune match inspected arithmetic and equal replacement; broader existing actual-engine fixtures remain separate |
| Pattern context and overlap | ActionAutoGeneratePatterns.cs:235–310,863–950 | pattern_job/region.rs, pattern_region.rs and pattern_schedule.rs; bounds/32-step extension and immediate-next clipping inspected |
| Pattern NPS | ActionAutoGeneratePatterns.cs:954–1022; EditorChart.cs:2007 returns0 | editor_timing.rs:835–851 and pattern_schedule.rs; start0 and last nonempty marker semantics match inspected code |
| Constructor repeated patterns | ActionAutoGeneratePatterns.cs:54–62 duplicates enumerable | native batch normalizes selected markers once. First duplicate's generated notes are cut off by its identical successor, and NPS counts are clipped likewise. No changed chart output demonstrated; no speculative fix applied |
| Cost priority and float threshold | SearchNode.cs:1438–1501 | confirmed precision mismatch corrected here; threshold oracle executes actual original method |

Existing `editor-generation-parity-reference.py`, mutations/active-witnesses
and pairs scripts execute actual original engines for their selected contexts.
`editor-pattern-actions-reference.py` explicitly records worker dispatch instead
of running `ActionAutoGeneratePatterns`: its callback/history proof does not
cover the actual constructor or asynchronous generation worker. Counts or
unchanged mutation outputs do not close unexercised scoring branches.
