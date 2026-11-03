//! Observe the existing decoder/callback boundary with a real short WAV.
//! These assertions describe current facts, not completed editor EOF parity.
//! One test owns process-global clocks; no audio device or sleeps are involved.
use deadlib_audio::stream::lifecycle::{DecoderPhase, DecoderSnapshot};
use deadlib_audio::stream::{Cut, MusicDecodeContext, OutputFormat, spawn_music_decoder_thread};
use deadlib_audio_core::{
    CallbackClockSource, CallbackInfo, MixControls, MusicMapSeg, OutputBufferMut, PlaybackPosMap,
    RenderState, activate_music_track, bump_music_map_generation, music_total_frames,
    music_track_active, music_transport, reset_music_stream_clock_state, reset_music_target_gain,
    stop_music_track,
};
use std::{path::PathBuf, sync::Arc};

const HZ: u32 = 48_000;
const SOURCE_FRAMES: usize = 480;
const CALLBACK_FRAMES: usize = 2048;

struct Wav(PathBuf);
impl Wav {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "deadlib-editor-lifecycle-{}.wav",
            std::process::id()
        ));
        let size = SOURCE_FRAMES as u32 * 2;
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(36 + size).to_le_bytes());
        data.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        data.extend_from_slice(&HZ.to_le_bytes());
        data.extend_from_slice(&(HZ * 2).to_le_bytes());
        data.extend_from_slice(b"\x02\0\x10\0data");
        data.extend_from_slice(&size.to_le_bytes());
        for _ in 0..SOURCE_FRAMES {
            data.extend_from_slice(&12_000_i16.to_le_bytes());
        }
        std::fs::write(&path, data).unwrap();
        Self(path)
    }
}
impl Drop for Wav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Observation {
    first: Vec<i16>,
    first_maps: Vec<MusicMapSeg>,
    second: Vec<i16>,
    second_maps: Vec<MusicMapSeg>,
    callback_frames: u64,
    active_after_eof: bool,
    decoder: DecoderSnapshot,
}

fn observe(path: PathBuf, start_sec: f64) -> Observation {
    reset_music_stream_clock_state();
    reset_music_target_gain();
    activate_music_track();
    let callback_start = music_total_frames();
    let generation = bump_music_map_generation();
    let (mut stream, render) = music_transport(HZ, 2);
    let worker = spawn_music_decoder_thread(
        path,
        Cut {
            start_sec,
            ..Cut::default()
        },
        false,
        1.0,
        true,
        stream.writer,
        MusicDecodeContext {
            decode_options: Default::default(),
            output: OutputFormat {
                sample_rate_hz: HZ,
                channels: 2,
            },
            generation,
        },
    );
    // Entire fixture plus pre-roll is below the producer pool watermark.
    // Joining proves producer termination; a quiet callback alone cannot do so.
    let status = worker.status_reader();
    let _writer = worker.thread.join().expect("decoder worker terminates");
    let mut renderer = RenderState::new(render, Arc::new(MixControls::new()), 2);
    let mut callback = |anchor| {
        let mut pcm = vec![i16::MIN; CALLBACK_FRAMES * 2];
        renderer.render(
            OutputBufferMut::I16(&mut pcm),
            CallbackInfo {
                anchor_nanos: anchor,
                clock: CallbackClockSource::Instant,
            },
            std::iter::empty(),
        );
        let mut maps = Vec::new();
        while let Some((tag, _asset_epoch, map)) = stream.played_map.pop() {
            assert_eq!(tag, generation);
            maps.push(map);
        }
        (pcm, maps)
    };
    let (first, first_maps) = callback(1_000_000_000);
    let (second, second_maps) =
        callback(1_000_000_000 + CALLBACK_FRAMES as u64 * 1_000_000_000 / u64::from(HZ));
    Observation {
        first,
        first_maps,
        second,
        second_maps,
        callback_frames: music_total_frames() - callback_start,
        active_after_eof: music_track_active(),
        decoder: status.snapshot(),
    }
}

#[test]
fn real_wav_negative_preroll_eof_and_beyond_eof_expose_existing_clock_contract() {
    let wav = Wav::new();
    let result = observe(wav.0.clone(), -0.005);
    assert_eq!(result.decoder.phase, DecoderPhase::Exhausted);
    assert!(result.decoder.opened_successfully);
    const LEAD: usize = 240;
    assert_eq!(&result.first[..LEAD * 2], vec![0; LEAD * 2]);
    assert_eq!(
        &result.first[LEAD * 2..(LEAD + SOURCE_FRAMES) * 2],
        vec![12_000; SOURCE_FRAMES * 2]
    );
    assert!(
        result.first[(LEAD + SOURCE_FRAMES) * 2..]
            .iter()
            .all(|sample| *sample == 0)
    );
    assert_eq!(
        result.first_maps.iter().map(|map| map.frames).sum::<i64>(),
        (LEAD + SOURCE_FRAMES) as i64
    );
    assert_eq!(result.first_maps[0].music_start_sec, -0.005);
    assert!(result.second.iter().all(|sample| *sample == 0));
    assert!(
        result.second_maps.is_empty(),
        "EOF silence produces no further decoded mappings"
    );
    assert_eq!(result.callback_frames, CALLBACK_FRAMES as u64 * 2);
    assert!(
        result.active_after_eof,
        "EOF does not itself issue an explicit track stop"
    );

    // Existing position maps extrapolate outside their retained decoded samples.
    // This does not prove each OS backend retains that map after normal EOF.
    let mut map = PlaybackPosMap::default();
    for segment in result.first_maps {
        map.insert(segment);
    }
    let (after, seconds_per_frame) = map.search((CALLBACK_FRAMES * 2) as f64).unwrap();
    assert!((after - (-0.005 + CALLBACK_FRAMES as f64 * 2.0 / f64::from(HZ))).abs() < 1e-10);
    assert!((seconds_per_frame - 1.0 / f64::from(HZ)).abs() < 1e-12);
    stop_music_track();
    assert!(!music_track_active());

    let beyond = observe(wav.0.clone(), 0.02);
    assert!(
        beyond
            .first
            .iter()
            .chain(&beyond.second)
            .all(|sample| *sample == 0)
    );
    assert!(beyond.first_maps.is_empty() && beyond.second_maps.is_empty());
    assert!(beyond.active_after_eof);
    assert_eq!(beyond.decoder.phase, DecoderPhase::Exhausted);
    assert!(beyond.decoder.opened_successfully);
    let failed = observe(wav.0.with_extension("missing"), 0.02);
    assert!(
        failed
            .first
            .iter()
            .chain(&failed.second)
            .all(|sample| *sample == 0)
    );
    assert!(failed.first_maps.is_empty() && failed.second_maps.is_empty());
    assert_eq!(failed.decoder.phase, DecoderPhase::Failed);
    assert!(!failed.decoder.opened_successfully);
    assert!(failed.decoder.failure.is_some());
    // PCM/mapping absence alone currently cannot distinguish successfully opening
    // beyond EOF from decoder-open failure. A typed acknowledgement is required.
    assert_eq!(failed.active_after_eof, beyond.active_after_eof);
    stop_music_track();
    retained_runtime_acknowledges_each_generation_and_keeps_stop_observable(&wav);
}

fn retained_runtime_acknowledges_each_generation_and_keeps_stop_observable(wav: &Wav) {
    use deadlib_audio::stream::{MusicStreamRuntime, StreamCommand};
    use std::time::{Duration, Instant};
    reset_music_stream_clock_state();
    let (stream, _render) = music_transport(HZ, 2);
    let mut runtime = MusicStreamRuntime::new(
        stream.writer,
        OutputFormat {
            sample_rate_hz: HZ,
            channels: 2,
        },
    );
    let monitor = runtime.decoder_status();
    assert!(monitor.snapshot().is_none());
    let generation = bump_music_map_generation();
    runtime.handle(StreamCommand::PlayMusic {
        asset_epoch:deadlib_audio_core::music_asset_epoch(),
decode_options: Default::default(),
        path: wav.0.clone(),
        cut: Cut::default(),
        looping: false,
        rate: 1.0,
        preserve_pitch: true,
        generation,
    });
    let wait = |generation, phase| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(snapshot) = monitor.snapshot()
                && snapshot.command_generation == generation
                && snapshot.phase == phase
            {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "decoder acknowledgement: {:?}",
                monitor.snapshot()
            );
            std::thread::yield_now();
        }
    };
    let ended = wait(generation, DecoderPhase::Exhausted);
    assert!(ended.opened_successfully);
    assert!(ended.final_music_seconds.is_some());
    // No callback has rendered: producer EOF must not be mistaken for heard EOF.
    assert!(!deadlib_audio_core::music_track_has_started());
    let seek_generation = bump_music_map_generation();
    runtime.handle(StreamCommand::SeekMusic {
        seconds: 0.02,
        generation: seek_generation,
    });
    let seek = wait(seek_generation, DecoderPhase::Exhausted);
    assert!(seek.opened_successfully);
    assert_eq!(seek.requested_start_seconds, 0.02);
    let rate_generation = bump_music_map_generation();
    runtime.handle(StreamCommand::SetMusicRate {
        rate: 0.25,
        generation: rate_generation,
    });
    assert!(wait(rate_generation, DecoderPhase::Exhausted).opened_successfully);
    runtime.handle(StreamCommand::StopMusic{asset_epoch:deadlib_audio_core::music_asset_epoch()});
    let stopped = wait(rate_generation, DecoderPhase::Stopped);
    assert_eq!(stopped.command_generation, rate_generation);
    assert!(!music_track_active());
    let failure_generation = bump_music_map_generation();
    runtime.handle(StreamCommand::PlayMusic {
        asset_epoch:deadlib_audio_core::music_asset_epoch(),
decode_options: Default::default(),
        path: wav.0.with_extension("absent"),
        cut: Cut::default(),
        looping: false,
        rate: 1.0,
        preserve_pitch: true,
        generation: failure_generation,
    });
    let failed = wait(failure_generation, DecoderPhase::Failed);
    assert!(!failed.opened_successfully);
    assert!(
        failed
            .failure
            .as_ref()
            .is_some_and(|detail| detail.len() <= 2048)
    );
    for command in 0..3 {
        let generation = bump_music_map_generation();
        runtime.handle(match command {
            0 => StreamCommand::SeekMusic {
                seconds: 0.0,
                generation,
            },
            1 => StreamCommand::SetMusicRate {
                rate: 2.0,
                generation,
            },
            _ => StreamCommand::SetPreservePitch {
                enabled: false,
                generation,
            },
        });
        let failed = wait(generation, DecoderPhase::Failed);
        assert!(!failed.opened_successfully);
    }
    runtime.handle(StreamCommand::StopMusic{asset_epoch:deadlib_audio_core::music_asset_epoch()});
    assert_eq!(monitor.snapshot().unwrap().phase, DecoderPhase::Stopped);
}
