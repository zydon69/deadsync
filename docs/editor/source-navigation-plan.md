# Six original navigation callbacks

## Delivery and validation state

- Patch: `editor-navigation6.patch`, based on primary Action184 with source39, Density/Dark and watcher. Adds six real callbacks; Action190 afterward. `git apply --check` passed against primary on delivery.
- No primary implementation changes, Cargo execution, staging or commits by this agent.
- Original C# oracle built and executed successfully: 768 queries, eight event trees, sixteen fractional cursor positions and six unchanged callback methods. A Python transcription of the Rust lookup matched all 768 expected results.
- Rust syntax parsed by rustfmt (isolated copies, no Cargo). Five new integration tests and three private native tests are delivered but NOT executed. Parent must run `navigation_source_workflows`, key preferences/source profile tests, existing autoplay tests and native editor tests serially in EN/FR.
- Durable original runner: `scripts/editor-navigation6-reference.py`; output and request/provenance fixtures under `tests/fixtures/grooveauthor/navigation-source6.expected*`.

## Exact source contracts

Source pin 48e18f71655d0f0d86144c63239331833501fda2, Editor.cs registrations700–705 and callbacks6404–6572. IDs `MoveToPreviousRowWithSteps`, `MoveToNextRowWithSteps`, `MoveToPreviousRowWithEvent`, `MoveToNextRowWithEvent`, `MoveToStartOfStream`, `MoveToEndOfStream`; all repeat. Event IDs are singular; method names plural.

All six compare the fractional current chart position strictly, never wrap. Steps include taps, holds/rolls heads, lifts and explicit/region/warp-faked heads. Mines are events but not steps. Hold tails are contained in their head EditorEvent, not separate tree entries. Streams preserve repeated same-row chord steps: determine bordering step, prefer spacing to opposite neighbor, otherwise search-direction neighbor, then walk equal spacings; no density threshold or stream preferences.

All-event inventory shares the already initialized autoplay editor tree: timing, attacks, patterns and heads, plus fractional preview/end-hint widgets. Dynamic selected-row range start is merged at query time. This avoids a notes-only parser or a stale selection cached under content revision. Index binding checks session/content/chart identity. Native Keysound/Attack note kinds remain events but not steps, as they have no original IsStep counterpart.

The original executable runner copies actual six Editor methods, original tree search/unset helpers and comparison behavior using pinned Fumen/library binaries. Adapter supplies initialized event positions/IsStep flags and records MoveToChartPosition. It does not execute the full original chart loader or UI; native capture, selection preservation and transport need the separate Rust tests.

## Native behavior

Native defaults for these six actions are unassigned. Source preset recovers its actual defaults: first four unassigned, streams physical Ctrl/Command+Up/Down according to source platform. Registration enables source import and removes unavailable blockers automatically; original import archive is unchanged.

Available globally and inside checked-selection picker. Other modal editors retain their local keys. Movement keeps checked choices and freezes the existing range endpoints, preserving the selected-row widget. Exact focus retains fractional or negative positions while integer edit cursor is clamped/floored. No document/history mutation.

`navigate_snap` and event navigation now share exact-position transport application: respect StopPlaybackWhenScrolling, otherwise seek the existing decoder, clear mapped acknowledgment, preserve fractional focus and update audio command. No reopening file/decoder or synthesized key press.

Cache is rebuilt lazily when content/chart/session changes. Capture reuses a temporary complete autoplay timeline, then releases it and retains compact sorted row vectors. This has a temporary allocation cost on first navigation after an edit; subsequent held-key repeats only query cached vectors. A future shared tree snapshot can remove the transient lane/actionable arrays without introducing a second event policy.

## Pending validation

1. `cargo test -p deadsync-editor --test navigation_source_workflows --test autoplay_workflows --test key_binding_preferences_workflows --test source_profile_workflows -- --test-threads=1` (parent only; adapt actual profile target name).
2. Existing autoplay unit/integration suite after accessor addition and native edition EN/FR tests, especially playback transport / snap navigation (shared extracted application).
3. Manual fractional navigation, stream jumps through chords/fakes, selected range while playing with both stop-scroll preference settings.

This closes six registered source callback gaps, not the full editor port. Source remaining report is a snapshot and must be recomputed after concurrent conversion/pack work.
