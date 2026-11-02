use super::*;
use deadlib_audio_core::{
    CallbackClockSource, CallbackInfo, MixControls, MusicMapSeg, OutputBufferMut, RenderState,
};
use std::time::Instant;
fn wav(path: &Path) {
    let frames = 9600u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&48000u32.to_le_bytes());
    bytes.extend_from_slice(&192000u32.to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(frames * 4).to_le_bytes());
    for _ in 0..frames * 2 {
        bytes.extend_from_slice(&12000i16.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}
// Consume only spans acknowledged by the real RenderState. Underrun silence
// is never counted as source PCM; output offsets come from the actual map.
fn collect(
    path: &Path,
    hz: u32,
    cut: Cut,
    partial: u64,
    required: usize,
) -> (Vec<i16>, Vec<MusicMapSeg>) {
    let source = decode::prepared::PreparedAudioSource::load(
        path,
        decode::DecodeOptions::STEP_MANIA_TIMELINE,
        &AtomicBool::new(false),
    )
    .unwrap();
    deadlib_audio_core::resume_music_track();
    deadlib_audio_core::reset_music_stream_clock_state();
    deadlib_audio_core::reset_music_target_gain();
    deadlib_audio_core::clear_music_stream_clock_seed();
    let generation = deadlib_audio_core::bump_music_map_generation();
    let epoch = deadlib_audio_core::advance_music_asset_epoch();
    deadlib_audio_core::activate_music_track();
    let (stream, handle) = deadlib_audio_core::music_transport(hz, 2);
    let mut played = stream.played_map;
    let mut render = RenderState::new(handle, Arc::new(MixControls::new()), 2);
    let mut runtime = MusicStreamRuntime::new(
        stream.writer,
        OutputFormat {
            sample_rate_hz: hz,
            channels: 2,
        },
    );
    let phase = (partial > 0).then_some((
        cut.start_sec + partial as f64 / f64::from(hz),
        deadlib_audio_core::music_total_frames(),
    ));
    runtime.handle(StreamCommand::ReplaceEditorPrepared {
        source,
        path: path.into(),
        cut,
        looping: true,
        rate: 1.,
        preserve_pitch: false,
        generation,
        asset_epoch: epoch,
        follow_live_phase: false,
        preview_phase: phase,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !runtime
        .decoder_status()
        .snapshot()
        .is_some_and(|s| s.opened_successfully && s.render_ready_generation == Some(generation))
    {
        assert!(
            Instant::now() < deadline,
            "prepared decoder did not become genuinely ready"
        );
        thread::yield_now();
    }
    let mut pcm = Vec::new();
    let mut maps = Vec::new();
    while pcm.len() / 2 < required {
        assert!(Instant::now() < deadline, "no two actual preview loops");
        let before = deadlib_audio_core::music_total_frames();
        let mut output = [0i16; 480];
        render.render(
            OutputBufferMut::I16(&mut output),
            CallbackInfo {
                anchor_nanos: 1_000_000_000 + before * 1_000_000_000 / u64::from(hz),
                clock: CallbackClockSource::Instant,
            },
            std::iter::empty(),
        );
        let relative_before = before as i64 - deadlib_audio_core::music_track_start_frame() as i64;
        while let Some((actual_gen, actual_epoch, map)) = played.pop() {
            assert_eq!((actual_gen, actual_epoch), (generation, epoch));
            let offset = usize::try_from(map.stream_frame_start - relative_before).unwrap();
            let count = usize::try_from(map.frames).unwrap();
            assert!(offset + count <= 240);
            pcm.extend_from_slice(&output[offset * 2..(offset + count) * 2]);
            maps.push(map);
        }
        assert!(
            !runtime
                .decoder_status()
                .snapshot()
                .is_some_and(|s| s.phase == lifecycle::DecoderPhase::Failed)
        );
        thread::yield_now();
    }
    drop(runtime);
    deadlib_audio_core::stop_music_track();
    pcm.truncate(required * 2);
    (pcm, maps)
}
#[test]
fn actual_prepared_preview_fades_survive_two_loops_partial_reload_and_dedicated_eof() {
    let directory =
        std::env::temp_dir().join(format!("physical-preview-fades-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("constant.wav");
    wav(&path);
    // 48k traverses the direct/conversion worker hook; 24k traverses the real
    // resampler+processed-output hook. An unfaded real run is its PCM control.
    for hz in [48000, 24000] {
        for dedicated in [false, true] {
            for partial in [0, 300] {
                let total = if dedicated {
                    u64::from(hz) / 5
                } else {
                    u64::from(hz) / 20
                };
                let start = if dedicated { 0. } else { 0.05 };
                let required = (total * 3 - partial) as usize;
                let plain = Cut {
                    start_sec: start,
                    length_sec: if dedicated { f64::INFINITY } else { 0.05 },
                    ..Default::default()
                };
                let (base, base_maps) = collect(&path, hz, plain, partial, required);
                let faded = Cut {
                    fade_in_sec: total as f64 * 0.8 / f64::from(hz),
                    fade_out_sec: total as f64 * 0.8 / f64::from(hz),
                    ..plain
                };
                let (actual, maps) = collect(&path, hz, faded, partial, required);
                assert_eq!(actual.len(), base.len());
                assert!(!base_maps.is_empty());
                assert!(
                    (maps[0].music_start_sec - start - partial as f64 / f64::from(hz)).abs()
                        <= 1. / f64::from(hz)
                );
                let fade_in = (faded.fade_in_sec * f64::from(hz)) as u64;
                let fade_out = (faded.fade_out_sec * f64::from(hz)) as u64;
                for (index, (actual, base)) in
                    actual.chunks_exact(2).zip(base.chunks_exact(2)).enumerate()
                {
                    let frame = (index as u64 + partial) % total;
                    let tail = total as i64 - fade_out as i64;
                    // Verbatim source branch order and Fumen's double-then-float Lerp.
                    let gain = if frame as i64 > tail {
                        (1. - (frame as i64 - tail) as f64 / fade_out as f64) as f32
                    } else if frame < fade_in {
                        (frame as f64 / fade_in as f64) as f32
                    } else {
                        1.
                    };
                    for channel in 0..2 {
                        let expected =
                            (f32::from(base[channel]) * gain.clamp(0., 1.)).round() as i16;
                        assert_eq!(
                            actual[channel], expected,
                            "hz={hz},dedicated={dedicated},partial={partial},frame={frame},index={index}"
                        );
                    }
                }
                assert!(
                    maps.windows(2)
                        .filter(|pair| pair[1].music_start_sec < pair[0].music_start_sec)
                        .count()
                        >= 2,
                    "real mapping must cross two loop boundaries"
                );
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
