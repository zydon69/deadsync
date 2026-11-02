//! Compare a retained decoder seek with a fresh cut through the real DSP/render path.
//! One test owns this process's global transport clock. No device or encoder required.
use deadlib_audio::stream::{
    Cut, MusicDecodeContext, MusicStreamRuntime, OutputFormat, StreamCommand,
    spawn_music_decoder_thread,
};
use deadlib_audio_core::{
    AudioRenderHandle, CallbackClockSource, CallbackInfo, MixControls, MusicMapSeg,
    OutputBufferMut, PlayedMapReader, RenderState, activate_music_track, bump_music_map_generation,
    music_total_frames, music_track_start_frame, music_transport, reset_music_stream_clock_state,
    reset_music_target_gain,
};
use deadlib_audio_decode::DecodeOptions;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
const LENGTH: f64 = 0.08;
#[derive(Clone, Copy, Debug)]
struct Mode {
    rate: f32,
    pitch: bool,
    hz: u32,
    options: DecodeOptions,
}
impl Mode {
    fn frames(self, start: f64) -> usize {
        (LENGTH * f64::from(self.hz)).round() as usize
            + ((-start).max(0.0) / f64::from(self.rate) * f64::from(self.hz)).round() as usize
    }
    fn output(self) -> OutputFormat {
        OutputFormat {
            sample_rate_hz: self.hz,
            channels: 2,
        }
    }
    fn cut(self, start: f64) -> Cut {
        Cut {
            start_sec: start,
            length_sec: LENGTH,
            ..Cut::default()
        }
    }
}
struct FixtureCopy {
    directory: PathBuf,
    path: PathBuf,
}
impl FixtureCopy {
    fn new(source: &Path) -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        loop {
            let directory = std::env::temp_dir().join(format!(
                "deadlib-seek-codecs-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let path = directory.join(source.file_name().unwrap());
                    fs::copy(source, &path).unwrap();
                    return Self { directory, path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
    }
    fn hide_original(&mut self) {
        let held = self.path.with_extension("retained");
        fs::rename(&self.path, &held).unwrap();
        self.path = held;
    }
}
impl Drop for FixtureCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
struct Snapshot {
    pcm: Vec<i16>,
    timing: Vec<MusicMapSeg>,
}
struct Capture {
    renderer: RenderState,
    played: PlayedMapReader,
    hz: u32,
}
impl Capture {
    fn new(render: AudioRenderHandle, played: PlayedMapReader, hz: u32) -> Self {
        Self {
            renderer: RenderState::new(render, Arc::new(MixControls::new()), 2),
            played,
            hz,
        }
    }
    fn until(&mut self, generation: u64, frames: usize, label: &str) -> Snapshot {
        let mut result = Snapshot {
            pcm: Vec::new(),
            timing: Vec::new(),
        };
        let mut output = [0; 512];
        let deadline = Instant::now() + Duration::from_secs(5);
        while result.pcm.len() < frames * 2 {
            assert!(
                Instant::now() < deadline,
                "{label}: stalled after {} of {} samples",
                result.pcm.len(),
                frames * 2
            );
            let start = music_total_frames();
            self.renderer.render(
                OutputBufferMut::I16(&mut output),
                CallbackInfo {
                    anchor_nanos: 1_000_000_000 + start * 1_000_000_000 / u64::from(self.hz),
                    clock: CallbackClockSource::Instant,
                },
                std::iter::empty(),
            );
            while let Some((tag, asset_epoch, segment)) = self.played.pop() {
                if tag != generation || asset_epoch != deadlib_audio_core::music_asset_epoch() {
                    assert!(
                        result.timing.is_empty(),
                        "{label}: stale generation resumed"
                    );
                    continue;
                }
                let callback_frame = start.saturating_sub(music_track_start_frame());
                let first = (segment.stream_frame_start as u64 - callback_frame) as usize * 2;
                let end = first + segment.frames as usize * 2;
                result.pcm.extend_from_slice(&output[first..end]);
                result.timing.push(segment);
            }
            std::thread::yield_now();
        }
        assert_eq!(result.pcm.len(), frames * 2, "{label}: output length");
        result
    }
}
fn reset() -> u64 {
    reset_music_stream_clock_state();
    reset_music_target_gain();
    activate_music_track();
    bump_music_map_generation()
}
fn reference(path: &Path, mode: Mode, start: f64) -> Snapshot {
    let generation = reset();
    let (stream, render) = music_transport(mode.hz, 2);
    let worker = spawn_music_decoder_thread(
        path.to_owned(),
        mode.cut(start),
        false,
        mode.rate,
        mode.pitch,
        stream.writer,
        MusicDecodeContext {
            decode_options: mode.options,
            output: mode.output(),
            generation,
        },
    );
    // This short cut fits below the pool watermark, preserving the one-shot join contract.
    let _writer = worker.thread.join().unwrap();
    Capture::new(render, stream.played_map, mode.hz).until(
        generation,
        mode.frames(start),
        &format!("reference {path:?} {mode:?} {start}"),
    )
}
fn endpoint(snapshot: &Snapshot) -> (f64, f64) {
    let first = snapshot.timing.first().unwrap();
    let last = snapshot.timing.last().unwrap();
    (
        first.music_start_sec,
        last.music_start_sec + last.frames as f64 * last.music_sec_per_frame,
    )
}
#[test]
fn retained_seeks_match_fresh_cuts_across_codecs_rates_pitch_and_output_rates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../deadlib-audio-decode/tests/fixtures");
    let mut cases = 0;
    for name in [
        "seek.wav",
        "seek.flac",
        "seek.ogg",
        "seek.opus",
        "seek.mp3",
        "seek-long.opus",
    ] {
        let suffix = name.rsplit('.').next().unwrap();
        let positions = [
            0.0,
            0.125,
            if name == "seek-long.opus" { 2.29 } else { 0.55 },
            0.0,
            -0.025,
        ];
        let profiles: Vec<_> = if suffix == "mp3" {
            vec![DecodeOptions::STEP_MANIA_TIMELINE, DecodeOptions::default()]
        } else {
            vec![DecodeOptions::STEP_MANIA_TIMELINE]
        };
        for options in profiles {
            for rate in [0.8, 1.0, 1.2] {
                for pitch in [false, true] {
                    for hz in [44_100, 48_000] {
                        let mode = Mode {
                            rate,
                            pitch,
                            hz,
                            options,
                        };
                        let source = root.join(name);
                        let expected: Vec<_> = positions
                            .iter()
                            .map(|&position| reference(&source, mode, position))
                            .collect();
                        let mut copy = FixtureCopy::new(&source);
                        let generation = reset();
                        let (stream, render) = music_transport(hz, 2);
                        let mut runtime = MusicStreamRuntime::new(stream.writer, mode.output());
                        runtime.handle(StreamCommand::PlayMusic {
                            asset_epoch: deadlib_audio_core::music_asset_epoch(),
                            decode_options: options,
                            path: copy.path.clone(),
                            cut: mode.cut(0.0),
                            looping: false,
                            rate,
                            preserve_pitch: pitch,
                            generation,
                        });
                        let mut capture = Capture::new(render, stream.played_map, hz);
                        let label = format!("{name} {mode:?}");
                        let initial = capture.until(generation, mode.frames(0.0), &label);
                        assert_eq!(initial.pcm, expected[0].pcm, "{label}: initial cut");
                        copy.hide_original(); // A reopen fallback cannot silently make this test pass.
                        for (&position, expected) in positions[1..].iter().zip(&expected[1..]) {
                            let generation = reset();
                            runtime.handle(StreamCommand::SeekMusic {
                                seconds: position,
                                generation,
                            });
                            let label = format!("{name} {mode:?} position={position}");
                            let actual = capture.until(generation, mode.frames(position), &label);
                            let first_difference = actual
                                .pcm
                                .iter()
                                .zip(&expected.pcm)
                                .position(|(a, b)| a != b);
                            if first_difference.is_some() {
                                eprintln!(
                                    "{label}: actual {:?}; expected {:?}; endpoints {:?} {:?}",
                                    &actual.pcm[..16],
                                    &expected.pcm[..16],
                                    endpoint(&actual),
                                    endpoint(expected)
                                );
                            }
                            assert!(
                                first_difference.is_none(),
                                "{label}: PCM differs at sample {first_difference:?}"
                            );
                            let (a, b) = endpoint(&actual);
                            let (c, d) = endpoint(expected);
                            assert!(
                                (a - c).abs() < 1e-10 && (b - d).abs() < 1e-10,
                                "{label}: timestamp endpoints {:?} != {:?}",
                                (a, b),
                                (c, d)
                            );
                            // Resampling tags use each chunk's actual consumed/produced
                            // ratio, not a uniform nominal slope. Compare every mapped
                            // output frame with the fresh-cut timing contract instead.
                            let times = |snapshot: &Snapshot| {
                                snapshot
                                    .timing
                                    .iter()
                                    .flat_map(|segment| {
                                        assert!(
                                            segment.music_sec_per_frame.is_finite()
                                                && segment.music_sec_per_frame > 0.0
                                        );
                                        (0..segment.frames).map(move |frame| {
                                            segment.music_start_sec
                                                + frame as f64 * segment.music_sec_per_frame
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            };
                            let actual_times = times(&actual);
                            let expected_times = times(expected);
                            assert_eq!(actual_times.len(), expected_times.len());
                            assert!(
                                actual_times
                                    .iter()
                                    .zip(&expected_times)
                                    .all(|(a, b)| (a - b).abs() < 1e-10),
                                "{label}: per-frame clock differs"
                            );
                        }
                        bump_music_map_generation();
                        runtime.handle(StreamCommand::StopMusic {
                            asset_epoch: deadlib_audio_core::music_asset_epoch(),
                        });
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 84);
}
