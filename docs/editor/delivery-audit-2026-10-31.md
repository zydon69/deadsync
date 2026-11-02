# Current delivery audit — 31 October 2026

Reference: GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`.
The requested result remains the complete editor functionality in DeadSync,
with the accepted simplified, keyboard-accessible interface. This is an open
requirement audit, not a completion certificate. Historical gap lists and source
inventories are not proof of present coverage.

## Current status after interrupted background validation

The last complete public corpus finished288 modules and1312 tests:1311 passed,
one failed, zero ignored. Its remaining whitespace authoring-boundary fixture
was corrected and its three tests pass separately. The corpus now has289
modules after the two new migration receipt tests, which also pass separately.
The final harness/manifest must be regenerated after the remaining integrations;
these targeted passes are not a full current-corpus green claim.

Current complete core library passed216 tests, zero failures/ignored, with
unchanged captured source hashes. Latest selected native SaveAs, readonly
opening and migration workflows passed13/0/0, hashes verified. The last complete
native editor+diagnostic scope passed517/0/0 before the subsequent migration/
SaveAs lots; it must be rerun for the final integrated sources. Evidence:
`/tmp/deadsync-editor-save-as-intent-core-20261031.json`,
`/tmp/deadsync-editor-save-as-acknowledgement-20261031.json` and
`/tmp/deadsync-editor-native-panels-structural-20261031.json`.

Event timing, chart offset and Song Properties clock/music changes are being
corrected in coordinated isolated lots. No root Cargo is currently running.
Final public/native validation, build/package and refreshed GPU interaction
remain open. The last review application predates these changes. No editor
commit/push/PR has been made.

Earlier runtime checks on disposable charts observed left/right orientation,
snap-disabled central movement, aligned overview bodies, note Undo/Redo,
Save/Reload and chart switching with an advancing logical transport. Subsequent
GPU review must match the final integrated sources. Sections below retain
chronological findings and their evidence boundaries.

## Requirements inspected in this pass

| Requirement | Current consumer inspected | Execution evidence | Still required |
|---|---|---|---|
| Enter without a gameplay song; explicit file, empty entry and startup preference | Shell `editor_entry.rs` preserves native paths; `app/mod.rs::resumed` dispatches the launch; `app/screen_nav.rs` calls `prepare_standalone_editor`; `app/editor_host.rs` delegates to `State::prepare_entry`; native `edition/entry.rs` resolves the persisted startup target and guarded open | Current native regression log contains successful `empty_entry_needs_no_gameplay_or_song_and_creates_no_files_en_fr` and `entry_explicit_uses_guarded_worker_and_retains_empty_state_until_publish` | Shell targeted integration passed 28 tests; fresh packaged launch with an explicit scratch file; actual persisted startup/MRU and empty entry checks |
| Original stream breakdown can populate chart Description through a keyboard command | `edition/stream_description.rs::use_stream_description` starts a guarded density job, not the RSSP report; tests use F5 from the actual chart properties panel | Current native regression log passes `physical_use_stream_works_with_hidden_graph_and_one_undo_sm_ssc_en_fr` and `unreviewed_drafts_and_stale_reports_never_replace_description` | Fresh product interaction; do not infer all analysis parity from these two workflows |
| Per-lane minimap is part of the current native scene | `edition/minimap_content_state.rs` calls `minimap_content_scene::build_on_background`, with retained cache/mesh adapters | The inspected scene module has an obsolete introductory comment claiming disconnection; that comment is not evidence of a missing consumer | Actual GPU review of both minimaps, matching bottom edges, headings outside content, no overflow |
| Review app carries working-tree editor assets and notices | `scripts/build-macos-app.py::copy_editor_resources` explicitly validates/copies ten assets and four notices | Three Python packaging tests passed; separate copy checked all fourteen source/destination byte pairs | Actual package and sealed signature passed; pipeline checked the fourteen assets/notices; fresh runtime rendering remains open |

Native log: `/tmp/deadsync-editor-native-regressions-fixed-20261030.log`.
No hardware keyboard, audible audio or actual packaged rendering is certified by
these native test names.

## Reported visual/navigation regressions — evidence boundary

The current native run explicitly passes:

- `command_bindings::tests::physical_lane_keys_keep_left_and_right_in_their_chart_columns`;
- `command_bindings::tests::snap_none_raw_keys_move_the_real_chart_projection_with_signed_fractional_phase`;
- `overview_layout::tests::both_sidebar_bodies_align_and_labels_stay_above_them_at_all_sizes`.

The last test compares actual native density background actor extents against
minimap mount bounds at 854×480, 1100×760 and 3440×1440, at DPI 1 and 2; it also
checks both white label clips end before the shared content top. These establish
native geometry and command regressions, not GPU pixels or physical input.
Fresh packaged visual review remains open for all three user reports.

## Completion gates still open

- All public editor workflow executables must finish; successful compilation and
  the completed subset do not establish a green complete corpus.
- Shell integration passed 28 targeted tests; the corrected exact Practice transport test passed (1 test, 0 failures).
- Current build, packaging, signature and editor resources passed. The fresh
  bundle is `/tmp/DeadSync-Editor-Review-20261031-012547-74195.app`; runtime
  interaction remains open (launched process 22651, no visible window yet).
- In the fresh app, verify the reported left/right lane orientation, navigation
  with snap disabled, both minimap bounds, central chart movement and text.
- Review real startup, file/save/recovery, selection/edit/Undo, chart/song/pack
  properties, timing/attacks, generation/profiles, comparison, preferences,
  waveform/transport/audio, diagnostics/help and exit workflows against the
  pinned source. Earlier per-domain evidence must be matched to these actual
  requirements; a file inventory or command count is insufficient.
- User review remains required before an editor PR. No editor commit, push or PR
  is authorized by this audit.

The broader theme suite has fifteen classified failures outside the editor;
this document does not claim a green full theme suite or silently remove any
editor requirement because unrelated checks fail.

## Additional confirmed gaps found during the current review

Three production corrections were applied after the earlier 474-test native run:

1. General Options disabled custom-DPI value rejected every key before Escape
   and navigation. The value guard now follows panel navigation; edits remain
   disabled. A physical-key EN/FR regression covers exit/navigation, no numeric
   editor admission, unchanged document/preferences and no new Undo action.
2. Snapped wheel navigation previously added a row delta to the truncated edit
   cursor. It now shares `navigate_snap` with keyboard movement, preserving the
   original integer snap alignment from fractional and negative exact positions.
   A regression covers both wheel directions and Reverse at three positions.
3. Decrease/IncreaseSnap stopped audio before cycling the subdivision, unlike
   the original callbacks. That stop was removed. A physical-arrow regression
   checks subdivision changes while chart transport remains active and sends
   no stop/pause/seek/start command.

These three regressions passed in the 479-test native run (0 failures, 0 ignored),
recorded in `/tmp/deadsync-editor-native-post-navigation-20261031.log`. That run
predates the later six-test chart navigation correction described below.

The fourth gap has now been implemented: a typed About/licence panel supplies
the actual DeadSync version, complete GPL/retained MIT texts and a lightweight
coherent updater classification. The same snapshot feeds About and passive
notifications; only explicit Enter queues a release URL. Its source contract is
`UIAbout.cs:188–212`. Both native keyboard tests passed in the 479-test native run. The two updater
snapshot tests and GPU review remain pending; the updater run is queued after
the current application build.

## Callback availability cross-check

A current static cross-check of `key_commands/source.rs` against the actual
`key_binding_preferences::Action::ALL` entries and their `id()` arms finds all
164 pinned source callback IDs among 217 registered native IDs. No registered
variant was left unparsed by the cross-check. This supersedes historical counts
of 89 or 136 registered source actions; it does **not** certify the behavior of
all callbacks or infer complete UI/configuration coverage. Public profile-import
and native command workflow tests remain the relevant execution evidence.

## Shell filter scope checked

The completed shell command filters on `editor`. The current new audio regressions
are inside modules named `gameplay_prewarm::editor_tick_prewarm_tests`,
`gameplay_runtime::editor_logical_music_clock_tests`,
`gameplay_runtime::editor_waveform_tests` and
`gameplay_runtime::editor_separate_gain_adapter_tests`; the full Rust test path
therefore matches that filter, including the real asset-service → waveform test.
The terminal shell log now confirms all four named module regressions passed,
including the real MAIN PCM reload/reset waveform workflow: 28 passed, 0 failed,
0 ignored, exit 0. This validates that software bridge scope; it does not prove
physical output-device playback or audible tick behavior.

## Newly confirmed chart navigation gap — correction in preparation

`command_bindings.rs` stops audio before PreviousChart/NextChart queueing, while
original `Editor.cs:6171–6222` delegates to SetChartFocused and retains main
playback (only Preview stops in OnMusicChanged/OnMusicPreviewChanged). The native
focus transition already supports retained main transport, so this dispatch stop
is an actual remaining defect. A separate correction is being prepared, including
review of clock-driven row changes during queued sorting and the original
chartless first/last-chart behavior. It is not yet applied or validated. The
current passing shell run must not be used to certify this pending correction.

### Chart navigation correction integrated

The reviewed patch is now applied, including six native tests. Adjacent-chart
shortcuts retain main audio; genuine chartless state opens the first/last sorted
chart even for one chart; sorting/list requests accept scoped clock-driven row
progression but reject a changed exact manual focus anchor. Publication uses the
current canonical song time. Source/focus/content/culture/modal guards remain.

Integration occurred after the previous native test executable compiled. Its
153-source-file snapshot is recorded in
`/tmp/deadsync-editor-native-practice-snapshot-20261031.json`; results from that
executable do not certify this correction. The fresh post-app/updater Cargo run
requires at least 485 native passes and explicit success of all six new chart
navigation tests. Current status is **implemented, validation pending**.

## Startup graph preference chain — static review

The pinned FTUE StepGraphs choice (`UIFTUE.cs:171–190`,
`UIOptions.cs:179–184`) is consumed by source startup loading
(`Editor.cs:4932–4975`). The native path was followed through
`edition/ftue.rs:89–96`, `edition/startup_graphs.rs:96–148`,
`edition/general_options.rs:44–62`, core `options_preferences/startup.rs:121–153`,
and core `startup_graphs.rs::Task::capture/run`. Native target-layout indexes
are converted to chart IDs before capture, the host starts the task through
`entry.rs:53`, and the original startup selection is retained with an explicit
pending-restart state. No additional discrepancy was confirmed in this inspected
chain. This is static consumer evidence, not proof of a real restart, memory
usage or execution of all eleven graphs in the current test batch.

## Default chart opening preference — static review

Source `DefaultChartListProvider:38–40` delegates to
`EditorSong.SelectBestChart:951–1040`. Core `open_preferences.rs:85–145` uses
exact type/difficulty matching, the same difficulty fallback order 4/3/2/1/0/5,
paired dance/pump types and the retained TYPE_ORDER; it validates the supplied
ordering permutation. Native `edition/file_menu.rs:352` captures the application
opening preferences on the actual open path. No discrepancy was confirmed for
this inspected contract. OS/cultural comparator fidelity and actual file-open
interaction are separate requirements and are not certified by this reading.

## Confirmed minimap fractional paused-position defect — open

Native `edition/minimap.rs::minimap_effect` publishes a new exact focus but
leaves `paused_playback` intact after minimap capture pauses transport.
`edition/playback.rs::paused_position` validates only integral edit row plus
source/revision/chart. Moving within the same integral row, or returning to it,
can therefore leave an older paused instant preferred over the new focus by
`playback_position` and resume. Source `Editor.cs:3213–3248` updates the current
position throughout capture and restarts there. The targeted correction and three native regression workflows are integrated,
including same-row/return navigation, retained-asset resume and capture cancellation.
They are pending compilation and execution in the serialized fresh native run.
The current review application predates this correction.

## External notification audio correction — integrated, not yet executed

Source `Editor.cs:5395–5424` presents `ShowFileChangedModal:4829–4880`
without stopping main playback; Ignore and suppression only dismiss/change the
notification preference. The native unconditional `stop_audio` before opening
that notification was removed. A new EN/FR workflow uses a real asynchronous
file probe and raw keyboard Ignore/Suppress commands, and checks transport
identity, subsequent clock progress and unchanged resident document/history.
It uses typed logical audio snapshots and does not certify physical audio.
This and the three minimap tests were integrated while only the updater Cargo
was running, before the queued native compilation began; the current review app
predates both fixes. The native suite is now expected to contain at least 489
tests. Explicit success of the four additional tests must be inspected too.

## Fresh packaged runtime observation

The fresh bundle started after approximately ten minutes of macOS process load.
The first UI acquisition had launched a second instance without arguments
(PID 22741); that empty QA instance was terminated, leaving explicit-file
PID 22651. The latter visibly opens the scratch Dr. Wily chart, reports three
charts and waveform ready, and renders receptors left/down/up/right correctly.
The initial menu/empty observation was from the duplicate instance and is NOT
an editor launch regression. No startup correction is justified by that result.

A visible FTUE layout problem remains: the large panel uses very small text
relative to the surrounding editor UI at the observed 3200×1860 screenshot.
This is being traced before correction. Both-minimap and snap-None navigation
review remains open behind the FTUE panel. The runtime bundle predates the
subsequent minimap pause and external-notification audio fixes.

### Actual packaged overview/navigation review

After completing the six keyboard FTUE pages without changing their values and
dismissing the passive update notice, raw Left selects `Pas Désactivé`. Both
sidebar bodies are visibly aligned (approximately y540–1546 in the 3200×1860
capture), below their headings, with no visible overflow or diagonal unfilled
bottom. Holding Down advances the displayed cursor from 0 to 412 and the central
chart visibly changes from the initial timing events to notes/holds near measure
104. Up returns to displayed cursor 408 with visibly different notes near measure
103. This establishes real keyboard input, central projection movement and
sidebar alignment for this observed app/window; it is not all sizes/backends.
Original four song files and all four scratch copies remain byte-identical to
the fixture SHA256 manifest after navigation. No chart edits or save performed.

### Actual note editing, Undo/Redo and Save on disposable SM

At displayed beat 408, raw `1` adds a left-lane note at the receptor and shows
the dirty marker. Cmd+Z visibly removes that note and clears the marker.
Cmd+Shift+Z restores the note; Cmd+S reports Saved and clears the marker.
A direct disk diff of the disposable SM confirms the intended row change
`0032` → `1032`, plus the editor's resident song metadata/version tags. Saved
scratch SHA256: `143f2efd154770a11713c1e850ba9b5eddc23ce027052d44ffb6aad086e5684e`.
All four original song file hashes remain unchanged. This supersedes the earlier
scratch-unchanged statement only for the disposable SM after this deliberate
save. Actual reload/reopen persistence and advanced edit workflows remain open.

### Reload and latest native integration

Actual keyboard navigation F1 → File → Reload returned to the scratch chart at
beat408, preserving the saved left note and reporting waveform ready. This
completes the observed disposable note Save/Reload path, not all storage cases.

The FTUE/UI DPI correction is now integrated before queued native compilation:
row registration uses an explicit rendering-pass guard under virtual metrics,
and FTUE uses source width/height classes with room for vertical keyboard rows.
One EN/FR actor projection regression spans all six pages/controls and four
viewports including the observed Retina size. Current native expectation is
at least490 tests. Build, test execution and a refreshed GPU review remain open.

The complete public sweep is NOT green: `d-l/editing_workflows.log` records
11 passes and two failures (save/Undo metadata baseline and unsupported note
symbol/precision expectation). Both are under source-contract triage. Neither
is waived or reclassified without source evidence. Public batches continue.

### Actual chart navigation with active transport

The current native binding uses Enter for Play/Pause; Space selects a row range
and was not treated as evidence of audio playback. Enter visibly advances the
chart from displayed beat408 to426.701. Ctrl+Tab switches from Couple (2/3) to
Routine (3/3); the displayed beat advances to510.664 and then557.936 on a later
capture. Both Routine player colors and all eight lanes are rendered. Playback
was then toggled off. This demonstrates advancing native transport across the
chart switch in this app; it does not prove sample-continuous physical output
or all deferred/cancellation cases covered by the queued native regressions.

### Editing workflow failure triage and test update — rerun pending

Pinned `EditorSong.cs:2000–2033` writes custom song metadata at Save. The old
Save-after-Undo assertion incorrectly expected output bytes to exclude that
metadata. Resident Undo still must match original source bytes exactly; the
updated workflow separately checks those resident bytes, exact generated output,
all chart counts/note blocks, all original tag values and explicit metadata JSON.

The opaque-cell test previously expected any X cell to prohibit grid reading.
Current grid semantics retain X without fabricating a note, reject overwrite,
and permit unrelated edits while preserving opaque bytes. Updated assertions
check atomic overwrite refusal, unrelated-edit reparse retaining X, exact Undo,
and explicit refusal/preservation of an occupied seven-row subdivision.
No production behavior was loosened. The already-finished original executable
still has two failures; its result is not relabeled. The changed target must be
recompiled and rerun after the current serialized validations. New test source
SHA256: `6fe01e7d94881f76e84aefae0c2c60926e6ee25e4f935962ad7ef360e89712af`.

## Updater snapshot validation completed

`/tmp/deadsync-editor-updater-about-20261031.log` confirms both snapshot tests
passed (2 passed, 0 failed, 0 ignored, exit0): classification distinguishes absent,
unknown and confirmed current; concurrent replacement cannot combine release tag
and URL generations. The serialized native suite now compiles all current
editor corrections (Cargo PID39106); its results are pending.

## Public corpus acceleration — prepared and waiting

All 286 original Cargo targets were compiled previously; their disjoint executable
runners continue. A temporary `tests/__codex_public_corpus.rs` includes each current
source module by path, preserving its file-relative fixtures and package manifest
path. Static review found no executable-name, crate-root, process-argument or
shared exported-symbol dependencies in these sources. The new controller waits
for the current native/editing/app chain to finish, then performs one Cargo
compilation and one process per original module with distinct TMPDIR and one test
thread. It uses the exact names returned by libtest --list, avoiding substring
collisions between similarly named modules. All 286 modules must be nonempty;
source attribute counts, listed/executed counts, hashes and executable identity
are checked, and ignored tests prevent a full-green result. This proves the same
public test bodies under process isolation, not independent target packaging.
Temporary harness must be removed before any editor PR. Controller session10951:
`/tmp/deadsync-editor-public-batches-20261030/run_combined.py`.

The empty-chart assertion update is integrated and awaiting this execution.
It checks each original tag payload byte-for-byte, chart counts, timing identity
metadata and unchanged Undo/Redo, rather than requiring the old full byte stream
to remain a contiguous prefix when shared song metadata is inserted. Original
failed logs remain recorded; no old failure has been relabeled as a pass.

### Pre-compilation test assertion review correction

Root review caught an error in both proposed assertion deltas: GA JSON stores
`dance_single` (source enum), distinct from STEPSTYPE `dance-single`.
`Choice::output_fields` uses the underscore spelling, matching the C# enum
serializer. Both assertions were corrected before their new execution. The
waiting-only combined controller was replaced deliberately after refreshing the
source snapshot; no compilation/test was interrupted. New session56398 replaces
10951. This is not a timeout restart and no result is claimed for the new tests.

## Advanced generation focus discrepancy — open

Source `Editor.cs:7413–7467` and `ActionAutoGeneratePatterns.cs:85–184` regenerate
notes without navigation. Native job publication in `edition.rs:1535–1538`
automatically seeks to the first PatternSet entry before review. This replaces
the integral cursor, invalidates an exact fractional focus and affects both
later Apply capture and Discard/Escape. Explicit entry selection navigation in
`pattern_set.rs:159–170` is a separate intentional operation. A targeted fix and
regressions are being prepared; not yet integrated or certified by the current
native compilation. Full completion remains unproven.

An additional old public result fails in `minimap_position_transaction`: its
invalid fixture uses width2, which source UIMiniMapPreferences permits (minimum2)
and the native raster accepts. A reviewed assertion update is prepared separately,
with invalid width1 and a positive width2/spacing0 save/reopen check. It is not
yet integrated, and the original failed result remains recorded.

## Native490 result and next corrections

The first fresh run finished489 passed/1 failed/0 ignored. All five minimap,
external-notification and FTUE-layout regressions explicitly passed. The failure
was real chartless dispatch gating: PreviousChart/NextChart were omitted from
the no-active-chart allowlist. Both are now allowed; the physical-key regression
is unchanged. PatternSet focus correction is also integrated, including typed
seed-publication presentation receipts and three real-worker native workflows.
A new full editor run (session82146, Cargo45366) requires at least493 passes and
explicit chartless/PatternSet successes. Log:
`/tmp/deadsync-editor-native-pattern-focus-20261031.log`.

The app watcher stopped automatically at the failed prerequisite. The combined
controller was intentionally stopped while still waiting to refresh the reviewed
minimap-width test snapshot; no running validation was interrupted. Both need
restart after the pending event-shift fixture review/snapshot refresh. The
minimap test update is integrated; event-shift update remains a sidecar only.

## Event shift fixture update and resumed dependent validation

The old event-shift helper read SM Song timing while Selection and mutations now
use the resident Chart owner in either format (matching source
ActionShiftSelectionRow:71–90). Fixture setup/inspection now uses that same owner.
SM label cases create actual chart events through label::Draft while explicitly
asserting raw SM LABELS remain ignored; the existing source oracle now runs all
24 format/case combinations, removing its old five-case SM exclusion. Production
is unchanged. Execution remains pending; the old failure is retained.

The286-module manifest is refreshed before execution, including that update and
the width2/spacing0 positive minimap case. Current core source hashes are recorded
alongside the test-module snapshot. App-chain session79525 waits for native493;
combined-corpus session91275 waits for that app chain to pass. A failed prerequisite
stops dependent work. No green complete corpus or final app review is claimed.

## Native493 first result and fixture correction

The next run finished490 passed/3 failed/0 ignored. The previously failing
chartless physical shortcut now passes. All three new PatternSet tests stopped
before worker creation: their fixture omitted METER, so source generation
correctly rejected an invalid numeric chart difficulty. Both fixture charts now
have explicit DIFFICULTY Edit/METER9 and a parsed-meter assertion. Production
validation was not weakened. The new full suite is active (session51733,
Cargo48101), log `/tmp/deadsync-editor-native-pattern-valid-fixture-20261031.log`.
Dependent app/corpus watchers stopped automatically at the failed prerequisite
and were restarted after this deliberate fixture correction (sessions23268/36329).
No earlier failing run is relabeled as passing.

### Native 493 valid-fixture result

The current native editor run passed 493 tests, with zero failures and zero ignored tests. Log: `/tmp/deadsync-editor-native-pattern-valid-fixture-20261031.log`. This includes the chartless dispatch correction and the three PatternSet focus tests with valid chart meter metadata. The dependent pipeline has started the separate 13-test editing workflow rerun; app packaging and current combined public-corpus execution remain pending.

## Additional bounded source review after native 493

Three property-control discrepancies require correction and new regression
execution: chart MusicOffset remains editable when its override is disabled
(source UIChartProperties:108–113 and ImGuiLayoutUtils:1688–1697 versus native
edition.rs:2310–2331); the native song Apply9ms control can repeat although the
source enables it only when SyncOffset DoubleEquals(0) (UISongProperties:153–162);
and exact numeric entry is absent for chart/song MusicOffset, song SyncOffset,
SampleStart/Length and EndHint. Separate patches are being prepared without
changing sources used by the active compilation. These findings are open.

A separate native recovery defect was confirmed in storage.rs: publication via
persist precedes directory sync, but receipt publication follows sync. If sync
fails after rename, the next autosave retains an obsolete CAS receipt and rejects
its own sidecar. A typed publication/durability correction is being prepared;
no failure injection or corrected execution has yet passed. This is native
recovery robustness, not a claim that upstream GrooveAuthor has autosave parity.

The existing review app's Help → About → GPL licence can be opened entirely
with arrow/Enter keys and escaped without editing the chart. The licence text
renders, but these panels show the same downward positioning/small text problem
as the pre-fix FTUE. The new shared DPI-row correction must be reviewed on the
fresh package before these panels can be certified. No release URL was opened.

## Public audio fade assertion triage

The original `cargo/audio_source_preferences_workflows.log` records 2 passing
and 1 failing test. The failure is its final expected fade_out=0 for an imported
raw value -2. Current `audio_preferences/fields.rs:145–160` intentionally retains
signed fade values, matching source `MusicManager.SetPreviewParameters:433–438`,
which converts fade durations directly without a nonnegative clamp. Source main
and music volume setters at 464–475 do clamp, and those effective gain checks
already passed. The source UI editing boundary is separate from raw import.

A test-only patch is prepared at
`/tmp/deadsync-audio-fade-contract/audio-fade-contract.patch` (SHA256
`ad08aae94bb2527c02d0ef0f7d236f504282fc5570f3ed0d20b3da7db52c365c`).
It expects signed imported fade-in/out, checks separate clamped mix gains and
unchanged raw disk fields. It is not yet applied or re-executed; the original
failure remains a failure. It must join the next deliberate corpus snapshot
update before current-source public execution.

## Licence page DPI gap distinguished from menu rows

Current source inspection confirms the licence body path in
`edition/editor_about.rs:164–173` returns without registering any ui_row. The
shared collecting-pass correction therefore cannot fix this particular view.
`ui_dpi.rs` falls back to the first title text when no rows exist, and its
page=None vertical auto-position moves that title to the viewport midpoint,
pushing the rest of the modal downward. The earlier package visibly exhibited
this same placement. A separate correction and actor-projection regression is
being prepared outside the worktree; fresh licence GPU review remains required.

## Separate editing rerun completed

The current standalone editing_workflows target passed all13 tests, with zero failures and zero ignored tests, including saved metadata/Undo and opaque-note atomic refusal. Log: `/tmp/deadsync-editor-editing-rerun-20261031.log`. This supersedes that target's original failed result only for the reviewed current source snapshot; other pending public targets remain unproven. The app build is active as Cargo53991 in controller session23268; it predates the property/recovery/licence sidecars still in preparation.

## Recovery/property/licence patches integrated; core208 passed

The reviewed recovery v3 patch, four-file metadata precise-entry patch and
licence pagination/layout patch are integrated. The separate audio fade test
contract correction is also applied. Full core library validation completed
208 passed, zero failed, zero ignored (session20417, log
`/tmp/deadsync-editor-core-recovery-properties-20261031.log`), including all seven
real recovery fault/ownership/retry regressions. The fresh full native suite is
active (session31849, Cargo56638), requiring498 passes and explicit success of
the four metadata workflows and licence actor-projection workflow. Its dependent
app pipeline waits in session80670; no native498 result is yet claimed.

The previous combined controller was terminated only while waiting for the app,
before Cargo compilation, to incorporate the reviewed audio-test snapshot and
current recovery/property core sources. It exited143; this was not a timeout
restart. The refreshed controller session67352 waits for the latest app pipeline.
It now verifies recorded core source hashes as well as286 test modules/harness
before and throughout execution. Complete corpus validation remains pending.

## Actual refreshed FTUE runtime review

The493-source review app `/tmp/DeadSync-Editor-Final-Review-20261031-48210.app`
passed package/signature/resource checks and opens the explicit disposable
Dr. Wily chart (PID54879), with saved left note and waveform ready. It replaces
the owned earlier QA process22651. Actual keyboard Help → Replay introduction
completes all six steps without editing the settings or opening a reference URL.
Screenshots explicitly inspect pages1,2,3,5,6: modal centered, readable wrapped
French text and all controls within its bounds at3200×1860. Page4 was traversed
but not independently screenshot-certified. This proves the observed shared
row-capture/FTUE DPI correction on this GL app/window; other sizes remain actor
regression scope. This bundle predates the newly integrated recovery/property/
licence patches, so their runtime review remains open.

## Native498, repeated editing13 and refreshed package passed

The full native run completed498 passed, zero failed, zero ignored. All four
metadata numeric/gate workflows and the licence actor-projection regression
explicitly passed. The separate editing target again passed13/0/0 after the
recovery/property integration. The dependent build/package/deep strict signature
and14 resource checks passed. Latest app:
`/tmp/DeadSync-Editor-Latest-Review-20261031-56657.app`; launched as PID59403 on
the same disposable chart. Runtime review of these latest properties/licences is
pending; an earlier app cannot certify them.

## Concrete generated-preview rendering gap discovered in real app

On the prior493-source app, F3 Single → Right selects dance-couple; Calculate
successfully produces1318 notes and128 mines, with zero dropped. The resulting
field renders eight receptors but no notes at beat408 or its last-note End
position549.5. Source inspection confirms `edition.rs` uses preview.grid for
rendering while `region_selection.rs::source_viewport_plan` admits identities
from session.loaded_grid(chart). Their row/lane identities can differ, and the
renderer's explicit admitted-position filter consequently hides generated notes.
A coherent preview admission correction and real worker/actor regression are
being prepared. Escape restored the source four lanes/three charts; scratch
bytes remain SHA256143f2efd154770a11713c1e850ba9b5eddc23ce027052d44ffb6aad086e5684e.
The existing498 passing tests do not establish this newly discovered behavior.

## Music output assertion applied; sibling updates deferred

The original music workflow failure was independently decoded as exactly117
output-context bytes added to478 resident bytes, with no other difference.
Only music_file_workflows and its independent expected-byte support helper are
updated now; its original target already finished. It retains exact resident
Undo/Redo and exact published bytes plus both note grids, music/rating/offset,
peer metadata, vendor and original assets. Rerun remains pending. Prepared
song_text/chart_properties sibling fixes remain unapplied while their original
runner targets retain their source guards. Resident-timing's after-reopen
baseline is correct and untouched. Combined controller67352 was terminated
while still waiting, before any compilation, to refresh this reviewed test/
helper snapshot together with the pending preview correction. No full public
corpus success is claimed.

## Completed public corpus and user-requested background stop

The combined run `combined-run-20261031T015847.048606Z` is terminal:
286/286 modules executed, 1299 listed and executed tests, 1269 passed,
30 failed, zero ignored. Its status and per-module logs remain under
`/tmp/deadsync-editor-public-batches-20261030/`. These failures require
individual source-contract triage; this is not a complete validation.
The four older original-target runners were terminated following the user
report of background jobs blocking the shell. No replacement was launched.
Partial original-target evidence must not be presented as complete.

## F11 first-frame widget cache correction applied, validation pending

Real runtime review exposed an uninitialized draft widget cache immediately
after opening Song Properties. The opener now calls the existing
`refresh_sample_region` path after installing its draft, preparing draft
widgets, sample presentation and waveform before the first frame. A regression
exercise for physical F11, note/widget actor presence, unchanged resident bytes
and revision, precise preview draft editing and Cancel was added for EN/FR.
Both help texts now describe precise entry and the zero-Sync 9 ms gate.
Patch context and helper call chain were reviewed; no test or build was run
after this change. The running review app predates this fix.

## Public duplicate/order contract triage after terminal corpus

Reviewed original `StepManiaEditor/Editor.cs` FinishAddingChart (7295–7304)
and GetPlayer (7534–7543): new Routine charts inherit the active player,
clamped to their player count. Updated the duplicate workflow expectation
from player 0 to player 1; added positive note ownership, negative peer
ownership and Undo/Redo checks retaining exact serialized resident state.
No production player-memory change was justified.

Reviewed chart_deletion::Draft::prepare_song_timing and Session::remove_chart:
deleting the song timing chart with multiple successors requires a complete
current cultural order. Updated chart_order workflow to first require atomic
refusal without that order (bytes/revision/GUIDs/focus unchanged), then prepare
the actual sorted order and exercise the existing delete/Undo/Redo identity
checks. No production guard was relaxed. Both corrected tests remain unrun;
the prior terminal corpus retains its original failures as historical evidence.

## Export loss review corrected; output-context assertions integrated

The writer omits raw SM WARPS/FAKES/COMBOS/LABELS/SCROLLS/SPEEDS and
the source-compatible loader intentionally does not activate them as timing.
Preflight previously skipped them as known tags, hiding their loss. Nonempty
raw song-scope tags now produce Warning/UnknownProperty before that skip;
normal export acknowledgement is required. Writer/event interpretation is
unchanged. Added chartless/chartful SM-to-SM/SSC coverage checking warning,
refusal without acknowledgement, inactive editor/clock behavior, output omission
and unchanged native bytes. These tests have not been run.

Integrated prepared song_text/chart_properties/clear_pattern/conversion_profile
assertion fixes after the corpus became terminal and original runners stopped.
Original EditorSong.SerializeCustomSongData explicitly publishes SyncOffset and
DefaultChartType/Index. Tests now compare exact published bytes against the
independent two-property expected-byte builder, retaining exact resident history,
Save immutability, reopened grids/metadata and peer assertions. No broad byte
normalization or ignored fields were introduced.

Checked timing tests now assert the actual Chart owner and unchanged Song
properties plus Undo/Redo. Chartless malformed OFFSET is required to fail at
prepare_output; limits and cancellation are exercised on separate valid input
with a positive prepare control. All follow-up validation remains pending;
no test/build/runner was started after the user's interruption report.

## Further stale public-contract fixtures corrected without execution

Applied prepared NewSongOptions fixture correction: invalid values are refused
when the complete General preferences owner opens, not by a later lazy getter.
Tests retain unchanged input/output checks and additionally exercise actual
untitled workspace values, rejected source import, and nonfinite creation.
Applied archived keybinding CAS fixture: a deterministic dormant synthetic
entry creates a genuine concurrent change; current source preset callbacks no
longer necessarily leave archived blockers. No runtime callback was invented.

Reviewed source_review_bindings: ShiftEarlier/Later now have real selected
mixed-event handlers in command_bindings.rs, routing to checked-shift review.
Updated the old 56-count and unavailable assertions to 58 and explicit source
registration/review/import/no-blocker assertions. Existing semantic handler
coverage remains separate and must be rerun.

Reviewed song_assets staging against EditorSong image field loading and
Serialize/Save preparation (EditorSong.cs 1790–1800). Jacket is an editable
staged field for SM as well as SSC; format-specific writer behavior follows
later. Corrected the stale SM-only stage count/path expectation and retained
only-unset, repeated no-op and atomic invalid candidate checks, adding
Jacket/Lyrics/Title preservation to the latter. No production behavior changed
for these fixture corrections; none has been rerun.

Generated-preview patch review found an unintended deletion of the newly added
F11 regression in the sidecar. Integration was deferred and the agent asked
to preserve that test and finish source-B/Apply/overview semantic review.

## Effective participant baseline fixes byte-preserving Undo

Confirmed production cause for participant_count and unsupported-destination
pattern clipboard Undo failures: Session::grid applies GA MaxPlayerIndex, but
serialize_document compared against raw Grid::parse_layout without that metadata.
A touched empty grid after Undo therefore differed only in its declared owners
and was rewritten into normalized empty blocks. Extracted document_grid as
the shared effective parser for loading and serialization comparison, retaining
metadata validation and note-owner minimums. Original failing byte assertions
were not changed. Added a regression across SM/SSC, dance-routine/pump-routine/
smx-team, declared one/three players, temporary note edit/reopen/Undo/Redo and
participant count/reopen/Undo/Redo. No tests or compilation run after this fix.

## Participant correction validated; generated preview integrated

Single targeted Cargo run completed successfully: participant_count_workflows
5 passed, 0 failed, 0 ignored, including the new effective-baseline regression
and the original failing exact Undo fixture. Compilation took 24.96 seconds;
source hashes for history.rs and the public target were verified unchanged
after execution. Evidence: /tmp/deadsync-editor-participant-baseline-20261031.log
and corresponding .json. This is targeted proof, not complete editor proof.

Reviewed and integrated generated-preview sidecar SHA256
17bbb4c1333783b3a5726a3e1a21982c5dd0c62a87c8b5bc0dcf9463dadb78c7.
The focused viewport admits the generated immutable grid, with source-chart
timing, metadata, classification, proportional quotas, MiniMap and background
density; inactive views stay committed. Apply rebinds focus at actual musical
time; Cancel restores committed presentation. F11 regression was retained.
Root review additionally moved the preview modal exception from MiniMap input
to MiniMap actors (the candidate had allowed input but still hidden actors),
and made refresh_misc_inventory's chart list mutable for the extra source.

Native edition validation is now the sole active Cargo process, session52071,
log /tmp/deadsync-editor-native-preview-20261031.log, source identities in .json.
Its result is pending. No parallel runner was restarted.

## Initial timing-choice phase checked in full constructor

A provisional concern about Choice::load physical order was rejected after
reviewing EditorSong.cs777–789: DeserializeCustomSongData and fallback
ChooseTimingChart execute BEFORE UpdateChartSortInternal. Initial saved
reference and fallback therefore use physical order within type. No loader
change was applied. Post-deletion reconciliation still requires current
cultural order. Original sorted Save and native physical-order Save may encode
different per-type indices for the same selected chart identity. Independent
context expectations must account for this distinction.

Applied reviewed open-loadable and timing conversion fixtures. Opaque cells
are readable but protected against overwrite; negative fixtures now use truly
invalid widths and retain unchanged-byte/focus/history checks. Conversion
oracles read effective Chart timing, including inherited unchanged fields,
and retain Song/peer isolation. StepF2 metadata-only tests cover both readable
opaque cells and invalid width. Deferred the unrelated nonleading Save hunks
that derived expected bytes from the production serialize_for_output path;
independent expected-byte evidence is still being prepared.

## Fresh integrated native502 and core209 passed

Native edition suite completed: 502 passed, 0 failed, 0 ignored; compiled in
1m33s and executed in59.83s. Every recorded core/native source hash remained
unchanged. All three actual generated-worker preview tests and the F11 first
frame regression explicitly passed. Evidence:
/tmp/deadsync-editor-native-preview-20261031.log and .json. Core library suite
then passed209/0/0, explicitly including immutable preview head classification:
/tmp/deadsync-editor-core-preview-20261031.log and .json. No GPU or hardware
input certification follows from these native tests.

Integrated the reviewed varied-context saved-output test patch. Independent
byte reconstruction accounts for eight original failed comparisons, with each
fixture explicitly declaring sync/layout/per-type index and preserving existing
SM metadata alias spelling. Original resident Undo/Redo and protected raw-byte
checks remain exact. Source phase ordering was reviewed before assigning
nonleading fixture's physical per-type index0. Evidence is retained in
/tmp/deadsync-global-fidelity/preference-functional/saved-context-varied-layouts/.

New complete public-corpus controller is active as session22663, run directory
/tmp/deadsync-editor-public-batches-20261030/combined-run-20261031T021746.949221Z.
It uses combined-preview-sources.json with22 reviewed public/support source
updates and current complete core hashes. The controller gates on actual
current native502/core209 evidence, not the older packaged app. Original corpus
results remain unchanged. Only one Cargo is active; each module uses a separate
process from one compiled artifact with exact identities and counts. Sources
are frozen during this run. Public results and final packaged review remain
pending; no completion or upstream PR is claimed.

## Public retry after a compile-only test error

The first current public controller stopped at compilation: source_timing
assert_context was passed &&[u8;166], rather than &[u8]. Removed only the extra
reference and refreshed that test's manifest hash; no assertion was changed.
The failed compile and prior manifest are retained in the original run directory.
New session29402 runs combined-run-20261031T021844.083150Z. Compilation and
exact listing passed:1302 tests across286 modules. At the recorded checkpoint,
71 modules/343 tests passed with0 failures; this is partial evidence only.
The package pipeline is prepared but has not been started: it requires terminal
green public evidence plus unchanged current native source hashes.

## Current public corpus terminal:1299 passed,3 failed

Session29402 completed all286 modules/1302 tests with1299 passed,3 failed,
zero ignored. Remaining failures are one layout_alias workflow and two
song_text workflows. Original logs and source/executable checks are retained.

Layout alias failure occurred immediately after structural normalization,
BEFORE Save. Corrected the independent expected-byte context at that resident
phase (not publication), retaining eight SM/SSC/alias cases, exact disk-original
before Save, identical resident/published normal form, exact Undo original and
unchanged disk, exact Redo before second Save/reopen. Follow-up unrun.

Song-text failures are not assumed equivalent: SM Jacket is editable in the
original UI/model, so its old legacy-path assertion after explicit edit is stale.
SM raw PREVIEW is different: EditorSong.cs724 reads Song.PreviewMusicFile,
which is empty in the original SM loader, while native Text::read currently
activates the raw tag. This is a source-load divergence under investigation;
the original reader expectation must not be replaced to hide it. A coherent
authored-versus-imported field policy must be considered before correction.

The package pipeline remains unstarted because the full public gate is red.
Native502/core209 remain valid for their recorded unchanged production source
snapshot; they do not certify resolution of these newly identified field cases.

## PREVIEW raw/authored boundary correction in preparation

Source inspection confirms SMReader does not populate PreviewMusicFile from
raw PREVIEW, EditorSong.cs724 copies only that effective field, but the original
properties UI accepts explicit preview edits in either format. A simple blank
Text::read would break authored previews after transactions reparse the document.
Preparing a versioned native DSSongPreview owner: raw SM bytes stay opaque and
inactive; explicit authored values (including empty Clear) remain distinguishable
through native lossless Save, recovery and reparse. This follows the already
established native resident timing adaptation. Source-compatible SM output must
review the omission; SSC output must materialize the effective field. Core owner,
export/relocation and native consumer workflows are assigned disjoint files in
sidecars; no integration or execution claim yet.

Applied the independently reviewed Jacket test correction: original UI permits
explicit Jacket edits in SM, so the accepted replacement must be asserted rather
than expecting legacy.png. Original exact Undo/Redo/pending peer checks remain.

## Zero-stop conversion validity boundary clarified

Source ActionChangeWarpsToNegativeStops107–114 can construct Stop(-0), despite
EditorStopEvent.IsStopLengthValid32–35 excluding zero. Its constructor only
asserts validity, and release insertion/timing/writer can retain the invalid event.
Native atomic refusal is a restrictive validity correction of this source bug,
not a numerical/codec impossibility. GenerationTiming already accepts Stop(0).
The constructor test now states that boundary and the oracle workflow name no
longer calls zero unrepresentable. No conversion behavior or oracle assertion
was altered. Source Warp constructor explicitly clamps zero length to one row;
native max(1) follows that actual constructor. No silent invalid-event omission
or global parser activation was introduced.

## Media scope checked against actual source consumers

Reviewed original EditorImageData.cs and Texture/EditorTexture.cs72–175.
The texture loader explicitly skips video files, while decoding GIF frames
and still images. The boolean passed for Background is cacheTextureColor,
not permission to play video. Native UnsupportedVideo is therefore not, by
itself, an unported source playback feature.

Reviewed all original StepManiaEditor references to LyricsPath/PreviewVideoPath:
EditorSong stores, relocates and exports them; Editor and UISongProperties
provide file selection/editing, and ActionSetSongAssets supplies discovery.
No lyric/video rendering consumer is present in those referenced source paths.
Corrected README's historical blanket claim that their rendering was pending.
This does not remove asset metadata, discovery, path relocation, history or
export requirements, nor certify those workflows without matching evidence.

## Integrated resident preview boundary and current targeted validation

Integrated core owner patch7410557cfd62474575633491105f3f769feff8518078272b9aa766b0a468c016,
export/relocation integration and native consumer patch
f50a59275dbba01d81a1356f08d8acfe696c83d25ff62f119e21e8a0c0aac650.
DSSongPreview version1/path distinguishes explicit SM edits from ignored raw
PREVIEW, including explicit empty Clear. Draft mutation validates the owner;
infallible display reads cannot activate malformed/raw data. Native lossless
Save/recovery/reparse retain authored intent. Export reviews raw/authored losses
and materializes SSC effective media; relocation handles the typed owner and
its history values without treating opaque SM raw PREVIEW as active media.

Core cache and autoplay sample widgets follow the effective preview, not a
format shortcut. Native asset registration/Opus warnings/sample region/MiniMap
now consume the effective field; draft preview changes refresh region caches.
Changing only preview media preserves main musical transport; dedicated
audition routing has separate native tests. Diagnostics were added in EN/FR.
No claim of audible PCM or final GPU rendering is made.

First targeted compilation failed because a new public test used crate-private
content-token helpers. Replaced these with public revision/content revision,
chart identity and empty-history assertions; no production API was exposed.
Failure log retained as
/tmp/deadsync-editor-song-preview-core-20261031-compile-failure.log.
Current targeted session45255 uses one Cargo invocation with lib+four public
targets; core library212/0/0 has passed, public target execution is still in
progress. Source hashes are captured in
/tmp/deadsync-editor-song-preview-core-20261031.json.

Inventory now has288 top-level public workflow targets (two new Preview
workflows), no removed targets. The next complete corpus must include both new
modules; the earlier286-module harness is insufficient for the new snapshot.

## Targeted preview progress and complete module inventory

Current session45255 has completed core212/0/0, layout_alias4/0/0 and
song_preview_output4/0/0. The output cases explicitly passed malformed-owner
atomic refusal, recovery override/Clear, SaveAs current/history rebasing while
retaining opaque raw PREVIEW, and source-export review/mapping. Resident and
song-text target results are still pending. No restart was made for cold
executable loading; the same live process/session is being observed.

Prepared (not executed) combined-resident-preview-sources.json and refreshed
the temporary harness with all288 current top-level workflow modules, including
both new preview targets; no prior target was removed. Current complete core
and support source hashes were captured. The old286-module controller needs
a matching288-module gate before reuse. A fresh native run and the targeted
green result remain prerequisites. The harness is explicitly temporary and
must be removed before delivery.

## Targeted232 complete, native resident validation active

Session45255 terminated with exit0: lib212, alias4, preview output4, preview
resident6, song text6 passed; zero failures/ignored in each scope. All captured
core/test source hashes match after execution. Both former song_text failures
now pass with the original Fumen expected fields unchanged. Evidence remains
/tmp/deadsync-editor-song-preview-core-20261031.log and .json.

Native session39383 is the sole Cargo process, running the entire edition
filter plus the song-timing/preview diagnostic test. Snapshot hashes include
all core/theme Rust sources and both language files. Log/status:
/tmp/deadsync-editor-native-resident-preview-20261031.log and .json.
Prepared288-module controller verifies exhaustive current target inventory and
gates on current targeted/native results; it has not been launched.

## Native resident507 exposed physical F11 transport defect

Session39383 completed507 selected tests:506 passed,1 failed,0 ignored; all
captured source hashes remained unchanged. The diagnostic translation test,
raw/authored preview routes, MiniMap metadata and Opus warning tests passed.
The main-transport physical test failed while staging, before Apply. Root
inspection identified Action::OpenSongProperties calling stop_audio before
open_song_properties in command_bindings.rs1435. Changing the core presentation
receipt alone cannot preserve playback when the actual physical opener already
stops it. A native-only correction and earlier-stage regression assertions
are being prepared; the failure remains retained and the test is not relaxed.

Current complete public288 and package gates remain unstarted until a fresh
green native snapshot is available. Adjacent readonly inspector opener stop
policies are under source review; no broad stop removal was made blindly.

## Physical Song Properties opener correction integrated

Applied eb301a94ffa129919ac2610cc9e0bee2693776cfcbc4945033347b332817fcd6.
Removed only Action::OpenSongProperties' unconditional stop_audio. Source
UISongProperties.cs46–50 only sets Show/Focus; Editor.cs2409–2413 handles
changed preview with StopPreview, not main StopMusic. Existing physical
preview tests now assert playback/main-sample role immediately after F11,
each navigation key and entry transition, with Clear traced on main transport.
Original Apply/owner/generation/epoch/instant/command assertions remain.
The change is integrated but not yet executed.

Source review confirmed similar premature stops before readonly Analysis,
Timing, PatternSet and other tool openings, including requests which then
refuse without changing any state. A separate reviewed native-only lot is
being prepared; no broad unconditional stop removal has been applied.
The next native validation will include those opener contracts coherently
before the full288 public run. No current Cargo or test runner remains live.

## Recovery after background-task interruption

Process inspection confirmed no Cargo, rustc or public-corpus runner remained.
The last complete public corpus had actually terminated: 288 modules, 1312
tests, 1311 passed, one failed, zero ignored. Evidence: combined-run-20261031T024559.647377Z/status.json under /tmp/deadsync-editor-public-batches-20261030.
The remaining whitespace fixture wrote raw SM PREVIEW through Session::set_text
and then expected effective authoring media. It now exercises the real Song
Properties Draft capture/set/apply boundary for all19 fields, retaining exact
whitespace, reparse, Undo and Redo assertions. The unchanged raw-reader Fumen
cases remain covered. Targeted standalone result:3 passed,0 failed,0 ignored;
/tmp/deadsync-editor-text-whitespace-authoring-20261031.log, session13026 exit0.
This is not a fresh complete-corpus green claim.

Integrated generated-preview source-clock navigation patch0758b9901281779c30ce00762da66badecf08febc5a2202f5416cf622c25cfdf after review. Home/End/Page derive destination from sourceB, synchronize focusedA by musical time and preserve exact B anchors through equal-time endpoints. Main and paused transport use the new instant; Apply/Cancel subsequently consume it. Two physical-worker regressions are prepared, not executed. Native opener and structural chart lots are still pending review. No final application, completion or publication claim.

## Readonly and structural transport lots integrated

Integrated readonly-panel patch (15 dispatch openers, FileMenu, OutputSettings,
ChartEventCopy and manual PackProperties routes), plus separate chart-set
history API and structural chart transport patches. New/Delete commit and
structural Undo/Redo use canonical focus-audio handoff; chartless deletion
retains loaded override. No broad other-history stop removal was made.
Prepared seven core/native structural tests, three readonly native tests and
two source-clock navigation tests require execution. Core lib validation live
session83651, log /tmp/deadsync-editor-structural-core-20261031.log; source
snapshot /tmp/deadsync-editor-structural-core-20261031-sources.json.
Remaining source reviews: TimingMigration live mutation after readonly opener,
SaveAs dialog vs publication transport. Distinct agents prepare sidecars only.

Native session87906 ended at compilation with two new-test errors: nonexistent
EditorStopPreview enum variant and private region_geometry access. Corrected
to the actual audition-stop command StopMusic and accessible chart_viewport
without widening production visibility or dropping transport assertions.
Failure retained in native-transport-20261031.log. Fresh sole-Cargo session46730
uses native-transport-fixed-20261031.log and matching sources.json snapshot;
execution pending. Core214 remains source-hash verified.

Native session46730 terminal:517 tests,509 passed,8 failed,0 ignored.
Eight new structural/navigation and readonly tests exposed assertions to
investigate; existing resident F11 failure now passes. Full failure evidence
retained in native-transport-fixed-20261031.log. Core214 remains green.
Two readonly failures shared a fixture panic: it removed canonical action keys
used by toolbar labels. Fixture now clears chord values while retaining the
full key map; original continuity assertions remain. Structural5 and preview
navigation1 failures are being diagnosed by their authors in isolated sidecars.
No fresh validation or final runtime claim yet.

Preview-navigation fixture now asserts the real published EditorSeekMusic
owner/seconds (exactly one) and rejects residual generic SeekMusic; paused and
stopped branches reject both forms. Scoped conversion is in audio_scope.rs.
Readonly canonical-key-map fixture corrected separately. Targeted five native
tests now running in sole-Cargo session14025, log
/tmp/deadsync-editor-panel-preview-targeted-20261031.log and sources.json.
Structural fixture diagnosis: source-profile import cannot import native-only
CreateEmptyChart/DeleteCurrentChart IDs; agent prepares real native binding
fixture with all original transport assertions retained. Not yet validated.

Targeted session14025 finished5 tests:4 passed,1 failed. Both sourceB
navigation regressions and actual F1 routes/refusal continuity passed.
Remaining InspectAttack fixture had a head only at time0, while main's real
logical snapshot advanced the editor cursor to row60/time0.625. Added an actual
second attack head to cover that position, preserving the cursor/transport
assertions; dedicated audition still exercises the original row0 head.
Structural fixtures now install validated complete native command profiles.
Fresh targeted session76982 runs all eight panel/structural tests; log and
source hashes at /tmp/deadsync-editor-panel-structure-targeted-20261031.*.
Results pending; no final green or runtime claim.

Session76982 terminal7 passed,1 failed,0 ignored: all five real structural
keyboard tests now pass. Readonly Cancel expected immediate no-panel, whereas
NoteAttributes/Conversion/Owners intentionally return to Transforms(11/9/10).
Test now verifies exact parent and transport witness before the second Escape,
then verifies final dismissal and unchanged witness. No production navigation
was changed to satisfy this fixture. Fresh full native+diagnostic session19763
(log native-panels-structural-20261031.log with sources.json) is now active.
No other Cargo is scheduled concurrently. Timing/SaveAs lots remain isolated.

Native session19763 completed successfully:517 passed,0 failed,0 ignored,
including all new physical opener, structural history and generated-source
navigation regressions plus resident-preview/F11 diagnostics. All captured
core/theme/language hashes match after execution. Evidence:
/tmp/deadsync-editor-native-panels-structural-20261031.log/.json.
This green snapshot precedes the pending TimingMigration/live timing and
SaveAs corrections; final completion and packaged runtime remain open.

TimingMigration presentation/history patch integrated after the green517
receipt was sealed. The migration validates editor-event and offset equality
before mutation, carries opaque before/after provenance and preserves paused
resume stamps on Apply/Undo/Redo. Four new core/native tests are prepared, not
executed. Live timing mutation and SaveAs companions remain under preparation.
No Cargo or test executable is currently left running by root.

Migration core session59804 passed214/0/0, captured hashes verified.
SaveAs transport and pure source-path comparison API patches integrated after
review. Dialogue leaves Main/Paused unchanged; review validates explicit intent
while allowing current callback projection, checks relocation rebind before
publication, then uses canonical focus audio handoff at current musical time.
Chartless retained override and pending durability paths stay explicit.
Native targeted session11598 validates migration2 +SaveAs5; log and source
snapshot at /tmp/deadsync-editor-migration-save-as-targeted-20261031.*.
Core/public/final native revalidation and current app remain pending.

Targeted11598 compilation rejected a mutable-self closure overlapping the
reviewed migration draft borrow (E0500). Fixed by taking the draft locally,
applying by reference and restoring it before handling success/refusal; success
clears it, refusal retains the same reviewed draft. Failure log retained.
Fresh targeted76233 uses migration-save-as-fixed-20261031.log/sources.json.
No test result is claimed yet. SaveAs review additionally identified a normal
backend acknowledgement during review being rejected by the frozen generation
stamp; a separate guarded follow-up with restart/seek controls is being prepared.

Native targeted76233 terminal6 passed,1 failed,0 ignored: both migration
workflows pass; four SaveAs workflows pass. Cross-directory/history/recovery
fixture refuses during review, under diagnosis (possible stale view projection
after fixture-only direct Workspace note mutation). No assertion weakened.
Current sole-Cargo session2622 validates full core library plus
legacy_timing_workflows, including the new public migration receipts; log
/tmp/deadsync-editor-migration-legacy-core-20261031.log and sources.json.

Session2622 terminal0: core215 passed and legacy_timing_workflows7 passed,
zero failures/ignored, captured source hashes unchanged. The two NEW migration
receipt tests are a separate public target timing_migration_presentation_workflows,
not the legacy target; they were not executed by2622. Fresh targeted command
started for that actual new target; log
/tmp/deadsync-editor-migration-presentation-public-20261031.log.
Authoritative public target inventory is now289, so the final harness/manifest
must be regenerated rather than reusing the prepared288 snapshot.

Session77846 terminal0: both new timing_migration_presentation_workflows
tests pass, zero failures/ignored, all recorded source hashes unchanged.
Opaque before/after receipt validity, exact migration Undo/Redo bytes and stale
draft/receipt refusal are now executed public evidence. Combined289-module
public validation is still required after the remaining live-timing/offset/
SaveAs-acknowledgement integrations. No Cargo remains running by root.

SaveAs acknowledgement-guard follow-up integrated after review. Transport intent
uses existing SourceVisualClock.started rather than generation/epoch snapshots;
normal initial-bind/reload acknowledgements may advance without revoking review,
whereas Play/Resume/seek/rate reanchors invalidate it. Paused carries the same
existing origin, without introducing a new epoch. Three regression tests added.
Fixture now uses actual State.command(Place) to refresh native views and checks
old recovery bytes exactly. Readonly binding fixtures use valid canonical
unbound alternatives and validate the complete profile before installation.
Sole-Cargo session86384 compiled successfully and is executing13 selected
native workflows; log and source snapshot at
/tmp/deadsync-editor-save-as-acknowledgement-20261031.*. Result pending.

Session86384 terminal0: all13 selected native SaveAs/opener/migration
workflows pass, no failures/ignored; all captured source hashes unchanged.
The prior cross-directory/history/recovery refusal is resolved using the real
native mutation path, with exact old recovery bytes still asserted. Initial
decoder acknowledgement and changed asset epoch are accepted; actual same-row
restart/seek/rate/Resume-Pause changes revoke review without publication.
Evidence /tmp/deadsync-editor-save-as-acknowledgement-20261031.log/.json.
Final full native/core/public validation still awaits live timing/offset lots.

Source follow-up confirmed Song Properties clock/audio changes need the same
live-transport work: Draft.preserves_presentation_clock is false for OFFSET/
SyncOffset changes, so current Apply can leave the captured playback clock
unchanged; changing only Music currently calls stop_audio through the receipt
consumer. Pinned Editor.cs2402–2429 keeps Main while loading a changed asset and
recomputes offset/sync at the current double row. A third isolated lot now
handles Song Properties, coordinated with event timing and chart offset helpers.
Sole-Cargo core validation started for the new SaveAs intent-origin API;
/tmp/deadsync-editor-save-as-intent-core-20261031.log and sources.json.
No final completion or packaged-runtime claim.

Core session17294 terminal0:216 passed,0 failed,0 ignored; captured hashes
verified unchanged. This includes the new source visual-clock intent-origin
regression, all SaveAs core workflows and structural history predicates.
Receipt /tmp/deadsync-editor-save-as-intent-core-20261031.json. No root Cargo
remains running; live-timing/chart-offset/song-properties lots remain isolated
and are not yet validated by this green snapshot.

## Current translation key/static-format check

The current Edition sections contain2186 keys in each of EN and FR. A direct
section/key comparison found no missing or extra French keys and no differing
named-placeholder sets. This is a static coverage/format check, not a language
quality or runtime rendering certificate; live timing diagnostics are still
being prepared and must be included in the final check.

Integrated live timing final patch0b4674a79b98c734dca292de5e956332857fa260a25fa9d37c555827ede6941a. Only the closed live Plan produces TimingEdit history; the shared Target path remains unchanged for mixed clipboard transactions. Core session10890 passed219/0/0, source hashes unchanged. Eleven timing kinds are exercised with musical-time and positive/negative fractional-row anchors, plus stale/epoch and closed-view history refusal. Native session61465 now compiles the actual timing_live_tests module. Its extra diagnostic filter was misspelled; the intended timing_migration_and_live_diagnostics_are_translated_in_both_locales remains explicitly pending a separate exact invocation.
Prepared disposable final runtime fixture /tmp/deadsync-editor-final-transport-fixture.json with three charts (different clocks/signatures/offsets and a warp); original source file hashes were checked unchanged. It has not been opened in the game yet.

Native live-timing61465 stopped at compilation: timing_neighbor matched Result
although navigate_exact_position returns unit and reports its own error. Fixed
to invoke that canonical wrapper directly; no regression assertion removed.
Failure log retained. ChartOffset patch644de79a133d8e90c2707144d29d42325d3f89e70ac60579fbade233382d5b11 integrated after review. Its prospective Session setup now delegates the shared timing_live::presentation_candidate instead of duplicating identity/revision/grid/attack cloning. Main offset edits retain source double ROW, and typed chart-property Undo/Redo use canonical swap/seek without stop. Metadata receipts retain their exact clock path; nonfocused chart changes retain focus and current time.
Core/public session97983 validates full core and chart_offset_presentation_workflows3. Source hashes captured. Public inventory now290 targets; final harness refresh required. Native clock/offset tests and diagnostic exact test remain pending.

Chart offset session97983 terminal0: core219 passed and new public offset3
passed, zero failures/ignored. All captured core/public source hashes match.
Supplemental source review confirms source-compatible SaveAs sorts only the
detached output snapshot; resident chart indices remain stable, so the new
history Effect.chart must not be rebased. No external history replay occurs.
A further actual Marker metadata issue is confirmed in commit_marker: after
Apply it implicitly Seek(start), unlike the source metadata setter, replacing
the current fractional/focused position even on no-op. This is being corrected
in a separate isolated lot; no transport-stop claim is made for State.command.

Full native99324 completed535 tests:534 passed,1 failed,0 ignored; captured
source hashes unchanged. All four chart-offset tests and both live timing
Apply/stale workflows passed; the real EN/FR diagnostic exact test passed.
The sole failure expected generic SeekMusic after scoped command publication,
which correctly emitted EditorSeekMusic{owner:1013,seconds:2.0}. Fixture now
requires exact scoped owner/time and rejects generic publication; stopped
navigation additionally requires canonical pause. Removed the obsolete unused
UI Draft.apply path so Timing UI publication has only the closed Plan path.
Fresh20 selected native workflows running in session7788; log/sources at
/tmp/deadsync-editor-live-offset-neighbor-fixed-20261031.*. Not yet green.

Targeted7788 completed20/0/0, source hashes unchanged; neighbor scoped seek
and scroll-stop policy now both pass. MarkerMetadata patch integrated after
review; Apply/Delete and typed Undo/Redo use the exact preserved presentation
receipt and no implicit Seek. Core28754 completed221/0/0, hashes unchanged.
Native marker workflows remain pending; SongProperties closed producer is the
last known active mutation lot. Functional matrices prepared independently in
/tmp/deadsync-functional-matrix/ and /tmp/deadsync-final-functional-matrix/
list21/25 user workflow domains and concrete runtime checks. They identify
consumers/tests, not completion by file inventory; old snapshot results are
explicitly historical. Final full corpus and packaged QA remain required.

Native final snapshot completed555 tests:550 passed,5 failed,0 ignored. All727 captured inputs matched at terminal; failed receipt is /tmp/deadsync-editor-final-native-fixed-20261031.json. Reviewed fixture corrections preserve the source contracts: final-close retained override differs from an explicit chartless Song.MusicChanged (clears registration); automatic assets discovers images/lyrics rather than MUSIC; music-path comparison uses the published resource-base literal spelling; moved marker callbacks reopen the inspector and require closing its keyboard context before global history. Marker presentation assertions now capture the effective validated transport position rather than requiring the retained navigation intent anchor to be current. A new callback-memory test checks actual remembered marker rows through Apply/Undo/Redo and focus changes.
Fresh complete native selection is running once in session36221; no background controller or package chain was relaunched. A separate confirmed fidelity gap remains under review: image/lyrics automatic-assets and metadata-only SongProperties history still reach the generic audio-stop Undo path. Their Apply continuity must not be used as proof of Undo continuity. Core221 receipt remains historical until its source hashes are rechecked. Current public corpus and packaged runtime verification remain pending; no complete-port claim, commit, push or PR.

Current static Edition locale comparison:2191 keys in each of EN/FR, no missing/extra keys or named-placeholder mismatches. Evidence /tmp/deadsync-editor-edition-locale-parity-20261031.json freezes both language hashes; this does not certify translation quality or rendered glyphs. Rechecked final-core receipt:292 core source files still match exactly, so221/0/0 remains valid for current core before the pending metadata-history production lot.

Native fixture-correction suite36221 terminal:556 selected,554 PASS/2 FAIL/0ignored, all727 input hashes unchanged. Its three former media/override failures and physical marker Apply/Delete now pass. Remaining failures were incorrectly scoped fixtures: focus navigation is source music-time preserving, not bitwise row preserving; a direct Workspace note edit bypassed the native paused-presentation receipt before the stale-request assertion. Corrected tests retain exact row/time bits across metadata history and use a real physical New Seed mutation before stale Apply refusal. Targeted14801 terminal:3 PASS/0FAIL/0ignored, source hashes unchanged, receipt /tmp/deadsync-editor-marker-final-fixtures-20261031.json. This does not convert the preceding full native result into a green receipt; a fresh whole selection is still required after active production changes.
Additional source-field review found user controls not yet ported: Hotbar detailed position, Snap Limit, precise current zoom; source LimitZoomToSize checkbox is undoable while the native ZoomSettings publication is wholly direct. Source minimap basic→advanced can discard an uncommitted basic draft in the native adapter. These are open fidelity requirements, being corrected in isolated sidecars, not deferred out of scope. Root waveform field review /tmp/deadsync-waveform-final-field-review/ maps each source control to the actual runtime consumer; no additional missing waveform preference control confirmed by that reading. GPU/physical PCM validation remains distinct.

Song metadata history production patches integrated after review: closed receipt producer wraps only main-clock/main-path-preserving metadata; low-level Draft remains composable for widget clipboard. All five recursive history/relocation integration sites inspected. Core65317 terminal:221 library +5 new metadata public +4 existing widget clipboard PASS, zero failed/ignored; captured hashes unchanged. Combined receipt /tmp/deadsync-editor-song-metadata-history-20261031.json is bounded evidence, not the single-suite final-core gate. Full selected native suite now running once in86075, /tmp/deadsync-editor-native-metadata-integrated-20261031.log; do not infer green.
Correction to the zoom finding scope after checking exact source call arguments: UIScrollPreferences62/64 marks interpolation duration and zoom multiplier undoable too, while SizeCap and spacing speeds are direct. The required lot therefore has THREE undoable leaves (duration/multiplier/limit) and direct cap; a limited-only patch would be incomplete. Root field-review note corrected. Hotbar toolbar Beat itself already uses exact current focus through playback_position; no general idle-floor defect is claimed. The missing detailed readouts remain confirmed. Minimap/receptor sidecar is prepared, not integrated or executed while native sources are frozen.

Full native86075 terminal:560 PASS/0FAIL/0ignored, all730 captured source/language hashes unchanged,560 distinct passed names. Receipt /tmp/deadsync-editor-native-metadata-integrated-20261031.json. All marker and four new metadata/audio scenarios passed, including DedicatedPreview reload/stop-only-audition, sample region update, Main/Paused and chartless preservation. This green snapshot precedes the next lot.
Integrated Minimap shared-draft/receptor-gates patch1fed4eb89631a970c7383737b8d7997f2f75c3aa89ae01f387427a9e2697bc4e after review. Basic↔advanced snapshots now keep staged viewport/content; explicit Back appended without changing existing row indices; Cancel clears all drafts. Receptor lock/center gates now share actor/key/precise-entry policy and localized inactive feedback. Targeted native52431 running with frozen inputs; no result yet.

Minimap/receptor selected52431 terminal:30 PASS/0FAIL/0ignored, all captured source/language hashes unchanged. Receipt /tmp/deadsync-editor-minimap-receptor-controls-20261031.json. Covers physical two-page draft Apply/Cancel/Restore/Undo for disk and memory owners; receptor gate actor/key/numeric entry combinations; existing numeric/visual/raw-minimap regressions. Full560 native receipt remains an earlier snapshot and is not silently upgraded. Hotbar and three-field Zoom history lots still active.
Final public/package verifier preparation corrected to include root src/ and scripts/ in both frozen input inventory generators, in addition to crates/assets/licenses/.cargo and Cargo/build files. The prior manifest did not freeze root src/main.rs; all old manifests therefore require regeneration with this stronger input inventory. Python syntax parsed only, no final controller/package run launched.

Source Pack property review confirmed another open user workflow: source UIPackProperties156–175 makes property actions globally undoable (affectsFile=false), whereas native Pack Settings writes PackFile directly and only supplies text-buffer-local Undo. NoteEdit→PackSync→close→globalUndo therefore skips the latest source-equivalent action. Required fix is typed global application Pack-property history with existing PackFile ownership/CAS/durability and disjoint document/audio preservation. Source Create/Delete Pack remain distinct non-Undo operations, not automatically added to this lot. Additional Song/Chart property field review remains active; no broad completion inferred from menu presence.

Zoom history lot integrated after source-argument review: private RestoreScroll initial publication contains the direct SizeCap only for first forward commit; subsequent history owns duration/multiplier/limit leaves only. Cap-only publication preserves redo and renews the application review epoch. Canonical native draft baseline/SessionBinding, prospective Manager validation, changed-leaf dual CAS and unchanged-spacing animation preservation reviewed. Core50067 terminal221 library +4 Zoom history public tests PASS, zero failed/ignored, captured sources unchanged. Receipt /tmp/deadsync-editor-zoom-history-core-20261031.json.
Native8760 compilation failed before execution because root strengthened its paused-state assertion using a private sibling-module method. Production code was not implicated. Failure receipt sealed; test now uses the existing public scoped transport query while requiring chart transport stopped, which detects an invalid pause binding without exposing private internals. Fresh native90125 running, /tmp/deadsync-editor-zoom-history-native-fixed-20261031.log.
Song/Chart field review also identified a missing source UI route: TimingChart Apply presets copy from the song timing owner independently of focus. Generic core copy exists; native entry currently fixes source to focused chart. This source-B route plus Advanced entry is required and remains open; it must retain the real timing/audio notification policy, not fake parity by moving focus.

Zoom native90125 terminal28 PASS/0FAIL/0ignored, source hashes unchanged. Hotbar production patch860c6abb integrated after review. Its first compile71705 failed on an ambiguous floating literal in a new test; explicit f64 fixed without changing production. Full native41996 terminal570 selected:568 PASS/2FAIL (NoSong menu fixture and geometry), hashes738 inputs verified. Original Hotbar source oracle independently replayed1000 snap-lock triples and8 fractional-measure bit cases successfully: /tmp/deadsync-hotbar-original-root-fresh-20261031.json +provenance/log. Old reference tree/.NET SDK under/tmp was incomplete; root used a fresh pinned Fumen clone and a fresh official .NET10.0.403 task-local SDK with verified official SHA512, rebuilt Fumen cleanly, retained strict pin guards. No old expectation fixture was relabeled executed proof.
Integrated Hotbar fixture/menu distinction (NoSong FileMenu vs loaded chartless EmptyMenu) and actor size×scale/align measurements. Integrated Chart readonly source-statistics patch440deaa12ba64b2a4ab1ca625aece42296ca0fea4846e967f06552c9053aace0 after checking actual source Notes=StepCount, fake-aware StepTotals and original f32 ties-even lane percentages. Core49214 terminal222 library +11 existing density workflows PASS; all captured hashes unchanged. Native73357 terminal573 selected:571 PASS/2FAIL geometry (Hotbar +ChartDetails, en854x480 dpi1 row0), source hashes unchanged. All actual Hotbar input/snap/clock tests and all16-layout ChartDetails/source-cache workflows pass. Geometry remains OPEN; prior size×scale test fix did not resolve the failure and no render success is inferred.
Independent review found a ChartDetails/RSSP status ownership issue: both reuse Panel::Analysis, so an older RSSP job can overwrite the visible Source Statistics status unless poll_analysis visibility also excludes chart_details.active. A bounded fix and real-worker regression are being prepared. Pack property patch is prepared but not integrated pending merge-chain continuity and net-zero text-group review; Timing-owner Apply route remains active.

Shell interruption revalidated: no Cargo/test/controller process remained active before resuming. No automatic background pipeline restarted. Integrated TimingChart Apply patches core7bed9c75/nativeaab95ef5 after source/closed-plan/history recursion review. Quick presets bind staged TimingChart B independently from focused A; Advanced uses independent host-ordered source chooser. Core session33866 terminal:222 library +4 new chart-copy live +10 existing chart-copy workflows PASS, zero failed/ignored, captured core/test hashes unchanged. Receipt /tmp/deadsync-editor-chart-copy-targeted-20261031.json is combined bounded evidence, not a final single-suite gate. A further API review found Anchor payloads ignored by chart-copy projection; explicit focus-consistency validation is being prepared, not yet integrated.
Integrated two assertion diagnostics for Hotbar/ChartDetails geometry without changing tolerances or production layout. Integrated RSSP foreground-status ownership fix d0625f78 plus real-worker success/unsupported-layout regression; background cache remains active. New controlled native session53263 running all screens::edition tests against captured inputs: /tmp/deadsync-editor-timing-readout-native-20261031.log. No terminal result yet. Edition locale parity current2227 EN/FR keys, no duplicates or brace-placeholder mismatch: /tmp/deadsync-editor-edition-locale-parity-current-20261031.json. This is static language parity only. Pack property v2 still under preparation, original provisional Group patch never applied.

Native53263 terminal563 PASS/3FAIL/0ignored, hashes unchanged: /tmp/deadsync-editor-timing-readout-native-20261031.json. Bounded filter screens::edition:: omitted12 i18n::editor_errors cases included by previous broad edition filter; selection_complete=false explicitly. Four new TimingApply tests:3PASS, Advanced host-order fixtureFAIL at chart_copy_tests.rs304 (source0 expected1); root suspects Identity cache already populated then test swaps Reverse under unchanged key, pending actual cache review. RSSP realworker ownership test PASS. Numeric geometry evidence proves card size actors Px0/Px0 with scale760×456/830×456; test computes0×scale, incorrectly omitting solid native1×1 resolution in compose.rs7889+. Strict fixture correction is pending; no production layout defect inferred from those assertions. Anchor consistency delta66fc07d9 integrated AFTER sealing this run, therefore untested by it.

Advanced fixture root-reviewed against actual chart_list::Cache signature/reuse, corrected with4295df00: a counted reverse host collator is installed from initial timing-chart request and reused by Advanced; production cache unchanged. Geometry fixture root-reviewed against resolve_sprite_size_like_sm and corrected with6eacad66 using actual compose::build_screen on the exact Solid card, not a duplicated resolver or weakened containment. An additional composition regression distinguishes default unit Solid zoom dimensions from explicit dimensions that ignore scale. Broad native filter edition (including12 i18n diagnostics) session70905 now running against frozen inputs /tmp/deadsync-editor-readout-composer-native-20261031-sources.json; result not yet known.

Native70905 terminal566PASS/1FAIL/0ignored, hashes unchanged. Hotbar geometry, ChartDetails geometry, RSSP ownership, all four TimingApply native tests now PASS. Sole failure is newly added helper regression: DSL setsize100×50:zoom2 correctly composes200×100, not100×50. Root inspected dsl.rs1367–1390: builder folds zoom into explicit size and carries scale1; resolver does not see the unzoomed builder dimensions. Test expectation corrected to200×100, and raw Actor explicit size100×50/scale2 case independently requires100×50. No production change or containment relaxation. Receipt /tmp/deadsync-editor-readout-composer-native-20261031.json sealed failed evidence. Filter edition still omitted the12i18n cases (editor_errors != edition); next session60586 explicitly passes BOTH libtest filters screens::edition:: and i18n::editor_errors::. Snapshot /tmp/deadsync-editor-readout-composer-fixed-native-20261031-sources.json.

Native60586 terminal579PASS/0FAIL/0ignored. Both filters explicit, all previous573 names included,579 unique actual pass names and captured core/theme/language hashes unchanged. Receipt /tmp/deadsync-editor-readout-composer-fixed-native-20261031.json. Hotbar/ChartDetails actual-composer geometry, new builder/raw sizing regression, RSSP realworker status, TimingApplyB/Advanced/cache and12locale diagnostics all passed. This is the integrated snapshot BEFORE Pack v2, not proof of the subsequently integrated version.
Root reviewed and integrated ONLY clean Pack v2 40c9e2e542c8788266b1fb115f28478eda26f36ccfdf610597f1d3c723f0c5b1; v1 never applied. Shared owner keeps logical CAS tokens independent of durable Save; actual text staging captures Review before input, publishes once at deactivation, validates stale/no-op before changing history; no Group/history-stack amendment API retained. Source Browse/Clear routed to guarded Pack-root Image picker. Native typed global history avoids generic audio stop and synchronizes presentation cache. Core31455 now running complete library +chart_event_copy_live_workflows +pack_storage_workflows, /tmp/deadsync-editor-pack-history-core-20261031.log; no terminal result claimed. Pack six native regressions are prepared, not executed yet.

Pack31455 terminal248PASS/0FAIL/0ignored (230 library including8 Pack unit regressions,5 chart-copy live including strict Anchor payload rejection,13 Pack storage workflows), captured source hashes unchanged. /tmp/deadsync-editor-pack-history-core-20261031.json. Native78324 terminal576PASS/9FAIL, captured hashes unchanged; failure receipt sealed. Eight actual Pack departure/external typing workflows regress after staging; new Banner fixture incorrectly compares macOS /var alias against canonical /private/var. Root corrected only fixture canonical expected root, then native57469 targeted Banner route1PASS EN/FR Browse/Cancel/select/Clear/Save/global UndoRedo with transport retained, hashes verified in /tmp/deadsync-editor-pack-banner-canonical-native-20261031.json.
Root integrated reviewed Pack departure delta84a74d44: capture local Review after Ignore while keeping disk CAS strict at actual commit/Save/history; atomic flush of all retained fields before coupled Save, independent of obsolete row index. Existing eight failing workflows unchanged, three native+onecore regressions added. Root also corrected new invalid-field Save diagnostics with exact catalog entries PackInvalidText/PackInvalidYear and a bilingual no-publication test. Native69883 full editor+diagnostics in progress /tmp/deadsync-editor-pack-departure-native-20261031.log; no result claimed.
Final proof inventories strengthened again before any final controller run: include vendor patched local dependencies, root LICENSE and docs/editor/licenses, in addition to previous root src/crates/assets/licenses/scripts/.cargo inputs. Existing snapshots must regenerate. Both sidecar scripts AST parsed only.
Further source checks found genuine fidelity work, not validation-only: four held queries MouseSelectionAltBehavior/ControlBehavior/ShiftBehavior/LockReceptorMoveAxis are retained at import but not consumed/configurable. Sidecar implementation assigned. Default platform Ctrl/Super aggregation already works; an initial claim of broken Mac default was corrected, the gap concerns custom/imported queries. Original pinned cimgui first-frame witness EXECUTED exit0: /tmp/deadsync-profile-table-first-spec.json/log shows Pattern NoteType ascending and Performed StepSpeedMin ascending on frames0/1, not the native initial cultural Name order. Comparator consumers need corrected initial specs; source oracle tests explicit sorts separately. Source UIModals system alert remains a confirmed gap: Mac AudioServicesPlaySystemSound(0x1000), Windows Exclamation, Linux no-op; sidecar host intention assigned after Pack fixes. No complete source parity claim until these features and runtime proof are resolved.

Native23272 terminal593PASS/0FAIL/0ignored, all captured input hashes verified, receipt /tmp/deadsync-editor-pack-intent-profile-native-20261031.json. Pack intentional departure and original profile table initial sorting now pass. This is historical before subsequent source Reset and system-alert changes. Source per-action Reset now uses current-platform source defaults only in source mode, preserving source archive, unrelated commands and native-mode defaults. Four public reset workflows executed4PASS/0FAIL. System alert patch9596dffe integrated after review; OS host notification independent of PCM, ordinary preferences/rebinding silent. Physical reset/cancel/save/history ENFR regression added. Native targeted alert+reset currently session12084, no result claimed. Preliminary visual candidate built/packaged/strict-signed and launched in isolated portable Resources; keyboard fallback and French FTUE observed. It predates recent changes, so final same-source app/runtime gate remains open. No automatic background pipeline, commit, push or PR.

Targeted native12084 terminal6PASS/0FAIL: real physical source Reset Cancel/Save/global UndoRedo ENFR plus five typed system-alert workflows;776 captured inputs unchanged. Receipt /tmp/deadsync-editor-alert-reset-native-20261031.json is targeted, not a complete native gate. Reviewed and integrated held-query core f4faeb47 +native38d70cac; six noncallback queries now captured/persisted and consumed in actual region/click-selection and explicit 2D receptor gesture. Source defaults/platform aliases and foreign archive retained. Core public held/reset/source-profile selection currently84120; not yet terminal. URL QA cause verified against true Miso main section:403units at0.4 produces161.2width and6px ink. Integrated mapping15/24 with noncompressing clip and real-font/composer ENFR/DPI tests, not yet executed. File menu QA containment finding assigned separately. Preliminary FTUE receipt /tmp/deadsync-editor-visual-candidate-ftue-observation-20261031.json records observed screens and explicitly excludes final proof or unconfirmed file load.

Held/reset/source-profile core public selection84120 completed20PASS/0FAIL,595 input hashes unchanged; receipt /tmp/deadsync-editor-held-queries-core-20261031.json. Complete unfiltered core47154 completed231PASS/0FAIL/0ignored,297 source hashes unchanged, receipt /tmp/deadsync-editor-final-core-held-20261031.json. Native92935 failed compilation with5 new-test access/iterator errors, no tests executed. Root kept production privacy: moved receptor held-axis tests under playback, replaced direct event index assignments by actual Home/Down input, used vertices.iter(). Native80162 then completed611selected:604PASS/7FAIL/0ignored with all779 input hashes unchanged. Every previous593 workflow and new receptor-axis/source Reset/system-alert workflows pass. New failures isolated to URL mesh fixture, asset startup fixture, selection fixture expectations and preserved second capture alternative; detailed diagnosis assigned to agents, not classified as fixture-only until inspected. Failed receipt /tmp/deadsync-editor-final-native-held-visual-fixed-20261031.json sealed. Prepared corpus296modules/2323support/8293inputs is stale after test corrections; regenerate before execution. No current complete native/public/package/runtime gate.

Native25473 terminal612PASS/0FAIL/0ignored,779 inputs unchanged; all prior593 actual pass names included. Receipt /tmp/deadsync-editor-final-native-real-font-held-20261031.json. New failures were confirmed against source: strict lower/inclusive upper region bounds; current-row initial list inventory; untouched capture alternatives retained. Tests now use actual three-row inventory and real key navigation, preserving source behavior. Real-font tests use decoded PNG pages and production TextureStore; NullTextureContext cannot produce visible meshes and a negative witness proves it. File-menu test also loads production white texture and true fallback fonts; all geometry assertions pass. Updated generic held-query label ENFR. Fresh locale receipt2235 unique keys each, exact placeholder parity, no duplicate Edition keys. Fresh snapshot296 modules/2323support/8294inputs; exact regenerated harness matched primary. Explicit public controller launched root session25059, output /tmp/deadsync-public-final-pipeline/combined-run-20261031T054654.532969Z; actual executable lists1345tests and partition audit covers296modules. Live execution, not a terminal pass; no automatic package chain. Core231PASS remains matching. Final app build/package, shell/runtime review, temporary harness removal and user trial remain pending.

Public controller25059 terminalexit0:296/296 current modules,1345/1345 listed tests passed,0failed/ignored. Current source/fixture/frozen identities and hashes checked by controller across compile and each module; status /tmp/deadsync-public-final-pipeline/combined-run-20261031T054654.532969Z/status.json is complete+green. This certifies original test bodies in one path harness with separate module processes, not296 independent Cargo target packagings. Explicit separate package controller59921 now building, status /tmp/deadsync-final-package-real-font-held-20261031/status.json. No automatic chain; final same-source app/runtime still pending.

Final package controller59921 completed successfully: strict signature and14 resource pairs verified, original bundle /private/tmp/deadsync-final-package-real-font-held-20261031/DeadSync-Editor-FinalReview-79abb1149b0741d29ef9ce7bef918ce2.app retained untouched. Runtime review uses separately signed portable QA copy /tmp/DeadSync-Editor-Final-QA-20261031.app. Actual French FTUE six pages and modified options observed; persisted restart/replay still pending. Readable update URL and contained File menu observed. SnapOff navigation verified on distinct nonperiodic fixture: Down beat0→4 changes center notes and labels, Up returns original; earlier repeated-pattern apparent immobility was a false diagnosis, withdrawn. Actual UI note2 insertion at beat0 and SaveAs to review-copy.ssc succeeded. Independent disk receipt /tmp/deadsync-editor-final-runtime-evidence-20261031/save-as-disk.json confirms3charts retained, first row0000→0100, other charts unchanged, relative review.ogg exists. CtrlShiftS is recovery draft, as designed; actual SaveAs selected through menu CtrlF12. Undo/redo intermediate geometry was not separately captured, so no full history runtime certification. Reopen, cross-directory relocation and remaining eight workflow scopes still pending. No new production changes, Cargo process, commit, push or PR.

Actual final QA continuation: File/Open reopened review-copy.ssc with3charts and added beat0 Down note visible. F3 selected independent sourceB (60BPM offset0.75) while focusA (120BPM) remained; actual worker preview128/128notes displayed60BPM timing. PreviewCancel returned original focusA120BPM,3charts, clean document. Screenshot/detailed receipt /tmp/deadsync-editor-final-runtime-evidence-20261031/generation-cancel.json. Apply/history/save of generation and remaining runtime matrix still pending; full port completion unproven.

Final QA actual sourceB generation Apply/Undo/Redo/Save observed separately with screenshots generate-applied/undone/redone.png. Apply4/4 dirty; Undo1/3 clean originalA; Redo4/4 dirty; Save clean. Independent disk receipt generation-apply-disk.json confirms4charts, generated128notes and sourceB BPM60/offset0.75. Generated reopen and other runtime scopes remain pending.

Generation saved/reloaded actual UI verified: FileReload loaded4charts; CtrlB chose generated duplicateB (sorted display3/4, document source4/4), notes visible; screenshot generate-reloaded-opened.png. Initial restored focus selected originalB because descriptors identical; inspected pinned StepManiaEditor/Preferences/SavedSongInformation.cs64-79,258-274 and native recent_songs.rs: first exact descriptor match is source behavior, not confirmed port defect. Generation apply/history/save/reload scope now observed; other runtime matrix scopes pending.

New actual runtime finding: EventSelectionHelp long unwrapped block overflows card horizontally/vertically and overlaps EventShiftHelp/editor status. Primary event_selection.rs changed to separate wrapped bounded footer blocks (selection y=h-215,max100; shift y=h-110,max30; card bottomh-75). This is an initial correction, NOT runtime/font geometry verified yet. Previous native612/public1345/package proofs predate this production edit and cannot certify current tree; fresh relevant validation/build required. Manual hold gesture could not be executed: CUA rejects multiple non-modifier key chord3+Down with keyPressIncludedMultipleNonModifierKeys. No native hold runtime proof claimed; existing raw press/release tests remain historical matching that unchanged logic. Current QA app still previous build, EventSelection modal open on generated chart beat0.

Selection footer candidate cargo build39156 exit0 (48.49s), package23751 exit0. New isolated portable /tmp/DeadSync-Editor-Selection-QA-20261031.app strict signature verified after portable marker/INI copy. Receipt /tmp/deadsync-editor-selection-build-20261031.json; runtime not yet verified, full native/public gates not rerun. Build emits existing unused/deprecated warnings and linker unwind warning; no warning-free claim. No active Cargo/package process.

Selection candidate actual French1280x720 Retina UI verified wrapped help fits inside modal, shift block separate. Remaining duplicate full EventSelectionHelp in global status still overflowed; corrected status-only publications (open, default selection interaction, region cancel) to short EventSelectionTitle, keeping full help in modal. Latest source now newer than selection candidate; rebuild/visual and current native/public tests pending. Candidate currently EventSelection on first chart,3 current-row events.

Targeted native90463 exit0: selection_status_does_not_repeat_multiline_modal_help_en_fr1PASS/0FAIL/0ignored (2120filtered); full suite not claimed. Actual candidate copied3events Abeat0→Bbeat1; Undo clean/Redo dirty/Save clean observed/screenshots. Independent mixed-paste-disk.json confirms only chartindex3 changed: BPM0=60 preserved +1=120, signature0=3=4 preserved +1=4=4. Target already contained identical Down tap atbeat1, so notes unchanged; no empty-cell insertion claim. Other3charts unchanged. Reopen pending; latest short-status source requires rebuild. No active Cargo process.

Actual clipboard empty-cell case completed: step1/8, paste mixed clipboard atbeat1.5 generated Down tap; independent exact parsed-cell delta only(row72,lane1,tap), existing notes and other3charts unchanged. Save/FileReload/CtrlB explicit generated duplicate selection/Up to1.5 shows blue Down persisted; mixed-empty-cell-disk.json and saved/reloaded screenshots. Latest short-status source build65689 exit0 (45.22s), not packaged/launched yet. Full current native/public gates remain pending.

Latest short-status build packaged81103 exit0 in /tmp/DeadSync-Editor-Selection-Fixed-QA-20261031.app, portable INI+editor-preferences copied from prior QA and strict signed. Actual app startup with completed persisted FTUE skips wizard and restores recent song/views (new process/new bundle, copied persisted preferences; not same-install restart proof). EventSelection status is short and duplicate help gone. Empty picker reveals separate generic ui_dpi fallback defect: no rows causes first title centered, shifting card/footer down offscreen. Primary ui_dpi.rs now excludes title fallback for empty EventSelection only; needs regression plus build/runtime validation. Latest fixedQA currently empty EventSelection atBbeat1.5. New patch invalidates current fullnative/build proof, no completion claimed.

Empty-picker DPI regression23234 exit0:1PASS/0FAIL/0ignored,2121filtered; actual panel_actors under ENFR and1x/2x verifies no registered rows and modal frame stays centered instead of title fallback scroll. Full native suite not claimed. Actual fixedQA profile manager N→F2 Unicode name→End→Return creates Pattern copy QA Motif été20261031, catalog4/4; JSON independently parsed in portable AutogenConfigs. Receipt profile-pattern-create.json. Other profile operations/families and current empty-picker rebuild/runtime still pending. No active Cargo.

Current native edition rerun85182 terminalexit0:602PASS/0FAIL/0ignored,1520filtered,85.81s. Filter screens::edition:: only; diagnostics not included (older612combined receipt not relabeled). All latest selection/status/empty-picker tests included. Actual fixedQA all3profile families N/F2 Unicode rename/End/Return publish named copies; independent JSON receipt profile-three-families-create.json verifies one named valid file each. Full lifecycle/export/import/restore/delete/history/conflicts still pending. No active Cargo, no commit/push/PR.

Actual fixedQA expressed profile E/F11 export to ecc-qa-export.json, F10 import same file into draft, F2 Unicode renamed QA Réimport été20261031, End/Return SaveCopy publishes catalog6/6. Independent profile-expressed-roundtrip.json confirms Config exact equality and fresh distinctGuid. Full profile lifecycle and otherfamilyroundtrip remain pending. Latest primary ui_dpi empty fix tested602editionPASS but not built/launched; fixedQA still pre-empty-fix. No active Cargo.

Actual expressed profile RestoreDefaults F5/Return restores staged determination from useDefaultMethod to chooseMethodDynamically; native disabled field refuses edits as expected. Modified enabled method again, SaveOriginal actual UI wrote useDefaultMethod; F7 globalUndo independently read disk chooseMethodDynamically; F8 globalRedo disk bytes exactly equal saved baseline. Evidence profile-before-undo/after-undo/after-redo.json. Build43097 exit0 48.82s includes empty-picker fix, not yet packaged/runtime observed. Remaining lifecycle/families/runtime scopes still open; no active Cargo.

Current package57779 observed terminal exit0. Portable Empty-Picker-QA app configured with copied INI/editor preferences, strict codesign verified. Actual French1280x720 Retina empty EventSelection shows title, empty message, wrapped help and shift footer contained; short global status retained. Screenshot/receipt empty-picker-fixed.png/json under runtime evidence. Pattern disposable profile deletion, CtrlZ restoration (exact bytes), CtrlY deletion, second CtrlZ exact restoration verified in one editor session; profile-delete-history.json. Separate Expressed deletion observed but editor was exited before Undo, so its Undo runtime is NOT certified; CtrlZ after editor recreation did not restore it, consistent with discarded session history. No completion claim; remaining runtime matrix and final matching full gates still required. No active Cargo/package process, commit, push or PR.

Actual current portable QA cross-directory SaveAs to /tmp/deadsync-editor-relocation-runtime-20261031/déplacement.ssc succeeded. Independent parsed SSC receipt save-as-relocation.json confirms4charts, all bytes apart from escaped MUSIC field equal before subsequent edit, decoded relative music resolves to exact original review.ogg. Initial verifier needed SSC backslash-unescaping; literal filesystem miss was verifier error, not product defect. Added tap at row72 lane0 in document chart1, CloseSong opens dirty guard, default ContinueEditing cancels close, then Save persists exact single-cell delta; other3charts unchanged, source review-copy.ssc unchanged. close-dirty-guard/close-cancelled screenshots and close-cancel-disk.json. Actual Reload succeeds4charts; duplicate descriptor focus behavior remains source-equivalent. No new production edit or Cargo process; loss review, dirty Pack exit and broader matrix still pending.

Confirmed small-height event-picker overlap: fixed8rows end321 while footer starts265 at480logical height. Replaced fixed capacity with shared page_size(height), reserving24px below last event;4rows at480,8 at720. Both key PageUp/PageDown and rendered page counts/slices use same capacity. Targeted57490:3PASS then stronger76889:4PASS/0FAIL/0ignored, including real ENFR State selection navigation and rendered text counts at480/720. Current-source build69912 running; prior app cannot prove adaptive runtime yet. Final complete gates remain pending.

Adaptive picker build69912 exit0,47.46s; package34403 exit0 at /tmp/DeadSync-Editor-Adaptive-Picker-QA-20261031.app, not yet portable configured/launched. Same-install restart verified on previous EmptyPickerQA app: CmdQ did not exit, native close button did, app absent from inventory, exact same path relaunched; entering Editor restored recent song without FTUE; same-install-restart.png. Update notice dismissed normally. Source current fullsuite and adaptive smallscreen runtime still pending. No active build/package process.

AdaptivePickerQA portable configured854x480 and strict signature verified. Actual French Retina selection129/163,41pages; Home page1 and PageDown page2 show4rows and separated contained help, screenshots adaptive-picker-small-page1/2.png. Current edition suite50548 exit0:604PASS/0FAIL/0ignored,1520filtered,86.27s. Real Pattern preview sourceB region1.5–5.5,17notes leaves disk unchanged. Apply creates persistent marker and reopens inspector; close inspector before Save. Independent GAChartData parsed: chart1 only, row72 Length192 seed0 withdefaultconfigGUIDs; other3charts byte-exact unchanged. Undo+Save exact original, Redo reopens inspector; close then Save exact applied (initial save inside inspector did not save, not a history failure). pattern-before/applied.ssc, pattern-history.json, pattern-marker-parsed.json. AltP attempted from global state produced no observable batch preview; regeneration action remains unverified. No active Cargo/package, commit/push/PR; full objective still pending remaining runtime matrix and final gate snapshot.

PatternActions runtime advanced: F1 Regeneration→F8 exposes all-pattern scope; Prepare required before Start. Fixed-seed1marker preview17notes cancelled, disk unchanged. New-seed1marker produced2145561444; Apply/Save changed notes andseed. FirstUndo restoresnotes while keepingnewseed; secondUndo+Save exact baseline;2Redos+Save exact applied, matching separate seed history contract. pattern-set-history.json andSSCs. Prepared separate two-markers.ssc fixture from portable QA copy with secondmarker row504 length192 seed1234; no user originals touched. DuplicateB descriptors sorted tie chose source4 first; source2 selected via otherB entry, then prepared2markers. Actual preview2ready/0empty/0fail,34notes; Right result2 row10.5 seed1234. Apply/Save/UndoSave/RedoSave: independent parsedcells+metadata undo exact,othercharts0/2/3 unchanged,eachregion17notes,marker metadata unchanged,Redo saved bytes exact. two-markers-result.json andSSCs/screenshots. Open render issue: small PatternSet preview leftmost arrow extends under minimap (two-markers-preview2.png); must inspect preview width/placement and fix before fullrender claim. No production changes this turn, no active Cargo/package/commit/push/PR.

PatternSet small-window overlap root confirmed: side panel gated only width>=800, candidate centered at(width-390)/2 places4lane field left136 at854 while left overview ends200. pattern_set_side_panel now requires actual scaled note envelope clear visible left-window density/minimap and lateral panel; source receptor projection retained. Otherwise existing bottom preview used; no preference/document mutation. Pure layout regression covers4/8/10columns,854/1280,customwideoverview,zoom anduserreceptor displacement. Targeted92993 running; earlier604native gate predates this new production edit. Runtime/build pending.

Preview layout targeted92993 exit0:1PASS/0FAIL/0ignored. Build2338 exit0,45.60s; package84223 exit0, /tmp/DeadSync-Editor-Preview-Layout-QA-20261031.app portable854x480 INI/prefs copied and strict codesign verified. Actual two-marker fixed-seed preview result2/2 at10.5 shows bottom panel and note sprites clear left overviews; pattern-set-layout-fixed-small.png. Cancelled preview after observation. Old604suite predates layout production edit; no full-current gate claim. Current app restored duplicateB by descriptor (source behavior) and explicit otherB selection prepared2markers; no file changes for this visual check. No active processes/commit/push/PR.

Current PreviewLayoutQA marker504 lifecycle actualUI: Clear preview17notes/0holds, ApplySave; parsed removal exactly17within504..696,noadded/changedcells,allmetadata andother3charts unchanged. UndoSave exact bytes baseline. F6 Delete confirm thenSave removes onlymarker504,allchartnotes intact,peer72 unchanged; Undo reopensinspector,EscapeSave exact baseline. marker-clear-delete-parsed/history.json andSSCs/screenshots. Fullcurrent edition35892 terminalexit0:605PASS/0FAIL/0ignored,1520filtered,85.56s; includespreviewlayoutfix. Closedclean song actualUI; no-songProfileManager opens. PatternN/F2Unicode/SaveCopy/F11export/F10import/F2rename/SaveCopy works chartless; independent profile-pattern-chartless-roundtrip.json confirms identicalConfig andfreshGuid. Importfile pc-ff3f0ec3-2cf5-43cc-b4ae-931c95f75842.json. CurrentUI no song,managerPattern selectedimportedcopy. Otherfamilies chartless lifecycles, profilesconflicts/assignments, remainingruntime matrix andfinalfrozen gates stillpending. NoactiveCargo/package/commit/push/PR.

Actual chartless profile continuation: Performed andExpressed N/F2Unicode/SaveCopy/F11export/F10import/F2rename/SaveCopy completed. Independent profiles-three-families-chartless-roundtrip.json verifies all3families sameConfig/freshGuid. Expressed importedpath ecc-b1133a41-227e-4358-bc56-8927539b744c.json. Delete confirmation0references/0charts→Delete fileabsent→CtrlZ exactbytesrestored, profile-chartless-delete-undo.json. Reopened sameprofile, F3stagedDescription; deliberately altered onlydisposableQA fileDescription externally; End+3Down SaveOriginal refused with localized diskchanged message andexternalbytes remainedexact. profile-chartless-conflict.png/json. Restoredfixtureoriginalbytes andEsc cancelleddraft. CurrentUI noSong/ProfileManager. No sourcechanges/no new tests needed;605currenteditiongate remainsmatching. NoactiveCargo/package/commit/push/PR.

Actual Help→ReplayIntroduction testedFrench854x480 withoutsong. All6pages captured/render-inspected, fields retained whileNext traversed; Done closesactualmodal. DiskFtueIndex6/LastCompletedFtueVersion0.1.0; comparingpreReplay-traversal snapshot onlythose2fields differ. ftue-replay-fr-1..6.png andftue-replay-fr-completed.json. InitialnoSongFileMenu navigation accidentallyopenedLog (ProfileManagerEscape returnedFileMenu17); closedLog andopenedHelp fromfreshFileMenu0. Do notclaim comprehensiveLogQA fromthatobservation: duplicatefooterhelp visible, notyetreviewed. ENwizard andliveclockpreservationremainpending; no sourcechanges,Cargo/package,commit/push/PR.

EnglishFTUE actualruntime: stoppedPreviewLayoutQA via nativeclose,absenceconfirmed; backedupQA INI/preferences under before-en-qa*,setLanguageen/FtueIndex0/LastCompletednull ONLYportablecopy,resignedstrict. This simulates first-runstate whilepreservingrecents,notblankinstallation. Launched→Editor restoredsong two-markers.ssc andFTUE1/6. All6Englishpages observed854x480; changeddefaulttype dance_double,newSongoffsetD0,openSongRight0.010,PageDowngraphdance_double thenRightdisabled. Done persistsFtue6/completed0.1.0,update notice dismissed. Songbytesexact andpausedvisiblecursor0.5remains; noaudible/liveclockclaim. ftue-en-1..6.png/result.json. RecentFiles alsochanged: duplicateB descriptors collapsedonreopen from3views to2 (knownsourcefirstmatchbehavior),storedChartPosition504unchanged; donotmisattribute toFTUEtimingmutation. Optionspersistednewchoices. CurrentQA English/no panel/restoredsong; actualuserconfiguntouched. No sourcechanges,Cargo/package,commit/push/PR.

Playback actualUI current PreviewLayoutQA: paused at359.108, opened/cancelled waveform preferences, visible beat remained359.108; Return resumed first observation361.391, secondReturn paused372.164. Saved playback-paused-before-panel/after-panel/resumed/paused-final screenshots and playback-pause-resume.json. This proves visible transport state conservation/resume, not audible PCM continuity or absence of tiny glitches. App left paused; no source edits or Cargo processes. Full port objective remains pending remaining matrix and final frozen gates.

Actual output refusal/cancel current PreviewLayoutQA: main24 OutputSettings, staged Advanced/SM, SaveAs /tmp/deadsync-editor-relocation-runtime-20261031/loss-review.sm. Identical timing default rejects BPMS/TIMESIGNATURES/OFFSET differences; explicit Accept attempt stays in loss review with blocking feedback. Escape returns editor Output cancelled/published policy unchanged. Independent bytes compare source two-markers.ssc unchanged and destination absent. export-sm-timing-refusal/blocked-confirmation.png +export-sm-refusal-cancel.json. CtrlShiftS is recovery draft (observed successful), not SaveAs; UI shows CtrlF12 SaveAs. Warnings-acknowledge/publish path still pending; no production edit/Cargo/commit/push/PR.

Actual SSC loss acceptance: AdvancedSSC/OmitCustomData, SaveAs loss-review.ssc. Six warnings (Patterns,GA metadata,GASongDataVersion,GASongData,GAChartData,GAChartDataVersion). Accept before acknowledgements blocked; individually acknowledged6 then explicit destinationWrite completed durably. Sourcebytes unchanged per CUA-read receipt; export contains4charts and no GA tags per corrected case-sensitive-name parser. Initial parser allowed uppercase only and missed mixedcaseGA tags, replaced receipt rather than claiming that result. Advanced writer materializes timing restored fromGAmetadata, so nonGA rawtag equality not expected. SM additional blockingWarp observed separately. Screenshots export-ssc-losses/export-sm-warp-error plus export-ssc-parsed/source-preserved.json. Reopen not yet verified: CtrlO no effect with current bindings; useFileMenu next. No productionedit/Cargo/commit/push/PR.

SSC export verifier clarification: raw note block equality false (writer formatting/resolution differs); exact rational beat/lane/type cell comparison across all4charts False. Do not claim raw blocks identical.

Export note discrepancy resolved by bijective chart comparison: source indices0,1,2,3 map to export0,2,3,1 respectively. All4 descriptions and exact rational beat/lane/type cells match, including129/154/128/129 cells. Prior positional list comparison false was chart sorting, not lost notes. Updated export-ssc-parsed.json retains raw comparison result and adds mapping/proven resolution. ActualFileMenuReload completes andCtrlB showsall4charts; export-ssc-reloaded-charts.png. Current app loss-review.ssc/chartlist. No production changes or Cargo; broader remaining matrix/final gates still pending.

Pack-only dirty OS exit actualUI complete: disposable /tmp/deadsync-pack-exit-runtime-20261031/Song/song.ssc +Pack.ini. Openedpackmain18, typedDisplayTitleUnicode,Escape retainsedit; nativewindowclose opensFileUnsavedguard. KeepEditing returnseditor anddiskPack.ini unchanged. Secondnativeclose→SaveAndContinue persistsDisplayTitle=QA Sortie non enregistrée andapp disappearsfromfilteredlistApps aftersave (firstimmediateinventorystillrunning, secondempty). pack-exit-guard.png /pack-exit-result.json. QAfixtureusedTitle instead ofDisplayTitle initially (unknownkeypreserved); realUIeditableDisplayTitle staged/saved correctly. CUA timedout once duringmenu navigation; reset/reboundandobservedindex24beforecontinuing, didnotreplayblindly. CurrentPreviewLayoutQA exited; no source/Cargo/commit/push/PR. Discard confirmation branch still separate from this Save/Cancel coverage.

Confirmed Log duplicatehelp cause: open_log publishes fullLogHelp asstatus whilelog_actors also renderssamehelpfooter. Changedsuccessfulopenstatus toLogTitle, preservingerrorstatus. AddedENFR realState regression checkingLogpanel, shortstatus, exactlyoneLogHelpactor. TargetedCargo51119 compilingcompleted37.89s thennativeharnessrunning; resultnotyetknown. Previous605editiongate/buildapp predatethisproductionchange.

Logtarget51119terminalexit0:7PASS/0FAIL/0ignored,2119filtered; newENFRpresentationregressionpassed. log-help-regression.json. Rebuiltappvisualcheck/fullcurrentgatespending; noactiveCargo.

StaticStreamfieldreviewagainstpinnedUIStreamPreferences/PreferencesStream:all7sourcefields present withsameUIbounds/defaults andUTF16markrestriction; configure_stream_preferences feedsdensity_settings/source_density.stream/refresh_source_density. Exactinputhashes/fieldmapping stream-field-review.json. This closesfieldpresencequestiononly, notruntimeGPU/descriptioncoverage. Fullcurrentedition63755stillrunning, nosecondCargo.

Fullcurrentedition63755terminalexit0:606PASS/0FAIL/0ignored,1520filtered,83.76s; includesLogshortstatusfix. edition-suite-log-fix.json. Notwholeworkspace/frozenfinalcorpus/runtimecertification.

Currentbuild7900exit0,44.79s; package34335exit0 at/tmp/DeadSync-Editor-Log-Fixed-QA-20261031.app identifierorg.deadsync.editor.logfixedqa.r20261031. PortableINI/editorprefs/marker copiedfromclosedPreviewLayoutQA; languageFR. Signafterconfiguration pending. NoactiveCargo/package.

LogFixedQA launchedFR, restoredpackfixture withoutwizard, updaternotificationdismissed. Mainmenu31Log actualsinglefooterhelp, shortstatus; Info→Warn→Error realfilter results21→0 atError, restoredInfo. log-help-fixed-fr.png. F10 isTiming (cancelled); Display29 differentpanel(cancelled). ShiftS→F12speedspage→Reverse row3 toggled: receptorbottom/futurenotesabove/minimapviewportbottom,Down moves0→-0.5 withcentralnotes; globalCtrlZreturnsforwardlayoutandkeeps-0.5. reverse-time-beat0/beat-half.png +reverse-time-runtime.json. OnlyConstantTime/854x480FRretina, notfullrendermatrix. No sourcechanges/Cargo/commit/push/PR; appleftforwardpausedbeat-0.5.

ActualspacingruntimeLogFixedQA: CtrlTrow at-0.5,Down→0 centralnotesmove; CtrlShiftTvariable at0,Down→0.5 centralnotesmove; speeds page confirmsVariableonlyenabled. Screenshots spacing-row/variable-before/after. Songdiskbytesunchanged. Confirmedrotatedrighttapleaksbelowchartbottom(variable0.5): rotated-note-bottom-clip-before-fix.png. Rootcausehold_render clip_to_viewport90/270cropmappingassumedclockwisescreenrotationwhilecomposeusesworldYup. Swappedquarterturnmapping, addedcornergeometryregression all4cardinalrotations×4viewportedgesusingcompositorhandedness. Target15729compiling/running; notvalidatedyet. Current606suite/app predatesfix. NootherCargo.

Rotatedclippingtarget15729terminalexit0:2PASS/0FAIL/0ignored,2125filtered. Cardinalcornerregressionandall-sourceholdhandednesspass. Independentnumericcounterexample savedrotated-clip-counterexample.json: old90bottomcropleavesYmax240vsviewport220; correctedYmax220. Fullsuite/build/renderverificationpending. NoactiveCargo.

Clipfirstfixbuild68529exit0 46.37s/package50117exit0 portable/signstrictverified at/tmp/DeadSync-Editor-Clip-Fixed-QA-20261031.app. ActualsameVariablebeat0.5 stillleaksbelowbottom: rotated-note-partial-fix-still-leaks.png. Thus prior2PASSdoesnotprovefix: testassumedcropoffsetrotates, but actualcompose.rs SpriteInstanceRawcenter usesunrotatedcropoffset. Addedactoroffsetcompensation rotatedDelta-minusDelta andrewrotecornertesttomatchactualnon-skewcompositor(center+unrotateddelta,rotatedcroppedhalfextent). Newtarget23138running. Firstnumericcounterexampleonlyprovesintendedlocalmapping, notactualpriorrenderer. Fullgoal/clipfixremainopenpendingnewtestandruntime.

Secondclippingtarget23138terminalexit0:2PASS/0FAIL/0ignored,2125filtered. Actualcompositorcenter-offsetregressionpasses. AppClipFixedQAstillfirstfix/stale; secondfixneedsbuild/package/reproductionbeforeclaimingvisualresolved. NoactiveCargo.

Secondclipbuild74729exit0 48.08s/package8365exit0 /tmp/DeadSync-Editor-Clip-Centered-QA-20261031.app portablefromclosedClipFixedQA/signstrictverified. ActualFRVariablebeat0.5 samefixture bottomrighttapcontained: rotated-note-bottom-clip-centered-fixed.png. Pillowpixelinspection same950..1079×856..885 ROI: prior604bluepixels(lowest883),new0. rotated-clip-runtime-pixels.json. Boundedruntimeproof of reproducedbottomrighttap only; otherrotations coveredgeometrically, notfullGPUmatrix. Fullcurrenteditionrerunstartedafterbuild.

Fullcurrentedition87733terminalexit0:607PASS/0FAIL/0ignored,1520filtered,85.07s; includessecondclipfix. edition-suite-clip-centered.json. Actualchartlessgeneration: CtrlWcloseslastview; F3opensgeneratorwithsourcesavailable, row7Rightselectsdocsource4B60offset.75, Calculate produces129/129notes/0discarded. Previewshowsstatistics/buttonsbutnocentralnoteviewwhilechartless; chartless-generation-preview.png. Escape returnsnoactivechart unchangedstatus. Investigate visualpreviewrequirement/sourcebehavior next; no fullchartlessApply/historyclaim. AppCenteredQAleftnoactivechart, songloaded. NoactiveCargo/commit/push/PR.

Chartlessgenerationfidelityreview: originalUIAutogenChartdirectActionAutoGenerateCharts/addchart, noseparatevisualpreviewrequired; sourcehashesreceiptchartless-generation-source-review.json. ActualnativeCancelthennewF3selectBsource4,129notes,Applyopensnewsource5; CtrlZreturnsnoactiveview,CtrlYrestoresnewchart; Save4→5charts. Independentchartless-generation-disk.json verifiesexisting4chartchunksstrip-byteexact and129nonzeronewcells. ActualFileReload→CtrlB lists5charts; chartless-generation-applied/undone/reloaded.png +before/appliedSSC. DuplicateBrestorefirstdescriptor behavior remainsknownsourceequivalent. Sourceprofilealternatives/co-opbatchstillpending. No sourcechanges/Cargo/commit/push/PR.

MixedStepF2batchactualruntime: disposablemixed.ssc containsrawpump-doublecoopX/Y +solopump-double +dance-single. Main10Batchsourcepump-doublecounts1(excludescoopeffectiveroutine),previewsource2only1ready/0fail32notes. Cancel diskexact; restarted samebatchApplySave addsonechart(3→4),original3chartchunksstrip-byteexactincludingcoop. UndoSave original3chartsexact, rawfilediffonlyfirstsaveGASongData syncdefault0.010fromQAOpenSongpref; notgenerationnotechange. RedoSave bytesexactapplied. coop-batch-admission/preview.png +before/applied/undoneSSC +coop-batch-result.json. ReproducedrighttimingwidgetoverlapPreviewMusic/BPMatrow0 inthisfixture; openrenderfindingfornextpass. No sourcechange/Cargo/commit/push/PR. Appcurrentmixed.ssc4chartsafterredo.

Widgetoverlapfindingreclassifiedafterreview: originalMiscEventWidgetLayoutManager173–175 groupsstrictcharttime/position, nativeusesidenticalkeypolicy/sharedselectionrects. MixedQAhasOpenSongSyncOffset0.010,sample0andBPM0projectnearbybutdifferentkeys. Preparedzero-offset.sscchangingonlyGASongDataSyncOffset0; actualsameUIbeat0nowstacksPreview/BPMseparately correctly, widgets-identical-position-zero-offset.png. widgets-near-position-review.jsonwithsourcehash. No confirmedportbug; overlapconsistentwithsourcepolicy, originalappnotexecutedforthiscase. Donotclaimarbitrarynearbywidgetsneveroverlap. No productedit/Cargo;607suite/appstillcurrent. Appzero-offset.ssc3/4dancefocuspaused0.

Largewindowruntime: nativezoom viaAXsecondaryaction2 enlarges screenshot3456x2168 vs1710x1026 restore. FRzero-offset.ssc Variableforward: density/minimapalignedtop/bottomunderlabels, samepositionPreview/BPMsidebysidewhenwidthpermits. ShiftVReverse actualreceptorbottom/futurenotesabove/minimapviewportbottom; Down0→-0.5 movescentralnotes; CtrlZreturnsforwardkeeps-0.5. Inspectedreverse-navigatedframevia view_image. Captureswide-variable-{forward,reverse,reverse-navigated,undo-forward}.png; window-restored-after-wide.png; wide-window-render.json. Retinaonly, notphysicalDPI1/backend/holdregioncertification. No source/Cargo/commit/push/PR; apprestoredsmallforwardpaused-0.5.

NEWconfirmedholdrenderdefect: disposableholds.ssc(8measures,longlane0hold0→768,rolllane1 48→192,shortholdlane3 24→96,latershorthold480→576,tapslane2). ActualVariableforward0and0.5: rollhead/tailbutnobody,shortlane3bodygap,partialnotice; holds-variable-start/body-budget-defect.png. Rootview_render::notes300hold_strips_remaining=note_limit conflatesadmittednotecount(callerpositions.len())withbodytilecount; earlierholdconsumestilesandlaterrolltruncates. hold_geometry::plan111bodybudgetstopconfirmed. OPENfixneedsseparateboundedgeometrybudgetplusregression/runtime. No codechangeorCargo;607suitepassesbutdoesnotcoverthisscenario. Appholds.sscpaused0.5.

Holdbodybudgetfiximplemented: view_render::notes no longer sharesnote_limit astilebudget; eachadmittedholdusesviewport_body_budget(width,height), conservativeceil(height*128/width)+4 boundbecauseatlasbody>=1sourcepixel andscale=width/128, hardcap16384forextremesubpixelgeometry. Admission/querylimitsunchanged, planstillreportsquotaexhaustion. Regressionrealnotesrender2admittedheads(longholdtail1e9 +adjacentroll),3viewportheights assertsnopartialandbodyactorsinbothlanes. Target6764running; build/runtime/fullsuitepending. Sourcefilesfrozenthroughrun.

Holdbodytarget6764terminalexit0:4PASS/0FAIL/0ignored,2124filtered; newrealrendererregressionpasses3heights, oldnotecandidate/querylimitsremainpassing. hold-body-budget-regression.json. CurrentappCenteredQAand607suitepredatebodybudgetfix; runtime/fullrerunpending. NoactiveCargo.

Holdbodybuild69711exit0 77s/package19759exit0 /tmp/DeadSync-Editor-Hold-Bodies-QA-20261031.app,portablefromclosedCenteredQA/signstrictverified. ActualsameholdsfixtureVariable0.5: rollorangebodycomplete, shortlane3holdbodygapresolved, no partialnotice; holds-variable-body-budget-fixed.png. ShiftVReverse bodiesremainvisible/clipped; holds-variable-reverse-fixed.png. OrangebodyROI775..838×490..769countindependentlycomparedagainstpre-fix; hold-body-budget-runtime.json. Bounded4laneFRRetinasmallscreenproof, notalllayouts/backendcertification. NoactiveCargo; fullsuiteafterbodybudgetfixpending, previous607predatesfix. AppleftReversepaused0.5.

Fullcurrenteditionbodybudgetrerun15785started andconfirmedrunning viaCargo output; do nottreatasgreenuntilterminal. SoleactiveCargo.

Fullcurrentedition15785terminalexit0:608PASS/0FAIL/0ignored,1520filtered85.20s; edition-suite-hold-body.json. AttemptedSpace/Downsequence doesnotproveheldrangegesture(CUApressreleaseskey; capitalSpaceunsupportedkeyNotFound). CtrlShiftEinspectorF3wholechartNotesApplyselects36/38(32taps+4holds/roll),Escapeviewstillordinaryred/whiteheads/bodies. holds-whole-chart-note-selection.png. Confirmednativeview_render::notes selectedflagonlychecksworkspace rowrange, while misc_event_actors uses explicit event_selection. OPENexplicitnoteselectionrenderwiring: inspectPolicy/selectionpipeline, addregression andruntimebeforeclaimingselectedholdrender. No productedit/Cargo. Appholds.sscforwardpaused2.0, explicit36notesselected.

Explicitnoteselectionrenderimplemented: Stategetter filterscurrentSessionSelection/chart/activefocus andexcludesgenerationpreview, producesBTreeSetpositions; passedtofocusedandpeersharednotesrenderer; shortnoteandhold/rollselectedflagsunionrowrange+explicitmembership. ExistingdirectrendertestcallsupdatedNone. Addedactualsprite tint/UV equivalence explicitvsrowrange forTap/Hold/Roll andrealStatechartisolationtest. Target88456compiling/running, noresultyet. 608suiteandHoldBodiesQA predatesnewselectionfix. NootherCargo; no commit/push/PR.

Firstselectiontarget88456exit0:2PASS. Strengthenedcontractafterreview: currentexplicitselection(includingempty)mustoverridelegacyrowrange, notunion, so useruncheckcannotstayhighlighted. GetterreturnsOption<BTreeSet> todistinguishnonefromempty; rendererusesexplicitmembershiporfallsbacktorange. Addedemptyexplicit+nonemptyrangeassertion; secondtarget38666running, currentnewsourceunverifieduntilterminal.

Secondselectiontarget38666terminalexit0:2PASS/0FAIL/0ignored,2128filtered; explicit-note-selection-regression.json. NativeStatechartisolation +actualspritesTap/Hold/Roll +explicitdeselectprioritypass. Fullsuite/build/runtimepending; noactiveCargo. CurrentHoldBodiesQAappisstale(selectionfixnotincluded),36notesremainselectedinUI. No commit/push/PR.

Explicit selection current-source build60039 exit0 (46.66s), package80399 exit0, portable Explicit-Selection-QA configured and strict signature verified. Full edition suite63756 exit0:610PASS/0FAIL/0ignored/1520filtered,84.98s. Actual FR Retina854x480 whole-chart Notes preset then CtrlC closes inspector preserving selection: taps/hold/roll visibly gain selected contours/body color, explicit-all-notes-after-copy.png. Escape clearing selection agrees with pinned source Editor.OnEscape; earlier after-Escape screenshot is not evidence of missing selection rendering. Wiring fix now proven by real selected render and targeted tests, without changing Escape behavior. No active Cargo/package. Broader runtime matrix/final frozen gates remain open; no commit/push/PR.

Performed profile actual-runtime lifecycle completed on ExplicitSelectionQA: New/Unicode rename/SaveCopy; Facing.MaxInwardPercentage1→0.9999/SaveOriginal independently parsed; F5RestoreDefaults confirmed/SaveOriginal restores entire baselineConfig with sameName/Guid; F7Undo exact editedbytes, F8Redo exact restoredbytes. performed-restore-history.json and3JSON snapshots. Initial manager returnedfocusPattern aftersave; transient Pattern edit cancelled withoutsave before selecting correctPerformed row. No production source changes, no active Cargo, latest610suite remains current. Checkpoint top-level stale app/test/context fields refreshed. Pattern restore/assignments and broader matrix still open; no commit/push/PR.

Pattern profile real lifecycle: New/Unicode name/SaveCopy; BeatSubDivision4→3 SaveOriginal; RestoreDefaults/SaveOriginal exact baselineConfig andsameName/Guid; F7Undo exact editedbytes/F8Redo exact restoredbytes. pattern-restore-history.json. Marker profile assignment actualGUI on disposable profile-assignment.ssc sourcechart1 row72: customPattern322c0613, built-inPerformed0dde2261 and chartExpressed0c0ba200 persisted; marker504 andother3charts byteexact, notes/nonGAchartbytes unchanged; UndoSave exact baseline,RedoSave exact applied. Pausedbeat1.5/time2.000,Bfocus/Apeer unchanged. profile-assignment-history.json/screenshots/SSCs. Initial CtrlO is not bound in QA; typing whileglobal led to unintendedscratchnewdocument; scratchdiscardguard confirmed, realfileopened throughFileMenu already-active pathfield. No original files changed; this is a test-navigation issue, not a validated CtrlO productbinding claim. Cross-chartmarkerfocusA/co-op andremainingrender/audio/OS still open. No sourcecode changes or active Cargo;610suite remains matching.

Multiplayer autogen scope clarified against pinned EditorChart.cs1156/3115/3217: source excludes multiplayer autogen and Pattern events. Native effective-layout batch admission and actual mixed StepF2 runtime already verify bounded exclusion/preservation. multiplayer-autogen-scope.json records hashes/contract; do not track new multiplayer generation as missing original functionality. README latest checkpoint added to supersede stale historical pending lists without erasing their evidence limits. Full port remains incomplete; no active build/test or commit/push/PR.

Actual About keyboard runtime on ExplicitSelectionQA:4licenses opened, first/last screenshots saved, Escape returnscorrectrow; version0.5.2373/updateravailablev0.5.2475 visible; Bfocus/Apeer time2.000 unchanged. about-licenses-runtime.json; noteveryintermediateGPLpage or externalURLs certified. WaveformpreferencesShow/Enabledtrue, W persistedtoggleverified; signalnotvisible at2.000. Independent ffmpeg8sdecode confirmsnon-silentstereo44100Hz mean-12.7dB peak-0.7dB, so nextinspectpreparedsource/drawpolicy. No confirmedrootcause/productfixyet. CUA17-stepcalltimeoutreboundandscreenshotshowedMenu29; no blindreplay. NoactiveCargo/build or commit/push/PR;610suite remains matching.

Waveform investigation progressed: actual OpenGL FXAAoff and Reload stillblank though WaveformReady; matching Metal clone logconfirmsMetal andsameblank, so notOpenGL-only. Rootsharedproducer buildsSolidquad zoomto(width,height), thenreplacesSpriteSourcewithRenderTarget; defaultsize0andscalerelative1x1 then compositor multipliesnewintrinsictexture1024xheight, creatinghuge sampling rectangle. Fixedwithsetsize explicitbounds/unitScale. StrengthenedrealproducerFXAAon/offtest checksdimensions/scales; target76867 exit0:1PASS/0FAIL/2129filtered. Addedrealassetservice shelltest escapedUnicode relativeMusic/twofocuschoices:95939 exit0:1PASS/0FAIL/445filtered. Build22537 current running; latest610suite/apps predateproductionfix. No runtimefixclaim untilnewapp; no commit/push/PR.

Waveformbounds build22537 exit0,42.08s; package37994 exit0,portable fromclosedExplicitSelectionQA includingAutogenConfigs, strictsignatureverified. Actual OpenGLFR854x480Retina atsame2.000 nowshows2greenchannelscontainedbehindnotes; FXAAoff thenon bothvisuallyverified/screenshots. waveform-destination-runtime.json. Fullcurrentedition20648 exit0:610PASS/0FAIL/0ignored/1520filtered,83.01s; edition-suite-waveform-bounds.json. Metaloldblankcomparison recordedbutnewMetalnotyetchecked. First20msmainmenuDownsnotacceptedandPlayopened; returnedvia2Escapes,120msDownconfirmedEditorbeforeEnter. No original song edits; noactiveCargo/package or commit/push/PR. Remainingfullmatrix/frozengates stillopen.

Current WaveformBoundsQA actualReverse/Variable/FXAAon:2channelsvisibleandclipped,peerexcludedbysameMusicOffsetpolicy; changinglocationtoAllChartsdrawssignalbehindbothviews;2CtrlZrestoresSameMusicpolicyandforwardlayout,cursortime2.000unchanged,songbytesexactappliedbaseline. waveform-reverse-policy-runtime.json/screenshots. BoundedOpenGLRetina check; newMetal/sameoffsetpeer/allspacing remainseparate. No production sourcechanges oractiveCargo/package;610suite remainsmatching,fullgoalstillinprogress.

Frozen snapshot-waveform-bounds-20261031 covers8294inputs/296publicmodules. Core4582 exit0:231PASS; native32363 exit0:610PASS/0FAIL/0ignored,86.61s; verifiedhashreceipts beforepublicrun. Publiccontroller53857 exit0:296modules/1345PASS/0FAIL/0ignored, completegreen status combined-run-20261031T101807.702565Z/status.json. Originalpublicbodiesoneartifact/separateprocesspermodule,notwholeworkspace or296independentCargotargetpackagings. Currentcorrectedbinarycopiedtoclos edMetalQA/signstrictverified/realrendererMetal inlog: waveformvisiblebehindnotes at2.000, waveform-metal-fixed.png. Metal small labels/widgets show separatefitlimitations; nofullUIcertification. ActualdirtyPack discard: DisplayTitleRAMchange, nativeclose→Discard→secondconfirm→Return exits(processabsentCUAinventory), Pack.inibytesexactbaseline, pack-discard-runtime.json. OPENPackpropertiesduplicatehelp observed: ownfooterPackHelp+globalstatusPackHelp overlaps atsmallheight; nextfixbeforefinalrenderclaim. NoactiveCargo/build/package; temporarypublicharnessstillpresent; no commit/push/PR.

Pack help/layout fiximplemented: open/reopen/reload status usesPackPropertiesTitle, ownPackHelpfooteronly; panelheight min(viewport-30,500), rowpitch fits10rows above status, footer wraps. AddedENFRrawproducer regression480/720 foruniquehelp/rowbounds/reopen. Firsttarget37498 exit101 because hand-writtenPack.ini omittedrequiredmetadata andopened2rowreadrefusal,not10roweditform; replacedfixturewithrealData::canonical. Secondtarget87463 running. Prior231/610/1345frozengatespredatePackpresentationfix; currentappstale. No commit/push/PR.

Packcanonicaltarget87463 exit101 reachedreal10rowform buttestlastlabel lookup expected1prefixspacewhereproducerhas2. Correctedlookuptrim_startonly; thirdtarget36995 exit0:1PASS/0FAIL/0ignored/2130filtered,0.05s. ENFR480/720 uniquehelp+lastrow/statusseparation+panelbounds/reopen pass. pack-help-layout-regression.json. Currentbuild95702 running; application/runtime/fullcurrentnative pending. No source edits during build orotherCargo.

PackHelp build95702exit0,105s; package47080exit0,portablefromclosedWaveformBoundsQAinclAutogenConfigs/signstrictverified. ActualFR854x480RetinaOpenGL panelall10rows+shortstatus+singlewrappedhelpcontained;9DownselectsfinalDelete rowwithoutoverlap,Escapecloses,Pack.inibytesunchanged. pack-help-runtime.json/screenshots. Fullcurrentedition64086exit0:611PASS/0FAIL/0ignored/1520filtered,88.39s; edition-suite-pack-help.json. Priorfrozen231core/1345publicunchangedcorebutfrozenwholemanifestpredatesPackpresentationchange; donotrelabelfinalmatchinggate. NoactiveCargo/package orcommit/push/PR; fullgoalremainsopen.

Overviewcaption followup: actualMetalimage showedMinimapOverviewtruncated; overview_label nowsetsuniformfit_width(width-2)/zoom andfit_height24, retainingclipandnohorizontalmax_width. NewretainedMiso/composer geometrytest ENFR/DPI1/2,width90/120/180 checksallglyphvertices retained/uniformratio/fullunclippedmeshfitscolumn/lineheight. FirstCargo66822 gotstuckafterdisk145MiB; removed6closeddisposableQAapps(ClipCentered,ClipFixed,LogFixed,HoldBodies,ExplicitSelection,WaveformMetal), retainimages/receipts/latestPackHelp/WaveformBounds. Atleast7.7GiBfree afterAPFSreclaim. OwnedCargo63379/rustc63403 SIGTERM explicitlyafterresourcefailure, session66822exit143; diagnostic sample20147exit143. RecoveryCargo54943liveatlastpoll withrustc65908zeroCPUwait; noresultclaimed. libpastecodesignverify61607eventuallyexit0valid; codesigndisplay73554stillpendingatlastpoll. Latest611app/suitepredatescaptionproductionchange. Checkexacthandlesbeforerestart; nosecondCargo. No commit/push/PR orfullcompletionclaim.

Caption recovery54943 compiledsuccessfully but regressionexit101: EN/DPI1/90width actualglyphquadextent90.13335 exceeds90. Logicaladvances omitMisoquadpadding. Added4px DPI-scaledsymmetricinset beforeuniformfit andstrengthenedregressiontoassertabsolutehorizontalcontainment,notjustextent. Newtarget44550 pending; no pass/build/runtime claim. README latestcheckpoint corrected611 andboundedwaveform/Packevidence.

Caption final targeted45074 exit0:1PASS/0FAIL/0ignored. Symmetric4px DPI inset contains glyph overhang; fit_height15 matches retained Miso Baseline19-Top4 and prevents unintended enlargement. Full ENFR/DPI1/2/threewidth actualcomposer regression retains allglyphs/uniformscaling/absolutehorizontalcontainment/verticalextent. overview-caption-geometry.json records source/log hashes. Fullnative40189running; app/runtimepending.

Full native caption suite40189 exit0:612PASS/0FAIL/0ignored/1520filtered,106.05sec; receipt edition-suite-overview-caption.json. Build46767 now running serially. Current app predates captionfix; final frozen gates/fullruntime remain open. No commit/push/PR.

Caption build46767exit0,331sec; package2028exit0. Portable settings/AutogenConfigs from closedPackHelpQA, resigned+strictverified24355exit0. ActualOpenGL andMetal rendererlogverified, Frenchsmallwindowcaptioncomplete/overviewbodiesaligned/waveformvisible; overview-caption-runtime.json/screenshots. Newruntimefinding: UpdateNoticeURL shiftedoffleft onMetal854x480/highDPI. Addedactualcomposer smallviewport/all3selectedactions ENFR regression20745 pre-fix pending. Genericminimum854x480 vs virtual427x240 is suspected; nofixclaimed. No commit/push/PR.

UpdateNotice regression discovery:20745/33907passed but oldverticeshelper ignored drawcamera SharedTransform; insufficientonscreenproof. Correctedhelper multipliescamera then inversebaseprojection, includinghomogeneousdivide.53668exit101 exactruntimeleftoffscreen reproduces. UpdateNotice nowownsrealvirtualviewport (no854mincanvas), backgroundheightclampedviewport; allupdate_url_geometry_tests91128running. ExistingcaptionQAappstaleonlyforthisproductionfix. No fullpass/runtime/goalcompletionclaim.

UpdateNotice targeted91128exit0:4PASS/0FAIL/0ignored/2129filtered,0.28sec. Camera-aware smallwindow/all3selectedactions ENFR nowgreen afterred53668; actualfontgeometry/longURL/originaltarget testsalso pass. Fullnative85933running; newapp/runtimepending.

Fullnative85933exit0:613PASS/0FAIL/0ignored/1520filtered,104.19sec. Build91689running. CaptionQA closed(CUAinventoryempty);portable settingsbackup evidence/caption-portable-settings forsameapp replacementtoavoiddiskduplication. Finalfrozen/sourceparity/hardware gatesstillopen; no commit/push/PR.

UpdateDPI build91689exit0,64s; package34861exit0,portable settingsrestored/resigned/strictverified. ActualMetal854x480/fr fullURLvisibleatall3keyboardselections, Escape preserves1.000; update-dpi-runtime.json/screenshots. DarksourceUI/Preferences andEditor2732-2790read: show,size,order,RGBA/defaults mapped. ActualCharts/AfterWaveform/red0.2/alpha0.8 bounds+attenuationobserved; onlyDarkJSONchanges; Undoexact/RedoVisual/finalUndoexact/songunchanged, dark-runtime.json. InitialdensityloadingafterApply remainsunexplained; laterUndo/Redo restoredreport, noassertionofcause. FixedmisleadingDarkScrolllabels withnewENFRDarkSettings Defaults/Save/Saved/Discarded; sourceconsumer assertions strengthened,target60721pending. Appnowstaleonlyforthislabelchange. No goalcompletion/commit/push/PR.

Darklabel60721exit0:1PASS/0FAIL/0ignored/2132filtered,0.84sec. ExistingrealENFRsourceconsumer nowassertslocalizedDarkactionsandSave/Cancelstatuses alongsideunknownenum/default/globalUndoRedo. Fullnative39600running; noactivebuild.

Fullnative39600exit0:613PASS/0FAIL/0ignored/1520filtered,101.25sec, matchingdarklabels. Actualmarkerinteriorruntimecompleted: newtwochartfixture uniqueB; cursor2.500, markerstartbeat1.5/row72. ApplyonlyIgnorePrecedingDistribution, UndoSaveexactbaseline/RedoSaveexactapplied; Deleteonly72 retains504/allnotes/otherchartbytes; UndoDeleteSaveexactapplied; finalUndoSaveexactbaseline. Cursor2.500throughout. DeleteUndoautomaticallyreopensinspector, EscapebeforeglobalSave required. marker-interior-runtime.json/screenshots/SSC snapshots. Boundedidlecase, no universaltransportclaim. No activeCargo before nextbuild; no commit/push/PR.

Build4037 verifiedwait: theme89341 eventuallyexited, shell96413liveatlastcheck withasync_traitmacrofd7. document_featuresverify29845exit0valid. Diagnostic28183 sampledoldtheme thenstalledsymbolprocessing; explicitlystoppedownsampler96381aftertargetcompilerexit, leavingbuildintact. Preparedexternalvalidate_dark_labels.py(astparsed,notexecuted): frozenhashguards core/native then296publicmodules, refusesactiveCargo. UIHotbar.cs442linesreadandnativehotbarread; position/zoom/snap mapped,othercontrolmappingreview stillbounded, no paritycertificationorfeatureclosureclaimed. No commit/push/PR.

Resume2November: initial8295frozeninputs unchanged, noCargo/rustc; removedclosedPackHelp/WaveformBoundsQAapps afterprocesschecks (settingsbackedup), free1.8→3.4GiB. Build76712blockedloadingmacro simd_helpers. ActualcurrentCaptionQA reproducedpersistentDensityLoadingafterDarkShow-onlyApply, includingidle+F1/Escape; Undorestoresreportandprefbytesexact,cursor2.500. Root: preference-onlyhistory invalidatesReportbinding but nojob is scheduled; pollreturnsimmediatelywhenjobNone. Stoppedownedobsoletebuild76712exit143 explicitlytointegrateactualdefect. AddedrealDarkApply/controller→frame/UndoRedo regression1195(redpending). Priorfrozenmanifest is nowstale; controllernotexecuted. No PR/push/commit.

Densitylifecycle red1195exit101:1failed preciselyno replacementworker/currentreport afterrealDarkApply andnexthostframe. No mocks/extraUIrefresh; sourceexcerpt/jobbindingmatchesactualpersistentLoading. Addedno-worker refresh guardtopoll_source_density, retainingcurrentreport/failedcaptureguards. Newgroup74606running; no green/runtimefixclaimyet. Priorfrozenmanifest/controllerstaleuntilregenerated.

Densitylifecycle green74606exit0:5PASS/0FAIL/0ignored/2129filtered,0.26sec. RealDarkApply→hostframe/UndoRedo ENFR fixesred1195; failedsnapshotretryguards also pass. Fullnative11116running. Preparednewfrozen snapshot-density-lifecycle-20261102 andcontroller validate_density_lifecycle.py (notexecuted); olddark-labels snapshot/controller superseded. Runtimefix/apprebuildnotyetclaimed.

Fullnative11116exit0:614PASS/0FAIL/0ignored/1520filtered,90.96sec, includesdensitylifecyclefixandDarklabels. Build48885running --bin deadsync, serialonlyCargo. CaptionQAclosed; currentportable-settingsbackupunder20261102evidence. Frozen snapshot8295inputs/296publicmodules preparedforfreshcore/native/public, notexecuted. No runtimefix/goalcompletionclaimyet.

Build48885exit0,41.73sec; package84720exit0,portable settingsrestored/resigned/strictverified. ActualcurrentMetal854x480/fr DarkApply densityautomaticallyvisible withoutadditionalinput; idlestable; UndoRedo retainreport/cursor2.500; finalprefsbytesexact/songbaselineexact. NewDarklabelsverified. density-lifecycle-runtime-fixed.json/screenshots. Frozencontroller35722active: core231/native614passes withbefore/after frozenhashguards; publicobserved276/296modules1286tests/0failures. Awaitterminalbeforeclaimingfullcorpus. No PR/commit/push/goalcompletion.

Frozencontroller35722exit0 completegreen. Independentreceiptverification checks core231/native614/public296modules1345tests allzero failures/ignored, complete names/loghashes/partitions/binaryidentity/sourcehashes; frozen-validation-verified.json. All8295 frozeninputshashesstillmatch afterpackaging. Removedverifiedgeneratedpublicharness aftergates, retainedexternal sidecar; deliberatecleanuprecorded (no compilerinput product change). Createddist/DeadSync-Editor-Review.app separateid, portable prefs+AutogenConfigs andisolatedcopied2chart/musicrelativepack; signstrictverifiedbeforelaunch. Actualmenudetects1song/1group; Editoropens2chartsat2.500,waveformready/densityvisible. review-app.json/screenshot. No activeCargo/controller, no commit/push/PR, fullsourceparitycompletionaudit/remainingruntime/usertrialsstillopen.

Note selection source parity follow-up: inspected ActionChangeNoteType and Editor.OnNoteTransformationEnd. Native reviewed conversion cleared selection on Apply and attached no selection history. Strengthened dispatcher test67067 exit101 reproduces None instead of converted head. Plan now records matched originals/results in existing chart/session-bound SelectionHistory and native Apply consumes it. Added two core regression cases for mixed checked source enumeration/history/session ownership and empty no-op avoiding unrelated undo entry. First core96062 exit101 (1pass/1fail) exposed sorted membership vs original selection order; bridge corrected to selected_events. Core retry96938 running; current apps/frozen gates predate correction. Spatial/owner transform selection contract remains next. No commit/push/PR.

Selection conversion native74511 exit0:1PASS. Spatial source ActionTransformSelectionLanes explicitly sorts originals/results, while owner actions retain enumeration. Added two spatial regressions10324 red0PASS/2FAIL (missing receipt); implemented source/result receipt and native Apply consumption. Added owner-order case. First43631 4PASS/1FAIL exposed incorrect test expectation that horizontal mirror swaps Down/Up, plus Position ordering differed from chronological source event order. Corrected mirror fixture expectation; source ordering now row/phase/lane/player, checked owner order retained, converted All source order also corrected. Core30149 exit0:5PASS/0FAIL/0ignored. Native full44564 running. Legacy direct transform/swap paths still need the same contract; current apps/frozen gates predate these changes.

Native44564 exit0:614PASS/0FAIL/0ignored/1520filtered,76.46sec, includes strengthened Mirror/Convert selection Apply/Undo/Redo. Fullcore73236 exit0:236PASS/0FAIL/0ignored/0filtered,1.78sec. Receipts native-note-selection.json and core-note-selection-full.json; changed source hashes captured, no source mutation during suites. Original1345public corpus/frozen manifest and applications predate changes. No activeCargo; next direct legacy transform/swap routes still bypass reviewed receipt contract. No commit/push/PR.

Legacy selection regression4975 exit101 reproduces missing receipt in direct transform route. Legacy transform::apply_positions and note_swap now delegate to existing note/selected-transform Plans with legacy labels and usize::MAX scan policy (no new quota). Native direct dialog/swap consumers sync receipt; new ENFR Apply/UndoRedo case added. Target97102 exit0:6PASS. Frozen snapshot-note-selection-20261102 retains8295inputs/296unchangedpublicmodules/2323supportfiles. Controller66760 active: core237/native615PASS, allzero failures/ignored, native76.51sec; public combined-run-20261102T095207.200178Z observed82modules393PASS/0FAIL, notterminal. No source changes during gates. New app/runtime and broader package/source parity checks pending; no commit/push/PR.

Frozen note-selection controller66760 exit0 completegreen: core237/native615/public296modules1345tests, zero failures/ignored. Independent proof.evidence/public_gate verifies complete names/counts, log and source hashes, partitions and exact executable identity; note-selection-frozen-verified.json. New apps not built yet. Started broader unfiltered whole-theme library72645, one Cargo only. Temporary public harness deliberately remains installed for exact gate/build verification. No completion/PR claim.

Whole-theme72645 exit101:2066PASS/15FAIL/54ignored,87.89sec. All14 failure names from27Oct checkpoint persist; one additional download pack allocation-budget test fails in whole suite but isolated retry exits0:1PASS. Relevant browser/download/test-input source paths unchanged vs HEAD. No clean HEAD execution or full regression attribution claimed; broader suite is not green. theme-full-note-selection.json records names/limits. Gated build/package94129 active (controller74681/Cargo74709/rustc74803 observedlive), output /tmp/deadsync-note-selection-package-20261102. Prepared independent four-tap fixture/settings under /tmp/deadsync-note-selection-runtime-20261102; original songs/settings unchanged. No source edits during gate/build.

Package94129 exit0: gated build/sign/resources verified, separate app da462920db1348709c436b8221a96e60. Isolated actual FR Metal four-tap runtime: SelectAllNotes→Mirror review4selected/2moved/0deleted, Apply/Save, UndoSave exact saved baseline, RedoSave exact applied; selected outlines remain all4. Direct ConvertSelectedTapsToMines from restored selection (no reselect) review4/4/4, ApplySave, UndoSave exact mirror, RedoSave exact mine output. Final2UndoSave exact saved baseline, cursor0.000 throughout. runtime-verified.json/screenshots/SSCs under /tmp/deadsync-note-selection-runtime-20261102. Initial test-settings empty shortcut alternatives rejected; repaired isolated setup, not product. QA app closed verified absent CUA inventory. Frozen8295inputs unchanged after runtime; exact temporary harness removed, sidecar retained. Next source audit found two more OnNoteTransformationEnd families without Session selection receipt: mixed event_shift/shift_all and paste. No production fix or completion claim yet for these.

Mixed shift follow-up: event_shift direct engine lacked receipt; reviewed checked wrapper already had one (found on deeper submodule inspection), global wrapper did not. Added original/result receipt to shared engine and removed duplicate wrapper ownership. Initial immutable Tempo/Signature/widgets excluded, negative-row deleted originals reselected on Undo; zero-change receipt does not attach to unrelated entry. Target83833 exit0:1PASS; fullcore65046 exit0:238PASS/0FAIL/0ignored,1.84sec. Native global ShiftAll now consumes common receipt; ENFR checked/global boundary deletion Apply/UndoRedo regression added. Fullnative46734 running. Apps and previous frozen source gates predate this change. Paste contract still open.

Mixedshift fullnative46734 exit101:615PASS/1FAIL. Diagnostic94871 showed global effects correct/current through Apply/Undo/Redo; checked scenario had only current row0 selected, so deleting its only movable note correctly returned empty. Fixed test to prepare intended0..192 selection, not weaker assertions. Removed all temporary diagnostics. Target63088 exit0:1PASS ENFR global/checked; fullnative retry started. Paste source contract additionally examined: OnCopy clones/sorts, ActionPasteEvents Undo supplies detached originals, so Undo should clear selected paste, not reselect source live objects. Atomic accepted-target selection and shared owner mapping remain to implement.

Mixedshift native95167 exit0:616PASS/0FAIL/0ignored/1520filtered,80.42sec. Paste red83750 exit101 reproduces absent selection receipt. Notes ForceAdd now records accepted target positions (actual owner mapping, valid lanes and surviving heads); mixed atomic paste consumes staged note effect and collects accepted other event identities, skips widgets/unsupported markers, reuses shared Event sort key, and publishes empty-before/result-after receipt. Coretarget70740 exit0:1PASS; fullcore15655 exit0:241PASS/0FAIL/0ignored,1.89sec, including mixed history, narrower target lanes/Routine owner, invalid destination guard. Native mixed consumer sync added; new ENFR mixed and note-only Apply/UndoRedo test8960 running. Current app/frozen gates stale for these changes; no completion/PR claim.

Native paste8960 exit0:1PASS ENFR mixed and note-only targets preserved on Apply/Redo, cleared on Undo, exact bytes/history and row384 preserved. Prepared frozen snapshot-paste-selection-20261102 with8295inputs/296unchangedpublicmodules/2323supportfiles; serial core/native/public controller launched. No new app/runtime claim; previous broader theme limits remain.

Frozen paste controller31125 exit0 completegreen: core241/native617/public296modules1345tests allzero failures/ignored. Independent proof.evidence/public_gate verifies current8295inputs, names/logs/partitions/executable; paste-selection-frozen-verified.json. Prepared mixed four-note/tempo/label fixture and valid test shortcuts /tmp/deadsync-paste-selection-runtime-20261102. New gated package controller launched /tmp/deadsync-paste-selection-package-20261102. Temporary public harness retained for build proof; no runtime/source-parity completion claim.

Paste package7242 exit0 ready_for_runtime_review app0508e61b56fe4923970b203f9e22e640, sign/resource gates verified. Prepared portable settings valid first attempt. Actual FR Metal mixed fixture: selected8, ShiftEarlier24rows review5moved/1deleted/2fixed; ApplySave, UndoSave exact saved baseline (all6 transformed originals selected), RedoSave exact applied (5survivors selected), finalUndoSave baseline. Copy all8 through mixed picker; Next×2 reachesbeat8; CtrlShiftV PasteSave selects4notes+4timing/label events. UndoSave exact baseline and checked-shift rejection proves selectionempty; RedoSave exact applied and subsequent ShiftLater review8moved proves all8targets selected without reselect. Cancel review/finalUndoSave exact baseline. Rejected shortcut briefly blanks waveform, returns on next idle frame; no persistent defect established. runtime-verified.json/screenshots/SSCs/log retained in /tmp/deadsync-paste-selection-runtime-20261102. QA closed confirmed absent CUA. All8295frozeninputs unchanged, exact harness removed/sidecar retained. No activeCargo/build/QA; no commit/push/PR, full functionality audit still open.
