use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
    generation::metadata::Guid,
};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
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
fn control(state: &mut State, code: KeyCode, shift: bool) {
    key(state, KeyCode::SuperLeft, true);
    if shift {
        key(state, KeyCode::ShiftLeft, true);
    }
    press(state, code);
    key(state, KeyCode::ShiftLeft, false);
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
fn setup() -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("views.ssc");
    fs::write(dir.path().join("song.ogg"), b"command-only fixture").unwrap();
    let mut source = String::from(
        "#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#MUSIC:song.ogg;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
    );
    for index in 1..=3 {
        source += &format!(
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:{index};#CHARTNAME:Identical;{}#NOTES:1000\n0100\n0010\n0001;",
            if index == 2 {
                "#BPMS:0=60;#OFFSET:0.25;"
            } else {
                ""
            }
        );
    }
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 1).unwrap();
    (dir, state, source.into_bytes())
}
fn texts(state: &State) -> String {
    fn walk(actor: &Actor, out: &mut String) {
        match actor {
            Actor::Text { content, .. } => {
                out.push_str(content);
                out.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    walk(child, out)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = String::new();
    for actor in actors {
        walk(&actor, &mut out);
    }
    out
}
fn ids(state: &State) -> Vec<Guid> {
    state
        .open_chart_views()
        .unwrap()
        .entries()
        .iter()
        .map(|view| view.identity().clone())
        .collect()
}
fn next(state: &mut State, native: &mut Native) {
    control(state, KeyCode::Tab, false);
    assert!(state.process_chart_order(native));
}
fn pin(state: &mut State) {
    control(state, KeyCode::KeyB, true);
}
#[test]
fn keyboard_headers_pin_temporary_replacement_focus_and_moving_match_resident_ids() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, source) = setup();
    let source_ids = state.workspace().file().session.chart_ids().to_vec();
    let mut native = Native { calls: 0 };
    assert_eq!(state.open_chart_views().unwrap().entries().len(), 1);
    pin(&mut state);
    assert!(
        state
            .open_chart_views()
            .unwrap()
            .focused()
            .unwrap()
            .dedicated()
    );
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartViewPinned")));
    next(&mut state, &mut native);
    assert_eq!(state.workspace().chart(), 2);
    next(&mut state, &mut native);
    assert_eq!(state.workspace().chart(), 3);
    assert_eq!(ids(&state), [source_ids[3].clone(), source_ids[1].clone()]);
    assert_eq!(
        state
            .open_chart_views()
            .unwrap()
            .entries()
            .iter()
            .filter(|view| !view.dedicated())
            .count(),
        1
    );
    pin(&mut state);
    control(&mut state, KeyCode::PageDown, false);
    assert_eq!(state.workspace().chart(), 1);
    control(&mut state, KeyCode::ArrowLeft, true);
    assert_eq!(ids(&state), [source_ids[1].clone(), source_ids[3].clone()]);
    assert!(texts(&state).contains("> 1/2"));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert_eq!(fs::read(dir.path().join("views.ssc")).unwrap(), source);
    assert!(!state.workspace().file().session.is_dirty());
    assert!(state.workspace().file().session.undo_label().is_none());
}
#[test]
fn closing_focus_prefers_right_then_left_and_last_close_keeps_song_available_for_reopening() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup();
    let mut native = Native { calls: 0 };
    pin(&mut state);
    next(&mut state, &mut native);
    pin(&mut state);
    next(&mut state, &mut native);
    pin(&mut state);
    assert_eq!(
        state
            .open_chart_views()
            .unwrap()
            .entries()
            .iter()
            .map(|view| view.source_index())
            .collect::<Vec<_>>(),
        [3, 2, 1]
    );
    control(&mut state, KeyCode::PageDown, false);
    assert_eq!(state.workspace().chart(), 2);
    control(&mut state, KeyCode::KeyW, false);
    assert_eq!(state.workspace().chart(), 1);
    control(&mut state, KeyCode::KeyW, false);
    assert_eq!(state.workspace().chart(), 3);
    control(&mut state, KeyCode::KeyW, false);
    assert!(!state.workspace().has_chart());
    assert!(state.open_chart_views().unwrap().entries().is_empty());
    control(&mut state, KeyCode::KeyZ, false);
    control(&mut state, KeyCode::KeyY, false);
    assert!(!state.workspace().has_chart());
    assert_eq!(
        state.workspace().file().session.document().charts().len(),
        4
    );
    control(&mut state, KeyCode::KeyB, false);
    assert!(state.process_chart_order(&mut native));
    assert!(texts(&state).contains(&*i18n::tr("Edition", "ChartViewsHelp")));
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(state.open_chart_views().unwrap().entries().len(), 1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn draft_panels_protect_view_commands_and_no_focus_list_requests_reject_panel_changes() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, _source) = setup();
    let mut native = Native { calls: 0 };
    press(&mut state, KeyCode::F11);
    control(&mut state, KeyCode::KeyW, false);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(state.open_chart_views().unwrap().entries().len(), 1);
    press(&mut state, KeyCode::Escape);
    control(&mut state, KeyCode::KeyW, false);
    assert!(!state.workspace().has_chart());
    control(&mut state, KeyCode::KeyB, false);
    press(&mut state, KeyCode::F1);
    assert!(!state.process_chart_order(&mut native));
    assert_eq!(native.calls, 0);
    press(&mut state, KeyCode::Escape);
    control(&mut state, KeyCode::KeyB, false);
    assert!(state.process_chart_order(&mut native));
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
}
#[test]
fn closing_final_view_retains_exact_musical_instant_without_claiming_continued_playback() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup();
    let mut native = Native { calls: 0 };
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    let mut audio = GameplayAudioSnapshot::default();
    audio.stream_clock.valid_at = Instant::now();
    audio.stream_clock.has_music_mapping = true;
    audio.stream_clock.music_nanos = 1_010_000_000;
    state.update_audio(audio);
    control(&mut state, KeyCode::KeyW, false);
    assert!(!state.workspace().has_chart());
    assert!(!state.is_playing());
    assert!((state.chart_focus_time().unwrap() - 1.01).abs() < 1e-10);
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    control(&mut state, KeyCode::KeyB, false);
    assert!(state.process_chart_order(&mut native));
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(state.workspace().row(), 96);
    assert!((state.chart_focus_time().unwrap() - 1.01).abs() < 1e-10);
    press(&mut state, KeyCode::Enter);
    assert!(
        matches!(&state.take_audio_commands()[..],[GameplayAudioCommand::PlayMusic{cut,..}] if (cut.start_sec-1.01).abs()<1e-10)
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}

#[test]
fn real_note_history_on_closed_view_does_not_reopen_it_or_take_another_views_focus() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, before) = setup();
    let mut native = Native { calls: 0 };
    press(&mut state, KeyCode::Digit2);
    let after = state.workspace().file().session.serialize().unwrap();
    assert_ne!(after, before);
    control(&mut state, KeyCode::KeyW, false);
    control(&mut state, KeyCode::KeyZ, false);
    assert!(!state.workspace().has_chart());
    assert!(state.open_chart_views().unwrap().entries().is_empty());
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        before
    );
    control(&mut state, KeyCode::KeyY, false);
    assert!(!state.workspace().has_chart());
    assert_eq!(state.workspace().file().session.serialize().unwrap(), after);
    control(&mut state, KeyCode::KeyB, false);
    assert!(state.process_chart_order(&mut native));
    press(&mut state, KeyCode::End);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 3);
    control(&mut state, KeyCode::KeyZ, false);
    assert_eq!(state.workspace().chart(), 3);
    assert_eq!(state.open_chart_views().unwrap().entries().len(), 1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        before
    );
}
