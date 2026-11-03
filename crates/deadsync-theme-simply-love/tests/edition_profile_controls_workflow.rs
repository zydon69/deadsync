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
#[test]
fn pattern_copy_is_paginated_preserves_other_choices_and_applies_to_marker_once() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open_marker_profiles(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    let performed = texts(&s)
        .into_iter()
        .find(|v| v.starts_with("Steps:"))
        .unwrap();
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    let expressed = texts(&s)
        .into_iter()
        .find(|v| v.starts_with("Interpretation (whole chart):"))
        .unwrap();
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::KeyE);
    assert!(has(&s, "Note subdivision: 1/16"), "{:?}", texts(&s));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Right foot:"));
    press(&mut s, KeyCode::PageDown);
    assert!(
        has(&s, "Step distribution check period:"),
        "{:?}",
        texts(&s)
    );
    assert!(!texts(&s).iter().any(|v| v.starts_with("ProfileField")));
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert!(has(&s, "Custom -"));
    assert!(has(&s, &performed));
    assert!(has(&s, &expressed));
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let profile = loaded.profiles.values().next().unwrap().profile();
    let deadsync_editor::generation::profiles::Config::Pattern(config) = profile.config() else {
        panic!()
    };
    assert_eq!(config.beat_sub_division, 6);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowUp);
    }
    press(&mut s, KeyCode::Enter);
    let data = deadsync_editor::generation::metadata::ChartData::read(
        s.workspace().file().session.document(),
        0,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        &data.patterns.unwrap()[&0].pattern_config_guid,
        profile.guid()
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn performed_copy_exposes_weights_and_can_be_selected_for_conversion_without_chart_edits() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..5 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::KeyE);
    assert!(
        has(&s, "Maximum inward-facing proportion:"),
        "{:?}",
        texts(&s)
    );
    press(&mut s, KeyCode::Digit0);
    press(&mut s, KeyCode::Period);
    press(&mut s, KeyCode::Digit5);
    for _ in 0..5 {
        press(&mut s, KeyCode::PageDown);
    }
    assert!(has(&s, "weight"));
    assert!(!texts(&s).iter().any(|v| v.starts_with("ProfileField")));
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert!(has(&s, "Custom -"), "{:?}", texts(&s));
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let deadsync_editor::generation::profiles::Config::Performed(config) =
        loaded.profiles.values().next().unwrap().profile().config()
    else {
        panic!()
    };
    assert_eq!(config.facing.max_inward_percentage, 0.5);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
