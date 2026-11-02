//! Compare prepared editor PCM against an independent fresh codec decode.
//! No output device, global transport clock, encoder, or generated codec stub.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

struct FixtureCopy {
    directory: PathBuf,
    path: PathBuf,
}
impl FixtureCopy {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../deadlib-audio-decode/tests/fixtures")
            .join(name);
        loop {
            let directory = std::env::temp_dir().join(format!(
                "prepared-codec-proof-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let path = directory.join(name);
                    fs::copy(&source, &path).unwrap();
                    return Self { directory, path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
    }
}
impl Drop for FixtureCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn baseline(
    path: &Path,
    options: decode::DecodeOptions,
) -> (Vec<i16>, usize, u32, lifecycle::DecodedCodec) {
    let mut opened = decode::open_file(path, options).unwrap();
    use lifecycle::DecodedCodec as C;
    let codec = match &opened.reader {
        decode::Reader::Wav(_) => C::Wav,
        decode::Reader::Flac(_) => C::Flac,
        decode::Reader::Mp3(_) => C::Mp3,
        decode::Reader::Ogg(_) => C::Vorbis,
        decode::Reader::Opus(_) => C::Opus,
    };
    let mut pcm = Vec::new();
    let mut packet = Vec::new();
    loop {
        packet.clear();
        let more = opened.reader.read_dec_packet_into(&mut packet).unwrap();
        pcm.extend_from_slice(&packet);
        if !more {
            break;
        }
    }
    assert!(
        pcm.iter().any(|sample| *sample != 0),
        "fixture must contain real non-silent PCM"
    );
    assert_eq!(pcm.len() % opened.channels, 0);
    (pcm, opened.channels, opened.sample_rate_hz, codec)
}
fn tail(reader: &mut EditorDecodeReader) -> Vec<i16> {
    let mut pcm = Vec::new();
    let mut packet = vec![123];
    loop {
        let more = reader.read_dec_packet_into(&mut packet).unwrap();
        pcm.extend_from_slice(&packet);
        if !more {
            assert!(packet.is_empty());
            break;
        }
    }
    pcm
}
#[test]
fn prepared_editor_all_real_codecs_equal_full_decode_and_exact_seeks_after_unlink() {
    use lifecycle::DecodedCodec as C;
    let fixtures = [
        ("seek.wav", C::Wav),
        ("seek.flac", C::Flac),
        ("seek.ogg", C::Vorbis),
        ("seek.mp3", C::Mp3),
        ("timeline.mp3", C::Mp3),
        ("seek.opus", C::Opus),
        ("seek-long.opus", C::Opus),
    ];
    let mut cases = 0;
    for (name, expected_codec) in fixtures {
        for options in [
            decode::DecodeOptions::default(),
            decode::DecodeOptions::STEP_MANIA_TIMELINE,
        ] {
            let fixture = FixtureCopy::new(name);
            let (expected, channels, hz, codec) = baseline(&fixture.path, options);
            assert_eq!(codec, expected_codec, "{name}: real decoder codec");
            let streaming = EditorDecodeReader::open(&fixture.path, options, None).unwrap();
            assert_eq!(
                streaming.reader.is_streaming_vorbis(),
                codec == C::Vorbis,
                "{name}: streaming preroll policy"
            );
            drop(streaming);
            let source =
                PreparedAudioSource::load(&fixture.path, options, &AtomicBool::new(false)).unwrap();
            let frames = (expected.len() / channels) as u64;
            assert_eq!(source.frames(), frames, "{name}: whole-source frame count");
            assert_eq!(source.channels, channels);
            assert_eq!(source.sample_rate_hz, hz);
            fs::remove_file(&fixture.path).unwrap();
            assert!(!fixture.path.exists());
            // Open must use retained PCM; the configured file no longer exists.
            let opened = EditorDecodeReader::open(&fixture.path, options, Some(&source)).unwrap();
            assert_eq!(opened.channels, channels);
            assert_eq!(opened.sample_rate_hz, hz);
            let mut reader = opened.reader;
            assert_eq!(reader.codec(), codec, "{name}: codec retained after unlink");
            assert!(
                !reader.is_streaming_vorbis(),
                "prepared Vorbis is exact PCM and must not apply streaming preroll"
            );
            assert_eq!(tail(&mut reader), expected, "{name}: complete PCM");
            assert_eq!(reader.current_frame(), frames);
            let independent =
                EditorDecodeReader::open(&fixture.path, options, Some(&source)).unwrap();
            assert_eq!(independent.reader.current_frame(), 0);
            // Include subpacket and packet boundaries, reverse seeks after EOF,
            // exact EOF, beyond EOF clamping, then return to the first frame.
            for frame in [
                0,
                1,
                17,
                1023,
                1024,
                frames / 3,
                frames.saturating_sub(1),
                frames,
                frames.saturating_add(37),
                u64::MAX,
                0,
            ] {
                reader.seek_frame(frame).unwrap();
                let clamped = frame.min(frames);
                assert_eq!(
                    reader.current_frame(),
                    clamped,
                    "{name}: exact seek {frame}"
                );
                assert_eq!(
                    tail(&mut reader),
                    expected[clamped as usize * channels..],
                    "{name}: PCM suffix at frame {frame}, {options:?}"
                );
                assert_eq!(reader.current_frame(), frames);
                assert_eq!(
                    independent.reader.current_frame(),
                    0,
                    "{name}: independent reader cursor"
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 14);
}
