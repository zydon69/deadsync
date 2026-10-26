use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|text| text.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn finish(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while s.is_generating() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn marker_song() -> Vec<u8> {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
        history::Session,
    };
    let mut session=Session::new(Document::parse(std::sync::Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000;".as_slice()),Format::Ssc).unwrap());
    ChartData {
        patterns: Some(std::collections::BTreeMap::from([(
            0,
            Definition {
                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                length: 96,
                ..Default::default()
            },
        )])),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    session.serialize().unwrap()
}
fn open_marker_profiles(s: &mut State) {
    press(s, KeyCode::F6);
    for _ in 0..12 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
}
fn open_movement(s: &mut State) {
    open_marker_profiles(s);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::KeyE);
}
fn number(s: &mut State, value: &str) {
    press(s, KeyCode::Delete);
    for c in value.chars() {
        press(
            s,
            match c {
                '0' => KeyCode::Digit0,
                '1' => KeyCode::Digit1,
                '2' => KeyCode::Digit2,
                '3' => KeyCode::Digit3,
                '4' => KeyCode::Digit4,
                '5' => KeyCode::Digit5,
                '6' => KeyCode::Digit6,
                '7' => KeyCode::Digit7,
                '8' => KeyCode::Digit8,
                '9' => KeyCode::Digit9,
                _ => panic!(),
            },
        );
    }
}
fn stage_speed(s: &mut State) {
    press(s, KeyCode::ArrowRight);
    press(s, KeyCode::ArrowDown);
    number(s, "140");
    press(s, KeyCode::ArrowDown);
    number(s, "190");
    for _ in 0..3 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
#[test]
fn bpm_helper_stages_exact_intervals_and_saves_wrapper_metadata_without_changing_song() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open_movement(&mut s);
    press(&mut s, KeyCode::F4);
    assert!(has(&s, "Note subdivision: 1/16"));
    assert!(has(&s, "Minimum BPM: 125"));
    assert!(has(&s, "Maximum BPM: 170"));
    stage_speed(&mut s);
    assert!(has(&s, "Travel-speed intervals staged"));
    assert!(!profiles.exists());
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let profile = loaded.profiles.values().next().unwrap().profile();
    let speed = deadsync_editor::generation::profiles::TravelSpeed {
        subdivision_index: 4,
        min_bpm: 140,
        max_bpm: 190,
    };
    assert_eq!(profile.travel_speed(), Some(speed));
    let deadsync_editor::generation::profiles::Config::Performed(c) = profile.config() else {
        panic!()
    };
    assert_eq!(
        (
            c.step_tightening.speed_min_time_seconds,
            c.step_tightening.speed_max_time_seconds
        ),
        speed.intervals().unwrap()
    );
    assert_eq!(c.step_tightening.speed_tightening_enabled, Some(false));
    let saved_path = loaded.profiles.values().next().unwrap().path().to_owned();
    // Update the same custom profile in place after the copy is selected.
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F4);
    press(&mut s, KeyCode::ArrowDown);
    number(&mut s, "150");
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let updated = deadsync_editor::generation::profiles::ProfileFile::open(&saved_path).unwrap();
    assert_eq!(updated.profile().guid(), profile.guid());
    assert_eq!(updated.profile().travel_speed().unwrap().min_bpm, 150);
    assert_eq!(
        deadsync_editor::generation::profiles::Loaded::directory(&profiles)
            .unwrap()
            .profiles
            .len(),
        1
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn invalid_bpm_cancels_without_losing_other_staged_settings() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open_movement(&mut s);
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::F4);
    press(&mut s, KeyCode::ArrowDown);
    number(&mut s, "0");
    press(&mut s, KeyCode::ArrowDown);
    assert!(has(&s, "Enter an ordered BPM range"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Home);
    assert!(has(&s, "Maximum inward-facing proportion: 0.000000"));
    assert!(!profiles.exists());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn direct_seconds_edit_after_bpm_staging_is_the_final_saved_value() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open_movement(&mut s);
    press(&mut s, KeyCode::F4);
    stage_speed(&mut s);
    let config = deadsync_editor::generation::profiles::Config::Performed(
        deadsync_editor::generation::config::PerformedConfig::preset(
            deadsync_editor::generation::config::PerformedPreset::PatternBalanced,
        ),
    );
    let editor = deadsync_editor::generation::profile_controls::Editor::new(&config).unwrap();
    let index = editor
        .fields()
        .iter()
        .position(|f| f.name() == "SpeedMinTimeSeconds")
        .unwrap();
    press(&mut s, KeyCode::Home);
    for _ in 0..index {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Delete);
    press(&mut s, KeyCode::Digit0);
    press(&mut s, KeyCode::Period);
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    let profile = loaded.profiles.values().next().unwrap().profile();
    assert_eq!(profile.travel_speed().unwrap().min_bpm, 140);
    let deadsync_editor::generation::profiles::Config::Performed(c) = profile.config() else {
        panic!()
    };
    assert_eq!(c.step_tightening.speed_min_time_seconds, 0.1);
}
