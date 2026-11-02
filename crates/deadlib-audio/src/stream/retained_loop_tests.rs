use super::*;
use deadlib_audio_core::{
    CallbackClockSource, CallbackInfo, MixControls, OutputBufferMut, RenderState,
};

fn wav(path: &Path, value: i16) {
    let frames = 48_000u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&192_000u32.to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(frames * 4).to_le_bytes());
    for _ in 0..frames * 2 {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn preview_loops_keep_original_open_asset_after_unlink_and_replacement() {
    let _audio_guard = crate::test_audio_guard();
    // Include cut zero, a nonzero cut, and full-file EOF looping. Reading more
    // than the entire fixed PCM pool proves that preloaded blocks cannot hide
    // a later reopen/substitution of the replacement pathname.
    for (case, start, length) in [(0, 0.0, 0.03), (1, 0.5, 0.03), (2, 0.0, f64::INFINITY)] {
        let directory =
            std::env::temp_dir().join(format!("retained-preview-{}-{case}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("preview.wav");
        wav(&path, -3000);
        deadlib_audio_core::resume_music_track();
        deadlib_audio_core::reset_music_stream_clock_state();
        deadlib_audio_core::reset_music_target_gain();
        let generation = deadlib_audio_core::bump_music_map_generation();
        let asset_epoch = deadlib_audio_core::advance_music_asset_epoch();
        let (stream, handle) = deadlib_audio_core::music_transport(48_000, 2);
        let required_frames =
            stream.writer.capacity_blocks() * deadlib_audio_core::ring::MUSIC_BLOCK_FRAMES + 96_000;
        let mut renderer = RenderState::new(handle, Arc::new(MixControls::new()), 2);
        let mut runtime = MusicStreamRuntime::new(
            stream.writer,
            OutputFormat {
                sample_rate_hz: 48_000,
                channels: 2,
            },
        );
        runtime.handle(StreamCommand::PlayMusic {
            asset_epoch,
            decode_options: decode::DecodeOptions::STEP_MANIA_TIMELINE,
            path: path.clone(),
            cut: Cut {
                start_sec: start,
                length_sec: length,
                ..Default::default()
            },
            looping: true,
            rate: 1.0,
            preserve_pitch: false,
            generation,
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !runtime
            .decoder_status()
            .snapshot()
            .is_some_and(|s| s.render_ready_generation == Some(generation))
        {
            assert!(std::time::Instant::now() < deadline);
            thread::yield_now();
        }
        std::fs::remove_file(&path).unwrap();
        wav(&path, 6000);
        let mut original_frames = 0;
        while original_frames < required_frames {
            assert!(
                std::time::Instant::now() < deadline,
                "retained preview stopped before repeated loops"
            );
            let mut output = [0i16; 480];
            let total = deadlib_audio_core::music_total_frames();
            renderer.render(
                OutputBufferMut::I16(&mut output),
                CallbackInfo {
                    anchor_nanos: 1_000_000_000 + total * 1_000_000_000 / 48_000,
                    clock: CallbackClockSource::Instant,
                },
                std::iter::empty(),
            );
            assert!(
                output.iter().all(|v| *v <= 200),
                "loop substituted replacement pathname PCM"
            );
            original_frames += output
                .chunks_exact(2)
                .filter(|frame| frame[0] < -1000)
                .count();
            assert!(
                !runtime
                    .decoder_status()
                    .snapshot()
                    .is_some_and(|s| s.phase == lifecycle::DecoderPhase::Failed)
            );
            thread::yield_now();
        }
        assert_eq!(deadlib_audio_core::music_asset_epoch(), asset_epoch);
        drop(runtime);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
