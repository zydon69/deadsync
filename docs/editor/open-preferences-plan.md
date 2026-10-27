# Preferred chart when opening a song

## Status and scope

New shared model, fixtures, modules, tests and integration patches are prepared.
None of this follow-up has been applied or compiled. The current Pack list's
application-setting inheritance fix is separate and belongs to its current
validation; this increment does not hide that correction.

Real .NET10.0.403 executed31 cases in en-US/fr-FR:58 successful selections and
four expected JSON conversion failures. The oracle compiles unchanged pinned
SelectBestChart, DefaultChartListProvider, ChartComparer, supported-type table
and default constants with property-only EditorSong/EditorChart adapters. The
actual Fumen PermissiveEnumJsonConverterFactory handles preference decoding.
It does not instantiate the complete EditorSong/UI/filesystem loader.

## Reviewed source

GA48e18f71655d0f0d86144c63239331833501fda2:

- EditorSong951–1033: complete SelectBestChart.
- EditorSong897–907: UpdateChartSortInternal sorts each type's list with
  ChartComparer before selection. Source file position is not this order.
- DefaultChartListProvider: complete original file, compiled unchanged.
- Editor156–175: original14 supported layouts; Couple entries commented out.
- PreferencesOptions88–89/124–125: default dance_single/Challenge.
- Preferences.cs serialization factory plus PreferencesOptions164–168:
  no explicit enum fallback registered for these two preference types.
- Editor5528–5551: guarded Open and provider built from common preferences.

Pinned Fumen0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89:
SMCommon.ChartType/ChartDifficultyType and real PermissiveEnumJsonConverterFactory.
The generation script verifies SMLbdbc427f/Fumen pins and clean source, reads
original editor blobs at the pin, and records Git blob IDs, source/extracted-body,
program/assembly/request/result SHA256 and SDK version. Only temporary generated
oracle files are written; no user pack is loaded or modified.

## Exact selection contract

Use the already existing cultural ChartComparer order, including difficulty,
rating, nonempty name/description, step count and GUID. Within that order:

1. Exact preferred layout/difficulty, first matching chart.
2. Preferred layout with difficulty priority Challenge, Hard, Medium, Easy,
   Beginner, Edit. This priority is independent of meter.
3. For dance-single/double or pump-single/double, try the paired layout in that
   same difficulty priority. Other types have no synthetic pair.
4. First chart in the original supported-type order, keeping its sorted order.
5. None when no editable supported charts exist.

Native TYPE_ORDER keeps the previously user-approved two Couple extensions;
original14 types retain their exact relative order. An explicit native-only
Couple/Routine test prevents inadvertently removing that functionality. Existing
Snapshot effective Pump Double→Routine comparison keys are reused; selection
never dirties or structurally normalizes source bytes.

## Common preferences

`open_preferences::Preferences` reads Options via the existing shared
`audio_preferences::File::opening()` getter. There is no second file, I/O adapter
or automatic write. Unknown Options and other groups remain preserved by the
existing conflict-safe preference writer. This increment adds reading/selection,
not a new Options editing panel or a separate persistence implementation.

Original missing fields retain initialized dance_single/Challenge. Present
invalid scalar values use enum zero (dance_single/Beginner), rather than the
missing-field defaults. Actual enum names use underscores and case-insensitive
parsing; whitespace and signed numeric strings are accepted, including unknown
numeric enum values. Named comma combinations are bitwise OR even for non-Flags
enums. Hyphen names, overflow and mixed numeric/name combinations fail to zero.
JSON numbers/null/booleans also use zero; nested arrays/objects fail conversion.
These surprising contracts are recorded by the real converter, not guessed.
The typed setter accepts canonical format names and Difficulty for future UI.

The existing43-type constant/canonical alias helper is extracted unchanged
from song_list into `document/chart_type.rs`. Pack metadata and preferred Open
reuse that one table; no tokenizer or note codec is duplicated.

## Integration patches

`open-preferences-core-hooks.patch` declares model/chart_type, moves the existing
metadata helper and adds the shared readonly Options getter.

`open-preferences-native-hooks.patch` changes strict preparation into a bounded
FileSession plus existing Snapshot built in the worker. The main thread asks
NativeChartOrder for its cultural permutation, validates it with Preferences,
rechecks SessionBinding/path/language and only then makes the Workspace/candidate.
No ICU object crosses threads. The existing application snapshot, Pack/song
confirmations, stale file guards and audio-command transfer remain in force.
A prepared candidate cannot be duplicated through Enter/F5 while awaiting order.
Shared process hooks explicitly accept both pack-row and chart orderers.

## Prepared tests and remaining validation

- `open_preferences_workflows`:7, both SM/SSC default selection, priority/pair,
  unsupported/chartless, invalid permutations/atomicity, common Options retained
  through audio saves, native Couple/Routine, effective StepF2 Routine keys.
- `open_preferences::tests`:1 comparing all62 original outcomes, exact decoded
  enum values and selected source indices. It consumes the original sorted
  indices to isolate SelectBestChart. Original ChartComparer/native ICU have
  separate differential coverage; this unit does not pretend to rerun ICU.
- `edition_preferred_song_open_workflow`:7 EN and corresponding FR target7:
  source default not first chart, explicit common preferences, missing preferred
  type pair fallback, invalid native chart permutation retains old editor, bad
  nonpreferred peer, unavailable preferred chart fallback and all-bad unfocused
  preservation with exact warnings/source bytes.
- Existing Pack song workflows must be replayed after the process signature
  update; the patch supplies their existing lexical chart-order test double.

All Rust targets above await central execution. Application settings are captured
at candidate publication preparation and remain separate from chart cursor/cache
state. Source preferences UI, generic New/Open/Recent and remembered per-song
active chart lists are later increments, not claimed by this selection contract.

## Loadable charts and unchanged strict boundaries

EditorSong753–771 catches a failed EditorChart setup separately and continues
other charts. Replacing the first-chart loader with global Snapshot::resident
would introduce a new failure when an unselected peer has an invalid grid/clock.
The separate open-loadable-hooks.patch therefore adds Snapshot::loadable and
Report{snapshot,failures}. The checked worker variant observes cancellation
between charts and after each existing parser; a cancellation returns no partial
Report and cannot be swallowed as an unavailable chart. It reuses the same private key/clock-building loop;
ordinary resident/read stay strict and preserve their existing errors.
Failures retain original source index, resident GUID and message; neither failed
nor unsupported source sections are removed or rewritten. Unknown formats stay
outside the editable set, as before. The editable Document tokenizer still
refuses missing/empty NOTES globally before this preparation; it is not relaxed.

Preferred selection only consumes actually prepared keys. A bad preferred chart
can fall back to valid peers. If all supported grids fail, new
Workspace::from_file_unfocused uses the common initializer without forcing one
bad grid; original source blocks remain available for later external repair or
addition of a new chart. The native candidate stores diagnostics and reports
unavailable count/first source-index error; song_open_warnings exposes exact
origin identities. Original source may omit failed charts during subsequent
save; native deliberately preserves those bytes rather than reproduce data loss.
This does not claim that every Rust grid/clock error is identical to Fumen's
full reader rejection set. Existing strict format/timing restrictions remain;
this lot changes error isolation at the preparation boundary only.

open_loadable_workflows adds six tests: good+bad/nonpreferred and preferred
fallback, per-chart clock error, all-bad unfocused retention, strict missing-NOTES
refusal, edit/Undo/Redo/save/reopen preserving the unavailable peer exactly, and checked
cancellation that never returns a misleading empty fallback. One private
loadable unit proves cancellation specifically between two chart preparations.
All these Rust tests and shared patches remain unexecuted/unapplied follow-up.

Existing full-editor import gap remains outside this selector: Pack metadata
canonicalizes Outfox SMX alias names, whereas raw Document steps types and the
current notes geometry inventory do not canonicalize those aliases during full
Open. This follow-up does not silently rewrite layout tags or claim to resolve
that wider import pipeline. The common chart_type table makes a later coherent
import adaptation possible without duplicating format inventories.

## Core integration checkpoint
Core preferences and tolerant per-chart preparation are applied. Seven preferred
selection workflows and six loadable workflows passed. The invalid-clock test
uses a genuinely nonfinite timing value; unparseable text is skipped by the
existing compatible loader and does not establish a clock failure. Integration
tests assert public revisions/GUIDs without exposing internal content tokens.
Native integration and private oracle/cancellation units are not yet validated.
