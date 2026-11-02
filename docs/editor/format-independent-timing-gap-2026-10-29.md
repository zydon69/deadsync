# Confirmed open contract: editing timing after opening an SM file

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
This is a current source/code comparison, not an executed source oracle or a
completion certificate. The full editor port remains incomplete.

## Evidence

- Source `Editor.cs:1609` CanEdit checks song/chart/action locking, without a file
  format condition. `UI/UIEditEvents.cs:1112–1150` computes the insertion row,
  rejects same-type collisions and queues ActionAddEditorEvent.
- Source AddInterpolatedScrollRateEvent, AddScrollRateEvent, AddWarpEvent,
  AddFakeRegionEvent, AddComboMultipliersEvent and AddLabelEvent use this same
  format-independent path (`UIEditEvents.cs:1435–1522`).
- Native `quick_add.rs:222` routes Warp/Fake through timing_edit::region::Draft.
  That Draft rejects Format::Sm (`region.rs:132`). Speed, Scroll, Combo and Label
  inspectors have equivalent format guards. NegativeStopsToWarps also rejects
  an SM document (`timing_edit/conversion.rs:123`).
- Native `timing_edit::Target::capture` additionally disallows per-chart timing
  when supports_chart_timing returns false. That predicate describes the file
  format/version, not a format-independent resident editor chart.
- The current region integration test explicitly asserts that both Song and
  Chart Draft capture fail for SM. This is existing test coverage of a narrower
  contract, not evidence of parity with GrooveAuthor.

## Required implementation boundary

Preserve the original SM reader behavior: unknown native WARPS/SPEEDS/etc in an
ordinary SM source must not suddenly become active source timing. Do not merely
remove guards: GenerationTiming::load intentionally ignores those raw SM tags,
so the UI could claim a successful edit that does not affect its clock.

Add a typed resident timing owner independent of the source/export format,
including chart ownership, inheritance/detachment and exact edits/history.
Route timing inspectors, Quick Add, mixed selection/clipboard/transforms,
visual/audio/generation clocks and source-compatible output through it.

NativeLossless Save, recovery and reopen must retain authored values without
silently changing file format. Source-compatible SM output must use its existing
explicit omissions/conversion review; SSC output must carry the resident timing.
Keep CAS/revision and chart identity guards. Preserve previously valid source
import/legacy-normalization behavior and ordinary untouched lossless bytes.

## Evidence needed before closing this gap

An actual source oracle plus Rust public workflows opening SM, then adding and
editing each event type; separate chart timing; clock/spacing effects; cross-chart
copy; one-step Undo/Redo; save/recovery/reopen; SSC export; and reviewed SM losses.
Add a physical keyboard/native inspector test for the same path. Importing a
raw unsupported SM tag alone must retain its original ignored semantics.
