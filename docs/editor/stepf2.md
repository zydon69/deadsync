# StepF2 co-op notes

## Current domain implementation

`stepf2` decodes the shared ten-lane Pump co-op stream into the existing
participant-aware `Grid`. The reference alphabet is `X/x`, `Y/y`, `Z/z`, `1/2`
for P1–P4 taps/holds. A tail belongs to the owner of its open physical-lane hold.
The co-op decoder retains Fumen's starting participant minimum from the source
layout: one for Pump Double and two for Pump Routine. Encountered owners raise
that count, even when the effective Double layout is promoted to Routine by
Fumen. Detection is restricted to Pump Double/Routine and requires an exclusive
`X/x/Y/y/Z/z` note marker, following Fumen. Standard `1/2` alone retain their
normal P1 meaning. Comments and keysound suffixes do not trigger detection.

Session loading, serialization, editor playback input, scheduled attack input,
and generation source reading use the layout-aware decoder. Viewing a chart
alone preserves the original bytes. Changed StepF2 grids keep their source
notation. `FileSession` obtains the complete serialized result before opening
an atomic replacement, so an incompatible export cannot partially write a song.

Compound fields have a typed domain API (`stepf2::Attributes` and
`note_attributes::Draft`). Visibility, fake, and reserved flags remain attached
to their note endpoint using a neutral opaque suffix. Player reassignment can
therefore regenerate the correct embedded player symbol without losing flags.
Raw edits to a valid head compound synchronize Tap/Fake classification. Malformed
suffixes stay editable in the draft but cannot enter history; inconsistent external
note classifications are refused rather than silently rewriting the fake bit.
The existing raw inspector remains available; typed native controls are not
implemented by this increment.

## Reference evidence

`scripts/editor-stepf2-oracle` executes the actual pinned Fumen SM/SSC readers,
StepF2-enabled writers, then readers of writer output. Its 24 cases cover both
formats, Pump Double/Routine, all owners and holds, compound fake/visibility,
standalone visibility, late co-op markers, ordinary Pump notation, fake notes
and mines. The request/output/source/assembly hashes and commits are recorded in
`tests/fixtures/grooveauthor/stepf2-notes.reference.json`.

The oracle was executed successfully. Its C# writer preserves the tested owner,
kind and hold-end projections but discards visibility and a fake hold's source
flag. Rust retains these decorations. A compound fake tap becomes `Kind::Fake`;
a fake hold remains `Kind::Hold` with a retained decoration, matching the C#
note-kind projection. This does not prove additional gameplay semantics for a
fake hold. GrooveAuthor explicitly documents that it does not maintain
Sudden/Hidden/Vanish/Heart attributes.

All 16 Rust StepF2 workflow tests and both differential tests passed. The four
existing note-attribute inspector workflows also passed in the combined domain
regression lot. The oracle does not run the
GrooveAuthor UI, render notes, or validate DeadSync hardware/audio behavior.

## Export protections and remaining work

Strict StepF2 export refuses more than four participants, the wrong lane count,
rolls, lifts, unsupported note kinds, non-P1 mines, and physical-lane overlap
between participants. GrooveAuthor warns and flattens non-P1 mines; this codec
refuses that loss. Decorated notes cannot be pasted into standard charts through a Session
transaction, and standard serialization also rejects retained decorations in
transient grids. An output containing P4 notes without any exclusive co-op
marker is also refused because its ownership would be ambiguous on reload.
An explicit standard-notation export/conversion UI remains to be integrated.

Pump Double co-op data is decoded, but the resident layout descriptor has not
yet been normalized to Pump Routine as Fumen does. That normalization needs a
reversible structural operation and matching native layout behavior. This
increment does not claim complete Pump Double normalization parity.

For ordinary non-coop Pump Routine files, Fumen defaults to two participants;
DeadSync's existing raw-grid contract can expose one until other participant
metadata/segments are present. Differential baseline cases compare notes,
while co-op cases compare participant counts exactly. This discrepancy remains
to be reconciled against the editor's participant metadata contracts.

Distinct combined visibility flags (for example `{S|h|0|0}`) are explicitly
refused while the typed model supports one visibility at a time; `{S|n|0|0}`
correctly retains Sudden. Unsupported charts remain byte-preserved.

Non-coop Pump special-note loading, explicit cross-notation clipboard conversion,
advanced export policy controls, and native typed inspector fields remain
required work. Neutral StepF2 decorations must not be presented as standard
StepMania note semantics when transferring notes to a different notation.
