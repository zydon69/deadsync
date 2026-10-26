# Editor transport outside decoded music

Investigation started 2026-10-26. The isolated real-WAV baseline
`editor_transport_lifecycle` has subsequently passed its single test through the
parent’s serialized Cargo run. No lifecycle production change has been made. GrooveAuthor checkout verified at
`48e18f71655d0f0d86144c63239331833501fda2`.

## Evidence and next increment

GrooveAuthor `StepManiaEditor/Sound/MusicManager.cs`:

- `StartPlayback`, lines 563–580: explicitly accepts negative positions and
  positions outside the sound's time range.
- `RenderMusicAndTicks`, lines 1038–1052: advances SampleIndex by callback frame
  count scaled by MusicRate while playing; EOF does not stop this transport.
- Lines 1110–1121: out-of-range PCM samples produce silence. Assist/beat ticks
  and the callback position continue to advance.
- `RenderPreview`, lines 917–1009: a separate preview loops its selected region,
  with fades. It must not be turned into silent post-EOF chart transport.

Current Rust evidence:

- `screens/edition/playback.rs::update_audio` rejects stale snapshots and pauses
  after an accepted mapping disappears. A false `has_music_mapping` alone cannot
  distinguish EOF, user stop, pending decoder setup, device loss, or failure.
- `edition_playback_follow_workflow.rs::older_mapped_or_unmapped_snapshots_cannot_rewind_or_stop_current_transport`
  deliberately expects mapping loss to pause. This proves the present fallback
  policy, not GrooveAuthor EOF parity.
- `deadlib-audio/src/stream.rs`, lines 957–987: normal EOF finishes the output
  generation, then retained decoders park and can seek again. EOF is explicitly
  distinguished from cancellation here. That distinction should survive into
  the editor-facing transport snapshot.
- `deadlib-audio-core/src/position.rs::MusicStreamClockSnapshot` and
  `deadsync-gameplay/src/runtime_config.rs::GameplayStreamClockSnapshot` currently
  publish position/rate/mapping/timestamp, without a typed terminal reason.
- Existing preview audition already loops (`toggle_sample_audio`); snapped
  navigation already supports continued playback through `SeekMusic`. Neither
  should be reimplemented just to claim progress.

**Next concrete increment:** preserve an authoritative editor transport clock
through silent ranges before the first and after the final decoded sample, while
keeping decoder-open failures, explicit stop and device failure distinguishable.
The exact backend EOF mapping lifecycle must be established by a real WAV test;
this reading does not claim that every backend currently clears its mapping at
EOF or that all of them advance the clock equivalently.

## Proposed contract

Add an editor transport policy to the start command rather than globally changing
ordinary gameplay or looping preview behavior. The backend that owns the output
callback should publish a monotonic command/generation identity and typed state:

```text
Pending                 // command accepted, decoder not established
PlayingDecoded          // mapped PCM from this successful decoder generation
PlayingSilenceBefore    // decoder established; negative source position
PlayingSilenceAfter     // normal EOF; output callback continues transport
Stopped                 // explicit stop/cancel; no automatic advance
Failed(reason)          // open/decode/device failure; no invented play success
```

Keep position and rate anchored to the audio callback frame clock, including
silent output. Do not start a wall-clock simulation from the moment a PlayMusic
command was sent. A successful decoder generation must be acknowledged before
the editor treats pre-roll or post-roll silence as playback. If the callback
stalls/disappears, follow the existing device/error policy rather than falsely
proving audible progress.

Pause freezes the last accepted position. Resume starts a new acknowledged
generation at that exact music time. Seek invalidates earlier snapshots and
reuses the retained decoder; rate changes update the callback mapping without
reopening. A seek back into the decoded range must resume actual PCM. Song
changes or explicit Stop invalidate silent transport as well. Tick scheduling
must use the same source time, although a later tick feature can be implemented
separately once this timing contract is proven.

## Focused proof required

1. Reference oracle: extract unchanged start/index advancement and out-of-range
   sample selection rules; compare positions around zero and the final frame at
   rates 0.25, 1 and 4. Confirm integer sample rounding behavior explicitly.
2. Real short WAV: negative start emits zero PCM before time zero, then actual
   samples; a known final PCM frame is followed by zero PCM while time advances.
3. Start beyond EOF: decoder success is still required; missing/corrupt input
   cannot masquerade as successful silent playback.
4. Pause in pre-roll/post-roll freezes exact fractional position; resume retains
   it. Explicit stop and decoder/device failure do not continue automatically.
5. Seek before/inside/after source range reuses decoder, rejects old generations,
   and never applies stale positions after a newer seek or rate change.
6. Rate changes preserve continuity in silence and decoded audio, including
   repeated changes and EOF during a pending command.
7. Existing audition loops/fades and ordinary gameplay EOF policy remain covered
   by regressions; editor-only policy must not silently change either behavior.

Run focused Cargo targets serially with `--locked --jobs 2`. Hardware listening
and device-unplug behavior remain separate runtime checks. No completion claim
can rest on synthetic mapped snapshots alone.


## Proposed bounded source acknowledgement API (awaiting checkpoint)

Keep decoder progress separate from playback position. A decoder can finish
seconds ahead of the callback, so a worker reporting source EOF must never imply
that buffered PCM has already been heard or that transport should stop.

Proposed public types in `deadlib-audio::stream::lifecycle`:

```text
DecoderPhase = Pending | Opened | Producing | Exhausted | Stopped | Failed
DecoderSnapshot {
    command_generation: u64,
    sequence: u64,
    phase: DecoderPhase,
    opened_successfully: bool,
    requested_start_seconds: f64,
    final_music_seconds: Option<f64>,
    failure: Option<Arc<str>>,
}
DecoderStatusReader::snapshot() -> DecoderSnapshot
MusicStream::status_reader() -> DecoderStatusReader
MusicStreamRuntime::decoder_status() -> DecoderStatusMonitor
DecoderStatusMonitor::snapshot() -> Option<DecoderSnapshot>
```

One shared latest-value slot per decoder, protected by a mutex used only on the
manager/decoder/UI threads. The audio callback must never acquire this mutex.
There is no unbounded event queue. Keep error text only for the latest failed
state. Request serialization advances generation and marks Pending before wake;
old-generation publications are rejected. A persistent open acknowledgement may
be reused after a retained seek, but applying that seek must still be acknowledged
for its own generation before a caller consumes the resulting state.

`Opened` is published only after `decode::open_file` succeeds, never merely after
thread creation or `activate_music_track`. `Exhausted` means producer reached
normal EOF/completed cut and queued all frames. The final mapped position is
optional because a start beyond EOF can yield no decoded frame. Failed open,
failed decode and worker panic publish Failed; explicit stop publishes Stopped
and cannot be overwritten by the decoder waking late. The runtime needs to retain
its latest reader after worker join so Stopped remains observable.

The next tests should verify generation-race rejection and latest-value bounds
in the isolated lifecycle module, then real-WAV Opened/Exhausted acknowledgements,
missing/corrupt files and explicit stop. Only after those pass should the callback
transport adopt an editor-only silent continuation policy. Audio callback time,
not producer completion time, chooses decoded/pre-roll/post-roll presentation.

## Current integration and precision follow-up

The lifecycle and native adapter have subsequently been implemented. Confirmed
historical validation reported by the parent: 3 lifecycle unit tests, 1 real-WAV
lifecycle test, 1 WAV/playback regression test, 7 audio-stream tests and 1 shell
adapter test passed. These results belong to the states validated at that time;
they do not prove the precision follow-up described below.

The precision follow-up passed parent validation: model 5 tests and native
English/French 16 tests in run 19227, plus audio-stream 8 tests (including f64
after 24 hours) in run 10280. The same native run passed chart-tab 10, playback
follow 10 and navigation 3 regression tests. The final shell f64 bridge requires
its separate compilation result; these counts do not claim hardware playback.

- Gameplay retains its existing f32 stream-time snapshot contract. A separate
  editor clock path carries absolute callback time as f64, before casting the
  compatibility snapshot to f32. No system-wall-time deltas are added by the
  native transport model.
- `State::set_decoder_stream_time(generation, valid_at, seconds)` supplies the
  precision side channel. It applies only to the exact snapshot timestamp and
  command generation. Non-finite values are ignored. Existing callers without
  this setter retain their previous f32 path.
- A real mapped position owns its exact source-time slope for the remainder of
  that generation. Silent continuation does not compare it for exact equality
  against a nominal f64 preference: f32 conversion and decoder chunk ratios can
  produce legitimate differences. Actual rate commands require a fresh issued
  generation and anchor. New tests exercise 1.1x and 0.8x backend f32 slopes.
- Long-uptime tests use an absolute stream time of 86400 seconds plus 2 ms. Both
  values collapse to the same f32 representation, while the f64 path crosses a
  note-row boundary from the retained fractional mapped position.

## Next reference-backed scrolling increment

Reference `Editor.cs::ProcessInputForMiniMap` (around 3185–3250) and density-graph
handling (around 3680–3734) implement a distinct gesture policy: if playing when
drag begins, stop/pause immediately; move the chart position during capture; on
release restart only when `StopPlaybackWhenDraggingScrollBars` is false. Its
default is false in `PreferencesScroll.cs` lines 25–26 and stored independently
from `StopPlaybackWhenScrolling` (lines 141–142). Current Rust scroll preferences
only model the latter snapped-navigation policy.

A useful next increment is to port this gesture state and preference together
with an operable compact minimap/density navigation control. In the simplified
menu, keyboard/pad focus can provide begin/move/release semantics; mouse support
can use the same state if added. Retain the exact fractional chart/music focus
when beginning and ending the gesture, reject stale audio snapshots during it,
and resume through the acknowledged generation mechanism. Tests should cover
playing/stopped entry, both preference values, cancellation, chart changes,
negative/pre-roll positions, repeat events and one final resumed start command.

This source review found looping playback in `MusicManager::RenderPreview`
(selected preview region or dedicated preview file). Normal chart playback does
not acquire a new general A/B-loop requirement from those functions. Existing
Rust preview audition already loops; its runtime fidelity should be tested,
rather than introducing an unrelated chart loop and calling it a port.

The next lot now has isolated, **unconnected and unvalidated** source modules:
`deadsync-editor/src/minimap_navigation.rs` implements Begin/Move/End and exact
Pause/Focus/Resume effects; `scroll_preferences/dragging.rs` implements the
distinct boolean preference. Its isolated `--test minimap_navigation` target
contains four tests. Existing preferences, edition State and playback files have
not been changed for this lot. This is preparation for the operable control,
not completed minimap functionality.

## Implemented continuation and remaining validation

The backend now publishes these acknowledgements and exposes a durable monitor
through `StreamReady`. `AudioControl` filters it against the synchronous current
command generation. Failed workers also acknowledge subsequent Seek/Rate/Pitch
commands as failed rather than leaving them Pending forever. Three lifecycle
tests passed, including a barrier-controlled failure/new-command race. Both the
extended real-WAV lifecycle test and the existing PCM playback regression passed
after this correction.

The editor transport model is connected to its playback state and uses callback
stream time, with actual consumed-PCM mappings taking priority. Its five model
tests and sixteen EN/FR native workflows passed. The native tests inject snapshots
and acknowledgements; they do not prove sound output or operating-system timing.

Ordinary gameplay's clock also reports optimistic startup seeds as mapped. The
editor must not treat those seeds as proof that a decoder opened. The new
`MusicClock::snapshot_with_decoded_mapping` explicitly distinguishes consumed PCM
and exposes an absolute callback clock that advances even if a request beyond
EOF never emits a first block. `snapshot_for_editor_time` retains precise f64
seconds before the legacy f32 cast. Eight audio-stream tests passed, preserving
the gameplay snapshot contract and verifying no-PCM callbacks and submillisecond
steps after a day. Exact mapped slopes survive nominal-rate rounding at 0.8x/1.1x.

The shell strips seed-only mapping flags exclusively for editor playback. It
drains pending editor commands before taking another snapshot, and the practice
adapter skips snapshots when its own update has just emitted a transport command.
The command-order unit test and the shell clock-adapter test passed, and the
integrated shell lib-test harness compiled with the f64 bridge. All 161 focused
theme checks passed in the final affected regression and pagination rerun.
Output-device failure and callback-stall policy,
hardware listening, and full runtime/rendering verification remain required.

### Callback interruption boundary

The editor clock has a focused regression for an output callback interruption.
With an unchanged callback window, time advances only through the last submitted
buffer and then stays at its end, even when the observation timestamp advances
by a minute. A resumed callback advances from its actual frame count, without
adding the interruption duration. The test
`editor_clock_stops_at_last_callback_end_until_output_resumes` passed. This
establishes bounded transport time for that clock path; it does not establish
device-disconnection detection, a user-facing failure notice, or physical output
recovery. Those remain required separately.
