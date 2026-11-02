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
        *index = (*index + 1) % 11;
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
                '/' => KeyCode::Slash,
                'x' => KeyCode::KeyX,
                'r' => KeyCode::KeyR,
                'o' => KeyCode::KeyO,
                'w' => KeyCode::KeyW,
                's' => KeyCode::KeyS,
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
    let output = screen
        .workspace()
        .file()
        .session
        .serialize_for_output()
        .unwrap();
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), output);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .serialize_for_output()
            .unwrap(),
        output
    );
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
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight); // Enable the independent chart offset.
    press(&mut screen, KeyCode::ArrowDown);
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
fn sm_keeps_shared_timing_and_legacy_ssc_migrates_before_local_timing() {
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
        if name.ends_with("ssc") {
            assert!(contains(&screen, "Convert timing and keep editing"));
            press(&mut screen, KeyCode::Enter);
            press(&mut screen, KeyCode::F10);
            press(&mut screen, KeyCode::ArrowLeft);
        } else {
            assert!(contains(&screen, "this chart"));
            press(&mut screen, KeyCode::ArrowLeft);
            assert!(contains(&screen, "shared song"));
        }
        let mut index = 0;
        choose(&mut screen, &mut index, 2);
        type_number(&mut screen, "160");
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        let session = &screen.workspace().file().session;
        let effective = TempoDraft::capture(session, 0, Scope::Song).unwrap();
        assert_eq!(effective.tempos().serialize(), "0=160");
        assert_eq!(
            session.document().text(None, "BPMS").as_deref(),
            if name.ends_with("sm") {
                Some("0=120")
            } else {
                Some("0=160")
            }
        );
        ctrl(&mut screen, KeyCode::KeyZ);
        if name.ends_with("ssc") {
            ctrl(&mut screen, KeyCode::KeyZ);
        }
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
fn numeric_controls_round_beats_adjust_bpm_after_explicit_help_dismissal() {
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
            .any(|actor| matches!(actor, Actor::Text { z: 8050, .. }))
    };
    assert!(help_visible(&screen));
    press(&mut screen, KeyCode::F10);
    assert!(help_visible(&screen)); // The help modal owns input until dismissed.
    press(&mut screen, KeyCode::Escape);
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
    assert!(!help_visible(&screen));
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
    assert_eq!(
        serialized(&State::open(&path, 0).unwrap()),
        screen
            .workspace()
            .file()
            .session
            .serialize_for_output()
            .unwrap()
    );
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
    for _ in 0..9 {
        press(&mut screen, KeyCode::Tab);
    }
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Delays (before notes)"));
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

fn region_request() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../deadsync-editor/tests/fixtures/grooveauthor/region-file-inherited.request.json"
    ))
    .unwrap()
}
fn open_region_panel(screen: &mut State) {
    press(screen, KeyCode::F10);
    for _ in 0..3 {
        press(screen, KeyCode::Tab);
    }
}
#[test]
fn native_warp_and_fake_edits_match_original_editor_flags_times_apply_undo_save_and_reopen() {
    use deadsync_editor::{
        generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
        timing_edit::region::{Draft, Kind},
    };
    use std::sync::atomic::AtomicBool;
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let request = region_request();
    let bytes = request["Content"].as_str().unwrap().as_bytes();
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_region_panel(&mut screen);
    let mut index = 0;
    assert!(contains(&screen, "Warps (length in beats)"));
    assert!(contains(&screen, "inherited"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Region staged"));
    assert_eq!(serialized(&screen), bytes);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Fake regions (length in beats)"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "3");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0.5");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    assert!(contains(&screen, "Fake regions"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Warps (length in beats)"));
    choose(&mut screen, &mut index, 6);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 96);
    assert!(contains(&screen, "Length: 2 beats"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let mut expected =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&expected, 0, Scope::Chart).unwrap();
    draft.set(Kind::Warp, 96, 96).unwrap();
    draft.set(Kind::Fake, 144, 24).unwrap();
    draft.apply(&mut expected, 0).unwrap();
    let after = expected.serialize().unwrap();
    assert_eq!(serialized(&screen), after);
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../deadsync-editor/tests/fixtures/grooveauthor/region-file-inherited.expected.json"
    ))
    .unwrap();
    let timing = GenerationTiming::from_simfile(&after, Format::Ssc, 0).unwrap();
    let timeline = EditorTimeline::build(
        &timing,
        screen.workspace().grid(),
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    let original = &reference["After"][0]["Editor"];
    for (head, node) in timeline
        .heads()
        .iter()
        .zip(original["Heads"].as_array().unwrap())
    {
        assert_eq!(head.position.row, node["Row"].as_u64().unwrap());
        assert_eq!(head.time, node["Time"].as_f64().unwrap());
        assert_eq!(head.fake, node["Fake"].as_bool().unwrap());
    }
    assert_eq!(
        timeline.heads().len(),
        original["Heads"].as_array().unwrap().len()
    );
    assert_eq!(
        timeline.step_count() as u64,
        original["StepCount"].as_u64().unwrap()
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), bytes);
    key(&mut screen, KeyCode::ShiftLeft, true);
    ctrl(&mut screen, KeyCode::KeyZ);
    key(&mut screen, KeyCode::ShiftLeft, false);
    let output = screen
        .workspace()
        .file()
        .session
        .serialize_for_output()
        .unwrap();
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), output);
    assert_eq!(
        serialized(&State::open(&path, 0).unwrap()),
        screen
            .workspace()
            .file()
            .session
            .serialize_for_output()
            .unwrap()
    );
}
#[test]
fn region_invalid_lengths_endpoint_overflow_pending_input_and_cancel_preserve_redo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let request = region_request();
    let bytes = request["Content"].as_str().unwrap().as_bytes();
    fs::write(&path, bytes).unwrap();
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
    open_region_panel(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    for value in ["0", "-1", "0.0001", "999999999999"] {
        type_number(&mut screen, value);
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        assert!(contains(&screen, "positive length of at least 1/48 beat"));
        assert_eq!(serialized(&screen), bytes);
        choose(&mut screen, &mut index, 2);
    }
    type_number(&mut screen, "1");
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Stage your input"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "44739242.64583333");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "positive length of at least 1/48 beat"));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(serialized(&screen), bytes);
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    open_region_panel(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "No timing changes"));
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
}
#[test]
fn initial_region_can_move_collide_navigate_and_delete_without_removing_other_type() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let request = region_request();
    let bytes = request["Content"].as_str().unwrap().as_bytes();
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_region_panel(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0.5");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "already starts at the destination"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Region move staged"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "44739242.64583333");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "positive length of at least 1/48 beat"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 6);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 48);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Region deletion staged"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let document = screen.workspace().file().session.document();
    assert_eq!(
        document.text(Some(0), "WARPS").as_deref(),
        Some("2=0.5,3=1")
    );
    assert_eq!(document.text(Some(0), "FAKES").as_deref(), Some("0=6,1=1"));
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), bytes);
}
#[test]
fn region_shared_scope_rounding_fine_controls_and_sm_type_filter_follow_format_rules() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let request = region_request();
    let bytes = request["Content"].as_str().unwrap().as_bytes();
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_region_panel(&mut screen);
    let mut index = 0;
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "shared song (2 charts"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0.02");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Length: 0.020833333333333332 beats"));
    choose(&mut screen, &mut index, 2);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let document = screen.workspace().file().session.document();
    let parsed = deadsync_editor::timing_edit::region::Regions::parse(
        deadsync_editor::timing_edit::region::Kind::Warp,
        &document.text(None, "WARPS").unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.values().get(&0), Some(&2));
    assert!(document.raw_property(Some(0), "WARPS").is_none());
    assert_eq!(document.text(Some(1), "WARPS").as_deref(), Some("2=1"));
    let sm = dir.path().join("song.sm");
    let bytes = b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&sm, bytes).unwrap();
    let mut screen = State::open(&sm, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..8 {
        press(&mut screen, KeyCode::Tab);
    }
    assert!(contains(&screen, "Hold tick counts"));
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Instant scroll rates"));
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Time signatures"));
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "BPM changes"));
    assert_eq!(serialized(&screen), bytes);
}

fn open_signature(screen: &mut State) {
    press(screen, KeyCode::F10);
    if screen.workspace().file().session.document().format == Format::Sm {
        press(screen, KeyCode::ArrowLeft); // Explicit shared timing for these SM scenarios.
        assert!(contains(screen, "shared song"), "{:?}", texts(screen));
    }
    key(screen, KeyCode::ShiftLeft, true);
    press(screen, KeyCode::Tab);
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Time signatures"), "{:?}", texts(screen));
}
fn configure_measure_geometry(screen: &mut State, directory: &std::path::Path) {
    let preferences = directory.join("measure-geometry.json");
    fs::write(&preferences, br#"{"Version":1,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1},"Receptors":{"PositionY":110}}"#).unwrap();
    screen.configure_audio_preferences(&preferences).unwrap();
}
fn second_measure_y(screen: &State) -> f32 {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    actors
        .into_iter()
        .find_map(|actor| match actor {
            Actor::Text {
                content,
                offset,
                scale,
                ..
            } if content.to_string() == "2" && scale[0] == 0.45 => Some(offset[1]),
            _ => None,
        })
        .expect("Second measure label")
}
#[test]
fn native_signatures_apply_change_measure_grid_and_undo_save_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    configure_measure_geometry(&mut screen, dir.path());
    assert_eq!(second_measure_y(&screen), 352.0);
    open_signature(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "3/4");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Signature staged"));
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    assert_eq!(serialized(&screen), SONG);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(second_measure_y(&screen), 304.0);
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(Some(0), "TIMESIGNATURES").unwrap(), "0=3=4");
    assert_eq!(
        document.text(Some(0), "STOPS"),
        document.text(None, "STOPS")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    assert_eq!(second_measure_y(&screen), 352.0);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened = State::open(&path, 0).unwrap();
    configure_measure_geometry(&mut reopened, dir.path());
    assert_eq!(second_measure_y(&reopened), 304.0);
}
#[test]
fn signature_validation_initial_immutability_cancel_and_unchanged_apply_preserve_file() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_signature(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "5/3");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Enter numerator/denominator"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Escape);
    open_signature(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Signature: 5/4"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Signature: 5/8"));
    press(&mut screen, KeyCode::Escape);
    open_signature(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "initial time signature"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "initial time signature"));
    press(&mut screen, KeyCode::Escape);
    open_signature(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
}
#[test]
fn signatures_on_sm_move_and_remove_in_draft_then_apply_one_shared_edit() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let bytes = b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_signature(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "7/8");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Signature moved"));
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Signature removed"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "5/8");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let session = &screen.workspace().file().session;
    let effective =
        deadsync_editor::timing_edit::signature::Draft::capture(session, 0, Scope::Song).unwrap();
    assert_eq!(effective.signatures().serialize(), "0=4=4,2=5=8");
    assert!(session.document().text(None, "TIMESIGNATURES").is_none());
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), bytes);
}

#[test]
fn file_reader_signature_denominators_can_move_without_being_revalidated_as_new_input() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    fs::write(
        &path,
        b"#BPMS:0=120;#TIMESIGNATURES:0=4=4,1=5=6;#NOTES:dance-single::Hard:9::1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_signature(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 7);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Signature: 5/6"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Signature moved"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let session = &screen.workspace().file().session;
    let effective =
        deadsync_editor::timing_edit::signature::Draft::capture(session, 0, Scope::Song).unwrap();
    assert_eq!(effective.signatures().serialize(), "0=4=4,2=5=6");
    assert_eq!(
        session.document().text(None, "TIMESIGNATURES").as_deref(),
        Some("0=4=4,1=5=6")
    );
}

fn open_scroll(screen: &mut State) {
    press(screen, KeyCode::F10);
    key(screen, KeyCode::ShiftLeft, true);
    press(screen, KeyCode::Tab);
    press(screen, KeyCode::Tab);
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Instant scroll rates"));
}
#[test]
fn native_scroll_zero_reverse_move_type_guard_apply_undo_save_and_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_scroll(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "-1");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Scroll staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    assert!(contains(&screen, "Instant scroll rates"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Scroll moved"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SCROLLS")
            .unwrap(),
        "0=0,1=-1"
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SCROLLS")
            .unwrap(),
        "0=0,1=-1"
    );
}
#[test]
fn scroll_empty_noop_cancel_and_sm_type_filter_preserve_file() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_scroll(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    open_scroll(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Scroll removed"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
    open_scroll(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowLeft);
    assert!(contains(&screen, "Scroll rate: 0.99x"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Scroll rate: 0.991x"));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(serialized(&screen), SONG);
    let sm = dir.path().join("song.sm");
    let bytes = b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&sm, bytes).unwrap();
    let mut screen = State::open(&sm, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::Tab);
    press(&mut screen, KeyCode::Tab);
    press(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Hold tick counts"));
    assert_eq!(serialized(&screen), bytes);
}

fn open_ticks(screen: &mut State) {
    let steps = 3; // Native SM and SSC share the full resident timing catalog.
    press(screen, KeyCode::F10);
    if screen.workspace().file().session.document().format == Format::Sm {
        press(screen, KeyCode::ArrowLeft); // These SM cases author shared song timing.
        assert!(contains(screen, "shared song"), "{:?}", texts(screen));
    }
    key(screen, KeyCode::ShiftLeft, true);
    for _ in 0..steps {
        press(screen, KeyCode::Tab);
    }
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Hold tick counts"), "{:?}", texts(screen));
}
#[test]
fn ticks_stage_zero_move_initial_guard_type_apply_undo_save_and_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_ticks(&mut screen);
    let mut index = 0;
    assert!(contains(&screen, "Ticks per beat: 4"));
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Tick count staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Tick count moved"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "8");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "TICKCOUNTS")
            .unwrap(),
        "0=8,1=0"
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(
        State::open(&path, 0)
            .unwrap()
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "TICKCOUNTS")
            .unwrap(),
        "0=8,1=0"
    );
}
#[test]
fn tick_integer_validation_controls_cancel_and_empty_apply_keep_document_clean() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_ticks(&mut screen);
    let mut index = 0;
    for value in ["-1", "1.5", "2147483648"] {
        choose(&mut screen, &mut index, 2);
        type_number(&mut screen, value);
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        assert!(contains(&screen, "Enter an integer tick count"));
        assert_eq!(serialized(&screen), SONG);
    }
    press(&mut screen, KeyCode::Escape);
    open_ticks(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Ticks per beat: 5"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowLeft);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Ticks per beat: 4"));
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Tick count removed"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
}
#[test]
fn sm_tick_events_are_accessible_and_signed_source_nodes_move_delete_and_save() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let bytes = b"#BPMS:0=120;#TICKCOUNTS:0=-2,4=4;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&path, bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_ticks(&mut screen);
    assert!(contains(&screen, "Ticks per beat: -2"));
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Tick count moved"));
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    let session = &reopened.workspace().file().session;
    let effective =
        deadsync_editor::timing_edit::tick::Draft::capture(session, 0, Scope::Song).unwrap();
    assert_eq!(effective.counts().serialize(), "1=0,4=4");
    assert_eq!(
        session.document().text(None, "TICKCOUNTS").as_deref(),
        Some("0=-2,4=4")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), bytes);
}

fn open_combos(screen: &mut State) {
    press(screen, KeyCode::F10);
    key(screen, KeyCode::ShiftLeft, true);
    for _ in 0..4 {
        press(screen, KeyCode::Tab);
    }
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Combo multipliers"), "{:?}", texts(screen));
}
#[test]
fn combo_pair_stage_move_zero_type_guard_apply_undo_and_save_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_combos(&mut screen);
    assert!(contains(&screen, "Hit / miss: 1x/1x"));
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "2x/3x");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Combo multipliers staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Combo event moved"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "0x/4x");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(Some(0), "COMBOS").unwrap(), "0=0=4,1=2=3");
    assert_eq!(
        document.text(Some(0), "STOPS"),
        document.text(None, "STOPS")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(
        State::open(&path, 0)
            .unwrap()
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "COMBOS")
            .unwrap(),
        "0=0=4,1=2=3"
    );
}
#[test]
fn combo_invalid_input_independent_controls_keypad_zero_and_noop_keep_history() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_combos(&mut screen);
    let mut index = 0;
    for value in ["2/3", "-1x/1x", "1x/2147483648x"] {
        choose(&mut screen, &mut index, 2);
        type_number(&mut screen, value);
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        assert!(contains(&screen, "Enter hitx/missx"));
        assert_eq!(serialized(&screen), SONG);
    }
    press(&mut screen, KeyCode::Escape);
    open_combos(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Hit / miss: 2x/1x"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Hit / miss: 2x/2x"));
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Delete);
    for code in [
        KeyCode::Numpad0,
        KeyCode::NumpadMultiply,
        KeyCode::NumpadDivide,
        KeyCode::Numpad0,
        KeyCode::NumpadMultiply,
    ] {
        press(&mut screen, code);
    }
    assert!(contains(&screen, "Hit / miss: 0x/0x"));
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Combo event removed"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
}
#[test]
fn signed_source_combo_moves_without_revalidation_and_sm_cycle_excludes_combos() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes = String::from_utf8(SONG.to_vec())
        .unwrap()
        .replace("#STOPS:", "#COMBOS:0=-2=3,4=1=1;#STOPS:");
    fs::write(&path, &bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_combos(&mut screen);
    assert!(contains(&screen, "Hit / miss: -2x/3x"));
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Combo event moved"));
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "1x/0x");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "COMBOS")
            .unwrap(),
        "2=1=0,4=1=1"
    );
    let sm = dir.path().join("song.sm");
    let original = b"#BPMS:0=120;#COMBOS:0=2=3;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&sm, original).unwrap();
    let mut screen = State::open(&sm, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..8 {
        press(&mut screen, KeyCode::Tab);
    }
    assert!(contains(&screen, "Hold tick counts"));
    assert_eq!(serialized(&screen), original);
}

fn open_speeds(screen: &mut State) {
    press(screen, KeyCode::F10);
    key(screen, KeyCode::ShiftLeft, true);
    for _ in 0..5 {
        press(screen, KeyCode::Tab);
    }
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Interpolated scroll speeds"));
}
#[test]
fn speeds_rows_seconds_two_decimal_fields_move_apply_undo_save_and_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_speeds(&mut screen);
    assert!(contains(&screen, "Speed / transition: 1x/0rows"));
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "2x/48rows");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Interpolated speed staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Speed moved"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, ".5x/.25s");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SPEEDS")
            .unwrap(),
        "0=0.5=0.25=1,1=2=1=0"
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(
        State::open(&path, 0)
            .unwrap()
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SPEEDS")
            .unwrap(),
        "0=0.5=0.25=1,1=2=1=0"
    );
}
#[test]
fn speeds_invalid_lengths_endpoint_limits_controls_control_shortcuts_and_noop_are_safe() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_speeds(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    for value in ["2x/1.5rows", "-1x/1s", "2x/2147483647rows"] {
        choose(&mut screen, &mut index, 2);
        type_number(&mut screen, value);
        choose(&mut screen, &mut index, 8);
        press(&mut screen, KeyCode::Enter);
        assert!(contains(&screen, "Enter 2x/48rows"));
        assert_eq!(serialized(&screen), SONG);
    }
    press(&mut screen, KeyCode::Escape);
    open_speeds(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Speed / transition: 1.01x/0rows"));
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Speed / transition: 1.01x/1rows"));
    ctrl(&mut screen, KeyCode::KeyS);
    assert!(contains(&screen, "Speed / transition: 1.01x/1rows"));
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Speed removed"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
    open_speeds(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, ".5x/.25s");
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert!(contains(&screen, "Speed / transition: 0.5x/0.251s"));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(serialized(&screen), SONG);
}
#[test]
fn speeds_preserve_signed_source_moves_and_full_precision_seconds_in_native_fields() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = String::from_utf8(SONG.to_vec())
        .unwrap()
        .replace("#STOPS:", "#SPEEDS:0=-1=-0.25=1,4=1=0=0;#STOPS:");
    fs::write(&path, source).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_speeds(&mut screen);
    assert!(contains(&screen, "Speed / transition: -1x/-0.25s"));
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Speed moved"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    ctrl(&mut screen, KeyCode::KeyS);
    let mut screen = State::open(&path, 0).unwrap();
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SPEEDS")
            .unwrap(),
        "2=-1=-0.25=1,4=1=0=0"
    );
    open_speeds(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "1.123456789012345x/0.123456789012345s");
    choose(&mut screen, &mut index, 3);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let value = screen
        .workspace()
        .file()
        .session
        .document()
        .text(Some(0), "SPEEDS")
        .unwrap();
    let parsed = deadsync_editor::timing_edit::speed::Speeds::parse(&value).unwrap();
    assert_eq!(
        parsed.values().get(&0).unwrap(),
        &deadsync_editor::timing_edit::speed::Speed::parse("1.123456789012345x/0.123456789012345s")
            .unwrap()
    );
}

fn open_labels(screen: &mut State) {
    press(screen, KeyCode::F10);
    key(screen, KeyCode::ShiftLeft, true);
    for _ in 0..6 {
        press(screen, KeyCode::Tab);
    }
    key(screen, KeyCode::ShiftLeft, false);
    assert!(contains(screen, "Section labels"), "{:?}", texts(screen));
}
#[test]
fn unicode_labels_stage_move_empty_apply_render_undo_and_save_reopen() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    assert!(!screen.text_input("ignored"));
    open_labels(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    assert!(screen.wants_text_input());
    press(&mut screen, KeyCode::Digit1);
    assert!(contains(&screen, "Label: keep"));
    assert!(screen.text_input("Pré-refrain 日本語 🎵 / "));
    assert!(!screen.text_input("\r"));
    choose(&mut screen, &mut index, 3);
    assert!(!screen.text_input("outside focus"));
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Label staged"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Tab);
    assert!(contains(&screen, "Apply or discard before switching"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "1");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Label moved"));
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "0");
    choose(&mut screen, &mut index, 2);
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Enter);
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let value = screen
        .workspace()
        .file()
        .session
        .document()
        .text(Some(0), "LABELS")
        .unwrap();
    assert_eq!(value, "0=,1=Pré-refrain 日本語 🎵 _ ,");
    assert!(texts(&screen).iter().any(|s| s
        == &format!(
            "{} Pré-refrain 日本語 🎵 _ ",
            i18n::tr("Edition", "EventSelectionLabel")
        )));
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), SONG);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(
        State::open(&path, 0)
            .unwrap()
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "LABELS")
            .unwrap(),
        value
    );
}
#[test]
fn label_composition_grapheme_deletion_equals_rejection_and_cancel_are_safe() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    open_labels(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    screen.text_preedit("に");
    assert!(contains(&screen, "Label: に"));
    press(&mut screen, KeyCode::Enter);
    assert!(screen.wants_text_input());
    assert_eq!(serialized(&screen), SONG);
    assert!(screen.text_input("日本語"));
    screen.text_preedit("未確定");
    screen.text_preedit("");
    assert!(contains(&screen, "Label: 日本語"));
    assert!(screen.text_input("e\u{301}👩‍👩‍👧‍👦"));
    key(&mut screen, KeyCode::Backspace, true);
    assert!(contains(&screen, "Label: 日本語e\u{301}"));
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::Backspace,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    key(&mut screen, KeyCode::Backspace, false);
    assert!(contains(&screen, "Label: 日本語"));
    key(&mut screen, KeyCode::SuperLeft, true);
    assert!(!screen.text_input("shortcut"));
    key(&mut screen, KeyCode::SuperLeft, false);
    press(&mut screen, KeyCode::Delete);
    assert!(screen.text_input("A=B"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "equals sign cannot be saved"));
    assert_eq!(serialized(&screen), SONG);
    press(&mut screen, KeyCode::Escape); // Discard the complete timing draft, including invalid text.
    open_labels(&mut screen);
    index = 0;
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(serialized(&screen), SONG);
    assert!(!screen.workspace().file().session.is_dirty());
}
#[test]
fn escaped_legacy_label_moves_preserve_text_and_sm_filter_does_not_change_unknown_labels() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes = String::from_utf8(SONG.to_vec())
        .unwrap()
        .replace("#LABELS:0=keep;", "#LABELS:0=Legacy\\/\\/intro,4=End;");
    fs::write(&path, &bytes).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    assert!(texts(&screen).iter().any(|s| s
        == &format!(
            "{} Legacy//intro",
            i18n::tr("Edition", "EventSelectionLabel")
        )));
    open_labels(&mut screen);
    let mut index = 0;
    choose(&mut screen, &mut index, 1);
    type_number(&mut screen, "2");
    choose(&mut screen, &mut index, 5);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Label moved"));
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened = State::open(&path, 0).unwrap();
    configure_measure_geometry(&mut reopened, dir.path());
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "LABELS")
            .unwrap(),
        "2=Legacy//intro,4=End"
    );
    assert!(
        texts(&reopened)
            .iter()
            .any(|s| s == &format!("{} End", i18n::tr("Edition", "EventSelectionLabel")))
    );
    let sm = dir.path().join("song.sm");
    let original = b"#BPMS:0=120;#LABELS:0=Keep;#NOTES:dance-single::Hard:9::1000;";
    fs::write(&sm, original).unwrap();
    let mut screen = State::open(&sm, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..8 {
        press(&mut screen, KeyCode::Tab);
    }
    assert!(contains(&screen, "Hold tick counts"));
    assert!(!screen.text_input("ignored"));
    assert_eq!(serialized(&screen), original);
}

#[test]
fn timing_menu_keeps_attack_rows_and_commits_attack_export_in_the_same_undo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let mut index = 0;
    choose(&mut screen, &mut index, 2);
    type_number(&mut screen, "240");
    choose(&mut screen, &mut index, 8);
    press(&mut screen, KeyCode::Enter);
    let timeline = deadsync_editor::attacks::placement::Timeline::capture(
        &screen.workspace().file().session,
        0,
    )
    .unwrap();
    assert_eq!(timeline.markers()[0].row, 96);
    assert_eq!(timeline.markers()[0].chart_time, 0.5);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        deadsync_editor::attacks::placement::Timeline::capture(
            &reopened.workspace().file().session,
            0
        )
        .unwrap()
        .markers()[0]
            .row,
        96
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), source);
}
