use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{audio_preferences::File, waveform_preferences::Preferences};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs, thread,
    time::{Duration, Instant},
};
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn open(state: &mut State) {
    key(state, KeyCode::ShiftLeft, true);
    press(state, KeyCode::KeyW);
    key(state, KeyCode::ShiftLeft, false);
}
fn down(state: &mut State, n: usize) {
    for _ in 0..n {
        press(state, KeyCode::ArrowDown);
    }
}
fn actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
}
fn has(state: &State, text: &str) -> bool {
    actors(state)
        .iter()
        .any(|actor| matches!(actor, Actor::Text { content, .. } if content.contains(text)))
}
fn color(state: &State, tint: [f32; 4]) -> bool {
    actors(state)
        .iter()
        .any(|actor| matches!(actor, Actor::Sprite { tint: actual, .. } if *actual==tint))
}
fn ready(state: &mut State) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.waveform_ready() {
        state.update();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
fn setup(hidden: bool) -> (tempfile::TempDir, State, String) {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let source =
        "#VERSION:0.83;#MUSIC:wave.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
            .to_string();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &source).unwrap();
    let pcm: Vec<i16> = (0..48000)
        .flat_map(|n| [if n % 2 == 0 { -30000 } else { 30000 }, 0])
        .collect();
    let bytes = pcm.len() as u32 * 2;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + bytes).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(48000u32.to_le_bytes());
    wav.extend(192000u32.to_le_bytes());
    wav.extend(4u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(bytes.to_le_bytes());
    for sample in pcm {
        wav.extend(sample.to_le_bytes());
    }
    fs::write(dir.path().join("wave.wav"), wav).unwrap();
    let prefs = dir.path().join("prefs.json");
    let mut file = File::open(&prefs).unwrap();
    let mut audio = file.audio();
    audio.set_fade_in(0.125).unwrap();
    file.set_audio(audio);
    let mut settings = file.waveform();
    settings.set_show(!hidden);
    file.set_waveform(settings);
    file.save().unwrap();
    let mut state = State::open(&path, 0).unwrap();
    state.configure_audio_preferences(&prefs).unwrap();
    (dir, state, source)
}
#[test]
fn keyboard_waveform_settings_save_reload_render_scales_and_preserve_audio_and_song() {
    let (dir, mut state, source) = setup(true);
    let path = dir.path().join("prefs.json");
    let before = fs::read(&path).unwrap();
    open(&mut state);
    press(&mut state, KeyCode::Enter);
    down(&mut state, 2);
    press(&mut state, KeyCode::ArrowLeft);
    key(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftLeft, false);
    assert!(has(&state, "89.1"));
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::Digit0);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!state.waveform_ready());
    down(&mut state, 2);
    press(&mut state, KeyCode::Enter);
    ready(&mut state);
    assert!((state.waveform_preferences().channel_width() - 0.891).abs() < 1e-12);
    assert_eq!(state.waveform_preferences().dense_scale(), 0.0);
    assert!(color(&state, [0.0, 0.35, 0.164, 0.35]));
    assert!(!color(&state, [0.0, 0.389, 0.183, 0.5]));
    assert_eq!(File::open(&path).unwrap().audio().fade_in(), 0.125);
    // A zero width hides all waveform geometry without discarding decoded data.
    let frames = state.waveform_frames();
    open(&mut state);
    down(&mut state, 2);
    press(&mut state, KeyCode::Digit0);
    down(&mut state, 3);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.waveform_frames(), frames);
    assert!(state.waveform_ready());
    assert!(!color(&state, [0.0, 0.35, 0.164, 0.35]));
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    ready(&mut reopened);
    assert_eq!(
        reopened.waveform_preferences(),
        state.waveform_preferences()
    );
    assert!(!color(&reopened, [0.0, 0.35, 0.164, 0.35]));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn cancel_defaults_and_session_toggle_have_separate_persistence_semantics() {
    let (dir, mut state, _) = setup(true);
    let path = dir.path().join("prefs.json");
    let before = fs::read(&path).unwrap();
    open(&mut state);
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Escape);
    assert!(!state.waveform_preferences().show());
    assert_eq!(fs::read(&path).unwrap(), before);
    open(&mut state);
    down(&mut state, 4);
    press(&mut state, KeyCode::Enter);
    assert_eq!(fs::read(&path).unwrap(), before);
    down(&mut state, 1);
    press(&mut state, KeyCode::Enter);
    ready(&mut state);
    assert_eq!(state.waveform_preferences(), Preferences::default());
    assert!(color(&state, [0.0, 0.389, 0.183, 0.5]));
    let saved = fs::read(&path).unwrap();
    press(&mut state, KeyCode::KeyW);
    assert!(!state.waveform_ready());
    assert_eq!(fs::read(&path).unwrap(), saved);
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    ready(&mut reopened);
    assert_eq!(reopened.waveform_preferences(), Preferences::default());
}
#[test]
fn disabling_decoding_clears_data_and_blocks_session_toggle_until_reenabled() {
    let (dir, mut state, _) = setup(false);
    ready(&mut state);
    open(&mut state);
    down(&mut state, 1);
    press(&mut state, KeyCode::Enter);
    down(&mut state, 4);
    press(&mut state, KeyCode::Enter);
    assert!(!state.waveform_ready());
    assert_eq!(state.waveform_frames(), None);
    press(&mut state, KeyCode::KeyW);
    assert!(has(&state, "Waveform decoding is disabled"));
    assert!(
        !File::open(&dir.path().join("prefs.json"))
            .unwrap()
            .waveform()
            .enabled()
    );
    open(&mut state);
    down(&mut state, 1);
    press(&mut state, KeyCode::Home);
    down(&mut state, 4);
    press(&mut state, KeyCode::Enter);
    ready(&mut state);
}
#[test]
fn external_and_future_preferences_refuse_writes_and_keep_reviewable_draft() {
    for future in [false, true] {
        let (dir, mut state, source) = setup(true);
        let path = dir.path().join("prefs.json");
        let external = if future {
            r#"{"Version":2}"#
        } else {
            r#"{"Version":1,"Future":true}"#
        };
        if future {
            fs::write(&path, external).unwrap();
            assert!(state.configure_audio_preferences(&path).is_err());
        }
        open(&mut state);
        press(&mut state, KeyCode::Enter);
        if !future {
            fs::write(&path, external).unwrap();
        }
        down(&mut state, 5);
        press(&mut state, KeyCode::Enter);
        assert!(!state.waveform_preferences().show());
        assert!(has(&state, "Show when the editor opens: Yes"));
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
        press(&mut state, KeyCode::Escape);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source.as_bytes()
        );
    }
}
