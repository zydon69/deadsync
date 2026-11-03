fn editor_main_position(sample_rate: u32) -> Option<(f64, f32, bool)> {
    let generation = deadlib_audio_core::music_map_generation();
    let epoch = deadlib_audio_core::music_asset_epoch();
    let control = deadlib_audio_core::editor_main_configuration()
        .filter(|c| c.asset_epoch == epoch && c.generation == generation)?;
    if !deadlib_audio_core::music_track_active() {
        return None;
    }
    let cursor = deadlib_audio_core::editor_main_cursor().filter(|c| {
        c.asset_epoch == epoch && c.generation == generation && c.sample_rate_hz == sample_rate
    });
    let decoded = cursor.is_some()
        && deadlib_audio_core::rendered_music_cursor()
            .is_some_and(|c| c.asset_epoch == epoch && c.generation == generation);
    Some((
        cursor.map_or(control.frame, |c| c.source_frame) as f64 / f64::from(sample_rate),
        control.rate as f32,
        decoded,
    ))
}

use crate::MusicClock;
use deadlib_audio_core::{
    CallbackClockSource, CallbackClockWindow, MusicStreamClockSnapshot,
    fallback_stream_position_frames, music_nanos_from_seconds, music_track_has_started,
    music_track_start_frame, seeded_music_position,
    stream_position_frames_from_window as audio_stream_position_frames_from_window,
};
use deadlib_platform::host_time::instant_nanos;
#[cfg(windows)]
use deadlib_platform::windows_rt::current_qpc_nanos;
use log::debug;
use std::time::Instant;

#[inline(always)]
#[must_use]
pub fn timing_diag_enabled() -> bool {
    log::log_enabled!(log::Level::Debug)
}

#[inline(always)]
fn current_callback_clock_nanos(valid_at: Instant, source: CallbackClockSource) -> Option<u64> {
    match source {
        CallbackClockSource::Instant => Some(instant_nanos(valid_at)),
        #[cfg(windows)]
        CallbackClockSource::Qpc => current_qpc_nanos(),
    }
}

fn load_callback_clock_snapshot_now() -> (Instant, u64, CallbackClockSource, CallbackClockWindow) {
    deadlib_audio_core::load_callback_clock_snapshot_now(current_callback_clock_nanos)
}

#[inline(always)]
fn stream_position_frames_from_window(
    sample_rate: u32,
    start_frame: u64,
    at_nanos: u64,
    window: CallbackClockWindow,
) -> f64 {
    if let Some(frames) =
        audio_stream_position_frames_from_window(sample_rate, start_frame, at_nanos, window)
    {
        return frames;
    }
    if timing_diag_enabled() {
        debug!(
            "AUDIO_DIAG stream_pos_fallback sample_rate_hz={} at_nanos={} last_nanos={} last_base_frames={} last_callback_frames={} prev_nanos={} prev_base_frames={} prev_callback_frames={} total_frames={} start_frame={}",
            sample_rate,
            at_nanos,
            window.last_nanos,
            window.last_base_frames,
            window.last_callback_frames,
            window.prev_nanos,
            window.prev_base_frames,
            window.prev_callback_frames,
            window.total_frames,
            start_frame,
        );
    }
    fallback_stream_position_frames(start_frame, window)
}

#[inline(always)]
fn music_stream_clock_snapshot_at_nanos(
    clock: &MusicClock,
    sample_rate: u32,
    start: u64,
    valid_at: Instant,
    at_nanos: u64,
    source: CallbackClockSource,
    window: CallbackClockWindow,
) -> (MusicStreamClockSnapshot, bool) {
    let stream_frames = stream_position_frames_from_window(sample_rate, start, at_nanos, window);
    let stream_seconds = stream_frames / f64::from(sample_rate);
    let (music_seconds, music_seconds_per_second, has_music_mapping, decoded_mapping) =
        match clock.lookup(stream_frames) {
            Some((music_seconds, slope)) => (music_seconds, slope, true, true),
            None => match seeded_music_position(stream_seconds) {
                Some((music_seconds, slope)) => (music_seconds, slope, true, false),
                None => (stream_seconds, 1.0, false, false),
            },
        };
    (
        MusicStreamClockSnapshot {
            stream_seconds: stream_seconds as f32,
            music_nanos: music_nanos_from_seconds(music_seconds),
            music_seconds_per_second,
            has_music_mapping,
            valid_at,
            valid_at_host_nanos: match source {
                #[cfg(windows)]
                CallbackClockSource::Qpc => at_nanos,
                #[cfg(windows)]
                CallbackClockSource::Instant => 0,
                #[cfg(not(windows))]
                CallbackClockSource::Instant => at_nanos,
            },
        },
        decoded_mapping,
    )
}

fn editor_clock_at_nanos(
    clock: &MusicClock,
    sample_rate: u32,
    start: u64,
    valid_at: Instant,
    at_nanos: u64,
    source: CallbackClockSource,
    window: CallbackClockWindow,
) -> (MusicStreamClockSnapshot, bool, f64) {
    let (mut snapshot, mut decoded) = music_stream_clock_snapshot_at_nanos(
        clock,
        sample_rate,
        start,
        valid_at,
        at_nanos,
        source,
        window,
    );
    // Continue the actual callback clock even if an opened request beyond EOF
    // never supplies a first block. An absolute origin also avoids a jump when
    // the first PCM block changes music_track_start_frame.
    let seconds = stream_position_frames_from_window(sample_rate, 0, at_nanos, window)
        / f64::from(sample_rate);
    if let Some((position, rate, actual_pcm)) = editor_main_position(sample_rate) {
        snapshot.music_nanos = music_nanos_from_seconds(position);
        snapshot.music_seconds_per_second = rate;
        snapshot.has_music_mapping = true;
        decoded = actual_pcm;
    }
    snapshot.stream_seconds = seconds as f32;
    (snapshot, decoded, seconds)
}

impl MusicClock {
    /// Returns the current stream position and the `Instant` it is valid for.
    pub fn snapshot(&mut self) -> MusicStreamClockSnapshot {
        self.snapshot_legacy_with_mapping().0
    }

    /// Distinguish a callback-consumed decoder mapping from the optimistic
    /// startup seed used by ordinary gameplay. A seed is not a decode acknowledgement.
    pub fn snapshot_with_decoded_mapping(&mut self) -> (MusicStreamClockSnapshot, bool) {
        let (snapshot, decoded, _) = self.snapshot_for_editor_time();
        (snapshot, decoded)
    }

    /// Precise absolute callback seconds for long editor sessions. The legacy
    /// snapshot's f32 field remains available for existing callers.
    pub fn snapshot_for_editor_time(&mut self) -> (MusicStreamClockSnapshot, bool, f64) {
        let sample_rate = self.sample_rate();
        let (valid_at, at_nanos, source, window) = load_callback_clock_snapshot_now();
        editor_clock_at_nanos(
            self,
            sample_rate,
            music_track_start_frame(),
            valid_at,
            at_nanos,
            source,
            window,
        )
    }

    /// Deterministic test seam: exact acknowledged render frame, with no host
    /// interpolation and no fabricated decoder mapping/acknowledgement.
    #[cfg(test)]
    pub(crate) fn snapshot_at_rendered_frame(&self) -> (MusicStreamClockSnapshot, bool, f64) {
        let hz = self.sample_rate().max(1);
        let absolute = deadlib_audio_core::music_total_frames();
        let relative = absolute.saturating_sub(music_track_start_frame()) as f64;
        let (seconds, rate, mapped, decoded) =
            if let Some((seconds, rate, decoded)) = editor_main_position(hz) {
                (seconds, rate, true, decoded)
            } else if let Some((seconds, rate)) = self.lookup(relative) {
                (seconds, rate, true, true)
            } else if let Some((seconds, rate)) = seeded_music_position(relative / f64::from(hz)) {
                (seconds, rate, true, false)
            } else {
                (relative / f64::from(hz), 1., false, false)
            };
        (
            MusicStreamClockSnapshot {
                stream_seconds: absolute as f32 / hz as f32,
                music_nanos: music_nanos_from_seconds(seconds),
                music_seconds_per_second: rate,
                has_music_mapping: mapped,
                valid_at: Instant::now(),
                valid_at_host_nanos: 0,
            },
            decoded,
            absolute as f64 / f64::from(hz),
        )
    }

    fn snapshot_legacy_with_mapping(&mut self) -> (MusicStreamClockSnapshot, bool) {
        let sample_rate = self.sample_rate();
        if !music_track_has_started() {
            if let Some((music_seconds, slope)) = seeded_music_position(0.0) {
                return (
                    MusicStreamClockSnapshot {
                        stream_seconds: 0.0,
                        music_nanos: music_nanos_from_seconds(music_seconds),
                        music_seconds_per_second: slope,
                        has_music_mapping: true,
                        valid_at: Instant::now(),
                        valid_at_host_nanos: 0,
                    },
                    false,
                );
            }
            return (
                MusicStreamClockSnapshot {
                    stream_seconds: 0.0,
                    music_nanos: 0,
                    music_seconds_per_second: 1.0,
                    has_music_mapping: false,
                    valid_at: Instant::now(),
                    valid_at_host_nanos: 0,
                },
                false,
            );
        }
        let start = music_track_start_frame();
        let (valid_at, at_nanos, source, window) = load_callback_clock_snapshot_now();
        music_stream_clock_snapshot_at_nanos(
            self,
            sample_rate,
            start,
            valid_at,
            at_nanos,
            source,
            window,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_map::clear_music_pos_map;
    use deadlib_audio_core::{
        CallbackInfo, MixControls, MusicBlockTiming, OutputBufferMut, RenderState,
        activate_music_track, clear_music_stream_clock_seed, music_map_generation, music_transport,
        reset_music_stream_clock_state, seed_music_stream_clock,
    };
    use std::sync::Arc;

    const SAMPLE_RATE: u32 = 48_000;
    const ANCHOR_NANOS: u64 = 1_000_000_000;

    #[test]
    fn editor_clock_stops_at_last_callback_end_until_output_resumes() {
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        reset_music_stream_clock_state();
        clear_music_pos_map();
        let (stream, _) = music_transport(SAMPLE_RATE, 2);
        let clock = MusicClock::new(stream.played_map, SAMPLE_RATE);
        let window = CallbackClockWindow {
            last_nanos: ANCHOR_NANOS + 1,
            last_base_frames: u64::from(SAMPLE_RATE) * 10,
            last_callback_frames: 480,
            ..Default::default()
        };
        let read = |delta, window| {
            editor_clock_at_nanos(
                &clock,
                SAMPLE_RATE,
                0,
                Instant::now(),
                ANCHOR_NANOS + delta,
                CallbackClockSource::Instant,
                window,
            )
            .2
        };
        assert!((read(5_000_000, window) - 10.005).abs() < 1e-12);
        let end = read(10_000_000, window);
        assert!((end - 10.01).abs() < 1e-12);
        assert_eq!(read(60_000_000_000, window), end);
        let resumed = CallbackClockWindow {
            last_nanos: ANCHOR_NANOS + 60_000_000_000 + 1,
            last_base_frames: window.last_base_frames + window.last_callback_frames,
            ..window
        };
        assert!((read(60_005_000_000, resumed) - 10.015).abs() < 1e-12);
        reset_music_stream_clock_state();
        clear_music_pos_map();
    }

    #[test]
    fn precise_editor_clock_retains_submillisecond_callback_steps_after_a_day() {
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        reset_music_stream_clock_state();
        clear_music_pos_map();
        let (stream, _) = music_transport(SAMPLE_RATE, 2);
        let clock = MusicClock::new(stream.played_map, SAMPLE_RATE);
        let window = CallbackClockWindow {
            last_nanos: ANCHOR_NANOS + 1,
            last_base_frames: u64::from(SAMPLE_RATE) * 86_400,
            last_callback_frames: 256,
            ..Default::default()
        };
        let read = |delta| {
            editor_clock_at_nanos(
                &clock,
                SAMPLE_RATE,
                0,
                Instant::now(),
                ANCHOR_NANOS + delta,
                CallbackClockSource::Instant,
                window,
            )
        };
        let (first, _, precise_first) = read(500_000);
        let (second, _, precise_second) = read(1_000_000);
        assert_eq!(first.stream_seconds, second.stream_seconds);
        assert!((precise_first - 86_400.0005).abs() < 1e-10);
        assert!((precise_second - precise_first - 0.0005).abs() < 1e-10);
        reset_music_stream_clock_state();
        clear_music_pos_map();
    }

    #[test]
    fn editor_callback_clock_advances_without_pcm_and_never_labels_a_seed_as_decoded() {
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        reset_music_stream_clock_state();
        clear_music_pos_map();
        let (stream, render_handle) = music_transport(SAMPLE_RATE, 2);
        let mut clock = MusicClock::new(stream.played_map, SAMPLE_RATE);
        let mut render = RenderState::new(render_handle, Arc::new(MixControls::new()), 2);
        activate_music_track();
        seed_music_stream_clock(2.0, 2.0);
        let mut callback = || {
            render.render(
                OutputBufferMut::F32(&mut [0.0; 512]),
                CallbackInfo {
                    anchor_nanos: deadlib_platform::host_time::instant_nanos(Instant::now()),
                    clock: CallbackClockSource::Instant,
                },
                [],
            );
        };
        callback();
        let (first, decoded) = clock.snapshot_with_decoded_mapping();
        assert!(!decoded);
        assert!(!music_track_has_started());
        for _ in 0..4 {
            callback();
        }
        let (later, decoded) = clock.snapshot_with_decoded_mapping();
        assert!(!decoded);
        assert!(later.stream_seconds > first.stream_seconds);
        // The existing gameplay snapshot keeps its deliberately seeded contract.
        let legacy = clock.snapshot();
        assert_eq!(legacy.stream_seconds, 0.0);
        assert_eq!(legacy.music_nanos, 2_000_000_000);
        assert!(legacy.has_music_mapping);
        reset_music_stream_clock_state();
        clear_music_pos_map();
    }

    fn snapshot_at(clock: &MusicClock, elapsed_nanos: u64) -> MusicStreamClockSnapshot {
        snapshot_with_source_at(clock, elapsed_nanos).0
    }

    fn snapshot_with_source_at(
        clock: &MusicClock,
        elapsed_nanos: u64,
    ) -> (MusicStreamClockSnapshot, bool) {
        let start = music_track_start_frame();
        let whole_seconds = elapsed_nanos / 1_000_000_000;
        music_stream_clock_snapshot_at_nanos(
            clock,
            SAMPLE_RATE,
            start,
            Instant::now(),
            ANCHOR_NANOS + elapsed_nanos % 1_000_000_000,
            CallbackClockSource::Instant,
            CallbackClockWindow {
                last_nanos: ANCHOR_NANOS + 1,
                last_base_frames: start + whole_seconds * u64::from(SAMPLE_RATE),
                last_callback_frames: 256,
                ..Default::default()
            },
        )
    }

    #[test]
    fn snapshots_preserve_precision_across_playback_states() {
        // Clock and seek tests serialize their process-global transport transitions.
        let _guard = crate::CLOCK_TEST_LOCK.lock().unwrap();
        for (origin, origin_nanos) in [
            (0.000_000_1, 100i64),
            (3_600.000_1, 3_600_000_100_000),
            (8_192.000_4, 8_192_000_400_000),
            (-3_600.000_1, -3_600_000_100_000),
        ] {
            for rate in [0.5f32, 1.0, 1.5, 2.0] {
                reset_music_stream_clock_state();
                clear_music_pos_map();
                let (mut stream, render_handle) = music_transport(SAMPLE_RATE, 2);
                let mut clock = MusicClock::new(stream.played_map, SAMPLE_RATE);

                let (unstarted, decoded) = clock.snapshot_legacy_with_mapping();
                assert!(!decoded);
                assert_eq!(unstarted.music_nanos, 0);
                assert!(!unstarted.has_music_mapping);

                seed_music_stream_clock(origin, rate);
                let (seeded, decoded) = clock.snapshot_legacy_with_mapping();
                assert!(!decoded, "startup seeds must not acknowledge decoding");
                assert_eq!(seeded.music_nanos, origin_nanos, "seed origin={origin}");
                assert_eq!(seeded.music_seconds_per_second, rate);
                assert!(seeded.has_music_mapping);
                assert_eq!(seeded.valid_at_host_nanos, 0);

                // An empty played map must preserve both the cut origin and
                // fractional stream time, even after hours of elapsed playback.
                activate_music_track();
                for elapsed_nanos in [100_000, 3_600_000_100_000, 8_192_000_400_000] {
                    let (fallback, decoded) = snapshot_with_source_at(&clock, elapsed_nanos);
                    assert!(!decoded, "elapsed seeded time is not consumed PCM");
                    let delta = (elapsed_nanos as f64 * f64::from(rate)).round() as i64;
                    assert_eq!(fallback.music_nanos, origin_nanos + delta);
                    assert_eq!(fallback.music_seconds_per_second, rate);
                    assert!(fallback.has_music_mapping);
                }

                clear_music_stream_clock_seed();
                let unmapped = snapshot_at(&clock, 3_600_000_100_000);
                assert_eq!(unmapped.music_nanos, 3_600_000_100_000);
                assert_eq!(unmapped.music_seconds_per_second, 1.0);
                assert!(!unmapped.has_music_mapping);

                let generation = music_map_generation();
                assert_eq!(
                    stream.writer.try_push(
                        &[0; 512],
                        MusicBlockTiming {
                            asset_epoch: deadlib_audio_core::music_asset_epoch(),
                            generation,
                            music_start_sec: origin,
                            music_sec_per_frame: f64::from(rate) / f64::from(SAMPLE_RATE),
                        },
                    ),
                    512,
                );
                stream.writer.publish_generation_ready(generation);
                let mut render = RenderState::new(render_handle, Arc::new(MixControls::new()), 2);
                render.render(
                    OutputBufferMut::F32(&mut [0.0; 512]),
                    CallbackInfo {
                        anchor_nanos: ANCHOR_NANOS,
                        clock: CallbackClockSource::Instant,
                    },
                    [],
                );
                // Once the callback supplies a map, it takes precedence over
                // the seed, including its playback slope.
                seed_music_stream_clock(123.0, 3.0);
                for elapsed_nanos in [0, 10_000, 20_834, 100_000, 400_000] {
                    let (mapped, decoded) = snapshot_with_source_at(&clock, elapsed_nanos);
                    assert!(decoded, "the callback consumed the generation's PCM");
                    let delta = (elapsed_nanos as f64 * f64::from(rate)).round() as i64;
                    assert_eq!(mapped.music_nanos, origin_nanos + delta);
                    assert_eq!(mapped.music_seconds_per_second, rate);
                    assert!(mapped.has_music_mapping);
                }
            }
        }
        reset_music_stream_clock_state();
        clear_music_pos_map();
    }
}
