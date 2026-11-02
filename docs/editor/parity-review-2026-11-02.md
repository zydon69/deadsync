# Source parity review — 2 November 2026

This is an incremental review, not a certificate of full GrooveAuthor parity.
The historical 21-flow matrix predates the current implementation. Its empty
test mappings must not be interpreted as missing features without inspection.

## Reference inputs

The 259 inventoried GrooveAuthor files and 83 Fumen files match their pinned
hashes. The older temporary StepManiaLibrary checkout has missing files; it is
not a reliable reference anymore. A fresh archive of commit
`bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8` was fetched with certificate
verification enabled. All 90 inventoried files match. No product source changed.
Receipts: `reference-inputs-resume.json` and
`stepmania-reference-restored.json` in the current runtime evidence directory.

## Pattern command mapping inspected

Source `Editor.cs` 7413 onward exposes four regeneration callbacks:
Selected/All with fixed/new seeds. New seeds form a separate history action
before asynchronous generation. Native `command_bindings.rs` dispatches these
four callbacks to `pattern_actions.rs`; the review keeps the checked scope and
seed mode. Preparing the review does not start the worker or commit seeds.

Existing executed tests cover exact selected marker identities, exclusion of
intermediate unchecked markers, two separate undo actions for seeds and notes,
cancelled generation retaining committed seeds, and clear preserving unchecked
notes and marker metadata. Native tests also exercise real worker publication,
Apply/Discard/Escape, and preservation of fractional or negative cursor positions.
The four-callback dispatcher test asserts Selected count 1 versus All count 3,
the requested seed mode, unchanged serialized document, and no worker before Apply.

Evidence bodies inspected:

- `crates/deadsync-editor/tests/pattern_actions_workflows.rs`
- `crates/deadsync-theme-simply-love/src/screens/edition/pattern_actions.rs`
- `crates/deadsync-theme-simply-love/src/screens/edition/command_bindings.rs`

These tests belong to the verified 2 November public/native runs. They were not
rerun during this static review. This mapping does not certify every generation
configuration, every generated sequence, or physical keyboard operation.

## Selected timing conversion mapping inspected

Source `UIEditEvents.cs` 821 onward requires a nonempty focused selection for
Warp → negative Stop and negative Stop → Warp. Native
`timing_conversion.rs::command_selected_timing_conversion` preserves that
selection requirement; it does not fall back to all events when selection is absent.

The public `selected_timing_callback_workflows.rs` tests preserve mixed selection
enumeration, filter positive Stops, require explicit collision review, restore
the original selection on Undo/Redo, and reject unrelated session receipts.
Native callback tests additionally cover global selection dispatch and focus
changes. This is evidence for these selected conversion callbacks, not all eleven
timing event editors or every timing migration path.

## Physical input limitation

The UI controller rejected holding `1` together with `Down` before dispatching
input: it accepts only one ordinary key per chord. The isolated chart remained
byte-identical. Separate keypress calls cannot prove held-note interaction.
Physical keyboard validation remains open; see
`/tmp/deadsync-held-input-runtime-20261102/runtime-limitation.json`.

## Additional runtime checks

The real French application Help → GrooveAuthor documentation summary action
opened the pinned GitHub TableOfContents page in the system browser. The rendered
page contains the expected general usage, timing, multiplayer and autogen sections.
This verifies one actual native host/OS/browser route; the remaining link mappings
have automated coverage, but their individual browser launches were not repeated.
Receipt and screenshot: `help-document-runtime.json`, `help-document-open.png`.

The real F10 timing inspector accepted `2x/48rows` at beat 2, staged it, applied it
and saved `#SPEEDS:2=2=1=0;`. Undo/Save removed the speed and the detached chart BPM;
Redo/Save reproduced the exact applied bytes. Note rows remained unchanged and
the main cursor stayed at time 0. The source text parser's unsigned target/duration
restriction was checked against `EditorInterpolatedRateAlteringEvent.cs` and
matches the native input policy; negative imported legacy values are a separate
preservation contract. This runtime check covers one idle chart, not all timing
types or transport states. The first save adds canonical song metadata, so the
Undo output is not claimed byte-identical to the pre-save file. Receipt:
`speed-runtime.json`, with applied/undone SSC snapshots and screenshot.

The temporary test application was closed after the final Undo/Save. The separate
review application was not changed by these checks.

## Remaining review work

Continue the remaining source requirements individually, including timing event
editors and migrations, advanced generation configuration consumers, and external
Help/OS integration. Preserve the distinction between source inspection, executed
automated assertions, runtime observations, and user validation. No editor commit,
push or pull request has been made.

## Chart event copy runtime follow-up

Inspected the pinned source dialog and native core/panel: all 13 event types
and four presets correspond. Existing verified public/native runs cover all
13 payload types and atomic multi-destination rollback.

The actual French application copied StepMania event types from B-source to
A-focus and C-target while focus remained A and the cursor remained 0. Saved
SSC parsing verified BPM 180, replacement labels, deletion of destination Stops
absent from the source, and unchanged notes, meter, description and chart type.
One Undo/Save exactly reproduced the canonical saved baseline; one Redo/Save
exactly reproduced the applied file. A final Undo/Save restored the fixture and
the temporary application was closed. This covers one idle SSC fixture, not
every payload or transport state. Receipt and snapshots:
`/tmp/deadsync-chart-copy-runtime-20261102/runtime-verified.json`.
No production source changed during this verification.

## Timing property input audit

Compared pinned EditorTempoEvent, EditorStopEvent, EditorDelayEvent,
EditorWarpEvent, EditorFakeSegmentEvent, EditorScrollRateEvent,
EditorTickCountEvent, EditorMultipliersEvent, EditorTimeSignatureEvent and
EditorLabelEvent setters against typed core drafts and native timing/draft.rs.
Minimum BPM is 0.000001; Stop is nonzero with negative values supported; Delay
is nonnegative; regions require at least one integer row; Scroll supports zero
and reverse values; newly entered ticks and combo multipliers are nonnegative
signed-32-bit integers; time signature denominators are powers of two at most
48 (largest valid 32). Imported signed values have separate preservation paths.
Native region text is in beats and converts to integer rows.

The label sanitizer matches source replacement characters. One deliberate
format guard differs: native entry rejects '=' because the source CSV reader
requires exactly two fields and otherwise drops the label on reopen. Existing
label tests explicitly cover this refusal and retention of prior data.

Rechecked all 8295 frozen input hashes against the latest verified run. The ten
property/migration modules have successful executed logs, with parser/setter
reference fixtures, save/reopen, scope, Undo/Redo and stale-draft guards. This
turn did not rerun tests. Receipt: `timing-property-audit.json` in the runtime
evidence directory. This closes the listed static setter comparison; it does
not certify every inspector through the rendered app or every transport state.

## Advanced generation configuration audit

Compared every [JsonInclude] property in the seven pinned StepManiaLibrary
configuration classes to the native serialized field declarations. Exact set
equality holds: Expressed 6, Pattern 16, Performed 5 groups, Facing 4, Lateral 4,
Step Tightening 12, Transitions 5. ArrowWeights is the dynamic map in Performed,
not a fixed number of leaf controls. Receipt: `generation-config-audit.json`.

Inspected profile_controls recursive leaf construction and source gate rules.
The six Expressed controls follow the original default-versus-dynamic bracket
branches; nullable movement flags disable dependent inputs until explicitly
true; specified foot/lane controls are hidden until selected. Native raw-key
tests inspect disabled edits and hidden-row navigation in profile_edit/source_gates.rs.
These are automated panel observations, not rendered application observations.

Inspected actual generation consumers in search.rs: facing penalties, transition
side calculation, speed/distance/stretch tightening, lateral NPS costs, and
normalized arrow weights. The original_search_single_leaf_mutations test
compares generated lanes, rows, timestamps, foot identities and performed costs
against pinned source fixtures for each listed mutation. Profile control tests
check every serialized preset leaf and round trips. Five linked public modules
have 53 successful tests in the latest frozen run. Tests were not rerun during
this inspection; no product source changed. Schema equality is evidence for
field coverage only, not a proof of arbitrary generated-chart equivalence.

## Profile panel runtime follow-up

The real French application reached generation profiles via F1 and the last
menu item. In the Pattern copy inspector, Automatic hides the specified foot
and Down skips it. Specified reveals Left; changing it to Right, hiding and
revealing again retains Right. In Expressed, the original default method enables
the method selector and disables four dynamic settings. Automatic reverses
these gates; Right on the disabled method leaves No Brackets unchanged.
Escape discarded both private drafts and returned to the original preset
selections. The SSC fixture remained byte-identical to the canonical baseline;
the temporary app was closed. Screenshots and receipt:
`/tmp/deadsync-profile-panel-runtime-20261102/runtime-verified.json`.
This verifies two rendered draft panels, not Apply/Export, every field, the
Performed panel, or arbitrary generation results.

## Performed profile runtime follow-up

In the actual French application, enabling lateral tightening activated the
dependent fields. Keyboard numeric entry set RelativeNPS to 42. Naming and
saving a copy created and selected QA movement 20261102. Reopening it displayed
the enabled flag and 42. Parsed saved JSON confirmed both effective values.
F7 Undo removed the live profile; all six original profile files remained exact.
Redo from the main chart restored exact saved profile bytes; final Undo removed
the test profile. Recoverable deletion and save-lock artifacts remain internal.
The song fixture stayed exact, and the temporary app was closed. Receipt:
`/tmp/deadsync-profile-panel-runtime-20261102/movement-runtime-verified.json`.

Open observation: Undo while inspecting the newly created profile leaves its
identity selected in the picker, which then reports Missing profile / choose a
replacement. Compare the source clone/selection policy before classifying this
as intended behavior or a correction. Speed popup and remaining movement fields
were not visually inspected in this turn.

## Profile creation Undo selection policy resolved

Pinned ActionClonePerformedChartConfig deletes the clone on Undo. Its manager
clears ActivePerformedChartConfigForWindow and hides the inspector. Native
profile_saved_history likewise closes profile_edit after deleting the live
profile and reloads the modal manager. The missing item observed there is a
pending picker choice, not a committed generation selection. Application manager
Continue/Back discard the picker; conversion staging rejects missing choices.
restore_profile_preferences independently removes a deleted creation from
committed generation overrides when it is actually active.

The source uses independent windows and native uses one modal panel. The
remaining missing-choice message is classified as a documented presentation
adaptation, not evidence of an active deleted profile or lost song data. No
automatic replacement is introduced: that could silently select a different
configuration. The runtime receipt now records this source-policy comparison.

## Performed speed and weights runtime follow-up

The actual French application enabled speed tightening and opened F4. Minimum
200 clamped to current maximum 170; changing maximum to 300 and preparing the
range produced 0.1 and 30/170 seconds per foot at subdivision 1/16. F6 removed
the dance-single weight override to inherit, rebuilding descriptors without
losing the speed draft. Save Copy and saved effective JSON confirmed enabled
speed tightening, 170/300 BPM, the computed seconds, and no dance-single map.
Reopening F4 and F6 displayed the saved range and inheritance. F7 removed the
test copy; all six original profile files remained byte-identical. The isolated
QA application was closed; the separate current review app remains available.
Receipt and screenshots: `/tmp/deadsync-profile-panel-runtime-20261102/`
`speed-weights-runtime-verified.json`. This covers one subdivision/layout.

## Eleven timing event inspectors runtime follow-up

The actual French application exercised F10 and each of its eleven event types
at beat 4: BPM180, Stop1s, Delay1s, Warp1beat, Fake1beat, text label,
Speed2x/48rows, Combo2x/3x, Tick8, Scroll-1 and Signature3/4. Each was staged,
applied, saved, then undone and saved. Parsing all eleven outputs confirmed the
expected exact tag entry, unchanged notes and chart metadata, unchanged peer
charts, and byte-identical Undo restoration to the canonical baseline. The
fixture is restored and the isolated QA app closed. This is one value/position
per event type in an idle SSC chart, not exhaustive transport/edit coverage.
Snapshots, staged screenshots and per-case hashes:
`/tmp/deadsync-timing-inspectors-runtime-20261102/runtime-verified.json`.

## Historical runtime reconciliation

Read the saved generation, cross-directory SaveAs, mixed-coop batch and
chartless three-family profile receipts, then attached existing receipt paths
and hashes to the 21-flow current index. Simple generation's receipt includes
actual reload (the older pending claim is superseded), source BPM60/offset0.75
and 128 generated cells. The no-active-view generation records Apply/Undo/Redo
and Reload with 129 generated cells and four existing charts exact. Mixed
pump-double batch admits one of two raw double charts, preserving the co-op
chart; generated32 notes, unchanged existing charts, exact Redo. Its first
Undo is intentionally not claimed raw-byte exact because canonical metadata
was materialized on first Save. Cross-directory SaveAs retains all content
except escaped MUSIC; decoded relative path resolves to original audio.

These are historical binary observations, not fresh runtime proof for all
current dependencies. The index now makes that age explicit rather than
repeating old pending statements as missing features. The next independent
review is interrupted-work recovery, distinct from ordinary Save/SaveAs.

## Interrupted-work recovery runtime follow-up

On a new isolated three-chart SSC, actual keyboard 2 added a Down note at row0;
CtrlShiftS created the explicit recovery sidecar while source bytes remained
exact baseline. SIGKILL targeted the unique exact QA executable path, leaving
review applications alone. Same-path relaunch loaded the original unchanged
chart. CtrlR recovered the added note into memory without changing source;
CtrlS then saved only first-row1000→1100. Peer chart payloads and metadata were
unchanged. The QA app was closed cleanly and the disposable source restored.
Receipt: `/tmp/deadsync-recovery-runtime-20261102/runtime-verified.json`,
with sidecar snapshot, interruption record, screenshots and recovered output.
This verifies explicit recovery after process interruption; it does not verify
a timed autosave interval, power-loss durability or an external-change scenario.

## Automatic recovery publication runtime follow-up

The actual application protected a sidecar left by the prior process: no write
during the 40-second probe, with the localized recover-or-replace notice. This
is the explicit existing-draft guard tested in auto_recovery_workflows.rs, not
a timer failure. After explicit CtrlShiftS replacement claimed the draft, a
second note was added with no further save shortcut. The periodic owner
published a changed sidecar. Independent V2 header/length parsing confirmed
row0=1010 and row24=0001, unchanged peer charts and unchanged source bytes.
Two Undos returned a clean song; the isolated QA app was closed. The elapsed
probe does not measure exactly 30 seconds from edit: the scheduler interval is
relative to its preceding attempt. Receipt: `auto-runtime-verified.json` under
`/tmp/deadsync-recovery-runtime-20261102/`.

## Diagnostics source/host audit

Read pinned UILog controls and message loop: level, four date styles, wrapping,
Clear, Copy and OS Open. The older matrix wording additionally mentioned search
and pause for logs; neither is a source UILog control, so the current requirement
now reflects the actual source. Native log controls correspond, with keyboard
message/long-line paging replacing the mouse scrollbar. Log records are captured
from the real accepted env_logger format_record path, not a fixture-only buffer.

Read pinned UIPerformance and native consumers. Pause, Independent/Shared/Explicit
scale, explicit plot maximum, frame count and defaults reach typed preferences.
The shell calls begin_performance_frame during redraw and records actual Update,
Draw, Present, PresentWait and EditorCpu durations; chart/waveform/minimap
consumers record their actual stages. The two public original-reference modules
have successful current frozen logs, and 12 native Log/Performance tests passed.
Consumer hashes still match the latest gate. Receipt: diagnostics-source-audit.json.
No tests rerun this turn; OS clipboard/open and arbitrary-load plots are not
claimed as observed runtime by this static audit.

## Diagnostics runtime follow-up

Actual French Log displayed 27 accepted records. Explicit Copy produced the
localized clipboard-success status, which is published only after the native
write returns success. The clipboard payload was not independently read back: a
local property paste attempt showed no text and is not used as payload proof.
Open launched Console with the exact QA Resources/deadsync.log document URL.
That document was closed. An existing TextEdit document was only read, never
modified; its unrelated content is not task instructions or evidence.

Actual Performance displayed frame intervals and a live graph. Apply Pause
persisted true; two later captures had identical displayed metrics and identical
yellow graph/metric pixel sets. Global Undo restored paused=false. Song bytes
remained exact; QA closed. Receipt: diagnostics runtime directory
`/tmp/deadsync-diagnostics-runtime-20261102/runtime-verified.json`.
A CtrlO attempt had no visible effect; do not infer a product defect until its
current binding/context is checked.

## Shortcut observation resolved; clipboard follow-up scoped

The exact QA editor-preferences.json contains KeyBinds.Open=[[]], the explicit
disabled source binding. CtrlO having no visible effect is therefore expected;
it is not evidence that the File/Open workflow fails. Inspected physical
modifier aggregation (Control and Super), modal command consumption, typed text
entry, clipboard serial/target validation and active-buffer rendering. None
establishes a defect explaining the empty paste. The next check must use simple
single-line text and a real configured modifier, avoiding a multiline log in a
single-line property. No speculative product edit was made. Receipt:
`/tmp/deadsync-diagnostics-runtime-20261102/key-context-review.json`.
User replied that the physical held-lane test has not yet been performed; it
remains pending, with no inferred validation.

## Simple text clipboard runtime follow-up

In the actual Chart Name draft, CUA paste inserted QA clipboard 20261102.
CtrlA/CtrlC followed by Backspace emptied the field; CtrlV restored the exact
line. This directly verifies a real host clipboard round trip for the control
modifier. No property Apply or Save was performed, and disk stayed exact.
Source Buffer::insert filters control characters but does not establish a
length limit explaining the older empty full-log paste. Do not attribute that
observation to an invented limit.

The subsequent automation super+a then super+x unexpectedly returned to the
main menu; this attempt is inconclusive for canonical macOS Cmd behavior and
requires focused input-route reproduction. The QA app is closed. Receipt:
`/tmp/deadsync-clipboard-runtime-20261102/runtime-verified.json`.

## Raw modifier regression in progress

Added one native EN/FR test covering ControlLeft/Right and SuperLeft/Right
select/copy/cut/paste in Chart Name, processing clipboard after full key release
and requiring no close request or committed chart mutation. Only a cfg(test)
module was added: removing it and its blank lines reproduces the previous frozen
file SHA256 exactly. The 8295-input gate is historical for this test-file delta;
new targeted compilation is still active (session96813), with no result claimed.
Receipt: `/tmp/deadsync-clipboard-runtime-20261102/raw-modifier-validation.json`.
The automated super+a exit also reproduced in shortcut capture, so received
physical keys were not established. AZERTY is a hypothesis, not a confirmed cause.

The raw modifier regression compiled successfully and passed (1 test, EN/FR x
four modifier sides, zero failures, 0.10s). The startup delay was before Rust's
harness; macOS signature verification eventually passed normally. Full native
screens::edition:: validation is now running as session57236; source hashes
for all 8295 prior frozen inputs were captured first. Only the new cfg(test)
module differs from the old gate; production code equality was checked above.
No full native result is claimed until that session completes.

Full native session57236 completed exit0: 618 passed, zero failed/ignored,
77.82s. All previous617 test names plus the new regression are present exactly
once; all8295 captured inputs unchanged after execution. Core/public histories
were not rerun; broad theme suite remains unqualified. The OS automation
observation is not overwritten by this raw-input test result.

## Completion audit: file lifecycle contracts

The new completion-audit JSON records eight explicit source file-lifecycle
contracts and fourteen exact current executed test names. Read the actual
begin_file_action and publish_file_candidate barriers, source menu, no-song
close, MRU fractional views/zoom, and containing-folder consumer/platform route.
Cancellation keeps source session/undo; failed loads keep original; Exit waits
for durable coupled save and revokes old authority after new edits.
The audit distinguishes this contract evidence from every OS chooser/input
method combination. Advanced Save remains a separate source requirement.

## Completion audit: media and advanced Save

Direct media routes and All/Unset asset actions are linked to seven exact native
executions and the public auto-assets reference workflows. Read their actual
untitled/existing-song publication, validation and audio/presentation boundaries.

Advanced Save has the six exact source keys, typed preference preservation,
per-loss review, original note readback fixtures and 128 metadata classifications.
However, the 60-writer-case loop skips output comparison whenever native
preflight reports an error and only requires blocked>=10. That is not proof
that each refused export matches a source failure or a reviewed policy.
Completion audit flow03 remains partial: classify each blocked case against
original Saved/Error and native IssueKind before treating exporter parity as
proved. This is an evidence gap, not yet a confirmed missing export feature.

## Export admission coverage

The original 60-case writer comparison previously accepted any preflight error as a skipped case. Added a separate native admission fixture with exact error kind, field and chart expectations for all cases, requiring exactly 28 accepted and 32 refused cases. This fixture is explicitly native policy, not an original writer oracle. Original writer success bypasses source editor pre-save checks; StepF2 layout/ownership safety requires separate review. Targeted execution log: `/tmp/deadsync-export-admission-tests-20261102.log`. The user has not yet performed the physical held-lane/Down test.

Targeted `advanced_export_writer_workflows` execution completed successfully (exit 0); see the log above for the exact test results.

## Attack inspector source review

Compared the original UIAttackEvent actions with native staged controls: position, add modifier (first modifier duration or one active-tempo beat), name, level, speed, duration, cursor duration, jump to end and modifier deletion. Reran five public modules: 21 passed, zero failed. Includes 168 original endpoint/cursor comparisons and eight original lifecycle cases. Exact source hashes, names and log hash are in completion-audit-2026-11-02.json. This validates the inspected engine contracts; it does not certify every physical keyboard interaction.

## Song and chart property source review

Inventoried original UISongProperties and UIChartProperties controls, matched all 19 song text fields and five chart text fields to typed native edit targets, and inspected offset, preview/end-hint formulas and stream-description dispatch. Four targeted public modules passed 32 tests with zero failures. Exact names and log hash are in completion-audit-2026-11-02.json. Two integration crates report the same unused shared test helper warning. Separate player/display-tempo/stream runtime coverage remains outside this four-module claim.

## Pack source review

Matched UIPackProperties controls with native metadata, banner, sync, create/save/delete and song-list routes. Inspected departure assertions covering Cancel/Save/Discard and invalid/external field preservation. Three public modules passed 28 tests, zero failures. Reference scope includes 37 pack codec cases, six creation-default cases and 116 song cases (115 loaded plus explicitly tested uppercase-extension exclusion). Current exact names and log hash are recorded in completion-audit-2026-11-02.json; historical pack rendering evidence remains labelled as historical.

## Chart list, focus and tabs source review

Compared UIChartList and Editor.SetChartFocused/SetChartHasDedicatedTab/CloseChart against the native list, focus and registry consumers. Three public modules passed 28 tests with zero failures, including 8,748 original ordering comparisons over 54 keys and three cultures. The tests cover temporary/dedicated view lifecycle, final close, exact music time across fractional positions/stops/warps, stale transitions and structural identity history. Corrected one obsolete module comment claiming simultaneous rendering was future work; view_render_screen already consumes active_views. This comment-only change does not alter executable behavior. Exact names and hashes are in completion-audit-2026-11-02.json. Actual decoder handoff and rendered multi-field interactions require their runtime evidence separately.

## Note operations and mixed selection review

Verified that all 297 core source hashes and 2,323 fixture/support hashes match the executed public snapshot. Six inspected unchanged public modules retain 27 passing tests; their exact log hashes and names are recorded in completion-audit-2026-11-02.json, flow 08. These are reused executions, not newly run tests. Inspected exact-head review dispatch (F8 conversions, F9 transforms, F7 checked timing, F3 presets) and the source UIEditEvents conversion/selection menus.

Strengthened note_swap_workflows: compare the complete actual note map with the original expected map, rather than only checking expected entries individually; assert request/oracle case names match. This closes an assertion gap for unexpected extra notes. New targeted execution is in /tmp/deadsync-note-swap-exact-review-20261102.log; its result is pending. The old note-swap test hash is no longer current. Marked edit-events-remaining-plan.md as historical to prevent already implemented gaps from being reported as remaining code.

Stronger complete-map note-swap validation completed: 2 tests passed, zero failed. The 40 original actions across SM and SSC retain exactly 60 accepted outcomes and 20 explicitly refused zero-duration sustain outcomes. Native owner-collision readback evidence was also inspected: two source cases lose a note and one loses the chart, in both formats.

## Timing conversions and global mixed shift

Inspected the source all/selected conversion routes and native checked-selection/collision review dispatch. Reused eight executed timing-conversion tests only after matching current core, fixture and test hashes. Strengthened the global shift regression from approximate moved/immutable counters to exact four moved, two protected, zero deleted; exact note positions, BPM/signature/label properties and reopened note grid are checked before exact undo/redo. Targeted run: three passed, zero failed. Exact names and evidence are in completion-audit-2026-11-02.json, flow 09. Source zero-length Stop refusal and SSC negative-Stop export compatibility remain explicit differences/limits.

## Autogen and pattern source review

Compared UIAutogenChart, UIAutogenChartsForChartType and UIPatternEvent controls to native source selection, batch, marker and pattern-action routes. Eighteen unchanged public modules retain 116 passing tests after current source/fixture hash checks; these are reused results, not a new execution. Pattern batch tests compare the complete original worker across 17 scenarios, preserving constructor enumeration behavior. Strengthened the batch generation test to compare all original grids before/after Apply and both generated grids against their accepted previews. Targeted batch run: 10 passed, zero failed. Evidence and limits are in completion-audit-2026-11-02.json, flow 10; profile field coverage remains separate.

## Generation profile management source review

Reviewed UIAutogenConfigs three-family entry point and native profile creation/edit/history controls. Revalidated seven SML configuration source hashes against the field-schema receipt and matched current core/support plus 17 unchanged profile test modules to their executed logs (73 passing tests). Recorded concrete requirements, exact names, log hashes, schema fields and the three scoped French profile runtime receipts in completion-audit-2026-11-02.json, flow 11. No new tests or runtime were executed for this reconciliation. This closes provenance ambiguity for the stated checks, not proof of every possible profile/seed combination.

## Recovery and chart event-copy review; confirmed open difference

Reconciled six auto-recovery and fifteen chart-copy public tests with current core/support/test hashes, plus actual isolated SSC recovery/periodic-publication and StepMania-copy runtime receipts. Existing evidence is scoped in completion-audit-2026-11-02.json flows 12 and 13.

Investigated the skipped original case first-event-destination-position-preserved using the same public fixture constructor and real native Plan::capture/apply. Probe completed successfully and confirms a semantic difference: destination raw BPMs 1=90,2=110 become 0=120,0.3333333333333333=180,4=240; source action expects initial BPM180 at destination row48 (beat1), then240 at row192. Signature has the analogous row-zero/source-row mismatch. The cause is synthetic initial nodes entering clipboard replacement instead of preserving destination first authored positions. This is required unfinished parity work, not a passing export safety exception. Probe receipt: /tmp/deadsync-chart-copy-initial-probe-20261102.json; temporary example removed. Fix must preserve initial raw positions and distinguish fallback/editor projections; do not broadly relax initial timing invariants without regression coverage.

## Initial-node chart-copy correction (withdrawn after loader review)

Implemented a shared typed initial-value replacement path distinct from event clipboard deletion/paste. It retains each destination first authored BPM/signature position, copies the source first value, and copies subsequent source nodes. Signature parsing exposes authored values while ordinary edit/clock parsing keeps its existing synthetic initial behavior. Reinstated all ten source oracle cases; the oracle projection explicitly compares authored data rather than synthetic row-zero nodes. Added separate single-type tests for preserving unselected timing, source values, notes, reopen and exact Undo/Redo. An initial test iteration exposed a legacy-SSC shared-timing regression; empty initial changes now return before local-target capture.

Final-source validation: 241 core + 5 live-copy + 11 copy + 5 signature + 10 tempo tests = 272 passed, zero failed. All 8,297 recorded input hashes matched after the run. Receipt: /tmp/deadsync-chart-copy-initial-validation-20261102.json. Broader native/public execution and app rebuilding remain pending. Colliding source/destination initial nodes need a separate source oracle; current code refuses that unrepresentable duplicate rather than silently resolving it. The authored-file correction is not a claim about every synthetic initial-node UI/clock behavior.

## Initial timing collision follow-up

Verified the actual source insertion chain: EditorChart.AddEventInternal -> EventTree -> Fumen RedBlackTree.Insert. Same-class, same-row unedited EditorEvents reach SMEventComparer equality regardless of BPM/signature value. Added a reproducible .NET probe using the pinned real comparer and tree; all four Tempo/Signature x equal/different cases throw ArgumentException. The earlier whole-action adapter uses a List and does not establish duplicate-container behavior, so it was not used as the collision oracle.

Corrected native replace_first to refuse equal-value collisions too (previously they merged). Added a four-case regression proving refusal preserves exact content, revision and existing redo. The chart-copy module passes all 12 tests, zero failed. Sources, expected/provenance fixtures and reusable probe are retained under scripts/editor-initial-copy-collision-oracle and tests/fixtures/grooveauthor/initial-copy-collision.*. This resolves the collision-policy question at the inspected container boundary; synthetic initial UI/clock behavior and broader post-change validation remain separate.

## Loader-precondition correction to the initial-node investigation

The previous apparent defect was an oracle-precondition error. Source
EditorChart.Load creates a row-zero Tempo when missing; Fumen AddTimeSignatures
inserts 4/4 at row zero. Therefore a chart with only first nodes at rows16/48
cannot reach the ordinary editor copy action without normalization. The
experimental raw-position preservation code changed the valid loaded behavior
and has been withdrawn. All three touched existing production files match their
original validated hashes; the experimental module is removed. The additional
native collision tests tied to that invalid state were replaced; the isolated
Fumen container probe remains a scoped development characterization.

Generated a normalized-input fixture and executed the exact previously pinned
original action assembly, then added a native regression starting from raw
nonzero initial positions. Effective source/destination timing, source and note
preservation, reopened output and exact Undo/Redo match the normalized original.
Final targeted result: 5 live-copy + 11 copy =16 passed, zero failed. Proof and
source-code boundaries are recorded in completion audit flow13. No new GUI-loader
execution is claimed. Production equivalence receipt:
/tmp/deadsync-chart-copy-load-production-equivalence-20261102.json.

## Help, FTUE, diagnostics and host layout reconciliation

Inspected source UILog/UIPerformance/UIAbout/UIFTUE controls and native Help/About/FTUE, log, performance and resolution routes. Current consumer file hashes match the native 618-test execution. Recorded nine native help/tutorial/about tests, twelve native diagnostic tests, the host resolution test and inspected actual shell mode revalidation/application. Three unchanged public modules retain twelve passing tests after hash checks. Historical English/French FTUE and real diagnostic receipts retain their explicit limits (OS clipboard payload not independently read back, no arbitrary performance claim). No new tests/GUI run was needed. Completion audit flows14-16 and the current remaining-work list now replace stale historical claims.

## Completed reference export admission classification

Reviewed all 32 blocked cases against pinned EditorSong/EditorChart pre-save controls and original Before/After readbacks. Classification: 20 statically matched source editor guards, six original writer note/chart-loss witnesses and six original reader chart exclusions; 28 accepted cases retain native/original note-layout-player comparison. Source offset compatibility was checked in DoTimingAndScrollEventsMatch; SSC omitted timing/shared-event checks, SM warp and SSC negative-stop errors, and StepF2 Routine player/roll/lift checks were inspected. Source files match the pinned commit byte-for-byte.

Extended the native admission fixture with explicit per-case bases and independent readback summaries. The test requires all four exact category totals and asserts loss/exclusion directly against original fixtures. Targeted module: five passed, zero failed. Provenance file hashes were appended afterwards without changing executable case expectations. This closes the formerly arbitrary blocked-case skip for this corpus, not universal format equivalence.

## Per-command review started

Added command-coverage-2026-11-02.json with all 164 source callbacks, actual native enum identity and executable-registry membership (217 native variants total). Initial catalogue matching remains distinct from behavior review. Inspected exact enum routing for thirteen quick-add commands and linked their native prepare/Apply/Undo loop plus five public tests including 52 original factory outcomes. Ten direct snap and four explicit-player commands have inspected exact-postcondition tests; native results remain applicable because only the selection test body changed.

Strengthened all_nineteen_source_presets_target_the_exact_kind_and_current_owner_across_the_chart: both Routine owners now have real holds and rolls in a later measure, expected kind sets no longer assume empty sustain selections, and explicit nonzero counts guard all/current owner variants. Production equivalence is verified by reversing only these cfg(test) edits in memory to reproduce the original source hash. Targeted compilation is live in session65703 (Cargo57781, rustc57847); at the last process snapshot rustc was sleeping with no CPU progress. A read-only process sample is live in session13063, processing symbols. Do not restart either solely due to observation delay. No passing result is claimed yet.

## Additional selected-command route review

Inspected and checked 33 more exact source-ID routes: fourteen scalar note conversions, ten swaps, seven spatial transforms and two Warp/negative-Stop directions. Verified typed parameters and the common exact checked-selection preparation rather than equating ID presence with behavior. Linked current matching public original-action workflows and native missing-selection/deferred-Apply/history checks. The two timing routes have their own configured-key and collision/positive-Stop tests. This adds layered route/model evidence, not 33 newly executed physical-key tests. Remaining owner-domain and other command groups stay pending in the per-command index.

The waiting native compilation was diagnosed by a rustc sample: loading a proc-macro dylib blocked in dyld mapSegments -> fcntl. Read-only codesign verification of libdocument_features eventually passed; all independently checked code pages also matched. The same compiler resumed and completed after22m42s without a restart or security change. The test process has now started; no test result is claimed until its live handle completes. Sample /tmp/deadsync-source-presets-rustc-sample-20261102.txt; verification session51486 completed exit0, sample session13063 completed exit0.

## Stronger nineteen-preset native test passed

The same Cargo process eventually completed after OS dylib loading resumed. The strengthened native test passed (one test looping all nineteen source selection actions,0.40s): real Hold/Roll witnesses for both Routine owners, exact kind/current-owner selection and unchanged serialized document. Per-command review now records79 inspected layered/native-exact workflows and85 inventory-only rows still requiring behavior review. This is not a port percentage. Full native editor suite is now live in session33397, log /tmp/deadsync-command-coverage-native-suite-20261102.log, with input hashes captured before dispatch.

Full native editor rerun completed:618 passed,0 failed,0 ignored,76.26s. All 8302 captured inputs matched afterwards; all previous native test names retained. Receipt /tmp/deadsync-command-coverage-native-validation-20261102.json. No complete theme/workspace/source-parity claim.

## Navigation/pattern/media command batch

Inspected native event-index navigation, measure/label/edge wrappers, exact pattern-inspector navigation, all four regeneration scope/seed modes, song preview/end-hint actions and All/Unset media routes. Linked their real native assertions and relevant public source-reference workflows. Added one cfg(test)-only module for direct measure/label/edge callback behavior:10 cases in EN and FR, passed,0 failed,0.12s. Literal expectations include49.25->193.25 and193.25->25.25 across signature changes, negative positions, strict label wrap and chart endpoints. Serialized bytes, revision and history remain unchanged. Existing native source matches the full618 gate after removing only the new conditional declaration. Per-command review is103 reviewed/61 pending; no full619 execution or universal OS-input claim.

## KeepChartOpen callback correction

Source `SetFocusedChartHasDedicatedTab` always calls `SetChartHasDedicatedTab(..., true)`. Native `ViewAction::Pin` incorrectly toggled the dedicated flag. It now sets it to true, so repeated KeepChartOpen commands cannot silently unpin and later replace the chart. New EN/FR semantic callback regression checks three repeated pins, preservation while another chart opens, focus traversal and exact unchanged song bytes/history. Targeted test passed (1/0), log `/tmp/deadsync-source-keep-chart-open-20261102.log`. Full native editor regression is running separately; the existing review app does not yet contain this correction. Command review: 106/164; 58 remain.

Full native editor regression after KeepChartOpen correction: **621 passed, 0 failed, 0 ignored** (78.89 s). All previous 618 test names retained. Receipt: `/tmp/deadsync-native-keep-chart-validation-20261102.json`; 8303 input hashes captured during execution after compilation and unchanged after completion. This does not certify full workspace tests or all source features.

## File and chart callback review continuation

Read original File handlers, native routing and existing workflow assertion bodies for New/Open/Reload/Close/Exit/OpenContainingFolder. Those rows now explicitly cite the current passing 621-test suite with static-route versus executed-workflow limits. Added EN/FR tab callback regression: literal ordering, bounded movement, circular focus, adjacent close, final-view close and reopen, unchanged song bytes and empty content history. Targeted test passed (1/0), log `/tmp/deadsync-source-chart-tabs-20261102.log`. Initial test setup assumed append-on-focus; corrected it to the original front insertion policy before verifying action results. No product change in this continuation. Callback review now 117/164, 47 pending; not an implementation percentage.

## Sorted chart navigation, snap cycling and Dark

EN/FR callback tests now assert literal difficulty ordering and wrap, chartless first/last reopening at the retained song instant, and all ten snap levels in both directions without moving a negative fractional chart position or changing bytes/history. All six tests in the callback module passed (log `/tmp/deadsync-source-chart-order-snap-20261102.log`). Reviewed original Dark toggle and unchanged native actual-key/transcript/conflict tests from the 621-test suite. Callback review: 122/164; 42 remain. No product change in this batch.

## Routine owner callback payload review

Read original UIEditEvents routing and ActionChangeNotePlayer/ActionSwapNotePlayer constructors. Added collision-free four-player fixture with Hold/Tap/Roll/Mine, head/tail attributes and an unselected later note. All four assign callbacks and all six pair swaps compare entire literal expected maps in EN/FR, defer mutation to Apply, preserve exact bytes through Undo/Redo and restore the exact result grid. Targeted test passed (1/0, 20 cases), `/tmp/deadsync-source-owner-callbacks-20261102.log`. The initial fixture selected timing nodes too; now explicitly selects only its four note heads. No product change. Callback review 132/164; 32 remain.

## Audio, player and mode callback review

Read original audio toggles and tenth-step rate handlers, plus original note-entry/spacing/waveform handlers and native raw-key tests. Linked the existing passing 621-suite mode and waveform workflows with unchanged source hashes. Added EN/FR literal audio callback tests: assist/beat booleans preserve revision and unrelated fields, 14 rate transitions include fractional/tenth/min/max values and exact Undo/Redo/noop contracts. Player callback cycles all four owners and clears peer cache; corrected initial test expectation to account for explicit selection already propagating its player default across charts, as original OnTogglePlayer/SetPlayer specify. All nine callback module tests passed, log `/tmp/deadsync-source-audio-player-callbacks-20261102.log`. No product change. Callback review: 140/164; 24 remain.

## All ten lane callback review

New configured raw-key test exercises Arrow0..9 and ArrowModification across all ten pump-double columns in EN/FR: tap, mine, hold and roll, 80 cases. Exact single-note inventory, lane, owner, kind and endpoint, deferred commit, repeat suppression, Undo/Redo bytes and untouched disk all pass. Log `/tmp/deadsync-source-ten-lane-callbacks-20261102.log` (1/0). Also inspected original Delete/selected-shift handlers and unchanged passing native workflows for precise checked scope and direction. Callback review 154/164, 10 pending. Physical user hold-entry review remains pending. No product change.

## Source callback review complete, overall delivery still open

All 164 callback rows now have bounded functional evidence rather than identifier-only presence: either direct native postconditions or inspected routing plus executed typed workflows, with limits recorded per row. Read existing saving/publication, row-selection and actual source clipboard workflows; their unchanged inputs match the passing 621 native run. New EN/FR transport callbacks test verifies play/pause/resume, Cancel stop-before-selection-clear, preview start/stop and literal preview request with explicitly acknowledged output lease and asset readiness. This command fixture does not decode audio. Targeted pass (1/0), `/tmp/deadsync-source-transport-callbacks-20261102.log`. Full current validation manifest, rebuilt review app and physical user review remain open; 164 reviewed callbacks do not establish universal feature completion.

## Current global editor gate

Fresh complete native editor run: **629 passed, 0 failed, 0 ignored** (86.06 s), all previous 621 test names retained and 8303 captured inputs unchanged. `final-validation-2026-11-02.json` consolidates this execution with 241 frozen core passes (all 297 current core hashes match) and 1346 public workflow passes across 296 modules. 291 module bodies and all 2323 prior support inputs remain unchanged; five strengthened modules cite their later executions. The total 2216 is consolidated evidence, not a newly executed combined run or whole-workspace green claim. Historical full-theme failures and pending physical review remain explicit. Current game binary build and review bundle delivery are in progress.

## Current review app delivered

Current game dev build succeeded (18m00s); no restart or security change was used during macro loading waits. New `dist/DeadSync-Editor-Final-20261102.app` packages that executable with a separate portable config and copied disposable review chart/audio. Strict deep signature verification passed before launch. Actual CUA launch by exact app path entered the French editor, showing both pinned charts at 2.500, ready waveform and aligned minimaps. Update notice was closed without installing. Original review apps/data were preserved. Receipt `/tmp/deadsync-editor-final-app-receipt-20261102.json`, screenshot `/tmp/deadsync-editor-final-ready-20261102.png`. User physical held-lane/Down/Undo review remains open; no commit, push or PR.
