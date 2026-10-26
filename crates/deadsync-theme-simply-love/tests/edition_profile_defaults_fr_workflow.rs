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
fn custom(
    path: &std::path::Path,
    profiles: &std::path::Path,
) -> (State, std::path::PathBuf, Vec<u8>) {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            metadata::ChartData,
            profiles::{Kind, Profile},
        },
        history::Session,
    };
    let profile=Profile::parse(Kind::Expressed,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Future":"keep","Config":{"MinLevelForBrackets":4}}"#).unwrap();
    let file = profile.export_new(profiles).unwrap();
    let mut session = Session::new(
        Document::parse(
            std::sync::Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    ChartData {
        expressed_chart_config: profile.guid().clone(),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let source = session.serialize().unwrap();
    fs::write(path, &source).unwrap();
    (
        State::open_with_profiles(path, 0, profiles).unwrap(),
        file.path().to_owned(),
        source,
    )
}
fn original_save(s: &mut State) {
    press(s, KeyCode::End);
    for _ in 0..3 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
}
#[test]
fn french_default_confirmation_cancel_and_staging_are_translated() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let before = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F5);
    assert!(has(&s, "Préparer les réglages d'origine"));
    assert!(has(&s, "Conserver le brouillon actuel"));
    press(&mut s, KeyCode::Escape);
    assert!(has(&s, "Restauration annulée"));
    press(&mut s, KeyCode::F5);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Réglages d'origine préparés"));
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
