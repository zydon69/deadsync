# Speed interpolation with actual chart event times

```sh
python3 scripts/editor-generate-scroll-interpolation.py \
  --editor-root /path/to/GrooveAuthor \
  --reference-root /path/to/StepManiaLibrary --dotnet /path/to/dotnet
```

The existing editor-oracle development runtime must already be prepared (pinned
library/Fumen checkouts, .NET 10 assemblies and ignored generated adapters).
Ordinary Rust tests need none of those dependencies.

Generation has two reference stages. First, the unchanged original editor timing
pass assigns timestamps to Fumen speed-marker events at their actual ordering
phase. Their placeholder speed values do not affect timestamps. Second, these
timestamps are supplied through a scalar getter override to the unchanged
original row/time speed interpolation members. The previous linear Row/96 getter
remains the default for the existing speed-edit fixtures. Row selection uses the
same sorted-event adapter as those fixtures; it does not run the original GUI
or event tree implementation.

Six cases compare actual speed origins and 84 interpolation queries, including
same-row delay/stop/warp ordering, tempo changes, negative stops, signed/zero
rates, late first events, fractional and negative positions, and empty metadata.
The generator checks that prior speed-seconds, speed-row-interruption and editor
timing baseline expected outputs remain unchanged. Pins, original method hashes
and adapter hashes are recorded in scroll-interpolation-reference.json.

These fixtures prove the timestamp/interpolation composition, not native visual
spacing traversal or waveform rendering in variable mode.
