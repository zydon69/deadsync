# Editor-only event inventory after Pattern clipboard

## Scope and evidence

Read-only inventory against GrooveAuthor
`48e18f71655d0f0d86144c63239331833501fda2`, examining the current Rust
implementation rather than treating older pending labels as evidence.
References below are relative to its `StepManiaEditor/` directory. No production
change or new capability restriction is proposed by this inventory.

## Concrete event types

| Original type / reference | Current Rust counterpart | Actual remaining work |
| --- | --- | --- |
| `EditorPatternEvent.cs:382–396,440–448` | Validated detached Pattern definitions in mixed clipboard; marker inspector, saved regions and generation previews already exist | No missing Pattern clipboard event type. Original comparator ambiguity and SM Label boundary remain documented in `pattern-clipboard-plan.md`; this does not certify every original Pattern widget interaction. |
| `EditorPreviewRegionEvent.cs:126–135,185–220,246–268` | `song_properties::Draft` sample start/length and cursor shortcuts; `edition/sample_region.rs` caches fractional positions and draws region/boundaries; native sample audition | Original inline drag widget is not present. The metadata, region rendering and keyboard editing are already implemented. Both original selectability methods return false: it must not be added to event copy/paste. |
| `EditorLastSecondHintEvent.cs:57–68,96–104,130–145` | `song_properties::Draft::{last_second_hint,set_last_second_hint}`, native SongSample field and cursor shortcut, SSC-only save/undo/reopen test | A dedicated chart-position widget has now been added in `edition/end_hint.rs`, pending parent validation; previously `sample_region.rs` only drew the sample bounds. Do not call the property itself missing. Both original selectability methods return false; no clipboard gap. |
| `EditorSelectedRowsEvent.cs:95–144` | Workspace row selection and native selected-region shading (`edition.rs`, selection actor block) | Internal selection visualization, not persisted/copied data. Both selectability methods return false. Exact color/geometry/pointer parity is a presentation review, not an omitted event codec. |
| `EditorMarkerEvent.cs:12–38` | Existing measure/signature grid and annotations | Standalone rendering helper, not even an `EditorEvent` subclass. No clipboard type to import. Exact display styling is not certified. |
| `EditorSearchEvent.cs:3–32` and `EditorEvent.cs:223–231` search-rate variants | Rust ordered/indexed event searches and timing geometry | Internal comparison sentinels; not user data. Both Search selectability methods return false. No public event operation is missing just because these C# classes have no one-to-one Rust class. |

`EditorEvent.cs:155–247` constructs the ordinary Fumen-backed events and the
special Preview/LastSecondHint/SelectedRows/search variants. The ordinary note,
timing, attack and Pattern families are all represented by the current mixed
selection. Within the examined set, **no additional selectable persisted
editor-only event type remains missing from the mixed clipboard**. This is a
bounded conclusion about event types, not a claim that all editor operations or
all rendering are finished.

## Chart-wide typed copying: core implemented, native validation passed

`UI/UICopyEventsBetweenCharts.cs:39–63,269–349` and
`EditorActions/ActionCopyEventsBetweenCharts.cs:100–230` implement a separate
operation: choose event types, source and destination charts, replace the chosen
types in each destination, preserve the special first tempo/signature rules,
clone definitions and skip Patterns for original multiplayer destinations.
The earlier Rust commands only provided cursor-relative CopyEvents/CutEvents/
PasteEvents. The new Workspace::copy_chart_events implements the separate
chart-wide multi-destination operation. Duplicating an
entire chart or pasting at row zero does not implement its replacement contract.
This was a genuine missing command, **not another missing event type**. The
new `chart_event_copy` core now passes nine workflows and nine original action
comparisons; one helper unit and eight native EN/FR workflows also passed. See
`chart-event-copy-plan.md` for current implementation and format boundaries.

The new increment executes the unchanged original action in an explicit
adapter and applies a revision-bound detached plan as one reversible gesture.
Fixtures cover replacement, unselected retention, first tempo/signature values,
overlapping Patterns, unsupported destinations, stale/foreign identities,
late failure, no-op redo/token preservation and save/undo/redo. The native panel
exposes type and destination choices through an optional keyboard interface.

## Small separate presentation increment

The last-second-hint chart widget is a separate bounded visual task, now
implemented in `edition/end_hint.rs` pending parent validation. The intended
contract reuses the existing property draft and mapped song/chart clock;
check sync/music offsets, fractional positions, visibility clipping, SSC support
and per-view projection. Retain keyboard access through the existing inspector.
Physical rendering and pointer behavior require their own validation and are
not demonstrated by headless actor tests alone.
