# Reproduce SM display-tempo source cases

```sh
python3 scripts/editor_display_tempo_source_oracle.py \
  --source /path/to/grooveauthor \
  --fumen /path/to/Fumen \
  --dotnet /path/to/dotnet \
  --output /path/to/sm-display-tempo.expected.json \
  --provenance /path/to/sm-display-tempo.provenance.json
```

Requires Python with `tarfile`'s data extraction filter and .NET10. The original Fumen project references Concentus2.2.2; it must be available to .NET restore (the replay here used the existing package cache). No Cargo or graphical editor is launched. Builds and execution occur in a temporary directory; the supplied source checkouts are read only.

The script reads original `DisplayTempo.cs` directly using `git show` at GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`. It builds the actual Fumen project from an immutable `git archive` at `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`, with no source edits and no unverified prebuilt DLL. The model's constructors/FromString/ToString and real SM/SSC readers/writers execute. The harness sets InvariantCulture as the original Editor startup does.

Source wiring from EditorSong/EditorChart is reproduced in the harness: copy the loaded global display into separate DisplayTempo objects; assign each serialized value to chart.Tempo; for SM use the fallback value on the song, for SSC remove the original global display tag. The provenance records hashes of these original routing files as well as the original model, Fumen archive/project, harness, fixture and SDK. This is not execution of the complete graphical Editor or its preflight checks.

Five writer/readback cases cover SSC local Specified/Random, local Actual/Random, SSC omitted timing with equal Specified values, and SM fallback Specified/Actual. Every writer return value, chart count, independent model identity and readback display value is checked. Source preflight rejects unequal display values when OmitChartTiming is enabled; the native workflow tests preserve that separate refusal.

The new script was replayed against pinned Fumen sources and produced a JSON fixture equal to the committed `sm-display-tempo.expected.json`. `sm-display-tempo.provenance.json` records that observed replay. No Rust/native test execution is claimed by this source oracle.
