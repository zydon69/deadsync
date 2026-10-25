//! Real editor key dispatch and actors; timing expectations use the shared core.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    timing_edit::{Scope, TempoDraft},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Arc, time::Instant};
const SONG: &[u8] = b"#VERSION:0.83;#TITLE:Timing;#BPMS:0=120,4=180;#STOPS:2=0.5;#OFFSET:0.125;#LABELS:0=keep;#VENDOR:preserve;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=150;#OFFSET:0;#NOTES:0001;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010;";
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(screen: &mut State, code: KeyCode, pressed: bool) {
    screen.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(screen: &mut State, code: KeyCode) {
    key(screen, code, true);
    key(screen, code, false);
}
fn ctrl(screen: &mut State, code: KeyCode) {
    key(screen, KeyCode::SuperLeft, true);
    press(screen, code);
    key(screen, KeyCode::SuperLeft, false);
}
fn choose(screen: &mut State, index: &mut usize, target: usize) {
    while *index != target {
        press(screen, KeyCode::ArrowDown);
        *index = (*index + 1) % 10;
    }
}
fn type_number(screen: &mut State, value: &str) {
    press(screen, KeyCode::Delete);
    for digit in value.chars() {
        press(
            screen,
            match digit {
                '0' => KeyCode::Digit0,
                '1' => KeyCode::Digit1,
                '2' => KeyCode::Digit2,
                '3' => KeyCode::Digit3,
                '4' => KeyCode::Digit4,
                '5' => KeyCode::Digit5,
                '6' => KeyCode::Digit6,
                '7' => KeyCode::Digit7,
                '8' => KeyCode::Digit8,
                '9' => KeyCode::Digit9,
                '.' => KeyCode::Period,
                ',' => KeyCode::Comma,
                '-' => KeyCode::Minus,
                _ => panic!("number"),
            },
        );
    }
}
fn texts(screen: &State) -> Vec<String> {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| {
            if let Actor::Text { content, .. } = actor {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn contains(screen: &State, value: &str) -> bool {
    texts(screen).iter().any(|text| text.contains(value))
}
fn serialized(screen: &State) -> Vec<u8> {
    screen.workspace().file().session.serialize().unwrap()
}
fn expected(scope: Scope, row: u64, bpm: f64) -> Vec<u8> {
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    let mut draft = TempoDraft::capture(&source, 0, scope).unwrap();
    draft.set(row, bpm).unwrap();
    draft.apply(&mut source, 0).unwrap();
    source.serialize().unwrap()
}
#[test]
fn f10_local_tempo_stages_without_mutation_accepts_once_undoes_redoes_saves_and_reopens() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    assert!(contains(&screen, "this chart (1 charts, inherited)"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "165,25");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "BPM staged"));
    assert_eq!(serialized(&screen), SONG);
    assert_eq!(fs::read(&path).unwrap(), SONG);
    choose(&mut screen, &mut index, 0);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "before switching scope"));
    assert!(contains(&screen, "this chart"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let after = expected(Scope::Chart, 96, 165.25);
    assert_eq!(serialized(&screen), after);
    assert_eq!(fs::read(&path).unwrap(), SONG);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    key(&mut screen, KeyCode::ShiftLeft, true);
    ctrl(&mut screen, KeyCode::KeyZ);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert_eq!(serialized(&screen), after);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), after);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(serialized(&reopened), after);
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "STOPS")
            .as_deref(),
        Some("2=0.5")
    );
}
#[test]
fn song_scope_reports_inheritors_auto_stages_pending_input_and_preserves_own_chart_timing() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "shared song (2 charts"));
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "133.5");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), expected(Scope::Song, 0, 133.5));
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(Some(1), "BPMS").as_deref(), Some("0=150"));
    assert!(document.raw_property(Some(0), "BPMS").is_none());
}
#[test]
fn next_previous_move_and_delete_nodes_are_staged_together_and_cancel_preserves_redo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    ctrl(&mut screen, KeyCode::KeyZ);
    let redo = screen
        .workspace()
        .file()
        .session
        .redo_label()
        .unwrap()
        .to_owned();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 192);
    assert!(contains(&screen, "BPM: 180"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "BPM move staged"));
    assert_eq!(serialized(&screen), SONG);
    choose(&mut screen, &mut index, 6);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 0);
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 96);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "deletion staged"));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(serialized(&screen), SONG);
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "No timing changes"));
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
}
#[test]
fn invalid_values_initial_node_collision_and_unstaged_navigation_cannot_mutate_chart() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "initial BPM"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "initial BPM"));
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Stage your input"));
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::F10);
    index = 0;
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "already occupies"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "finite BPM"));
    assert_eq!(serialized(&screen), SONG);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "999999999999");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "nonnegative beat"));
    assert_eq!(serialized(&screen), SONG);
    assert_eq!(fs::read(&path).unwrap(), SONG);
}
#[test]
fn chart_properties_exposes_timing_and_prevents_losing_pending_offset() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F7);
    for _ in 0..2 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::ArrowRight);
    for _ in 0..4 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "before opening timing"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::F7);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Apply timing (2 BPM nodes)"));
}
#[test]
fn sm_and_legacy_ssc_start_with_shared_timing_and_reject_local_scope() {
    init();
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        (
            "song.sm",
            b"#TITLE:SM;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;".as_slice(),
        ),
        (
            "old.ssc",
            b"#VERSION:0.6;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        ),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        let mut screen = State::open(&path, 0).unwrap();
        press(&mut screen, KeyCode::F10);
        assert!(contains(&screen, "shared song"));
        press(&mut screen, KeyCode::ArrowRight);
        assert!(contains(&screen, "SSC 0.7 or newer"));
        let mut index = 0;
        choose(&mut screen, &mut index, 2);
        type_number(&mut screen, "160");
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        assert_eq!(
            screen
                .workspace()
                .file()
                .session
                .document()
                .text(None, "BPMS")
                .as_deref(),
            Some("0=160")
        );
        ctrl(&mut screen, KeyCode::KeyZ);
        assert_eq!(serialized(&screen), bytes);
    }
}

#[test]
fn accepted_move_then_delete_are_single_history_gestures_and_restore_original_bytes() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let moved = serialized(&screen);
    let document = screen.workspace().file().session.document();
    assert_eq!(
        document.text(Some(0), "BPMS").as_deref(),
        Some("0=120,2=180")
    );
    // Cursor navigation is independent of the staged position field.
    press(&mut screen, KeyCode::F10);
    index = 0;
    choose(&mut screen, &mut index, 6);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 96);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "BPMS")
            .as_deref(),
        Some("0=120")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), moved);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    assert_eq!(fs::read(&path).unwrap(), SONG);
}
#[test]
fn malformed_local_bpm_cannot_silently_open_a_shared_song_draft() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes =
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:broken;#NOTES:1000;";
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    assert!(contains(&screen, "Invalid BPM segment"));
    assert!(!contains(&screen, "Timing: shared song"));
    assert_eq!(serialized(&screen), bytes);
}

#[test]
fn numeric_controls_round_beats_adjust_bpm_and_preserve_help_preference() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F1);
    for _ in 0..12 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    let help_visible = |screen: &State| {
        let mut actors = Vec::new();
        screen.actors(&mut actors);
        actors
            .iter()
            .any(|actor| matches!(actor, Actor::Sprite { z: 8010, .. }))
    };
    assert!(help_visible(&screen));
    press(&mut screen, KeyCode::F10);
    assert!(!help_visible(&screen));
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowLeft);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "BPM: 120.9"));
    type_number(&mut screen, "130.54");
    press(&mut screen, KeyCode::Backspace);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0.1");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), expected(Scope::Chart, 5, 130.5));
    assert!(help_visible(&screen));
}

#[test]
fn stops_and_delays_share_one_draft_and_one_undo_without_changing_bpm() {
    use deadsync_editor::timing_edit::pause::{Draft, Kind};
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Stops (after notes)"));
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "-0.5");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Pause staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Delays (before notes)"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0.25");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Duration: -0.5 s"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Apply or discard before switching"));
    assert!(contains(&screen, "Stops (after notes)"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let mut original = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&original, 0, Scope::Chart).unwrap();
    draft.set(Kind::Stop, 48, -0.5).unwrap();
    draft.set(Kind::Delay, 48, 0.25).unwrap();
    draft.apply(&mut original, 0).unwrap();
    let after = original.serialize().unwrap();
    assert_eq!(serialized(&screen), after);
    assert_eq!(fs::read(&path).unwrap(), SONG);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    key(&mut screen, KeyCode::ShiftLeft, true);
    ctrl(&mut screen, KeyCode::KeyZ);
    key(&mut screen, KeyCode::ShiftLeft, false);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(serialized(&State::open(&path, 0).unwrap()), after);
}
#[test]
fn pause_type_input_and_scope_guards_cancel_and_zero_delay_validation_preserve_history() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Tab);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Stage your input"));
    assert!(contains(&screen, "Stops (after notes)"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "nonzero stop length"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::F10);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::ShiftLeft, false);
    index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "-1");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "nonnegative delay"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Pause staged"));
    choose(&mut screen, &mut index, 0);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "before switching scope"));
    choose(&mut screen, &mut index, 9);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
}
#[test]
fn zero_row_stops_can_move_and_delete_with_occupied_target_validation() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Tab);
    let mut index = 0;
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "already occupies"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "3");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Pause move staged"));
    choose(&mut screen, &mut index, 6);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 96);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "STOPS")
            .as_deref(),
        Some("3=0.5")
    );
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "DELAYS")
            .as_deref(),
        None
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
}
#[test]
fn shared_pause_scope_leaves_local_charts_alone_and_pending_bpm_blocks_type_change() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "130");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    assert!(contains(&screen, "BPM changes"));
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::Tab);
    index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0.5");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(None, "STOPS")
            .as_deref(),
        Some("0=0.5,2=0.5")
    );
    assert!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .raw_property(Some(0), "STOPS")
            .is_none()
    );
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(1), "BPMS")
            .as_deref(),
        Some("0=150")
    );
}
