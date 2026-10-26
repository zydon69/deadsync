# Pattern markers in the mixed event clipboard

## Status and scope

Implemented and validated for this clipboard increment. The real original C#
oracle executed successfully with 15 cases under official .NET SDK 10.0.403.
The focused Rust suite passed six Pattern core tests (including 15 original cases
in SSC and 14 representable cases in SM), six existing mixed-clipboard tests,
and four native Pattern workflows per locale (eight EN/FR tests). Parent
validation terminal 11500 confirms the final corrected core fixtures; terminal
8341 confirms the native Pattern and 28 existing attack-event workflows.
These are headless checks, not a physical rendering or playback validation.

The detached clipboard now handles notes, timing/scroll/miscellaneous
StepMania events, attacks and saved pattern markers. Pattern markers
are actual selectable original editor events and belong in the requested
port. Preview regions and last-second hints are different: the original editor
marks them unselectable, so adding them to copy/paste would invent a capability.

References are GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`,
StepManiaLibrary `bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8`, and Fumen
`0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89`.

## Original behavior established from source

| Source | Contract |
| --- | --- |
| `Editor.cs:6130–6149` | Empty copy retains the previous clipboard. Selected events are cloned immediately and sorted, insulating the clipboard from later edits. |
| `Editor.cs:6156–6164` | Paste shifts the earliest copied event to the current row, clamped to at least row zero. The origin is the earliest event of any copied type. |
| `EditorActions/ActionPasteEvents.cs:58–76`, `125–176` | Preserve sorted processing, clone into the destination chart, shift rows, remap a single owner to the destination player and clamp unsupported owners in mixed-owner selections. `ForceAddEvent` captures side effects for undo. |
| `EditorChart.cs:2942–3100` | A miscellaneous event replaces existing events of the same concrete type at the same row. It does not remove a different miscellaneous type at that row. |
| `EditorChart.cs:3115` | A Pattern is skipped when pasting into a multiplayer chart. Other compatible events continue to paste. |
| `EditorEvents/EditorPatternEvent.cs:382–396` | Clone/GetDefinition clone the complete definition independently of the source marker. |
| `EditorPatternEvent.cs:440–448` | Pattern markers are selectable with modifiers, unlike ordinary lane-note selection. |
| `StepManiaLibrary/EventOrder.cs:24–31` | Pattern sorts immediately after `LaneTapNote` in the original event order. It is not an attack or a normal note suffix. |
| `EditorChart.cs:3935–3952` | Pattern definitions are saved by their head rows in chart editor metadata alongside, but independently of, chart offset/expression/player settings. |
| `EditorEvents/EditorPreviewRegionEvent.cs:246–254`, `EditorLastSecondHintEvent.cs:96–104` | Neither preview nor last-second hint is selectable with or without modifiers. Do not infer clipboard parity requirements from their rendered widgets. |
| `EditorActions/ActionCopyEventsBetweenCharts.cs:116–150` | Advanced type-based chart copying separately includes Patterns and skips them for multiplayer destinations. This operation is distinct from cursor-relative clipboard paste. |

A marker definition consists of pattern/performed profile GUIDs, length, seed,
start/end inclusivity and preceding-distribution policy. Copying a marker does
not generate, copy or delete its region's performance notes unless those notes
were independently selected. Overlapping marker regions remain independent;
only a same-type, same-head-row marker conflicts under ForceAdd.

The expressed profile belongs to the destination chart. Pasting a marker must
not replace that chart-wide setting with the source chart's expressed profile.
Profile GUID references are copied; profile files are not imported, renamed,
saved or repaired merely because the user copies or pastes a marker.

## Current Rust implementation

- `Event::Pattern { row }` selects marker heads independently of their region.
- Detached definitions preserve every field and nested unknown JSON value.
- Same-head paste replaces the definition; overlapping regions remain separate.
- Existing atomic edits cover notes, timing, attacks and marker metadata together.
- Cut/delete removes selected definitions while retaining generated notes.
- `PasteReport` returns changed state and counts of accepted/skipped markers and
  notes outside the destination lanes. Workspace returns the report directly;
  native EN/FR notifications display skipped counts without persistent reports.
- Force paste skips only invalid destination lanes, preserving compatible notes;
  strict note paste retains its equal-lane-count contract.
- The native mixed picker labels Pattern independently and reuses its keyboard
  selection, copy, cut and delete gestures.

## Implemented increment

1. Add `Event::Pattern { row }`, using the original Pattern comparison phase
   immediately after tap heads and before hold starts. Attack and delay phases
   precede tap heads in the original comparer. Capture marker
   **head rows** in range; a region crossing the selection boundary does not
   imply its head is selected, and its stored length is never clipped.
2. Capture selected definitions into a detached `BTreeMap<u64, Definition>`
   inside the existing clipboard. Deep-clone definitions and unknown fields.
   Include pattern heads in the global earliest-event origin. Do not retain
   source tokens, file handles, mutable profile objects or source chart IDs.
3. Before changing the destination, compute shifted rows with checked arithmetic
   and validate the complete resulting metadata, including `row + length`.
   Preserve unknown chart fields and unrelated metadata. Insert definitions at
   shifted heads with same-row replacement; allow legitimate region overlap.
4. Paste compatible markers only into original single-player chart families.
   Original multiplayer detection depends on the layout default, not the
   current occupied owner count. For Routine with explicit one-player metadata,
   patterns are still unsupported. Pattern-only paste there is a no-op; mixed
   paste still applies valid notes/timing/attacks. Report skipped markers through
   the existing localized status/report model instead of silently implying that
   every copied item was pasted.
5. Cut/delete remove only explicitly selected marker definitions. Keep chart
   expression, participant metadata, unselected markers and all unselected notes.
   Copy remains detached when the source marker is edited/deleted later.
6. Put all marker and represented event edits in the existing single atomic
   gesture, preserving attack anchoring. Invalid metadata or row arithmetic
   leaves both history branches and the complete destination unchanged.
7. Add native EN/FR selection labels and walkthroughs. Existing notes-only
   Ctrl+C/V behavior remains available; the mixed picker exposes Pattern choices.

SM/SSC cross-format copying needs no pattern format conversion: both formats
already store the same validated definition shape, with their own metadata
scope. No `#NOTES` decoration or standalone StepMania event is written for a
pattern marker.

## Known adjacent contracts to keep distinct

The mixed selection uses the original event phases: Attack 6, Delay 7,
LaneTapNote 8, Pattern 9, HoldStart 10 and other LaneNote 12. The original Fumen
comparer prioritizes lanes only when both events are lane notes, and types
otherwise; around Pattern and different lane-note types this can be nontransitive.
Rust deliberately keeps a deterministic total presentation order. The original
oracle verifies normal same-row phase ordering and business results, without
claiming identical presentation for ambiguous comparator cycles.

Original `ActionPasteEvents` tests individual lanes against the destination and
skips incompatible events. This behavior now applies to force paste and is
established by a real original cross-layout fixture. Pattern-only transfer has
no lane-count dependency. Strict note paste remains a separate public contract.

Original paste skips individual invalid rows, whereas existing Rust event
transactions reject invalid checked row arithmetic atomically. Preserve the
existing valid-input contract for this increment, test boundary failures, and
document any safety policy explicitly rather than assert that overflow behavior
is identical to the original unchecked integer path.

GrooveAuthor repairs unknown profile GUIDs while loading metadata
(`EditorChart.cs:4029–4048`). Rust preserves unknown metadata and existing
generation/profile workflows surface missing references. Clipboard should retain
GUIDs and not introduce an implicit profile repair side effect. A separate
explicit repair workflow can implement the original load policy where required.

Advanced whole-chart type-based event copying has different replacement and
first-tempo/signature protections. It remains a separate original feature;
cursor-relative marker paste is not evidence that it is implemented.

## Original oracle and tests

### Differential adapter

The development oracle executes unchanged original bodies for:

- `EditorPatternEvent.Definition.Clone` and `Matches` (serialization types are
  already extracted in `MetadataReference.cs.template`).
- `ActionPasteEvents.PasteUntransformedEvents` and its replay/undo paths, supplying
  a minimal editor/chart adapter for row, owner, event-kind and destination data.
- The lane-less conflict branch of `ForceAddEventInternal`, unchanged, including
  its side-effect list. The adapter must publish its event storage/comparison
  assumptions; it must not rewrite the conflict rule as expected JSON.
- Original event ordering via StepManiaLibrary/Fumen comparer with actual Pattern
  events. A handcrafted Rust ordering test is not a substitute for this check.

Record source snippets, pins, helper/adapter hashes, inputs, actual outputs and
assembly hash. Scope: original clone/shift/conflict/undo behavior with an explicit
event-container adapter. No claim of original GUI execution or GPU rendering.

### Core cases

1. Pattern-only SM/SSC copy/paste at a new row, same-row replacement, overlap
   without same-row conflict, deterministic replay, exact undo/redo/save/reopen.
2. All definition fields and unknown nested JSON survive detached copy, later
   source mutation, and destination application; destination expression/offset/
   player metadata and unselected definitions remain intact.
3. Mixed note + tempo/signature + attack + pattern selection with shared and
   different rows; earliest-origin offsets and original event order are exact.
4. Marker cut/delete leaves its generated notes; selective note cut leaves an
   unselected marker. Empty selection retains the prior clipboard.
5. Routine/Team with multiple owners and Routine with explicit one-player
   metadata skip markers; a compatible mixed selection still pastes other types.
6. Pattern-only cross-layout single-player and SM↔SSC transfers succeed; incompatible
   mixed note geometry skips only invalid destination lanes, preserving other
   events and returning an explicit skipped-note count.
7. Overflowing destination head/end, malformed metadata, stale selection,
   foreign Session identity and failed mixed transactions leave source/destination
   bytes, metadata, undo and redo intact.
8. Attack rows/times remain anchored when markers are pasted with timing/notes;
   a pattern definition alone has no timing or attack-placement effect.
9. Missing profile GUIDs remain explicit and preserved; clipboard never writes
   a profile directory or silently regenerates notes.

### Native EN/FR cases

Select individual pattern and note rows at a common head, copy/cut/delete,
paste at cursor, undo/redo, source edits after copy, same-row replacement and
mixed unsupported-marker reporting. Confirm independent checkboxes, bounded
pagination and labels with no raw template placeholders. These headless input
tests prove keyboard workflows, not physical rendering or audible playback.

## Completion condition for this increment

Pattern markers participate independently in detached mixed copy/cut/delete/
paste with original conflict behavior and the stated format/metadata contracts;
the original-helper fixtures and focused core/native workflows pass. Keep the
larger editor goal active: advanced chart-wide event copying and other remaining
features are not implied complete by this clipboard increment.

## Format boundary established during differential validation

The original ActionPaste oracle operates on in-memory events. Its
`pattern-replaces-same-head` case includes an unrelated Label at the marker head.
That complete case runs in SSC; it is explicitly classified as not representable
for SM rather than deleting the Label from expected results. Original pinned
Fumen `SMWriter.cs:101–109` emits BPMs, stops, delays, signatures and ticks, with
no Labels, while `SSCWriter.cs:111,236` emits song/chart Labels. Rust's
`timing_edit/clipboard.rs::Models::capture` likewise reads extended labels only
for SSC and its paste path refuses Label export to SM. Pattern same-head
replacement itself is covered for both formats by the detached-definition test.
The original 15-case fixture remains unchanged; Rust compares 29 format/case
combinations, including the complete Label case in SSC.

## Validated results for this increment

- Pattern core: 6 passed, including 29 source-oracle format/case comparisons.
- Existing mixed clipboard: 6 passed, including the corrected source-compatible
  cross-layout force paste and atomic failure/undo/redo contracts.
- Native Pattern: 4 English and 4 French workflows passed.
- Adjacent existing selection (6), force-paste (4), StepF2 conversion (3) and
  native attack-event (28) workflows passed in the parent validation runs.
- `git diff --check` passed. No tests were run by this subagent; Cargo execution
  and the final checkpoint regression are centralized by the parent agent.

The two fixture corrections used the real attack inspector API instead of the
generic text setter, which escapes structural ATTACKS separators. No production
parser or format capability was weakened to satisfy a fixture. The complete
original Label fixture remains in SSC and the explicit SM boundary remains as
documented above. Advanced chart-wide type copying remains a separate task.
