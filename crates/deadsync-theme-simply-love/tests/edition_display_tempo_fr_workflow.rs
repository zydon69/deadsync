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
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
#[test]
fn french_sm_scope_invalid_input_and_cancel_are_keyboard_accessible() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let source = "#BPMS:0=150;#NOTES:dance-single:test:Hard:9:0,0,0,0,0:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(has(&s, "Mode d'affichage : Tempo réel"));
    assert!(has(&s, "SM : concerne toutes les partitions"));
    press(&mut s, KeyCode::ArrowLeft);
    assert!(has(&s, "Mode d'affichage : Personnalisé"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Minus);
    press(&mut s, KeyCode::ArrowDown);
    assert!(has(&s, "Saisissez une valeur de BPM finie"));
    press(&mut s, KeyCode::Escape);
    assert!(has(&s, "Modifications du tempo affiché abandonnées"));
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
}
