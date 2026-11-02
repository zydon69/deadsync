use super::*;
#[test]
fn decoded_pcm_retains_both_preview_ramps_across_partial_cut_and_loop() {
    let root = std::env::temp_dir().join(format!("preview-fades-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("constant.wav");
    let frames = 48000u32;
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
    std::fs::write(&path, bytes).unwrap();
    let decoded = load_and_resample_sfx(
        &path,
        OutputFormat {
            sample_rate_hz: 48000,
            channels: 2,
        },
        decode::DecodeOptions::STEP_MANIA_TIMELINE,
    )
    .unwrap();
    assert!(decoded.iter().all(|sample| *sample == 12000));
    for (start, total, fade_in, fade_out) in [
        (0, 4800, 960, 960),
        (12000, 4800, 960, 960),
        (12000, 2400, 1920, 1920),
        (12000, 2400, 4800, 4800),
    ] {
        let fade = preview_fade::PreviewFade::new(fade_in, fade_out, Some(total)).unwrap();
        let original = &decoded[start * 2..(start + total as usize) * 2];
        let mut pcm = original.to_vec();
        for (chunk, block) in pcm.chunks_mut(514).enumerate() {
            fade.apply(block, 2, (chunk * 257) as u64);
        }
        for (frame, samples) in pcm.chunks_exact(2).enumerate() {
            let frame = frame as i64;
            let tail = total as i64 - fade_out as i64;
            let gain = if frame > tail {
                (1. - (frame - tail) as f64 / fade_out as f64) as f32
            } else if frame < (fade_in as i64) {
                (frame as f64 / fade_in as f64) as f32
            } else {
                1.
            };
            let expected = (12000. * gain.clamp(0., 1.)).round() as i16;
            assert_eq!(
                samples,
                [expected, expected],
                "decoded cut start={start},frame={frame}"
            );
        }
        let offset = total / 3;
        let mut partial = original[offset as usize * 2..].to_vec();
        fade.apply(&mut partial, 2, offset);
        assert_eq!(partial, pcm[offset as usize * 2..]);
        let mut next_loop = original.to_vec();
        fade.apply(&mut next_loop, 2, 0);
        assert_eq!(next_loop, pcm);
    }
    std::fs::remove_dir_all(root).unwrap();
}
