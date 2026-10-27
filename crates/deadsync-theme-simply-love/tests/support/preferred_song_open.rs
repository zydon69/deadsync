use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_order::{Orderer as ChartOrderer, Snapshot},
    pack::song_list::{OrderKey, Orderer},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
struct NativeFake;
impl Orderer for NativeFake {
    fn order(&mut self, keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
        let mut order: Vec<_> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| keys[a].title.cmp(&keys[b].title));
        Ok(order)
    }
}
impl ChartOrderer for NativeFake {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        snapshot.indices_with(str::cmp)
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
                for actor in children {
                    visit(actor, out);
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
fn pump(
    state: &mut State,
    mut chart_orderer: impl ChartOrderer,
    predicate: impl Fn(&State) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(10);
    while !predicate(state) {
        assert!(Instant::now() < until, "{}", text(state));
        state.process_pack_song_list(&mut NativeFake, &mut chart_orderer);
        std::thread::yield_now();
    }
}
fn fixture(options: Option<&str>, paired: bool) -> (tempfile::TempDir, PathBuf, PathBuf, State) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Pack/A/song.ssc");
    let target = root.path().join("Pack/B/song.ssc");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&source,"#VERSION:0.83;#TITLE:Active marker;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
    let mut song = String::from("#VERSION:0.83;#TITLE:Target marker;#BPMS:0=120;");
    let mut charts = vec![
        ("dance-double", "Challenge", 9),
        ("dance-single", "Easy", 2),
        ("dance-single", "Challenge", 13),
        ("dance-single", "Hard", 9),
        ("pump-single", "Easy", 5),
    ];
    if !paired {
        charts.extend([
            ("pump-double", "Hard", 17),
            ("pump-double", "Challenge", 20),
        ]);
    }
    for (kind, difficulty, rating) in charts {
        let columns = deadsync_editor::notes::layout_columns(kind).unwrap();
        song += &format!(
            "#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:{rating};#NOTES:1{};",
            "0".repeat(usize::from(columns - 1))
        );
    }
    fs::write(&target, song).unwrap();
    let mut state = State::open(&source, 0).unwrap();
    if let Some(options) = options {
        let path = root.path().join("preferences.json");
        fs::write(&path, format!("{{\"Version\":1,\"Options\":{options}}}")).unwrap();
        state.configure_audio_preferences(&path).unwrap();
    }
    (
        root,
        source.canonicalize().unwrap(),
        target.canonicalize().unwrap(),
        state,
    )
}
fn open_target(state: &mut State) {
    press(state, KeyCode::F1);
    for _ in 0..19 {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
    pump(state, NativeFake, |state| {
        text(state).contains("Target marker")
    });
    press(state, KeyCode::ArrowDown);
    press(state, KeyCode::Enter);
}
#[test]
fn default_open_selects_source_default_single_challenge_not_first_chart() {
    let (_root, _source, target, mut state) = fixture(None, false);
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert_eq!(state.workspace().chart(), 2);
}
#[test]
fn common_options_preferred_layout_and_difficulty_are_used_and_preserved() {
    let (root, _source, target, mut state) = fixture(
        Some(
            r#"{"DefaultStepsType":"pump_double","DefaultDifficultyType":"Hard","Vendor":"keep"}"#,
        ),
        false,
    );
    let bytes = fs::read(root.path().join("preferences.json")).unwrap();
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert_eq!(state.workspace().chart(), 5);
    assert_eq!(
        fs::read(root.path().join("preferences.json")).unwrap(),
        bytes
    );
}
#[test]
fn missing_preferred_type_uses_original_single_double_pair_fallback() {
    let (_root, _source, target, mut state) = fixture(
        Some(r#"{"DefaultStepsType":"pump_double","DefaultDifficultyType":"Hard"}"#),
        true,
    );
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert_eq!(state.workspace().chart(), 4);
}
#[test]
fn invalid_native_chart_order_keeps_original_editor() {
    struct Invalid;
    impl ChartOrderer for Invalid {
        fn order(&mut self, _snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
            Ok(vec![])
        }
    }
    let (_root, source, _target, mut state) = fixture(None, false);
    open_target(&mut state);
    pump(&mut state, Invalid, |s| {
        text(s).contains("Open chart order is not an exhaustive source permutation")
    });
    assert_eq!(state.workspace().file().path(), source);
    press(&mut state, KeyCode::Escape);
    assert_eq!(state.workspace().file().path(), source);
}
#[test]
fn unselected_invalid_peer_is_reported_and_preserved_without_blocking_default() {
    let (_root, source, target, mut state) = fixture(None, false);
    let before_source = fs::read(&source).unwrap();
    let mut bytes = fs::read_to_string(&target).unwrap();
    bytes.push_str("#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Hard;#METER:8;#VENDOR:keep-bad-peer;#NOTES:oops;");
    fs::write(&target, &bytes).unwrap();
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert_eq!(state.workspace().chart(), 2);
    assert_eq!(state.song_open_warnings().len(), 1);
    let warning = &state.song_open_warnings()[0];
    assert_eq!(warning.source_index, 7);
    assert_eq!(
        warning.identity,
        state.workspace().file().session.chart_ids()[7]
    );
    assert!(!warning.message.is_empty());
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        bytes.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read(&target).unwrap(), bytes.as_bytes());
    assert_eq!(fs::read(source).unwrap(), before_source);
}
#[test]
fn unavailable_preferred_chart_falls_back_to_loadable_hard_without_erasing_peer() {
    let (_root, _source, target, mut state) = fixture(None, false);
    let bytes = fs::read_to_string(&target).unwrap().replace(
        "#DIFFICULTY:Challenge;#METER:13;#NOTES:1000;",
        "#DIFFICULTY:Challenge;#METER:13;#NOTES:oops;",
    );
    assert!(bytes.contains("#NOTES:oops;"));
    fs::write(&target, &bytes).unwrap();
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert_eq!(state.workspace().chart(), 3);
    assert_eq!(state.song_open_warnings().len(), 1);
    assert_eq!(state.song_open_warnings()[0].source_index, 2);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        bytes.as_bytes()
    );
    assert_eq!(fs::read(target).unwrap(), bytes.as_bytes());
}
#[test]
fn no_loadable_chart_opens_unfocused_with_diagnostics_and_original_blocks() {
    let (_root, _source, target, mut state) = fixture(None, false);
    let mut bytes = fs::read_to_string(&target).unwrap();
    for notes in ["1000", "10000", "10000000", "1000000000"] {
        bytes = bytes.replace(&format!("#NOTES:{notes};"), "#NOTES:oops;");
    }
    fs::write(&target, &bytes).unwrap();
    open_target(&mut state);
    pump(&mut state, NativeFake, |s| {
        s.workspace().file().path() == target
    });
    assert!(!state.workspace().has_chart());
    assert_eq!(state.song_open_warnings().len(), 7);
    assert_eq!(
        state.workspace().file().session.document().charts().len(),
        7
    );
    for warning in state.song_open_warnings() {
        assert_eq!(
            warning.identity,
            state.workspace().file().session.chart_ids()[warning.source_index]
        );
    }
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        bytes.as_bytes()
    );
    assert_eq!(fs::read(target).unwrap(), bytes.as_bytes());
    assert!(!state.workspace().file().session.is_dirty());
}
