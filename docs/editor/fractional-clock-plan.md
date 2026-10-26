# Continuous chart-position clock

## Contract

GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
`StepManiaEditor/EditorChart.cs:1947–1962`, exposes
`TryGetTimeFromChartPosition(double, ref double)`. It selects the active rate
event before an equal chart position. It explicitly differs from event-time
queries, which account for the event's ordering phase.

`EditorTimeline::fractional_position_time` uses that position boundary with the
existing rate mapping math. `Clock::fractional_chart_time` and
`Clock::fractional_music_time` retain fractional rows and negative presentation
lead-in. They do not quantize or modify note-editing rows. Existing unsigned
integer APIs retain their range checks and event-order behavior.

A position inside a stop or warp is not an invertible time coordinate. These
APIs give the reference's position-based boundary; callers which already hold
an exact musical instant must retain it rather than round-trip through a row.

## Validation

Three new public workflows passed: fractional/negative positions with effective
offset and SyncOffset, exact equal-row boundaries and fractional positions
around stops/delays/warps, and fresh versus retained immutable clocks after
unsaved BPM changes. Nonfinite queries are rejected. Six existing playback and
three rate-mapping workflows also passed. The tests inspect working document
bytes, revision and dirty state to ensure that navigation does not edit data.

The new workflows are Rust checks against the cited source contract and existing
rate-mapping math. They are not a newly executed original C# timeline oracle.
Connecting ConstantRow minimap navigation to this API remains a separate step;
the first native minimap increment uses the exact ConstantTime axis.
