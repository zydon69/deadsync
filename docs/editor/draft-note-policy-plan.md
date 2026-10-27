# Prospective lane-note fake classification

Prepared against GrooveAuthor pin48e18f71655d0f0d86144c63239331833501fda2 and the
existing editor timing dependency pass. No separate warp/stop/FAKE formula is
introduced.

`draft_note_policy::Cache` retains `GenerationTiming` and an exact content/GUID
Binding. On a changed set of draft heads it builds a synthetic Grid containing
at most64 probes and calls `EditorTimeline::build_clock`. The source phase
ordering, same-row retroactive negative-stop/warp flags and FAKE boundaries
therefore run through the same code as committed autoplay heads. It neither
loads nor walks the committed chart Grid. Song/charts or content changes require
a fresh cache; Undo restoring the exact original content token can reuse it.

The cache key is canonical head position/kind, independent of input iteration
order and sustain tail extension. Fake assignment in editor_timing.rs depends
on the head event phase/row and timing state; tail length never participates in
that flag assignment. A tap-to-hold or hold-to-roll kind change preserves its
actual source head phase and causes a miss. Tail-only dragging causes no replay.

This is bounded work in probe count, not a claim of constant timing work: a miss
walks the timing events plus at most64 heads, then uses the pipeline's existing
rate preparation. Repeated frames and transport changes reuse the result. Work
counters expose builds, timing entries and probe heads; invalid/over-budget
input leaves the last valid result intact and returns an error.

The native pending hook prepares after lane reconcile and left-Shift kind
changes. `State::lane_draft_fake(draft)` returns `Option<bool>` to the short-note
renderer. None means stale/unprepared/failed data and must produce the existing
partial-view indication, not an ordinary-note fallback. Rendering stays read
only. Cancelled gestures have no overlay; committed changes invalidate/rebind
the cache through the normal content token.

Six core tests passed: parity against actually committed probes
through seven timing configurations, exact FAKE boundaries and multiple owners,
1000 cache hits, budgets/invalid probes, external/session/Undo identity, and
sustain-tail reuse. The parity fixture uses the committed `Presentation.fake` field. The native
cache and rendering bridge are now integrated.

Native workflow coverage passed in `edition_short_note_workflow` (five
scenarios). It exercises actual draft press/left-Shift/release,
unchanged serialized bytes before commit, source Fake marker before and after
commit, and Undo. The same target checks committed short-note source textures,
selection, shared colors and viewport cropping. Existing text-F and generic
owner-color assertions have a separate migration patch; their semantic checks
are retained as exact atlas-marker and Routine-fill comparisons.
