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
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
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
    // The source helper is gated by this actual profile control. Activate it
    // through the rendered keyboard field before opening the BPM helper.
    let label = i18n::tr_fmt(
        "Edition",
        "ProfileFieldSpeedTighteningEnabled",
        &[("value", "")],
    );
    let enabled = i18n::tr_fmt(
        "Edition",
        "ProfileFieldSpeedTighteningEnabled",
        &[("value", &i18n::tr("Edition", "PatternYes"))],
    );
    for _ in 0..512 {
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let selected = actors.iter().find_map(|actor| match actor {
            Actor::Text {
                color,
                content,
                z: 8021,
                ..
            } if *color == [1.0, 0.85, 0.25, 1.0]
                && content.as_str().starts_with(label.as_ref()) =>
            {
                Some(content.to_string())
            }
            _ => None,
        });
        if let Some(selected) = selected {
            if selected == enabled.as_ref() {
                return;
            }
            press(s, KeyCode::ArrowRight);
        } else {
            press(s, KeyCode::ArrowDown);
        }
    }
    panic!("travel speed control unavailable: {:?}", texts(s));
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
fn french_bpm_helper_validates_order_and_saves_repaired_values() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open_movement(&mut s);
    press(&mut s, KeyCode::F4);
    assert!(has(&s, "Subdivision des notes : 1/16"));
    assert!(has(&s, "Préparer les intervalles et revenir"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Delete);
    press(&mut s, KeyCode::ArrowDown);
    assert!(has(&s, "Saisissez une plage ordonnée"));
    assert!(!profiles.exists());
    number(&mut s, "140");
    press(&mut s, KeyCode::ArrowUp);
    stage_speed(&mut s);
    assert!(has(&s, "Intervalles de déplacement préparés"));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(
        loaded
            .profiles
            .values()
            .next()
            .unwrap()
            .profile()
            .travel_speed()
            .unwrap()
            .min_bpm,
        140
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
