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
fn reset_is_staged_and_original_save_preserves_pending_name_description_and_song() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let before = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Retained name"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F3);
    assert!(s.text_input("Retained description"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F5);
    assert!(has(&s, "Stage original default settings"));
    assert_eq!(fs::read(&file).unwrap(), before);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Default settings staged"));
    assert!(has(&s, "Minimum bracket level: 7"));
    assert_eq!(fs::read(&file).unwrap(), before);
    original_save(&mut s);
    let saved = deadsync_editor::generation::profiles::ProfileFile::open(&file).unwrap();
    assert_eq!(
        saved.profile().config(),
        &deadsync_editor::generation::profiles::Config::editor_defaults(
            deadsync_editor::generation::profiles::Kind::Expressed
        )
    );
    assert_eq!(saved.profile().name(), Some("Retained name"));
    assert_eq!(saved.profile().description(), Some("Retained description"));
    assert!(fs::read_to_string(&file).unwrap().contains("keep"));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn cancelled_reset_keeps_invalid_input_and_confirmed_reset_can_repair_it() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let before = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Minus);
    press(&mut s, KeyCode::F5);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Minimum bracket level: -"));
    assert_eq!(fs::read(&file).unwrap(), before);
    press(&mut s, KeyCode::F5);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Minimum bracket level: 7"));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn movement_reset_rebuilds_expanded_weight_controls_and_stages_original_bpm_defaults() {
    use deadsync_editor::generation::{
        config::PerformedConfig,
        profiles::{Config, Kind, Loaded, Profile, TravelSpeed},
    };
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut config = PerformedConfig::balanced();
    config
        .arrow_weights
        .retain(|layout, _| layout == "dance-single");
    config.facing.max_inward_percentage = 0.0;
    let profile=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Sparse movement","Config":config})).unwrap()).unwrap();
    let file = profile.export_new(&profiles).unwrap();
    let before = fs::read(file.path()).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
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
    press(&mut s, KeyCode::F5);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Default settings staged"));
    assert!(has(&s, "Save custom copy and select"));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    let copy = loaded
        .profiles
        .values()
        .find(|f| f.profile().guid() != profile.guid())
        .unwrap();
    assert_eq!(
        copy.profile().config(),
        &Config::editor_defaults(Kind::Performed)
    );
    assert_eq!(
        copy.profile().travel_speed(),
        Some(TravelSpeed::editor_defaults())
    );
    assert_eq!(fs::read(file.path()).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
