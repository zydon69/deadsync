use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn control(state: &mut State, code: KeyCode, backwards: bool) {
    key(state, KeyCode::SuperLeft, true);
    if backwards {
        key(state, KeyCode::ShiftLeft, true);
    }
    press(state, code);
    if backwards {
        key(state, KeyCode::ShiftLeft, false);
    }
    key(state, KeyCode::SuperLeft, false);
}
fn setup() -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let directory = tempfile::tempdir().unwrap();
    let source=b"#VERSION:0.83;#TITLE:Order;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Hard;#METER:9;#NOTES:10000000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#CHARTNAME:zebra;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:2;#CHARTNAME:clair;#NOTES:0100;".to_vec();
    let path = directory.path().join("song.ssc");
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 0).unwrap();
    (directory, state, source)
}
struct Fake {
    calls: usize,
    result: Option<Vec<usize>>,
    fail: bool,
    change_language: bool,
}
impl Fake {
    fn valid() -> Self {
        Self {
            calls: 0,
            result: None,
            fail: false,
            change_language: false,
        }
    }
}
impl Orderer for Fake {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        self.calls += 1;
        assert_eq!(snapshot.keys().len(), 3);
        assert_eq!(snapshot.keys()[0].steps_type, "dance-double");
        assert_eq!(snapshot.keys()[2].rating, 2);
        if self.change_language {
            let other = if LANGUAGE == "en" { "fr" } else { "en" };
            i18n::set_locale(deadsync_assets::language::load_for_tests(other));
        }
        if self.fail {
            return Err(Error::Unsupported("Native ICU unavailable".into()));
        }
        Ok(self
            .result
            .clone()
            .unwrap_or_else(|| snapshot.indices_with(str::cmp).unwrap()))
    }
}
fn texts(state: &State) -> String {
    fn walk(actor: &Actor, out: &mut String) {
        match actor {
            Actor::Text { content, .. } => {
                out.push_str(&content.to_string());
                out.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    walk(child, out);
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = String::new();
    for actor in &actors {
        walk(actor, &mut out);
    }
    out
}
#[test]
fn shortcuts_use_live_sorted_source_indices_both_directions_without_rewriting_song() {
    let _guard = SERIAL.lock().unwrap();
    let (directory, mut state, source) = setup();
    let ids = state.workspace().file().session.chart_ids().to_vec();
    let mut fake = Fake::valid();
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 0);
    // Presentation is [2,1,0]: from source0, forward wraps to source2.
    control(&mut state, KeyCode::Tab, false);
    assert_eq!(state.workspace().chart(), 0);
    assert!(state.process_chart_order(&mut fake));
    assert_eq!(state.workspace().chart(), 2);
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 1);
    control(&mut state, KeyCode::Tab, false);
    state.process_chart_order(&mut fake);
    assert_eq!(state.workspace().chart(), 1);
    control(&mut state, KeyCode::Tab, true);
    state.process_chart_order(&mut fake);
    assert_eq!(state.workspace().chart(), 2);
    assert_eq!(state.workspace().file().session.chart_ids(), ids);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert_eq!(fs::read(directory.path().join("song.ssc")).unwrap(), source);
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn invalid_permutations_and_backend_failures_keep_focus_and_localize_failure() {
    let _guard = SERIAL.lock().unwrap();
    for result in [
        vec![],
        vec![0, 1],
        vec![0, 1, 1],
        vec![0, 1, 3],
        vec![0, 1, usize::MAX],
        vec![0, 1, 2, 3],
    ] {
        let (_directory, mut state, source) = setup();
        let mut fake = Fake::valid();
        fake.result = Some(result);
        control(&mut state, KeyCode::Tab, false);
        assert!(state.process_chart_order(&mut fake));
        assert_eq!(state.workspace().chart(), 0);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
        assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartOrderFailed")));
    }
    let (_directory, mut state, source) = setup();
    let mut fake = Fake::valid();
    fake.fail = true;
    control(&mut state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut fake));
    assert_eq!(state.workspace().chart(), 0);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartOrderFailed")));
}
#[test]
fn stale_content_focus_panels_and_focus_loss_drop_requests_before_native_calls() {
    let _guard = SERIAL.lock().unwrap();
    for action in [KeyCode::Digit2, KeyCode::F1] {
        let (_directory, mut state, _source) = setup();
        let mut fake = Fake::valid();
        control(&mut state, KeyCode::Tab, false);
        press(&mut state, action);
        assert!(!state.process_chart_order(&mut fake));
        assert_eq!(fake.calls, 0);
        assert_eq!(state.workspace().chart(), 0);
    }
    let (_directory, mut state, _source) = setup();
    let mut fake = Fake::valid();
    control(&mut state, KeyCode::Tab, false);
    state.release_input();
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 0);
    control(&mut state, KeyCode::Tab, false);
    let mut first = Fake::valid();
    assert!(state.process_chart_order(&mut first));
    // Consuming a request prevents a second transport from replaying it.
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 0);
}
#[test]
fn culture_changes_before_or_during_native_work_discard_results() {
    let _guard = SERIAL.lock().unwrap();
    let (_directory, mut state, source) = setup();
    let mut fake = Fake::valid();
    control(&mut state, KeyCode::Tab, false);
    let other = if LANGUAGE == "en" { "fr" } else { "en" };
    i18n::set_locale(deadsync_assets::language::load_for_tests(other));
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 0);
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    control(&mut state, KeyCode::Tab, false);
    fake.change_language = true;
    assert!(!state.process_chart_order(&mut fake));
    assert_eq!(fake.calls, 1);
    assert_eq!(state.workspace().chart(), 0);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
}

#[test]
fn unsupported_source_gaps_do_not_block_navigation_or_accept_invalid_compact_indices() {
    let _guard = SERIAL.lock().unwrap();
    let (directory, _state, source) = setup();
    let source=String::from_utf8(source).unwrap().replace("#NOTEDATA:;#STEPSTYPE:dance-double;", "#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;#NOTEDATA:;#STEPSTYPE:dance-double;")
        .replace("#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;", "#NOTEDATA:;#STEPSTYPE:lights-cabinet;#NOTES:10000000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;");
    let path = directory.path().join("song.ssc");
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 1).unwrap();
    let mut fake = Fake::valid();
    control(&mut state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut fake));
    assert_eq!(state.workspace().chart(), 4);
    control(&mut state, KeyCode::Tab, true);
    assert!(state.process_chart_order(&mut fake));
    assert_eq!(state.workspace().chart(), 1);
    for invalid in [vec![0, 1, 2], vec![1, 3, 3], vec![1, 3, 5]] {
        fake.result = Some(invalid);
        control(&mut state, KeyCode::Tab, false);
        assert!(state.process_chart_order(&mut fake));
        assert_eq!(state.workspace().chart(), 1);
        assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartOrderFailed")));
    }
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
    assert!(!state.workspace().file().session.is_dirty());
}

#[test]
fn zero_or_one_editable_chart_never_opens_the_native_collator() {
    let _guard = SERIAL.lock().unwrap();
    let (directory, _state, _source) = setup();
    for charts in [
        "",
        "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        "#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
        "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
    ] {
        let path = directory.path().join("single.ssc");
        fs::write(&path, format!("#VERSION:0.83;#BPMS:0=120;{charts}")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let mut fake = Fake::valid();
        control(&mut state, KeyCode::Tab, false);
        state.process_chart_order(&mut fake);
        assert_eq!(fake.calls, 0);
        assert!(!state.workspace().file().session.is_dirty());
    }
}
