use deadlib_audio_decode::{DecodeOptions, open_file};
use deadsync_editor::waveform::{Builder, Waveform};
use deadsync_theme_simply_love::editor_audio::{Job, LoadError, Loader};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn wait(job: &mut Job) -> Result<Waveform, LoadError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = job.poll() {
            return result;
        }
        assert!(Instant::now() < deadline, "waveform worker did not finish");
        thread::sleep(Duration::from_millis(1));
    }
}
fn wav(path: &Path, pcm: &[i16]) {
    let data = pcm.len() as u32 * 2;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + data).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(192000u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data.to_le_bytes());
    for sample in pcm {
        bytes.extend(sample.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}
#[test]
fn background_real_wav_matches_pcm_across_packets_and_preserves_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Été stéréo.wav");
    let pcm: Vec<_> = (0..8193)
        .flat_map(|n| {
            [
                ((n * 7919) % 65536 - 32768) as i16,
                ((n * 3907) % 65536 - 32768) as i16,
            ]
        })
        .collect();
    wav(&path, &pcm);
    let original = fs::read(&path).unwrap();
    let mut direct = Builder::new(2, 48000).unwrap();
    direct.push_i16(&pcm).unwrap();
    let direct = direct.finish().unwrap();
    let loader = Loader::new(2 * 1024 * 1024);
    let mut job = loader.start(path.clone()).unwrap();
    let actual = wait(&mut job).unwrap();
    assert!(job.poll().is_none());
    assert_eq!(actual.frames(), 8193);
    assert_eq!(actual.channels(), 2);
    assert_eq!(actual.sample_rate(), 48000);
    for channel in 0..2 {
        for (start, end) in [(0, 8193), (1, 4097), (4095, 4100), (8192, 8193)] {
            assert_eq!(
                actual.envelope(channel, start, end),
                direct.envelope(channel, start, end)
            );
        }
    }
    assert_eq!(fs::read(&path).unwrap(), original);
}
#[test]
fn cancellation_errors_and_budget_refusal_allow_next_load_without_stale_result() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.wav");
    wav(&path, &vec![100; 20000]);
    let loader = Loader::new(16);
    let mut job = loader.start(path.clone()).unwrap();
    job.cancel();
    assert!(matches!(wait(&mut job), Err(LoadError::Cancelled)));
    let mut job = loader.start(path.clone()).unwrap();
    assert!(
        matches!(wait(&mut job), Err(LoadError::Decode(message)) if message.contains("memory budget"))
    );
    let mut job = loader.start(dir.path().join("absent.wav")).unwrap();
    assert!(matches!(wait(&mut job), Err(LoadError::Decode(_))));
    // The slot is released before publication, even following an error.
    wav(&path, &[0, 0]);
    let mut job = loader.start(path).unwrap();
    assert_eq!(wait(&mut job).unwrap().frames(), 1);
}
#[test]
fn mp3_waveform_uses_the_same_untrimmed_info_header_timeline_as_playback() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../deadlib-audio-decode/tests/fixtures/timeline.mp3");
    fn count(path: &Path, options: DecodeOptions) -> usize {
        let mut source = open_file(path, options).unwrap();
        let mut packet = Vec::new();
        let mut frames = 0;
        while source.reader.read_dec_packet_into(&mut packet).unwrap() {
            frames += packet.len() / source.channels;
            packet.clear();
        }
        frames
    }
    let expected = count(&path, DecodeOptions::STEP_MANIA_TIMELINE);
    assert_ne!(expected, count(&path, DecodeOptions::default()));
    let mut job = Loader::new(4 * 1024 * 1024).start(path).unwrap();
    let wave = wait(&mut job).unwrap();
    assert_eq!(wave.frames(), expected);
    assert_eq!(wave.channels(), 2);
    assert_eq!(wave.sample_rate(), 44100);
    // The retained Info frame is silence in the playback timeline.
    let silence = wave.envelope(0, 0, 1152).unwrap();
    assert_eq!((silence.min, silence.max), (255, 255));
}

use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::{Actor, SizeSpec};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn ready(state: &mut State) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.waveform_ready() {
        state.update();
        assert!(
            Instant::now() < deadline,
            "native waveform did not become ready"
        );
        thread::sleep(Duration::from_millis(1));
    }
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn quads(state: &State) -> Vec<([f32; 2], f32, f32)> {
    let palette = deadsync_editor::waveform_palette::Palette::default();
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Sprite {
                offset,
                size: [SizeSpec::Px(w), SizeSpec::Px(h)],
                scale,
                tint,
                ..
            } if tint == palette.resolved_sparse().rgba() || tint == palette.dense().rgba() => {
                Some((
                    offset,
                    if w == 0.0 { scale[0] } else { w * scale[0] },
                    if h == 0.0 { scale[1] } else { h * scale[1] },
                ))
            }
            _ => None,
        })
        .collect()
}
#[test]
fn keyboard_waveform_uses_chart_audio_clips_geometry_and_hides_without_editing_song() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("chart.wav");
    wav(&audio, &vec![10000; 96000]);
    let original_audio = fs::read(&audio).unwrap();
    let source = "#VERSION:0.83;#TITLE:Wave;#MUSIC:missing.wav;#PREVIEW:also-missing.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.wav;#NOTES:1000;";
    let path = dir.path().join("song.ssc");
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    assert!(quads(&state).is_empty());
    press(&mut state, KeyCode::Enter);
    assert!(state.is_playing());
    press(&mut state, KeyCode::KeyW);
    assert!(
        state.is_playing(),
        "display toggle must preserve audio playback"
    );
    ready(&mut state);
    assert_eq!(state.waveform_frames(), Some(48000));
    let visible = quads(&state);
    assert!(!visible.is_empty());
    for (offset, width, height) in visible {
        assert!(width > 0.0 && width <= 96.0);
        assert!(offset[1] - height * 0.5 >= 105.0);
        assert!(offset[1] + height * 0.5 <= 395.0);
        assert!(offset[0] >= 331.0 && offset[0] <= 523.0);
    }
    press(&mut state, KeyCode::ArrowDown);
    state.update();
    assert!(state.waveform_ready());
    press(&mut state, KeyCode::KeyW);
    assert!(!state.waveform_ready());
    assert!(quads(&state).is_empty());
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read(&audio).unwrap(), original_audio);
}
#[test]
fn visible_waveform_peak_moves_with_song_offset_using_playback_clock() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("pulse.wav");
    let pcm: Vec<_> = (0..96000)
        .flat_map(|frame| {
            let peak = if (24000..36000).contains(&frame) {
                if frame % 2 == 0 { -32768 } else { 32767 }
            } else {
                0
            };
            [peak, 0]
        })
        .collect();
    wav(&audio, &pcm);
    let first_peak = |offset: f64| {
        let path = dir.path().join(format!("offset-{offset}.ssc"));
        fs::write(&path, format!("#VERSION:0.83;#MUSIC:pulse.wav;#OFFSET:{offset};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        press(&mut state, KeyCode::KeyW);
        ready(&mut state);
        quads(&state)
            .into_iter()
            .find(|(_, width, _)| *width > 10.0)
            .unwrap()
            .0[1]
    };
    let without_offset = first_peak(0.0);
    let with_offset = first_peak(0.5);
    // At 120 BPM, half a second is 48 rows/pixels in this editor view.
    assert!((with_offset - without_offset - 48.0).abs() < 0.01);
}

#[test]
fn unavailable_audio_reports_error_and_keyboard_toggle_retries_without_song_changes() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = "#VERSION:0.83;#MUSIC:recover.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::KeyW);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        state.update();
        let mut actors = Vec::new();
        state.actors(&mut actors);
        if actors.iter().any(|actor| matches!(actor, Actor::Text { content, .. } if content.contains("Cannot load waveform"))) { break; }
        assert!(
            Instant::now() < deadline,
            "missing waveform error was not surfaced"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(!state.waveform_ready());
    assert!(quads(&state).is_empty());
    wav(&dir.path().join("recover.wav"), &[0, 100, -100, 200]);
    press(&mut state, KeyCode::KeyW);
    press(&mut state, KeyCode::KeyW);
    ready(&mut state);
    assert_eq!(state.waveform_frames(), Some(2));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
}
