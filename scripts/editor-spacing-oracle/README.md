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
The scalar helper runner does not reproduce the complete native renderer.
No Fumen or GUI assemblies are needed.

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


## Forward geometry sequences

The generator also runs eighteen Geometry traces (42 traces overall). Geometry
uses the unchanged original spacing helper and rate conversion members. Its
small driver reproduces the reviewed ActiveEditorChart transition sequence:
forward boundary Y before updating coefficients; backward coefficients before
boundary Y. The driver and row/order event selection are adapters, not an
unchanged copy of the complete renderer. Their hashes are recorded separately.

The 576 steps cover three spacing modes, both directions, positive/negative/zero
interpolation, repeated section traversals, and same-row event phases, including hold tails and cap offsets. Rust
selects its initial section automatically through Index; the oracle receives
that section explicitly. Separate index fixtures validate original tree lookup.
These traces prove helper arithmetic and the scoped transition sequence, not
viewport visibility, hold segmentation, or native display-mode integration.

```sh
cargo test -p deadsync-editor --test spacing_geometry_workflows --jobs 2 --locked
```

HoldEnd operations select the tail phase (11), call unchanged GetYForRow, then
apply the CheckForCompletingHold cap-offset formula. The completion method as a
whole is not compiled by this adapter. Reverse controls cap direction even when
constant/interpolated scroll is negative. Cap texture dimensions and clipping
remain native-renderer responsibilities.

## Waveform temporal spacing

The runner additionally compiles the complete unchanged private SetWaveFormPps
member from ActiveEditorChart into its scalar partial adapter. Six traces cover
all three spacing modes, all three waveform policies, and both Reverse flags:
450 calls include zero/negative/positive section and interpolation multipliers.
The returned speed is before view zoom and Reverse. MostCommonTempo is an
explicit scalar input here; its aggregation is validated separately below.
The generator now produces 48 spacing traces. Source/member/adapter hashes are
recorded in spacing-reference.json. This does not execute native waveform drawing.

```sh
cargo test -p deadsync-editor --test waveform_spacing_workflows --jobs 2 --locked
```

Seven waveform-tempo traces use the existing original timing runner's unchanged
RefreshEventTimingData and RefreshRateAlteringEvents methods. A new accessor reads
the computed MostCommonTempo field and initialized rate states. It adds no tempo
aggregation arithmetic. The fixtures cover equal durations, repeated tempos,
late first tempo, final rate cutoff, warp/stop timing and rounding history.
The rate phase accessor is a scalar adapter. Timing source/member and adapter
hashes are recorded in provenance.json.

```sh
python3 scripts/editor-generate-fixtures.py \
  --reference-root /path/to/StepManiaLibrary --editor-root /path/to/GrooveAuthor \
  --dotnet /path/to/dotnet --only waveform-tempo-
```
