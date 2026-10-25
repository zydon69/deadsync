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
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
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
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::KeyE);
}
#[test]
fn keyboard_edits_all_interpretation_controls_saves_copy_and_stages_chart_separately() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source =
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open(&mut s);
    assert!(has(&s, "Default bracket method:"));
    assert!(has(&s, "Save custom copy and select"));
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::Digit2);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert!(has(&s, "Custom -"), "{:?}", texts(&s));
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let file = loaded.profiles.values().next().unwrap();
    let deadsync_editor::generation::profiles::Config::Expressed(config) = file.profile().config()
    else {
        panic!()
    };
    assert_eq!(config.min_level_for_brackets, 12);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    let data = deadsync_editor::generation::metadata::ChartData::read(
        s.workspace().file().session.document(),
        0,
    )
    .unwrap()
    .unwrap();
    assert_eq!(&data.expressed_chart_config, file.profile().guid());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}
#[test]
fn cancel_keeps_profiles_and_song_untouched_and_invalid_input_cannot_save() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Minus);
    press(&mut s, KeyCode::ArrowDown);
    assert!(has(&s, "Enter an integer level"));
    press(&mut s, KeyCode::Escape);
    assert!(has(&s, "Profile changes discarded"));
    assert!(!profiles.exists());
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn inconsistent_thresholds_cannot_create_a_profile_and_can_be_repaired() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Default bracket method:"));
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit2);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "The aggressive threshold must exceed"));
    assert!(!profiles.exists());
    assert!(!s.workspace().file().session.is_dirty());
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Digit3);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert!(has(&s, "Custom -"));
    assert_eq!(
        deadsync_editor::generation::profiles::Loaded::directory(&profiles)
            .unwrap()
            .profiles
            .len(),
        1
    );
}
