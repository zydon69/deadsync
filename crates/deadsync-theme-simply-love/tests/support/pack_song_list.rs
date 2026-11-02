use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    pack::song_list::{OrderKey, Orderer},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
struct Lexical;
impl deadsync_editor::chart_order::Orderer for Lexical {
    fn order(&mut self, snapshot: &deadsync_editor::chart_order::Snapshot) -> Result<Vec<usize>, Error> { snapshot.indices_with(str::cmp) }
}
impl Orderer for Lexical {
    fn order(&mut self, keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
        let mut order: Vec<_> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| {
            keys[a]
                .title
                .cmp(&keys[b].title)
                .then_with(|| keys[a].folder.cmp(&keys[b].folder))
        });
        Ok(order)
    }
}
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn text(state: &State) -> String {
    fn visit(actor: &Actor, out: &mut String) {
        match actor {
            Actor::Text { content, .. } => {
                out.push_str(content);
                out.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    visit(child, out);
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = String::new();
    for actor in &actors {
        visit(actor, &mut out);
    }
    out
}
fn tr(key: &str) -> String {
    i18n::tr("Edition", key).to_string()
}
fn pump(state: &mut State, predicate: impl Fn(&State) -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !predicate(state) {
        assert!(Instant::now() < until, "Timed out: {}", text(state));
        state.process_pack_song_list(&mut Lexical, &mut Lexical);
        std::thread::yield_now();
    }
}
fn fixture(chartless: bool, target: &str) -> (tempfile::TempDir, PathBuf, PathBuf, State) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("Pack/A/song.ssc");
    let b = root.path().join("Pack/B/song.ssc");
    fs::create_dir_all(a.parent().unwrap()).unwrap();
    fs::create_dir_all(b.parent().unwrap()).unwrap();
    let source = if chartless {
        "#VERSION:0.83;#TITLE:Active marker;#BPMS:0=120;"
    } else {
        "#VERSION:0.83;#TITLE:Active marker;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
    };
    fs::write(&a, source).unwrap();
    fs::write(&b, target).unwrap();
    let state = State::open(&a, 0).unwrap();
    (
        root,
        a.canonicalize().unwrap(),
        b.canonicalize().unwrap(),
        state,
    )
}
const TARGET: &str = "#VERSION:0.83;#TITLE:Target marker;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:7;#NOTES:0100;";
fn open(state: &mut State, chartless: bool) {
    press(state, KeyCode::F1);
    for _ in 0..if chartless { 8 } else { 19 } {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
    pump(state, |state| text(state).contains("Target marker"));
}
fn choose_target(state: &mut State) {
    press(state, KeyCode::ArrowDown);
    press(state, KeyCode::Enter);
}
fn assert_current(state: &State, path: &Path) {
    assert_eq!(
        state.workspace().file().require_existing_path().unwrap(),
        path.canonicalize().unwrap()
    );
}
#[test]
fn keyboard_list_opens_strict_song_and_preserves_files() {
    let (_root, a, b, mut state) = fixture(false, TARGET);
    let before_a = fs::read(&a).unwrap();
    let before_b = fs::read(&b).unwrap();
    open(&mut state, false);
    assert!(text(&state).contains(&tr("PackSongsColumns")));
    choose_target(&mut state);
    pump(&mut state, |s| s.workspace().file().require_existing_path().unwrap() == b);
    assert_eq!(state.workspace().grid().iter().count(), 1);
    assert_eq!(fs::read(&a).unwrap(), before_a);
    assert_eq!(fs::read(&b).unwrap(), before_b);
}
#[test]
fn escape_cancels_background_discovery_without_replacing_session() {
    let (_root, a, _b, mut state) = fixture(false, TARGET);
    press(&mut state, KeyCode::F1);
    for _ in 0..19 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Escape);
    for _ in 0..10 {
        state.process_pack_song_list(&mut Lexical, &mut Lexical);
    }
    assert_current(&state, &a);
    assert!(!text(&state).contains(&tr("PackSongsTitle")));
}
#[test]
fn dirty_keep_cancels_open_and_save_open_saves_exact_current_edits() {
    let (_root, a, b, mut state) = fixture(false, TARGET);
    press(&mut state, KeyCode::Digit2);
    assert!(state.workspace().file().session.is_dirty());
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| text(s).contains(&tr("FileUnsavedHelp")));
    press(&mut state, KeyCode::Enter);
    assert_current(&state, &a);
    assert!(state.workspace().file().session.is_dirty());
    let edited = state.workspace().file().session.serialize().unwrap();
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| text(s).contains(&tr("FileUnsavedHelp")));
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_current(&state, &b);
    assert_eq!(fs::read(a).unwrap(), edited);
}
#[test]
fn discard_requires_separate_confirmation_and_does_not_write_source() {
    let (_root, a, b, mut state) = fixture(false, TARGET);
    let original = fs::read(&a).unwrap();
    press(&mut state, KeyCode::Digit2);
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| text(s).contains(&tr("FileUnsavedHelp")));
    for _ in 0..2 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains(&tr("FileDiscardConfirm")));
    press(&mut state, KeyCode::Escape); // Cancel the separate discard confirmation.
    assert_current(&state, &a);
    open(&mut state,false);choose_target(&mut state);
    pump(&mut state,|s|text(s).contains(&tr("FileUnsavedHelp")));
    for _ in 0..2 {press(&mut state,KeyCode::ArrowDown);}
    press(&mut state,KeyCode::Enter);
    press(&mut state,KeyCode::Enter); // Explicit common File discard confirmation.
    assert_current(&state, &b);
    assert_eq!(fs::read(a).unwrap(), original);
}
#[test]
fn header_only_target_remains_listed_but_strict_open_failure_keeps_source() {
    let (_root, a, _b, mut state) = fixture(
        false,
        "#VERSION:0.83;#TITLE:Target marker;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:8;",
    );
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| {
        text(s).contains("Chart is missing its note data")
    });
    assert_current(&state, &a);
    press(&mut state, KeyCode::Escape);
    assert_current(&state, &a);
}
#[test]
fn changed_target_during_dirty_confirmation_cannot_replace_source() {
    let (_root, a, b, mut state) = fixture(false, TARGET);
    press(&mut state, KeyCode::Digit2);
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| text(s).contains(&tr("FileUnsavedHelp")));
    fs::write(&b, TARGET.replace("Target marker", "External marker")).unwrap();
    for _ in 0..2 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_current(&state, &a);
    assert!(state.workspace().file().session.is_dirty());
    assert!(fs::read_to_string(&b).unwrap().contains("External marker"));
}
#[test]
fn same_pack_dirty_metadata_survives_song_transition_without_publishing_pack() {
    let (root, _a, b, mut state) = fixture(false, TARGET);
    fs::write(
        root.path().join("Pack/Pack.ini"),
        "[Group]\nVersion=1\nDisplayTitle=Pack\n",
    )
    .unwrap();
    press(&mut state, KeyCode::F1);
    for _ in 0..18 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains(&tr("PackPropertiesTitle")));
    assert!(text(&state).contains(&tr("PackSave")));
    assert!(state.text_input(" changed"));
    press(&mut state, KeyCode::Escape);
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state,|s|s.workspace().file().require_existing_path().unwrap()==b);
    assert_current(&state,&b);
    press(&mut state,KeyCode::F1);for _ in 0..18{press(&mut state,KeyCode::ArrowDown);}press(&mut state,KeyCode::Enter);
    assert!(text(&state).contains("changed"));assert!(text(&state).contains(&tr("PackDirty")));
    assert_eq!(
        fs::read_to_string(root.path().join("Pack/Pack.ini")).unwrap(),
        "[Group]\nVersion=1\nDisplayTitle=Pack\n"
    );
}
#[test]
fn chartless_source_can_browse_and_open_chartless_song() {
    let (_root, _a, b, mut state) =
        fixture(true, "#VERSION:0.83;#TITLE:Target marker;#BPMS:0=120;");
    open(&mut state, true);
    choose_target(&mut state);
    pump(&mut state, |s| s.workspace().file().require_existing_path().unwrap() == b);
    assert!(!state.workspace().has_chart());
}

#[test]
fn save_conflict_keeps_dirty_song_and_does_not_publish_candidate() {
    let (_root, a, b, mut state) = fixture(false, TARGET);
    press(&mut state, KeyCode::Digit2);
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| text(s).contains(&tr("FileUnsavedHelp")));
    let external = fs::read_to_string(&a)
        .unwrap()
        .replace("Active marker", "External marker");
    fs::write(&a, &external).unwrap();
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_current(&state, &a);
    assert!(state.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(&a).unwrap(), external);
    assert_eq!(fs::read_to_string(&b).unwrap(), TARGET);
    press(&mut state, KeyCode::Escape);
    assert_current(&state, &a);
}
#[test]
fn invalid_cultural_permutation_cannot_open_a_row_and_refresh_retries() {
    struct Invalid;
    impl Orderer for Invalid {
        fn order(&mut self, _keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
            Ok(vec![])
        }
    }
    let (_root, a, _b, mut state) = fixture(false, TARGET);
    press(&mut state, KeyCode::F1);
    for _ in 0..19 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    let until = Instant::now() + Duration::from_secs(10);
    while !text(&state).contains("complete permutation") {
        assert!(Instant::now() < until, "{}", text(&state));
        state.process_pack_song_list(&mut Invalid, &mut Lexical);
        std::thread::yield_now();
    }
    press(&mut state, KeyCode::Enter);
    assert_current(&state, &a);
    press(&mut state, KeyCode::F5);
    pump(&mut state, |s| text(s).contains("Target marker"));
    press(&mut state, KeyCode::Escape);
    assert_current(&state, &a);
}

#[test]
fn custom_preference_path_survives_open_and_invalid_reload_keeps_current_editor() {
    let (root, _a, b, mut state) = fixture(false, TARGET);
    let preferences = root.path().join("custom-editor-preferences.json");
    fs::write(
        &preferences,
        r#"{"Version":1,"Audio":{"MusicRate":1.5},"Vendor":"keep"}"#,
    )
    .unwrap();
    state.configure_audio_preferences(&preferences).unwrap();
    open(&mut state, false);
    choose_target(&mut state);
    pump(&mut state, |s| s.workspace().file().require_existing_path().unwrap() == b);
    assert_eq!(state.audio_preferences().music_rate(), 1.5);
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        if pressed {
            press(&mut state, KeyCode::ArrowRight);
        }
    }
    assert_eq!(state.audio_preferences().music_rate(), 1.6);
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&preferences).unwrap()).unwrap();
    assert_eq!(saved["Audio"]["MusicRate"], 1.6);
    assert_eq!(saved["Vendor"], "keep");

    let (root, a, _b, mut state) = fixture(false, TARGET);
    let preferences = root.path().join("custom-editor-preferences.json");
    fs::write(&preferences, r#"{"Version":1,"Audio":{"MusicRate":1.5}}"#).unwrap();
    state.configure_audio_preferences(&preferences).unwrap();
    open(&mut state, false);
    choose_target(&mut state);
    fs::write(&preferences, "invalid external preferences").unwrap();
    // Candidate publication now shares the canonical File controller, including
    // its preference-loading failure, rather than a private pack opener.
    let failure = tr("FileFailed");
    let prefix = failure.split("{message}").next().unwrap().to_owned();
    pump(&mut state, |s| text(s).contains(&prefix));
    assert_current(&state, &a);
    assert_eq!(state.audio_preferences().music_rate(), 1.5);
    assert_eq!(
        fs::read_to_string(&preferences).unwrap(),
        "invalid external preferences"
    );
    press(&mut state, KeyCode::Escape);
    assert_current(&state, &a);
}
