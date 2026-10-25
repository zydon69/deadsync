# Original spacing helper reference runner

Development-only .NET 10 runner for the entire unchanged `EventSpacingHelper.cs`
Git object at GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
The generator copies that exact source into ignored `obj/`. Its SHA-256 and pin
are recorded alongside the committed request/expected fixtures.

```sh
python3 scripts/editor-generate-spacing-fixtures.py \
  --editor-root /path/to/GrooveAuthor --dotnet /path/to/dotnet
```

`Adapters.cs` provides scalar preferences, event/region accessors, rate fields
and the direction flag. The two rate conversion members are extracted unchanged
from the pinned `EditorRateAlteringEvent.cs` into an ignored partial class. The
source and extracted-member hashes are recorded in the fixture manifest. These
are the original stop/warp/negative-lead-in formulas, not linear replacements.
The runner does not emulate EditorChart event selection, traversal or complete
native rendering across rate boundaries. No Fumen or GUI assemblies are needed.

Six scalar helper fixtures cover all three modes in both directions, with six successive
rates and four query sets per rate (144 query sets), including positive/negative
stops, warps and negative time/row queries. Valid nonzero rates are used for inverse
comparisons. Rust additionally checks invalid inputs, failed callbacks, missing
rate initialization and the zero-variable-rate case: forward geometry remains
valid but inverse projection returns an explicit error instead of NaN/infinity.

Ordinary tests need no .NET runtime:

```sh
cargo test -p deadsync-editor --test spacing_workflows --jobs 2 --locked
```

The same generator also runs eighteen original active-chart search fixtures;
see SEARCH.md for their event-selection scope and reproduction.
