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
| `EditorLastSecondHintEvent.cs:57–68,96–104,130–145` | `song_properties::Draft::{last_second_hint,set_last_second_hint}`, native SongSample field and cursor shortcut, SSC-only save/undo/reopen test | A dedicated chart-position widget is still absent: current `sample_region.rs` caches only sample start/length/music/sync, and draws sample bounds, not the end-hint widget. Do not call the property itself missing. Both original selectability methods return false; no clipboard gap. |
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

## Real missing operation: chart-wide typed copying

`UI/UICopyEventsBetweenCharts.cs:39–63,269–349` and
`EditorActions/ActionCopyEventsBetweenCharts.cs:100–230` implement a separate
operation: choose event types, source and destination charts, replace the chosen
types in each destination, preserve the special first tempo/signature rules,
clone definitions and skip Patterns for original multiplayer destinations.
Current Rust Workspace commands provide cursor-relative CopyEvents/CutEvents/
PasteEvents, not this chart-wide multi-destination operation. Duplicating an
entire chart or pasting at row zero does not implement its replacement contract.
This is a genuine remaining command, **not another missing event type**.

A suitable next increment would first execute the unchanged original action in
an explicit development adapter, then add a detached plan and one reversible
multi-chart application. Fixtures should cover selected-type replacement,
unselected-type retention, initial tempo/signature preservation, same-head and
overlapping Patterns, unsupported Pattern destinations, owners, chart identities,
unknown metadata, atomic failure, and save/undo/redo in representable formats.
The native interface should expose type and destination choices through the
existing optional keyboard panels. No implementation has started in this lot.

## Small separate presentation increment

If chart-wide copying is deferred, the last-second-hint chart widget is a bounded
visual task. Reuse the existing property draft and mapped song/chart clock;
check sync/music offsets, fractional positions, visibility clipping, SSC support
and per-view projection. Retain keyboard access through the existing inspector.
Physical rendering and pointer behavior require their own validation and are
not demonstrated by headless actor tests alone.
