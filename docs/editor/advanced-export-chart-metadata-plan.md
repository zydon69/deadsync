# Chart metadata export preflight follow-up

Frozen exporter sources have not been changed. The detached writer currently
refuses nonempty SSC-only chart fields on SM output through
`require_scope_mappings`; this prevents silent publication but is too generic
and blocks even equal music overrides. A precise review warning and scoped
writer acknowledgement must replace that guard before native UI publication.

## Real source evidence

`editor-chart-metadata-reference.py` ran 128 actual Fumen full reader → writer →
reader cases: MUSIC, CHARTNAME, CHARTSTYLE, CREDIT, SAMPLESTART, SAMPLELENGTH,
SELECTABLE and RADARVALUES × absent/empty/equal/different × SM/SSC × unchanged
source type / GA-style assignment of `Song.SourceType` to target. All saves
succeeded without exceptions. Fixture request/output/reference files record raw
bytes and pinned source/program/assembly hashes. This is not full EditorSong save.

The assignment distinction matters. With the GA-style assignment to SM:

- Different chart MUSIC becomes song MUSIC. An equal override retains effective
  audio but loses its independent chart scope. Empty MUSIC inherits song music
  in SSCReader before export. Warn about the different effective value, and
  explicitly review dropped chart scope where retention is requested.
- Different chart CREDIT becomes song CREDIT. Empty CREDIT inherits song author.
- CHARTNAME and CHARTSTYLE disappear. They have no song fallback in this probe.
- Chart SAMPLESTART, SAMPLELENGTH and SELECTABLE extras disappear; song-level
  values remain. These chart extras are not evidence of per-chart playback use.
- SSCReader stores RADARVALUES as a string, but SMWriterBase.GetRadarValues asks
  for List<double>. The actual writer emits empty radar even with matching source
  format. Native writer can preserve the raw valid header values, a retention
  extension; do not classify it as an unavoidable SM field loss.

Without the SourceType assignment the direct writer can choose fallback-chart
MUSIC for the song, and source-only song Credit/Selectable may disappear. Those
results must not be presented as GA's complete save behavior.

## Next implementation

Add typed preflight issues per source chart/GUID and omitted field, distinguish
metadata omission from effective song fallback, and require acknowledgement
before the detached writer drops reviewed fields. Keep unsupported unknown
scopes blocking. Compare typed numeric sample values to avoid warning solely for
spelling changes; retain exact raw bytes in source. Cover at least two charts
with different MUSIC so a fallback cannot hide the other chart's loss. Add tests
for absent/empty/equal/different values, anonymization interactions, unchanged
source/history, incomplete acknowledgements and strict target readback. The sidecar `advanced-export-chart-metadata.patch` now declares a child helper, adds
OmittedChartMetadata warnings and replaces the writer generic refusal only for
these known reviewed scopes. It is NOT applied. Two isolated Rust workflows
cover all128 source cases and multichart reviewed output, radar retention,
partial acknowledgements and anonymization. Cargo validation remains pending; no
UI export should be enabled yet.
