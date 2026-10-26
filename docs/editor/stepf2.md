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
The native inspector now offers eight typed rows for StepF2, with F2 switching
to the existing raw view on the same draft. Ordinary charts keep five raw rows.
Head/tail, visibility, fake and reserved edits are accessible with the keyboard.
Invalid raw text remains recoverable and cannot create a history entry.

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

Pump Double co-op normalization is implemented as an explicit structural
operation (`Session::normalize_stepf2_chart` / `Command::NormalizeStepF2`), exposed
in chart properties only for eligible legacy Double charts. It preserves source
comments, flags, peer edits and chart identities, and protects pending property
drafts. Its nine workflows passed for live layout, participants, undo/redo,
save/reopen and the added boundary cases. Merely viewing retains
the original Double descriptor and source bytes. Fumen's parser instead promotes
Double co-op to Routine automatically (`SMPropertyParsers.cs:1135–1146`).

Resident Session Pump Routine now defaults to two participants, matching
`SMCommon.cs:421`. Explicit GrooveAuthor metadata can lower this to one without
losing note owners, following `EditorChart.MaxPlayers` and its metadata loader
(`EditorChart.cs:472–500`, `3993–4004`). New unrelated Routine metadata retains the
default count. Low-level raw Grid parsing still reflects participant segments;
Session supplies the layout default. The revised differential test checks all
24 reader participant counts; both differential workflows passed.

Distinct combined visibility flags (for example `{S|h|0|0}`) are explicitly
refused while the typed model supports one visibility at a time; `{S|n|0|0}`
correctly retains Sudden. Unsupported charts remain byte-preserved.

Non-coop Pump special-note loading, explicit cross-notation clipboard conversion
and advanced export policy controls remain required work. Neutral StepF2
decorations must not be presented as standard StepMania note semantics when
transferring notes to a different notation.

## Current integration validation

The integrating agent confirmed the complete normalization lot passed nine core
workflows, the differential target passed both workflows (24 reader cases), and
participant-count (4), chart-property (7), metadata (11), conversion (3), and
StepF2 (16) targets passed. Both EN/FR native inspector targets passed seven
workflows each (14 total), including normalization actions. Existing
`edition_workflow` also passed 37 tests.

The four additional core boundary workflows also passed: normalization at a nonleading
chart index with two unsaved peer charts and undo after save; duplicate SSC
layout fields under the reader's last-value policy; unsaved P4 notes followed
by unrelated offset metadata; and metadata-only operations on unsupported raw
notes. The normalization target contains nine tests and its complete version passed.
Metadata-only initialization intentionally falls back to the Routine default
when notes cannot be decoded; its save path preserves those raw note bytes.
