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
    assert!(color(
        &state,
        Preferences::default().palette().resolved_sparse().rgba()
    ));
    assert!(!color(
        &state,
        Preferences::default().palette().dense().rgba()
    ));
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
    assert!(!color(
        &state,
        Preferences::default().palette().resolved_sparse().rgba()
    ));
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    ready(&mut reopened);
    assert_eq!(
        reopened.waveform_preferences(),
        state.waveform_preferences()
    );
    assert!(!color(
        &reopened,
        Preferences::default().palette().resolved_sparse().rgba()
    ));
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
    assert!(color(
        &state,
        Preferences::default().palette().dense().rgba()
    ));
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

#[test]
fn paginated_colour_editor_stages_cancels_and_saves_rgba_and_sparse_mode() {
    use deadsync_editor::waveform_palette::{Palette, SparseMode};
    let (dir, mut state, source) = setup(true);
    let path = dir.path().join("prefs.json");
    let before = fs::read(&path).unwrap();
    open(&mut state);
    press(&mut state, KeyCode::F2);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    assert_eq!(state.waveform_preferences().palette(), Palette::default());
    assert_eq!(fs::read(&path).unwrap(), before);
    open(&mut state);
    press(&mut state, KeyCode::F2);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::ArrowRight);
    assert!(has(&state, "Custom colour"));
    down(&mut state, 2);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftLeft, false);
    assert!(has(&state, "0.011"));
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::Digit0);
    down(&mut state, 5);
    press(&mut state, KeyCode::ArrowRight);
    down(&mut state, 4);
    press(&mut state, KeyCode::ArrowRight);
    assert!(has(&state, "Waveform colours 2/2"));
    down(&mut state, 3);
    press(&mut state, KeyCode::ArrowRight);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(state.waveform_preferences().palette(), Palette::default());
    press(&mut state, KeyCode::Escape);
    down(&mut state, 5);
    press(&mut state, KeyCode::Enter);
    // Saving colours must not implicitly enable a view configured as hidden.
    assert!(!state.waveform_preferences().show());
    assert!(!state.waveform_ready());
    let p = state.waveform_preferences().palette();
    assert_eq!(p.mode(), SparseMode::UniqueColor);
    assert!((p.dense().rgba()[0] - 0.011).abs() < 1e-6);
    assert_eq!(p.darken(), 0.0);
    assert_eq!(p.sparse().rgba()[0], 0.01);
    assert_eq!(p.background().rgba(), [0.01, 0.0, 0.0, 0.01]);
    assert_eq!(File::open(&path).unwrap().waveform().palette(), p);
    assert_eq!(File::open(&path).unwrap().audio().fade_in(), 0.125);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn semi_transparent_background_sparse_and_dense_are_disjoint_in_every_rendered_band() {
    use deadlib_present::actors::SizeSpec;
    use deadsync_editor::waveform_palette::{Color, SparseMode};
    use std::collections::BTreeMap;
    let (dir, mut state, _) = setup(false);
    ready(&mut state);
    let path = dir.path().join("prefs.json");
    let background = [1.0, 0.0, 0.0, 0.25];
    let dense = [0.0, 0.0, 1.0, 0.75];
    let sparse = [0.0, 1.0, 0.0, 0.5];
    let mut file = File::open(&path).unwrap();
    let mut settings = file.waveform();
    let mut p = settings.palette();
    p.set_background(Color::new(background).unwrap());
    p.set_dense(Color::new(dense).unwrap());
    p.set_sparse(Color::new(sparse).unwrap());
    p.set_mode(SparseMode::UniqueColor);
    settings.set_palette(p);
    settings.set_dense_scale(0.01).unwrap();
    file.set_waveform(settings);
    file.save().unwrap();
    state.configure_audio_preferences(&path).unwrap();
    ready(&mut state);
    for tint in [background, dense, sparse] {
        assert!(color(&state, tint), "{tint:?}");
    }
    let mut bands: BTreeMap<u32, Vec<(f32, f32)>> = BTreeMap::new();
    for actor in actors(&state) {
        if let Actor::Sprite {
            offset,
            size: [SizeSpec::Px(w), SizeSpec::Px(h)],
            scale,
            tint,
            ..
        } = actor
        {
            if ![background, dense, sparse].contains(&tint) {
                continue;
            }
            let width = if w == 0.0 { scale[0] } else { w * scale[0] };
            let height = if h == 0.0 { scale[1] } else { h * scale[1] };
            assert!(width > 0.0 && height > 0.0);
            assert!(offset[1] - height * 0.5 >= 105.0);
            assert!(offset[1] + height * 0.5 <= 395.0);
            bands
                .entry(offset[1].to_bits())
                .or_default()
                .push((offset[0] - width * 0.5, offset[0] + width * 0.5));
        }
    }
    assert_eq!(bands.len(), 290);
    for spans in bands.values_mut() {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!((spans.first().unwrap().0 - 331.0).abs() < 0.001);
        assert!((spans.last().unwrap().1 - 523.0).abs() < 0.001);
        for pair in spans.windows(2) {
            assert!(
                (pair[0].1 - pair[1].0).abs() < 0.001,
                "overlap or gap: {pair:?}"
            );
        }
    }
}
