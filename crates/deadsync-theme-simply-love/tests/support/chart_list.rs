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
fn control(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
}
struct Native {
    calls: usize,
}
impl Orderer for Native {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        self.calls += 1;
        snapshot.indices_with(str::cmp)
    }
}
fn setup(count: usize) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let mut source = String::from(
        "#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
    );
    for index in 0..count {
        source += &format!(
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:{};#CHARTNAME:Chart{index};#NOTES:1000;",
            count - index
        );
    }
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, if count == 0 { 0 } else { 1 }).unwrap();
    (dir, state, source.into_bytes())
}
fn texts(state: &State) -> String {
    texts_at_z(state, None)
}
fn list_texts(state: &State) -> String {
    // List text is drawn above the editor at z=8021. Resident chart headers
    // legitimately remain visible underneath and are not pagination rows.
    texts_at_z(state, Some(8021))
}
fn texts_at_z(state: &State, required_z: Option<i16>) -> String {
    fn walk(actor: &Actor, out: &mut String, required_z: Option<i16>) {
        match actor {
            Actor::Text { content, z, .. } if required_z.is_none_or(|required| *z == required) => {
                out.push_str(content);
                out.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    walk(child, out, required_z)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = String::new();
    for actor in actors {
        walk(&actor, &mut out, required_z)
    }
    out
}
#[test]
fn keyboard_list_is_sorted_paginated_and_cancel_does_not_change_active_source() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, source) = setup(10);
    let mut native = Native { calls: 0 };
    control(&mut state, KeyCode::KeyB);
    assert!(state.process_chart_order(&mut native));
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartListTitle")));
    assert!(list_texts(&state).contains("> 10/10"));
    assert_eq!(native.calls, 1);
    press(&mut state, KeyCode::Home);
    let visible = list_texts(&state);
    assert!(visible.contains("> 1/10"));
    assert!(visible.contains("Chart9"));
    assert!(!visible.contains("Chart0"));
    press(&mut state, KeyCode::PageDown);
    assert!(list_texts(&state).contains("> 9/10"));
    press(&mut state, KeyCode::Escape);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert_eq!(fs::read(dir.path().join("song.ssc")).unwrap(), source);
    control(&mut state, KeyCode::KeyB);
    state.process_chart_order(&mut native);
    assert_eq!(native.calls, 1);
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 10);
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn menu_opens_same_picker_and_language_changes_require_refresh_before_commit() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup(3);
    let mut native = Native { calls: 0 };
    press(&mut state, KeyCode::F1);
    for _ in 0..16 {
        press(&mut state, KeyCode::ArrowDown);
    }
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartListOpen")));
    press(&mut state, KeyCode::Enter);
    assert!(state.process_chart_order(&mut native));
    press(&mut state, KeyCode::Home);
    i18n::set_locale(deadsync_assets::language::load_for_tests(
        if LANGUAGE == "en" { "fr" } else { "en" },
    ));
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartListRefresh")));
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
    assert!(state.process_chart_order(&mut native));
    assert_eq!(native.calls, 2);
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 3);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn unknown_only_song_opens_empty_localized_list_without_native_work() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup(0);
    let mut native = Native { calls: 0 };
    control(&mut state, KeyCode::KeyB);
    state.process_chart_order(&mut native);
    assert_eq!(native.calls, 0);
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartListEmpty")));
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Escape);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert!(!state.workspace().file().session.is_dirty());
}
