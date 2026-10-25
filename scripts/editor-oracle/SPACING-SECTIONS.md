# Initialized spacing section reference traces

Regenerate the six `spacing-sections-*` cases with:

```sh
python3 scripts/editor-generate-spacing-sections.py \
  --editor-root /path/to/GrooveAuthor \
  --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet
```

This reuses the existing editor oracle development setup: pinned library/Fumen
checkouts and their .NET 10 assemblies, plus the ignored generated adapters from
`editor-generate-fixtures.py`, must already be prepared. Ordinary Rust tests need
none of those development dependencies.

The runner executes the unchanged pinned `RefreshEventTimingData` and
`RefreshRateAlteringEvents` members already used by timing fixtures. Scalar
`Init`/`UpdateTempo`/`UpdateScrollRate` adapters now retain the passed tempo and
scroll values, with mechanical getters. Original derived tempo/scroll getters
still return their event's source value. The new request flag returns initialized
rate state immediately after refresh, before generation/density operations.

The script verifies the library/submodule pins and records the original timing
method hash, adapter hash and reference assembly hashes. It also reruns and compares the old baseline,
warp and stacked-negative-stop reference outputs to ensure the added accessors
do not change the existing shared runner's outputs. This is evidence for rate
initialization, not for full native rendering, interpolated speeds or visibility
across nonmonotonic sections.
