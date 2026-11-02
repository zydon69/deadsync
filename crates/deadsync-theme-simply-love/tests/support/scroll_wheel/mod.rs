use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::time::{Duration, Instant};
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn state(mode: &str, reverse: bool) -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(&song,b"#VERSION:0.83;#TITLE:Wheel;#OFFSET:0;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"Scroll":{"SpacingMode":mode,"Reverse":reverse},"Zoom":{"ScrollInterpolationDuration":0},"KeyBinds":{"SnapToNone":[["F14"]],"OpenScrollSettings":[["F13"]],"Undo":[["F15"]],"Redo":[["F16"]],"New":[["F17"]],"ScrollZoom":[["LeftControl"]],"ScrollSpacing":[["LeftShift"]]}})).unwrap()).unwrap();
    let mut state = State::open_with_preferences(&song, 0, &prefs).unwrap();
    press(&mut state, KeyCode::F14);
    (root, state)
}
#[test]
fn actual_wheel_uses_time_rows_unknown_and_reverse_without_song_mutation() {
    for (mode, reverse, expected) in [
        ("ConstantTime", false, -0.25),
        ("ConstantTime", true, 0.25),
        ("ConstantRow", false, -0.5),
        ("99", false, -0.5),
    ] {
        let (_root, mut state) = state(mode, reverse);
        let bytes = state.workspace().file().session.serialize().unwrap();
        let initial = state.chart_focus_time().unwrap();
        let now = Instant::now();
        state.update_with_time(now);
        assert!(state.wheel_lines(1.));
        state.update_with_time(now + Duration::from_millis(16));
        assert!(
            (state.chart_focus_time().unwrap() - initial - expected).abs() < 1e-10,
            "{mode} {reverse}"
        );
        assert_eq!(state.workspace().file().session.serialize().unwrap(), bytes);
        assert!(!state.workspace().file().session.is_dirty());
    }
}
#[test]
fn physical_modifiers_priority_release_and_unknown_capture_are_real() {
    let (_root, mut state) = state("99", false);
    let now = Instant::now();
    state.update_with_time(now);
    let initial = state.chart_focus_time().unwrap();
    key(&mut state, KeyCode::ShiftRight, true);
    assert!(state.wheel_lines(1.));
    state.update_with_time(now + Duration::from_millis(16));
    assert_eq!(state.spacing_zoom(), 1.);
    assert_eq!(state.chart_focus_time().unwrap(), initial);
    key(&mut state, KeyCode::ControlLeft, true);
    key(&mut state, KeyCode::ControlRight, true);
    key(&mut state, KeyCode::ControlLeft, false);
    assert!(state.wheel_lines(1.));
    state.update_with_time(now + Duration::from_millis(32));
    assert!(state.spacing_zoom() > 1.);
    assert_eq!(state.chart_focus_time().unwrap(), initial);
    key(&mut state, KeyCode::ControlRight, false);
    key(&mut state, KeyCode::ShiftRight, false);
    state.release_input();
    assert!(state.wheel_lines(1.));
    state.release_input();
    state.update_with_time(now + Duration::from_millis(48));
    assert_eq!(state.chart_focus_time().unwrap(), initial);
}
#[test]
fn real_precise_input_aggregation_is_retina_invariant_and_modal_guarded() {
    let (_a, mut a) = state("ConstantTime", false);
    let (_b, mut b) = state("ConstantTime", false);
    let now = Instant::now();
    a.update_with_time(now);
    b.update_with_time(now);
    let start = a.chart_focus_time().unwrap();
    for delta in [0.5, 1.5, -0.25] {
        assert!(a.wheel_pixels(delta, 1.));
        assert!(b.wheel_pixels(delta * 2., 2.));
    }
    a.update_with_time(now + Duration::from_millis(16));
    b.update_with_time(now + Duration::from_millis(16));
    assert_eq!(a.chart_focus_time().unwrap(), b.chart_focus_time().unwrap());
    assert!(
        (a.chart_focus_time().unwrap() - start + 0.25 * f64::from((1.75_f64 / 40.) as f32)).abs()
            < 1e-12
    );
    press(&mut a, KeyCode::F1);
    let before = a.chart_focus_time().unwrap();
    assert!(!a.wheel_lines(1.));
    a.update_with_time(now + Duration::from_millis(32));
    assert_eq!(a.chart_focus_time().unwrap(), before);
}
#[test]
fn keyboard_modifier_inspector_captures_saves_and_undoes_actual_queries() {
    let (root, mut state) = state("ConstantTime", false);
    let original = state.workspace().file().session.serialize().unwrap();
    press(&mut state, KeyCode::F13);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::KeyA);
    press(&mut state, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("prefs.json")).unwrap()).unwrap();
    assert_eq!(json["KeyBinds"]["ScrollZoom"], serde_json::json!([["A"]]));
    let now = Instant::now();
    state.update_with_time(now);
    let before = state.chart_focus_time().unwrap();
    key(&mut state, KeyCode::KeyA, true);
    assert!(state.wheel_lines(1.));
    state.update_with_time(now + Duration::from_millis(16));
    assert!(state.spacing_zoom() > 1.);
    assert_eq!(state.chart_focus_time().unwrap(), before);
    key(&mut state, KeyCode::KeyA, false);
    press(&mut state, KeyCode::F15);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("prefs.json")).unwrap()).unwrap();
    assert_eq!(
        json["KeyBinds"]["ScrollZoom"],
        serde_json::json!([["LeftControl"]])
    );
    press(&mut state, KeyCode::F16);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("prefs.json")).unwrap()).unwrap();
    assert_eq!(json["KeyBinds"]["ScrollZoom"], serde_json::json!([["A"]]));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn live_spacing_survives_reverse_undo_redo_new_song_and_close() {
    let (root, _old) = state("ConstantTime", false);
    let prefs = root.path().join("prefs.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    json["Zoom"]["ScrollInterpolationDuration"] = serde_json::json!(0.2);
    std::fs::write(&prefs, serde_json::to_vec(&json).unwrap()).unwrap();
    let mut state = State::open_with_preferences(&root.path().join("song.ssc"), 0, &prefs).unwrap();
    press(&mut state, KeyCode::F14);
    let now = Instant::now();
    state.update_with_time(now);
    key(&mut state, KeyCode::ShiftRight, true);
    state.wheel_lines(1.);
    state.update_with_time(now + Duration::from_millis(16));
    key(&mut state, KeyCode::ShiftRight, false);
    let intermediate = state.scroll_preferences().speed(0).unwrap();
    assert!(intermediate > 300. && intermediate < 360.);
    press(&mut state, KeyCode::F13);
    state.update_with_time(now + Duration::from_millis(32));
    press(&mut state, KeyCode::F12);
    for _ in 0..3 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(state.display_reversed());
    let saved = state.scroll_preferences().speed(0).unwrap();
    assert!(saved > intermediate);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::F15);
    assert!(!state.display_reversed());
    assert_eq!(state.scroll_preferences().speed(0), Some(saved));
    state.update_with_time(now + Duration::from_millis(250));
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
    press(&mut state, KeyCode::F16);
    assert!(state.display_reversed());
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
    press(&mut state, KeyCode::F17);
    assert!(state.workspace().file().existing_path().is_none());
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(
        disk["Scroll"]["TimeBasedPixelsPerSecond"].as_f64(),
        Some(360.)
    );
    assert!(!state.workspace().file().session.is_dirty());
    let _ = state.request_host_close();
}
#[test]
fn external_preference_conflict_keeps_live_speed_and_retry_is_explicit() {
    let (root, mut state) = state("ConstantTime", false);
    let prefs = root.path().join("prefs.json");
    let baseline = std::fs::read(&prefs).unwrap();
    let now = Instant::now();
    state.update_with_time(now);
    key(&mut state, KeyCode::ShiftRight, true);
    state.wheel_lines(1.);
    state.update_with_time(now + Duration::from_millis(16));
    key(&mut state, KeyCode::ShiftRight, false);
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
    let mut external: serde_json::Value = serde_json::from_slice(&baseline).unwrap();
    external["FutureExternal"] = serde_json::json!("protected");
    let external = serde_json::to_vec(&external).unwrap();
    std::fs::write(&prefs, &external).unwrap();
    press(&mut state, KeyCode::F13);
    assert_eq!(std::fs::read(&prefs).unwrap(), external);
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let texts = texts(&actors);
    let prefix = i18n::tr("Edition", "ApplicationPreferencesSaveFailed")
        .split("{error}")
        .next()
        .unwrap()
        .to_owned();
    assert!(texts.iter().any(|text| text.contains(&prefix)));
    press(&mut state, KeyCode::F1);
    press(&mut state, KeyCode::F1);
    assert_eq!(std::fs::read(&prefs).unwrap(), external);
    std::fs::write(&prefs, &baseline).unwrap();
    assert_eq!(std::fs::read(&prefs).unwrap(), baseline);
    press(&mut state, KeyCode::F13);
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(
        disk["Scroll"]["TimeBasedPixelsPerSecond"].as_f64(),
        Some(360.)
    );
    assert_eq!(state.scroll_preferences().speed(0), Some(360.));
}
fn texts(actors: &[deadlib_present::actors::Actor]) -> Vec<String> {
    use deadlib_present::actors::Actor;
    let mut result = Vec::new();
    for actor in actors {
        match actor {
            Actor::Text { content, .. } => result.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => result.extend(texts(children)),
            Actor::SharedFrame { children, .. } | Actor::SharedTransform { children, .. } => {
                result.extend(texts(children))
            }
            _ => {}
        }
    }
    result
}
#[test]
fn close_flushes_current_interpolation_value_before_releasing_host() {
    let (root, _old) = state("ConstantTime", false);
    let prefs = root.path().join("prefs.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    json["Zoom"]["ScrollInterpolationDuration"] = serde_json::json!(0.2);
    std::fs::write(&prefs, serde_json::to_vec(&json).unwrap()).unwrap();
    let mut state = State::open_with_preferences(&root.path().join("song.ssc"), 0, &prefs).unwrap();
    let now = Instant::now();
    state.update_with_time(now);
    key(&mut state, KeyCode::ShiftRight, true);
    state.wheel_lines(1.);
    state.update_with_time(now + Duration::from_millis(16));
    key(&mut state, KeyCode::ShiftRight, false);
    let live = state.scroll_preferences().speed(0).unwrap();
    assert!(live > 300. && live < 360.);
    assert!(state.request_host_close());
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(
        disk["Scroll"]["TimeBasedPixelsPerSecond"].as_f64(),
        Some(live)
    );
    state.update_with_time(now + Duration::from_secs(1));
    assert_eq!(state.scroll_preferences().speed(0), Some(live));
}

#[test]
fn chart_playback_discards_old_wheel_interpolation_and_admits_new_detents() {
    for stop in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("song.ssc");
        std::fs::write(root.path().join("music.ogg"), b"command-only fixture").unwrap();
        std::fs::write(&song,b"#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#MUSIC:music.ogg;#OFFSET:0;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let prefs = root.path().join("prefs.json");
        std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"Scroll":{"SpacingMode":"ConstantTime","StopPlaybackWhenScrolling":stop},"Zoom":{"ScrollInterpolationDuration":1},"KeyBinds":{"SnapToNone":[["F14"]]}})).unwrap()).unwrap();
        let mut s = State::open_with_preferences(&song, 0, &prefs).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|command| match command {
                deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => {
                    Some(*owner)
                }
                _ => None,
            })
            .expect("editor scope");
        let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
        ack.editor.owner = owner;
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        s.update_audio(ack);
        s.take_audio_commands();
        press(&mut s, KeyCode::F14);
        let now = Instant::now();
        s.update_with_time(now);
        assert!(s.wheel_lines(-1.));
        s.update_with_time(now + Duration::from_millis(16));
        press(&mut s, KeyCode::Enter);
        assert!(s.is_playing());
        s.take_audio_commands();
        s.update_with_time(now + Duration::from_millis(32));
        assert!(s.is_playing());
        assert!(
            s.take_audio_commands().is_empty(),
            "old interpolation must not seek or stop transport"
        );
        assert!(s.wheel_lines(-1.));
        s.update_with_time(now + Duration::from_millis(48));
        assert_eq!(s.is_playing(), !stop);
        assert!(
            !s.take_audio_commands().is_empty(),
            "new playing detent still seeks or stops immediately"
        );
    }
}

#[test]
fn source_measure_and_labels_keep_fractional_negative_focus_and_offset_en_fr() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(root.path().join("music.ogg"), b"command-only fixture").unwrap();
    std::fs::write(&song,b"#MUSIC:music.ogg;#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#OFFSET:0.25;#BPMS:0=120;#LABELS:0=first,4=next,8=last;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"Scroll":{"SpacingMode":"ConstantTime"},"Zoom":{"ScrollInterpolationDuration":0},"KeyBinds":{"SnapToNone":[["F14"]],"MoveToPreviousMeasure":[["F15"]],"MoveToNextMeasure":[["F16"]],"MoveToPreviousLabel":[["F17"]],"MoveToChartEnd":[["F18"]]}})).unwrap()).unwrap();
    let mut s = State::open_with_preferences(&song, 0, &prefs).unwrap();
    // Geometry/navigation works without an audio device, after its owned scope
    // has received the same initial output lease acknowledgement as a real host.
    let owner = s
        .take_audio_commands()
        .iter()
        .find_map(|command| match command {
            deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => Some(*owner),
            _ => None,
        })
        .unwrap();
    let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
    ack.editor.owner = owner;
    ack.editor.output.available = true;
    ack.editor.output.leased = true;
    ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
    ack.editor.output.requested_period = Some(512);
    ack.editor.output.negotiated_period = Some(512);
    s.update_audio(ack);
    s.take_audio_commands();
    press(&mut s, KeyCode::F14);
    let now = Instant::now();
    s.update_with_time(now);
    assert!(s.wheel_lines(-1.0 / 48.0));
    s.update_with_time(now + Duration::from_millis(16));
    let initial = s.chart_focus_time().unwrap();
    press(&mut s, KeyCode::F16);
    assert!((s.chart_focus_time().unwrap() - initial - 2.0).abs() < 1e-8);
    press(&mut s, KeyCode::F15);
    assert!((s.chart_focus_time().unwrap() - initial).abs() < 1e-8);
    press(&mut s, KeyCode::F15);
    assert!(
        (s.chart_focus_time().unwrap() - initial + 2.0).abs() < 1e-8,
        "negative focus is retained"
    );
    press(&mut s, KeyCode::F16);
    press(&mut s, KeyCode::F17);
    assert!(
        (s.chart_focus_time().unwrap() + 0.25).abs() < 1e-8,
        "strict predecessor of fractional positive row is label zero, without looping to last"
    );
    press(&mut s, KeyCode::F18);
    assert_eq!(s.workspace().row(), 384);
    assert!((s.chart_focus_time().unwrap() - 3.75).abs() < 1e-8);
    press(&mut s, KeyCode::Enter);
    assert!(s.is_playing());
    s.take_audio_commands();
    press(&mut s, KeyCode::F15);
    assert!(s.is_playing());
    assert!(
        s.take_audio_commands().iter().any(|command| matches!(
            command,
            deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic { .. }
        )),
        "measure navigation seeks retained chart transport"
    );
    assert!(!s.workspace().file().session.is_dirty());
}
