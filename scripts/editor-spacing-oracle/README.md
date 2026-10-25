# Original spacing helper reference runner

Development-only .NET 10 runner for the entire unchanged `EventSpacingHelper.cs`
Git object at GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
The generator copies that exact source into ignored `obj/`. Its SHA-256 and pin
are recorded alongside the committed request/expected fixtures.

```sh
python3 scripts/editor-generate-spacing-fixtures.py \
  --editor-root /path/to/GrooveAuthor --dotnet /path/to/dotnet
```

`Adapters.cs` provides the helper's scalar preferences, event/region accessors,
and direction flag. Rate conversions are **explicit linear fixture callbacks**
using the supplied time, row and seconds-per-row. These callbacks deliberately
are not a reconstruction of EditorRateAlteringEvent's stop/warp/delay semantics.
Thus the traces compare the original helper's arithmetic and dispatch, previous
rates, signed geometry and region decisions; they do not establish full chart
projection or actual native rendering across rate boundaries. No Fumen or
original GUI assemblies are needed by this standalone runner.

Six fixtures cover all three modes in both directions, with three successive
rates and four query sets per rate. Valid nonzero rates are used for inverse
comparisons. Rust additionally checks invalid inputs, failed callbacks, missing
rate initialization and the zero-variable-rate case: forward geometry remains
valid but inverse projection returns an explicit error instead of NaN/infinity.

Ordinary tests need no .NET runtime:

```sh
cargo test -p deadsync-editor --test spacing_workflows --jobs 2 --locked
```
