# Original active-chart screen-position search

The spacing reference generator also copies the entire unchanged
`ActiveEditorChart.FindChartTimeAndRowForScreenSpaceY` member from the pinned
GrooveAuthor Git object into ignored `obj/SearchMethod.cs`. Its source/member
hashes are recorded in spacing-reference.json. SearchAdapter supplies fields,
scalar zoom/interpolation accessors and an indexed list enumerator.

The active section index is an explicit fixture input. The list adapter does
**not** reproduce or validate the original event tree's FindBest selection.
The directional loops, boundary tests, forward/backward rate changes and final
inverse calls are the original method, not a separately written reference walk.
The original spacing helper and rate-conversion members also remain unchanged.

Eighteen fixtures cover three modes, Reverse on/off, and positive/negative/zero
interpolation factors. Their rate arrays match the independently generated
initialized overlapping-warp/negative-stop section fixture. Each case queries
seven screen positions around three focal anchors (378 queries total).
Nonfinite C# inverse results are encoded as strings; Rust returns explicit
errors in these singular cases rather than passing NaN/infinity to rendering.

```sh
python3 scripts/editor-generate-spacing-fixtures.py \
  --editor-root /path/to/GrooveAuthor --dotnet /path/to/dotnet
cargo test -p deadsync-editor --test spacing_search_workflows --jobs 2 --locked
```

The generator now runs six scalar helper traces and eighteen search traces.
Native view wiring, automatic active-event selection and complete visibility
queries over folded geometry remain separate work.
