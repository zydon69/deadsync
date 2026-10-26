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
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::KeyE);
}
fn movement(
    path: &std::path::Path,
    profiles: &std::path::Path,
) -> (State, std::path::PathBuf, Vec<u8>) {
    use deadsync_editor::generation::{
        config::PerformedConfig,
        profiles::{Kind, Profile},
    };
    let mut config = PerformedConfig::balanced();
    config
        .arrow_weights
        .retain(|layout, _| layout == "dance-single");
    config
        .arrow_weights
        .insert("dance-single".into(), vec![1, 2, 3, 4]);
    config.facing.max_inward_percentage = 0.25;
    let profile=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Sparse movement","Config":config})).unwrap()).unwrap();
    let file = profile.export_new(profiles).unwrap();
    fs::write(
        path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open_with_profiles(path, 0, profiles).unwrap();
    let source = s.workspace().file().session.serialize().unwrap();
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..5 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    for _ in 0..20 {
        if has(&s, "Sparse movement") {
            break;
        }
        press(&mut s, KeyCode::ArrowRight);
    }
    assert!(has(&s, "Sparse movement"));
    press(&mut s, KeyCode::KeyE);
    (s, file.path().to_owned(), source)
}
#[test]
fn french_layout_weight_actions_keep_changes_staged_and_can_be_discarded_with_profile() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = movement(&path, &profiles);
    let before = fs::read(&file).unwrap();
    press(&mut s, KeyCode::F6);
    assert!(has(&s, "Agencement : dance-single"));
    assert!(has(&s, "Poids : Valeurs explicites"));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Poids : Hériter"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Poids par défaut préparés"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
