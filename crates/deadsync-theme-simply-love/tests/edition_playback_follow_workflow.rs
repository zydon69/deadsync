//! Transport positions are synthetic mapped audio snapshots; no sound-device claim.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_gameplay::GameplayAudioSnapshot;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(s: &mut State, code: KeyCode) {
    event(s, code, true);
    event(s, code, false);
}
fn event(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn mode(s: &mut State, mode: usize) {
    if mode == 0 {
        return;
    }
    event(s, KeyCode::SuperLeft, true);
    if mode == 2 {
        event(s, KeyCode::ShiftLeft, true);
    }
    press(s, KeyCode::KeyT);
    event(s, KeyCode::ShiftLeft, false);
    event(s, KeyCode::SuperLeft, false);
}
fn snapshot(time: f64) -> GameplayAudioSnapshot {
    let mut s = GameplayAudioSnapshot::default();
    s.stream_clock.valid_at = Instant::now();
    s.stream_clock.has_music_mapping = true;
    s.stream_clock.music_nanos = (time * 1e9).round() as i64;
    s
}
fn arrow(s: &State) -> f64 {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .find_map(|a| match a {
            Actor::Sprite {
                z: 8004, offset, ..
            } => Some(offset[1] as f64),
            _ => None,
        })
        .expect("visible note")
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}
fn setup(delay: bool) -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.ogg"), b"command-only asset").unwrap();
    let p = d.path().join("song.ssc");
    fs::write(&p,format!("#VERSION:0.83;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#MUSIC:music.ogg;#OFFSET:0;#BPMS:0=120;{}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;",if delay {"#DELAYS:0=0.5;"}else{""})).unwrap();
    let s = State::open(&p, 0).unwrap();
    (d, s)
}
#[test]
fn mapped_fractional_positions_move_all_three_views_without_waiting_for_next_integer_row() {
    for m in 0..3 {
        for reverse in [false, true] {
            let (d, mut s) = setup(false);
            let bytes = fs::read(d.path().join("song.ssc")).unwrap();
            mode(&mut s, m);
            if reverse {
                event(&mut s, KeyCode::ShiftLeft, true);
                press(&mut s, KeyCode::KeyV);
                event(&mut s, KeyCode::ShiftLeft, false);
            }
            press(&mut s, KeyCode::Enter);
            s.take_audio_commands();
            s.update_audio(snapshot(0.125));
            assert_eq!(s.workspace().row(), 12);
            let a = arrow(&s);
            s.update_audio(snapshot(0.126));
            assert_eq!(s.workspace().row(), 12);
            let b = arrow(&s);
            let displacement = [0.096, 0.3, 0.12][m];
            close(b - a, if reverse { displacement } else { -displacement });
            assert!(s.is_playing());
            assert!(s.take_audio_commands().is_empty());
            assert!(!s.workspace().file().session.is_dirty());
            assert_eq!(fs::read(d.path().join("song.ssc")).unwrap(), bytes);
            press(&mut s, KeyCode::Enter);
            let stopped = arrow(&s);
            s.update_audio(snapshot(0.2));
            close(arrow(&s), stopped);
        }
    }
}
#[test]
fn time_view_advances_through_delay_while_edit_cursor_stays_at_same_row() {
    let (_d, mut s) = setup(true);
    mode(&mut s, 1);
    press(&mut s, KeyCode::Enter);
    s.update_audio(snapshot(0.1));
    assert_eq!(s.workspace().row(), 0);
    close(arrow(&s), 280.0);
    s.update_audio(snapshot(0.2));
    assert_eq!(s.workspace().row(), 0);
    close(arrow(&s), 250.0);
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn negative_lead_in_and_stale_audio_keep_display_independent_of_integer_edit_cursor() {
    for m in 0..3 {
        let (_d, mut s) = setup(false);
        mode(&mut s, m);
        let stale = snapshot(9.0);
        press(&mut s, KeyCode::Enter);
        s.update_audio(snapshot(-0.125));
        assert_eq!(s.workspace().row(), 0);
        close(arrow(&s), [172.0, 197.5, 175.0][m]);
        s.update_audio(stale);
        close(arrow(&s), [172.0, 197.5, 175.0][m]);
        assert_eq!(s.workspace().row(), 0);
        assert!(!s.workspace().file().session.is_dirty());
    }
}
