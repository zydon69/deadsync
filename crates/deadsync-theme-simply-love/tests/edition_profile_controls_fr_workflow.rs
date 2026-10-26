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
#[test]
fn every_page_of_all_three_profile_families_has_french_labels_and_cancels_without_writing() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let source = marker_song();
    fs::write(&path, &source).unwrap();
    for kind in 0..3 {
        let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
        open_marker_profiles(&mut s);
        for _ in 0..kind {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::KeyE);
        for _ in 0..20 {
            let labels = texts(&s);
            assert!(
                !labels.iter().any(|v| v.starts_with("ProfileField")
                    || v.contains("ProfileChoice")
                    || v.contains('{')),
                "{kind}: {labels:?}"
            );
            press(&mut s, KeyCode::PageDown);
        }
        press(&mut s, KeyCode::End);
        assert!(has(&s, "Enregistrer et sélectionner une copie"));
        press(&mut s, KeyCode::Escape);
        assert!(has(&s, "Modifications du profil abandonnées."));
        assert!(!profiles.exists());
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
