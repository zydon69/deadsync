# Cross-format saving and advanced options

## Status and provenance

Source contract review plus an isolated Rust options/preflight implementation; no writer, shared mutation or Cargo run. The executed
`scripts/editor-save-format-reference.py` drives the pinned **real Fumen readers
and writers** through 60 synthetic cases. Request, emitted bytes, read-back
projections and reference hashes are in `tests/fixtures/grooveauthor/save-format.*`
under `crates/deadsync-editor`. All temporary input/output files are isolated.
The oracle does **not** execute GA's GPU-backed EditorSong/EditorChart constructors,
pre-save methods, anonymization or preference serialization. Those contracts
below are read from original source, not attributed to the writer oracle.

GA pin `48e18f71655d0f0d86144c63239331833501fda2`; SML pin
`bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`; Fumen pin
`0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`. Reference JSON records original
Editor/EditorSong/EditorChart/Preferences blobs in addition to writer/reader blobs,
SDK, program, assembly and request/output hashes.

## Six options, source defaults and meaning

Editor.cs:5248–5255 forwards these six Preferences fields to SaveParameters
(EditorSong.cs:56–119). Preferences/Preferences.cs:132–137 supplies defaults.

| Option | Default | Source meaning |
|---|---:|---|
| RequireIdenticalTimingInSmFiles | true | SM rejects unequal timing/scroll/offset/display tempo; false warns and uses TimingChart fallback |
| OmitChartTimingData | false | SSC removes chart timing only when all timing/scroll and split-trigger misc events agree |
| OmitCustomSaveData | false | Omits GA custom SongData/ChartData, including Patterns; warning for Pattern loss |
| AnonymizeSaveData | false | Clears the specific song/chart identity/media fields listed below |
| UseStepF2ForPumpRoutine | false | Writer receives UseStepF2ForPumpMultiplayerCharts; source preflight is narrower than writer |
| UseOutfoxFormatForSmx | true | Emits Outfox SMX aliases instead of canonical native names |

These fields live in the existing shared preferences document, not a new file.
Same-format lossless Save must retain its existing behavior; advanced output
requires an explicit immutable export plan and report, rather than making all
ordinary saves regenerate the file.

## Exact source preflight classes

EditorSong.cs:1476–1690 and EditorChart.cs:3395–3584:

- SM: Warp is a hard error. LastSecondHint, fake regions, labels, non-default
  combo multipliers, scroll rates and interpolated scroll rates warn of omission.
  Default row-zero rate 1 / instantaneous speed 1 and default multipliers avoid
  warning. Do not equate an explicit default tag with meaningful loss.
- SM: unequal attacks and tick counts warn only; selected TimingChart supplies
  shared data. Unequal timing/scroll is an error under the default strict option.
- SSC: negative stops are hard errors. With OmitChartTimingData, unequal timing,
  scroll, offset/display tempo **or** labels/fake regions/tick counts/multipliers
  are hard errors. Attacks are excluded from this split-trigger equality test.
- StepF2: GA's preflight checks only `pump-routine`: >4 players, Rolls and Lifts
  hard-error; P2+ mines warn that ownership will become P1.
- OmitCustomSaveData: Patterns warn that they will disappear. This is GA custom
  metadata, not a blanket permission to delete every unknown source tag.
- Opus warnings are extension checks, not an audio decoder or compatibility proof.

The timing comparator includes music offset, display tempo, rate-altering events
(BPM, stops, delays, scroll, signatures, warps) and interpolated speed events.
Compare typed/effective inherited values and every participating chart, with
explicit origin GUID/index; raw equality of timing tag strings is insufficient.

### Confirmed StepF2 source gap

Fumen SMWriterBase.cs:1339–1354 enables StepF2 for **any Pump chart with >1 player**,
including Couple. Its encoder drops owner >=4, converts Rolls to Holds, emits all
mines as unowned M, and cannot encode Lift as a dedicated StepF2 lift. GA's narrow
Routine-only guard does not protect Couple. The 60-case real writer oracle covers
both families and shows that the writer can report success while read-back loses
notes or entire charts. Native conversion must preflight **every chart actually
routed to this encoder**, and report/reject unrepresentable ownership and note
kinds before publication. This is a deliberate safety correction, not a claim that
GA already protects those cases. Do not just copy its incomplete chart-type guard.

## Writer behavior distinct from editor validation

EditorSong.cs:1810–1855 sets fallback DisplayBPM, custom properties, Stepmania
emission, least-common-multiple measures, ForceOnlySongLevelTiming and flags.
It sets `song.SourceType` to the output type even for cross-format Save, so a raw
Fumen reader→writer probe does not by itself reproduce the entire Editor path.
The current oracle explicitly exercises writer behavior, preserving its source
origin; a second matrix must cover this Editor source-type assignment before
claiming complete conversion parity.

SSCWriter.cs:198–249 disables all chart timing under ForceOnlySongLevelTiming;
its list includes offset, BPM, stops, delays, warps, signatures, tick counts,
combos, speeds, scrolls, fakes and labels. Chart attacks still have their own
emission. EditorSong config writes attacks from the fallback chart only for SM,
preventing SSC charts without attacks from incorrectly inheriting fallback ones.

SMWriter.cs writes song/shared timing and one NOTES header per chart. GA custom
chart metadata is flattened to indexed song-level keys in SM; SSC keeps chart
custom keys inside the chart. Unknown native tags, comments, duplicate properties,
unsupported charts and StepF2 decoration bytes need separate preservation/loss
classification; Fumen regeneration is not the existing lossless Document writer.

## Anonymize is a fixed field transformation

EditorSong.cs:1774–1806 clears Genre, Credit, Origin, Banner, Background, Jacket,
CDImage, DiscImage, CDTitle, LyricsPath and PreviewVid. It retains Title, Artist,
Music, preview music, timings and sample bounds. EditorChart.cs:3892–3908 clears
Description, Author/Credit, ChartName and ChartStyle. Unknown fields are not
universally scrubbed, and this feature must not be presented as guaranteed removal
of all identifying information. Custom metadata is controlled separately by
OmitCustomSaveData; both option combinations need explicit tests.

## Proposed safe increments

1. Immutable `ConversionPlan` bound to source Session/content token, chart GUIDs,
   source format, timing-chart GUID, options, source/destination baselines and
   resource caps. Build a typed loss report before any I/O or live mutation.
2. Preflight all charts, including currently unsupported/raw peers. Error on
   unknown transformations rather than silently skipping them. Reports identify
   chart, tag/note/owner, original semantics, replacement/omission and severity.
   Preserve all source bytes and live history if preparation fails or is cancelled.
3. Add source-type-assignment and GA transformation oracles; compare exported
   bytes through the actual target reader, including all ownership and timing
   events. A successful writer bool is never sufficient evidence of preservation.
4. Implement explicit output generation in isolated modules. Keep original Save
   and same-format Save As separate until new writer publication/history semantics
   are designed. Cross-format Save As cannot reuse old raw ranges in undo payloads
   without a migration contract; initial export can leave the live Session/source
   binding untouched and report that choice honestly.
5. Storage reuses the durable atomic publication contract; source/current Session
   is unchanged on preparation failure, destination conflict, pre-publication I/O
   failure or cancellation. A published-but-not-durable receipt remains recoverable
   and cannot masquerade as a clean save or unpublished output.
6. Native keyboard review lists exact losses, source TimingChart selection and six
   options EN/FR; defaults protect timing. Confirm a concrete prepared output plan,
   invalidate it on changes, and preserve Pack dirty/pending and Session guards.

## Required follow-up matrix

Both formats and directions; zero/one/multiple charts; shared/inherited/split timing;
offset/display tempo only differences; every timing and misc class above; attacks
with and without split-trigger fields; negative stops versus warps; custom Patterns
and unknown metadata; each anonymize/custom combination; all three SMX families
with Outfox on/off; Routine/Couple normal streams versus StepF2, 1–5 owners, Mines,
Rolls, Lifts, Fake/visibility/reserved/trailing attributes, shared-lane collisions;
unsupported/raw peers; cancellation and byte/event caps; late I/O and conflict
failures; source/history/redo/identity unchanged; target-reader semantic read-back.

Eight Rust public workflows are now prepared in `advanced_export_workflows.rs`, not executed. The isolated `advanced_export` module holds typed options, a bounded immutable source plan and concrete warning/error reports. `advanced-export-hooks.patch` declares it and integrates the six options into existing conflict-safe common preferences; it has not been applied.
The 60 writer/read-back cases are an initial characterization, not full coverage
of this required matrix or validation of production conversion.

## Implemented preflight boundary (not yet validated)

`Plan::prepare` serializes pending notes into an immutable snapshot, reuses strict
Document, typed timing parsers, GenerationTiming, ChartData and StepF2 validators.
It never publishes, writes a destination or changes source history/dirty/redo.
It validates the timing-chart index, byte/chart/note/issue caps and cooperative
cancellation between charts and every 1024 note heads. Existing tokenizer/grid
parsing remains synchronous; the cap is not a claim of cancellable inner parsers.

Unknown properties and unsupported layouts block generation pending a proven
mapping. Routine StepF2 decorations accepted by the existing codec remain valid;
standard export rejects decorated StepF2 notes until explicit conversion exists.
Native Pump Couple StepF2 export is blocked until an explicit Routine layout
conversion, rather than emitting unreadable notes. StepF2 mines owned by P2+ are
reported as a concrete warning (source flattening policy); a future writer must
implement that acknowledged transformation or refuse it, never bypass the existing
strict StepF2 serializer. `validate_review` checks all warning indices, all blocking
errors and exact source ContentToken/GUIDs; it is neither a write receipt nor proof
that a writer has been implemented.

The current API requires at least one editable chart and an explicit timing chart.
Empty-song export, full conversion publication/history migration and all writer
bytes remain future work. The original ordinary Save path is untouched.

### Detached writer preparation

A first isolated writer now exists in
`advanced_export/preflight/writer.rs`, still undeclared via the separate
`advanced-export-writer-hooks.patch`. Five public writer workflows are prepared, including all 60 original writer cases: blocking preflight cases must refuse, and accepted outputs compare chart/player/note read-back with the real writer fixtures.
It emits bounded SM/SSC output in memory, reuses native note/StepF2 serializers,
maps known custom metadata scopes, and applies acknowledged mine ownership loss.
It never writes files or publishes/rebinds Session/history. It requires the
song-custom hooks and participant-default hooks for their respective source
contracts before validation.

Before returning Output it parses the target with strict Document and verifies
chart count/layout, every note owner/kind/end/decoration, declared participants,
effective timing and retained SyncOffset. Unknown property scopes, fixed-layout
multistreams rejected by the original reader, non-integer ratings without an
explicit mapping, chart-scoped properties that SM cannot preserve, or mismatches
refuse output. These cases still need richer preflight issues before native UI;
a generation error is safe but does not complete the desired review experience.

The output buffer checks each append before allocation. Existing per-grid
serializers still allocate their own guarded buffers (up to their established
128 MiB limit) and are synchronous. This is not a guarantee that peak memory is
bounded by the requested final output size, nor cancellation within those codecs.
A prepared Output holds exact source content/GUID identity; durable storage,
destination conflict handling and cross-format publication remain unimplemented.
