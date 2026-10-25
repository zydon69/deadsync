use deadsync_editor::waveform::{Builder, Waveform};

fn build(channels: usize, pcm: &[i16]) -> Waveform {
    let mut builder = Builder::new(channels, 48000).unwrap();
    builder.push_i16(pcm).unwrap();
    builder.finish().unwrap()
}
fn quantize(sample: i16) -> u16 {
    (511.0 * (f32::from(sample) / 32768.0 + 1.0) * 0.5) as u16
}
#[test]
fn original_sample_quantization_and_distance_keep_stereo_channels_separate() {
    // SoundMipMap.FillMipMapWorker uses a 0.5 initial previous value and
    // truncates pixel coordinates and distances, rather than rounding them.
    let wave = build(2, &[-32768, 0, 32767, 16384]);
    assert_eq!(wave.channels(), 2);
    assert_eq!(wave.sample_rate(), 48000);
    assert_eq!(wave.frames(), 2);
    let left = wave.envelope(0, 0, 1).unwrap();
    assert_eq!((left.min, left.max, left.mean_distance), (0, 0, 766.0));
    let right = wave.envelope(1, 0, 1).unwrap();
    assert_eq!(
        (right.min, right.max, right.mean_distance),
        (255, 255, 255.0)
    );
    let left = wave.envelope(0, 0, 2).unwrap();
    assert_eq!((left.min, left.max, left.mean_distance), (0, 510, 894.0));
    let right = wave.envelope(1, 0, 2).unwrap();
    assert_eq!(
        (right.min, right.max, right.mean_distance),
        (255, 383, 255.0)
    );
}
#[test]
fn every_zoom_interval_retains_extrema_including_second_child_and_odd_tail() {
    let pcm: Vec<_> = (0..137)
        .flat_map(|n| {
            [
                ((n * 7919) % 65536 - 32768) as i16,
                ((n * 3907) % 65536 - 32768) as i16,
            ]
        })
        .collect();
    let wave = build(2, &pcm);
    for start in 0..137 {
        for end in start + 1..=137 {
            for channel in 0..2 {
                let values: Vec<_> = (start..end)
                    .map(|frame| quantize(pcm[frame * 2 + channel]))
                    .collect();
                let actual = wave.envelope(channel, start, end).unwrap();
                assert_eq!(actual.min, *values.iter().min().unwrap(), "{start}..{end}");
                assert_eq!(actual.max, *values.iter().max().unwrap(), "{start}..{end}");
                assert!((0.0..=1022.0).contains(&actual.mean_distance));
            }
        }
    }
}
#[test]
fn decoder_packet_boundaries_do_not_change_waveform_or_distance() {
    let pcm: Vec<_> = (0..43)
        .flat_map(|n| [n * 700 - 16000, 16000 - n * 500])
        .collect();
    let whole = build(2, &pcm);
    let mut builder = Builder::new(2, 48000).unwrap();
    for packet in pcm.chunks(6) {
        builder.push_i16(packet).unwrap();
    }
    let split = builder.finish().unwrap();
    assert_eq!(split.duration_seconds(), 43.0 / 48000.0);
    for channel in 0..2 {
        for start in 0..43 {
            for end in start + 1..=43 {
                assert_eq!(
                    whole.envelope(channel, start, end),
                    split.envelope(channel, start, end)
                );
            }
        }
    }
}
#[test]
fn empty_invalid_and_clipped_ranges_are_explicit_and_bad_packet_is_atomic() {
    assert!(Builder::new(0, 48000).is_err());
    assert!(Builder::new(33, 48000).is_err());
    assert!(Builder::new(2, 0).is_err());
    let mut builder = Builder::new(2, 48000).unwrap();
    assert!(builder.push_i16(&[1, 2, 3]).is_err());
    builder.push_i16(&[0, 32767]).unwrap();
    let wave = builder.finish().unwrap();
    assert_eq!(wave.frames(), 1);
    assert_eq!(wave.envelope(0, 0, usize::MAX), wave.envelope(0, 0, 1));
    assert!(wave.envelope(0, 0, 0).is_none());
    assert!(wave.envelope(0, 1, usize::MAX).is_none());
    assert!(wave.envelope(2, 0, 1).is_none());
    assert!(wave.envelope(0, 1, 0).is_none());
    let empty = build(1, &[]);
    assert_eq!(empty.frames(), 0);
    assert_eq!(empty.duration_seconds(), 0.0);
    assert!(empty.envelope(0, 0, 1).is_none());
}
