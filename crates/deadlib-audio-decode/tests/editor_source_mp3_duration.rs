//! Source FMOD DEFAULT may overestimate decoded sample allocation. Preserve
//! decoded EOF duration, including real coded silence, rather than that estimate.
use deadlib_audio_decode::{
    DecodeOptions, file_length_seconds, open_file, prepared::PreparedAudioSource,
};
use std::{path::Path, sync::atomic::AtomicBool};

#[test]
fn source_short_info_mp3_metadata_and_prepared_pcm_keep_8064_decoded_frames() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-source-short-info.mp3");
    let options = DecodeOptions::STEP_MANIA_TIMELINE;
    let mut opened = open_file(&path, options).unwrap();
    assert_eq!((opened.channels, opened.sample_rate_hz), (2, 44_100));
    assert_eq!(opened.frames_total_hint, Some(8_064));
    assert_eq!(
        file_length_seconds(&path, options).unwrap().to_bits(),
        ((8_064f64 / 44_100.) as f32).to_bits()
    );
    let mut pcm = Vec::new();
    let mut packet = Vec::new();
    while opened.reader.read_dec_packet_into(&mut packet).unwrap() {
        pcm.extend_from_slice(&packet);
    }
    assert_eq!(pcm.len() / opened.channels, 8_064);
    let last_nonzero = pcm
        .chunks_exact(opened.channels)
        .rposition(|frame| frame.iter().any(|sample| *sample != 0))
        .unwrap();
    assert_eq!(last_nonzero, 6_959);
    // Real decoded silence remains part of the source timeline. Do not trim
    // to the last nonzero frame, or extend to DEFAULT's 17_280-frame allocation.
    assert_eq!(8_064 - (last_nonzero + 1), 1_104);
    let prepared =
        PreparedAudioSource::load_editor(&path, options, &AtomicBool::new(false)).unwrap();
    assert_eq!(prepared.frames(), 8_064);
    assert_eq!(
        prepared.editor_samples().unwrap().len(),
        8_064 * opened.channels
    );
}
