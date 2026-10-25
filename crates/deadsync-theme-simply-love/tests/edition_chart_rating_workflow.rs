use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::chart_identity::{Difficulty, Identity};
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
    texts(s).iter().any(|v| v.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn choose(s: &mut State, from: usize, to: usize) {
    for _ in 0..((to + 10 - from) % 10) {
        press(s, KeyCode::ArrowDown);
    }
}
fn identity(s: &State) -> Identity {
    Identity::read(s.workspace().file().session.document(), 0).unwrap()
}
fn number(s: &mut State, value: &str) {
    press(s, KeyCode::Delete);
    for c in value.chars() {
        press(
            s,
            match c {
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
                '-' => KeyCode::Minus,
                _ => panic!("unsupported digit"),
            },
        );
    }
}
#[test]
fn native_identity_edits_stage_apply_undo_redo_save_and_reopen_both_formats() {
    init();
    for (ext, song) in [
        (
            "ssc",
            "#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;",
        ),
        (
            "sm",
            "#BPMS:0=120;#VENDOR:keep;#NOTES2:dance-single:Desc\\:more:Hard:9:0,0,0,0,0:1000;",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{ext}"));
        fs::write(&path, song).unwrap();
        let mut s = State::open(&path, 0).unwrap();
        press(&mut s, KeyCode::F7);
        press(&mut s, KeyCode::ArrowUp);
        press(&mut s, KeyCode::ArrowUp);
        number(&mut s, "14");
        press(&mut s, KeyCode::Enter);
        assert_eq!(identity(&s).rating(), 9);
        press(&mut s, KeyCode::ArrowUp);
        press(&mut s, KeyCode::ArrowRight);
        choose(&mut s, 7, 4);
        press(&mut s, KeyCode::Enter);
        assert_eq!(identity(&s).rating(), 14);
        assert_eq!(identity(&s).difficulty(), Some(Difficulty::Challenge));
        assert!(has(&s, "Challenge"));
        assert_eq!(fs::read_to_string(&path).unwrap(), song);
        let after = s.workspace().file().session.serialize().unwrap();
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            song.as_bytes()
        );
        ctrl(&mut s, KeyCode::KeyY);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
        ctrl(&mut s, KeyCode::KeyS);
        let reopened = State::open(&path, 0).unwrap();
        assert_eq!(identity(&reopened).rating(), 14);
        assert_eq!(
            identity(&reopened).difficulty(),
            Some(Difficulty::Challenge)
        );
        assert!(String::from_utf8(after).unwrap().contains("#VENDOR:keep;"));
    }
}
#[test]
fn invalid_rating_blocks_apply_and_timing_then_cancel_preserves_source() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    for invalid in ["0", "-1", "9.5", "2147483648", ""] {
        press(&mut s, KeyCode::F7);
        press(&mut s, KeyCode::ArrowUp);
        press(&mut s, KeyCode::ArrowUp);
        number(&mut s, invalid);
        choose(&mut s, 8, 4);
        press(&mut s, KeyCode::Enter);
        assert!(
            has(&s, "Enter an integer rating"),
            "{invalid}: {:?}",
            texts(&s)
        );
        choose(&mut s, 4, 6);
        press(&mut s, KeyCode::Enter);
        assert!(has(&s, "Apply or discard the chart properties"));
        press(&mut s, KeyCode::Escape);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            song.as_bytes()
        );
    }
}
#[test]
fn rating_arrows_clamp_and_shift_steps_are_staged_until_apply() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:1;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowLeft);
    assert!(has(&s, "Rating: 1"));
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::ArrowRight);
    key(&mut s, KeyCode::ShiftLeft, false);
    assert!(has(&s, "Rating: 101"));
    assert_eq!(identity(&s).rating(), 1);
    number(&mut s, "2147483647");
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Rating: 2147483647"));
    choose(&mut s, 8, 4);
    press(&mut s, KeyCode::Enter);
    assert_eq!(identity(&s).rating(), i32::MAX);
}
