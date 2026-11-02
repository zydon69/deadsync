# Generated chart metadata: explicit correction of source stale-Extras loss

Source GrooveAuthor48e18f71655d0f0d86144c63239331833501fda2.

## Executed original witness

The complete ActionAutoGenerateCharts class executes unchanged, together with
the entire original EditorChart(EditorSong,Chart) constructor and unchanged
SaveToChart, SerializeCustomChartData, DeserializeCustomChartData,
DeserializeV1CustomData and GetMusicOffset. Actual Fumen WorkQueue/Extras and
actual expression/performed graph engines execute. See provenance for declared
container/event-emitter/profile-accessor/dispatcher plumbing. This is an action
metadata witness, not a full editor or general engine-parity oracle.

Live state is pattern96, NoBrackets expression, explicit offset0.375 and display
range140:180. Three cases execute actual generation, Undo and Redo:

| Originally loaded metadata | Original generated state |
| --- | --- |
| No custom metadata; original display90 | No patterns, Dynamic expression, offset override disabled (effective song offset1.25), display90 |
| Pattern48/Dynamic/offset-0.25/display90 | Reverts to pattern48/Dynamic/-0.25/90 |
| Originally already matching live values | Preserves the live values |

The source SaveToChart copies OriginalChartExtras (EditorChart.cs:3879), writes
current custom data into a separate dictionary (3911,3941–3953), and removes
source OFFSET while supplying the current ChartOffsetFromMusic (3907–3908).
ActionAutoGenerateCharts.cs:198–202 ignores the custom dictionary `(chart, _)`.
Its generated Chart copies chart.Extras (181) and the current Tempo and offset
fields, but the EditorChart constructor then loads display/custom data from
those old Extras (550–567;3960–4057). The matching-original control demonstrates
that this is state-dependent stale-data resurrection, not a deliberate policy
to reset all generated patterns/expression/offset/display.

**Native contract deliberately corrects that source bug:** generated charts
retain the current resident authoring metadata and unknown fields. We do not
replace current native data with an obsolete loaded-file snapshot to achieve
blind equality with data loss in the source.

## Confirmed native SM omission fixed

SSC chart blocks already contain current chart-local GA metadata, so cloning
the current block retains it. SM GAChartDataVersionCHART#### and
GAChartDataCHART#### are global tags outside NOTES. Previously append_charts
cloned only resident timing/attack owners after cloning NOTES, losing GA state.
A shared metadata::clone_chart_owner now copies the current raw owner (including
unknown extensions) for both generation append and duplication. It refuses an
occupied destination owner in the staged document; no Session mutation,
history entry or chart GUID change occurs on collision. No source snapshot or
current metadata is rewritten by this helper.

New native tests cover absent/stale loaded GA data, live property/pattern/profile
edits, generation, actual save, reopen, then Undo/Redo in SM and SSC. All current
GA data, unknown chart/marker fields, five text fields, display range, offset,
source/peer notes and target geometry are asserted. Assertions compare canonical
native output bytes where publication normalization is part of the contract,
not a source prefix. An orphan destination data tag with no version exercises
atomic refusal after the first staged owner field could have been copied.

## Intentional replacements inspected separately

ActionAutoGenerateCharts.cs:164–186 explicitly constructs:

- New generated performance events and copied nonperformance events; native
  generated Grid replaces notes and resident timing/attacks retain their owners.
- Requested target Type, NumPlayers1 and target NumInputs. Native source
  eligibility requires one participant and target Grid/layout validation remains
  strict; the new test asserts eight target columns for dance-double.
- A new EditorChart (new GUID at EditorChart.cs:517). Native creates new chart IDs
  on append; no source chart identity is copied.
- Description/Credit/Music, artist/genre/transliteration, difficulty/rating,
  tempo/offset and Extras are explicitly copied, not intentionally cleared.
  SaveToChart updates editable name/style/music/text before that copy.

The stale custom/display behavior above must not be conflated with those
intentional geometry/identity replacements. This lot does not certify invalid
or fractional raw identity normalization, every inherited-timing permutation,
all original worker error branches, or unknown future metadata semantics.

## Reproduce

```
python3 scripts/editor_generated_metadata_source_oracle.py \
  --repo-root /path/to/deadsync \
  --editor-root /path/to/pinned/grooveauthor \
  --reference-root /path/to/pinned/StepManiaLibrary \
  --dotnet /path/to/dotnet \
  --output crates/deadsync-editor/tests/fixtures/generated-metadata/source.expected.json
```

The build is temporary and repository paths are parameters. Original source and
extracted method/model hashes, fixture/config hashes and explicit oracle limits
are saved separately in provenance. The original constructor random seed is
observed then set to42 through reflection for deterministic replay; no source
algorithm body is changed. Root owns Cargo execution of
`generated_metadata_workflows`; this sidecar agent only ran the .NET oracle,
syntax parsing and patch applicability checks.
