# Pack song discovery and presentation

## Status and validation

The core metadata/list increment is implemented and declared. Parent applied the
shared Document tokenizer extraction and `pack::song_list` declaration.
Central validation passed: `document_metadata_workflows` 5,
`pack_song_list_workflows` 9, `pack::song_list::tests` 3,
`editing_workflows` 13 and `property_workflows` 2 (sessions 56098/59366).
These are core checks, not a native GUI validation.

The real original .NET 10.0.403 oracle executed 116 directory cases: 18 base,
six numeric boundaries, 45 grammar forms in both SM/SSC, two mixed-rating cases.
115 rows loaded; the uppercase-only directory is intentionally excluded.
Original culture orders en-US/fr-FR, pre-load cancellation, comparer matrices,
indices and provenance hashes are retained in `pack-songs.*.json`.
It compiles unchanged pinned PackSong.cs against real Fumen. PackLoadTask/UI
were reviewed, not executed by this harness.

Native list/loading/collation modules and their integration patch are a separate
increment in preparation; they have not been compiled or applied yet.

## Reviewed source and provenance

GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`:

- `StepManiaEditor/PackLoadTask.cs`: complete file; exact Pack.ini discovery,
  immediate child-directory enumeration, extension selection, Task.WhenAll,
  failed-song exclusion, comparer, cancellation and banner phase.
- `StepManiaEditor/PackSong.cs`: complete file; actual metadata reader, SSC
  preference, ratings cache and both comparers. This whole unchanged source is
  compiled by `scripts/editor-pack-songs-reference.py`.
- `StepManiaEditor/UI/UIPackProperties.cs:19–27,41–54,215–291`: title, artist,
  credit, ratings and Open columns; Open disabled when Editor.CanLoadSongs is
  false; selected file sent to Editor.OpenSongFile after the table.
- `StepManiaEditor/EditorPack.cs:250–310`: pack is the active song folder's
  immediate parent; old results cleared, cancellable load, result installation,
  work-queue edit guard.

Fumen `0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`:

- `Fumen/Converters/Reader.cs`: real LoadMetaDataAsync contract.
- `SSCReader.cs:102–122,245–264,535–557`: metadata-only read and chart acceptance.
- `SMPropertyParsers.cs:1365–1424,1484–1518`: integer SM meter, known layout
  normalization/exclusion; SSC uses property conversion to double instead.
- `SMCommon.cs:17–102`: complete format layout names, canonical names and aliases.

The generator verifies clean SML/Fumen pins, reads original Git blobs at the
pins, and records Git object IDs, SHA256, real .NET SDK version, assembly hashes
and request/output hashes in `pack-songs.reference.json`. Temporary directories
are deleted. Only this generated scratch data is written by the oracle.

## Source contract, including surprising behavior

Discovery:

- Enumerate immediate child directories only. Simfiles at the pack root and
  grandchildren are not searched.
- A candidate file extension must equal lowercase `.ssc` or `.sm`; uppercase
  names are excluded by PackLoadTask, even though Reader's extension factory is
  itself case-insensitive.
- Retain one candidate of each format: the **last entry returned by GetFiles**.
  The filesystem enumeration order is not specified. Prefer selected SSC to SM.
  Do not fall back to SM if that SSC reader fails.
- Load metadata rather than note grids; chartless songs remain visible.
- Original launches one Task per song without a concurrency cap. After WhenAll,
  false results are removed, and remaining songs sorted. An exception in the
  outer load clears the whole pack result. Cancellation is checked between
  discovery, loading, sorting phases, and by actual readers.

Presentation:

- Display original Title, Artist, song-level CREDIT extra and cached Ratings;
  null values render empty, with folder retained as a separate identity.
- Sort by nonempty TitleTransliteration else Title (null -> empty), then folder
  name, both StringComparison.CurrentCulture. The UI does not offer search
  filters, selectable sort columns or chart-type filters. Such UI features would
  be deliberate additions, not missing source requirements.
- Ratings sort first by the exact 16-layout table also used by chart ordering.
  Other known Fumen game layouts follow those layouts.
- **Actual source bug:** the comparer branch labeled DifficultyType queries
  `DifficultyTypeOrder` using `c.Type`, not its DifficultyType. Consequently,
  ordinary same-layout charts sort by numeric meter, not difficulty names.
  Do not silently replace this with the general chart comparer.
- Ratings append `(int)DifficultyRating`, commas within consecutive same Type,
  `|` when the Type changes. The actual fixture
  `ratings-layout-meter` yields `7,7,9,15|12|13`, with Beginner15 after Hard9.
- Fumen's SM meter parser uses `int.TryParse`: `2.9` becomes zero. SSC meter is a
  double: `7.9` sorts as 7.9 and displays `7`. Unknown layouts are omitted by the
  original type parser; this is format recognition, not the 11-layout generation
  capability guard or the 16-layout editor-geometry limit.
- Do not promote PumpDouble StepF2 to Routine based on note grids during this
  metadata-only list read; it would add a full note parse and alter this source
  metadata path. The eventual full editor open can use its normal effective type.

## Shared readonly metadata reader

`document::metadata::Metadata` reuses the existing private `parse_parts`
tokenizer. It accepts known chart headers without NOTES, as the original Fumen
metadata reader does; strict `Document::parse` still rejects missing/empty note
data. The strict scanner already scanned the complete source before its final
NOTES validation. A later syntax error therefore keeps its existing priority;
comparative multi-chart tests cover these boundaries and original source ranges.

The readonly chart type has no note range, writer, edit conversion or fake NOTES
span. Optional source/tag/chart limits affect readonly reads only. This avoids
both a second parser and rssp's omission of SSC charts without NOTES. Malformed
note grids are irrelevant to this metadata read; explicit editor Open still
uses the strict existing loader and can fail without replacing the current song.

## Core API and bounds

- `Task::from_song_file(path, limits)` binds the canonical source file and its
  immediate pack directory identity. Editable text never controls file targets.
- `Task::run(reader, cancelled, progress)` / `spawn()` uses one worker and
  sequential metadata reads. No note Grid, audio decoder or generation runs.
- `Reader::read(bytes, format, limits)` receives the task's limits on every call.
- `Catalog` contains readonly rows and structured failure/ambiguity issues.
  `Row::is_current` checks logical/canonical identity, size and modification time
  before a later Open. `Selection` reconciles by canonical path after sorting.
- `Orderer` injects cultural ordering and must return an exhaustive permutation;
  invalid results are rejected atomically. Core lexical mocks are not cultural
  parity tests. Native ICU/CLDR versions can differ across platforms.

Default limits: 16,384 immediate directories, 262,144 enumerated entries,
16,384 entries per song directory, 16 MiB per file, 512 MiB total read bytes,
32 MiB retained text, 4,096 charts and 65,536 tags per song. Chunked reads detect
growth. Overall limit/cancellation failures publish no partial catalog;
per-song failures are reported as issues. Cancellation is checked on enumeration,
each read chunk, before/after metadata parsing and before publication. A single
synchronous parser call cannot be interrupted internally; its capped input
bounds that work. No user files are written.

Direct song-directory symlinks are followed without recursion; canonical target
aliases are diagnosed/deduplicated. Nested cycles cannot recurse. Root identity
is checked again before returning. The native bridge adds SessionBinding and
language/request guards; metadata alone does not authorize a transition.

## Explicit adaptations from source

Source GetFiles enumeration order is unspecified. Native discovery selects the
last ordinal-sorted same-extension filename and reports ambiguity. The oracle's
host picked a.ssc despite creation order a then z; that is not a portable source
ordering contract. SSC remains preferred and a failed SSC never falls back to SM.

Source unbounded Task.WhenAll becomes one bounded worker. Root/global failure
is fatal; individual failures are reported rather than hiding the whole pack.
Culture-equal rows retain deterministic discovery order. There is no new search,
chart filter, selectable sort column or file deletion behavior.

## Numeric grammar and order proofs

SM uses signed i32 parsing. SSC uses the observed invariant Float|AllowThousands
contract: commas are allowed only in the integer portion after its initial
digit and optional single sign, including repeated/trailing groups; commas in
decimal/exponent portions are invalid. No blind comma removal is performed.
The 45-form SM/SSC probes cover signs, separators, decimal/exponent syntax,
whitespace and invalid forms. SSC rejects inf/-inf/+inf/INF but accepts full
case-insensitive signed Infinity/NaN and exponent overflow. Symbols are handled
explicitly before Rust floating parsing; SM rejects them via its integer reader.
One unit compares all 96 actual reader doubles, independently of integer display,
so replacing 0.01 with zero cannot pass simply because both display zero.

On the recorded .NET runtime, formatter casts saturate: NaN displays 0, positive
infinity/overflow i32::MAX, negative i32::MIN, matching Rust's display cast.
All 115 loaded rows retain complete metadata/ratings comparison.

The unchanged comparer uses the sign of double subtraction, without an integer
cast. Close finite decimal index order exactly matches native ordering.
Its actual matrix returns -1 for NaN/NaN and infinity/infinity and equates ±0;
this is not a valid Rust sort comparator. Native total_cmp is reflexive, keeps
identical infinities stable, puts negative zero before positive zero and canonical
positive NaN last. The mixed fixture/unit records and asserts both exact source
indices and the differing native indices, although integer display coincides.
No general pathological presentation-order parity is claimed.

## Native boundary being prepared

Dedicated native modules prepare a keyboard list, cancellable discovery and
strict loading, ICU ordering on its owning main thread and deferred candidate
publication. Shared hooks remain isolated in `pack-song-native-hooks.patch`.
Opening must preserve song dirty state, Pack draft and pending receipts until
explicit save or discard choices resolve them. Keeping edits cancels opening;
there is no implicit stash. Failed or stale loads retain the current session.
Native workflows and actual cultural differential checks remain pending.
