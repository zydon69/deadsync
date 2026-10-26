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
fn add_missing_and_remove_explicit_layouts_are_staged_then_saved_without_resetting_other_fields() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = movement(&path, &profiles);
    let before = fs::read(&file).unwrap();
    press(&mut s, KeyCode::F6);
    assert!(has(&s, "Layout: dance-single"));
    assert!(has(&s, "Weights: Explicit values"));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Weights: Inherit"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Default panel weights staged"));
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "No change; existing values retained"));
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowLeft);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Inherited weights staged"));
    assert_eq!(fs::read(&file).unwrap(), before);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    let copy = loaded.profiles.values().find(|f| f.path() != file).unwrap();
    let deadsync_editor::generation::profiles::Config::Performed(c) = copy.profile().config()
    else {
        panic!()
    };
    assert!(!c.arrow_weights.contains_key("dance-single"));
    assert_eq!(
        c.arrow_weights["dance-double"],
        deadsync_editor::generation::config::PerformedConfig::balanced().arrow_weights["dance-double"]
    );
    assert_eq!(c.facing.max_inward_percentage, 0.25);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn adding_existing_values_is_a_noop_and_removal_can_be_saved_in_place() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = movement(&path, &profiles);
    let before = fs::read(&file).unwrap();
    press(&mut s, KeyCode::F6);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    let copy = loaded.profiles.values().find(|f| f.path() != file).unwrap();
    let copy_path = copy.path().to_owned();
    let deadsync_editor::generation::profiles::Config::Performed(c) = copy.profile().config()
    else {
        panic!()
    };
    assert_eq!(c.arrow_weights["dance-single"], vec![1, 2, 3, 4]);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F6);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Escape);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let updated = deadsync_editor::generation::profiles::ProfileFile::open(&copy_path).unwrap();
    let deadsync_editor::generation::profiles::Config::Performed(c) = updated.profile().config()
    else {
        panic!()
    };
    assert!(c.arrow_weights.is_empty());
    assert_eq!(c.facing.max_inward_percentage, 0.25);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn panel_weight_numeric_input_and_arrows_use_original_zero_to_hundred_bounds() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = movement(&path, &profiles);
    let original = deadsync_editor::generation::profiles::ProfileFile::open(&file).unwrap();
    let editor =
        deadsync_editor::generation::profile_controls::Editor::new(original.profile().config())
            .unwrap();
    let index = editor
        .fields()
        .iter()
        .position(|f| f.path() == ["ArrowWeights", "dance-single", "0"])
        .unwrap();
    for _ in 0..index {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Delete);
    press(&mut s, KeyCode::Minus);
    press(&mut s, KeyCode::Digit5);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "panel 1: weight 0"));
    press(&mut s, KeyCode::ArrowLeft);
    assert!(has(&s, "panel 1: weight 0"));
    press(&mut s, KeyCode::Delete);
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::Digit0);
    press(&mut s, KeyCode::Digit5);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "panel 1: weight 100"));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "panel 1: weight 100"));
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    let copy = loaded.profiles.values().find(|f| f.path() != file).unwrap();
    let deadsync_editor::generation::profiles::Config::Performed(c) = copy.profile().config()
    else {
        panic!()
    };
    assert_eq!(c.arrow_weights["dance-single"], vec![100, 2, 3, 4]);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
