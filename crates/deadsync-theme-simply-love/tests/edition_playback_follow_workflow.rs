#[path = "support/transport_host.rs"]
mod transport_host;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::space::{Metrics, set_current_metrics};
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
fn note_positions(s: &State) -> Vec<[f32; 2]> {
    transport_host::heads(s)
}
fn arrow(s: &State) -> f64 {
    note_positions(s).first().expect("visible note")[1] as f64
}
fn same_arrow(s: &State, x: f32) -> f64 {
    // Autoplay hides passed notes; pausing reveals them for editing. Track the
    // same lane instead of comparing whichever note happens to be first.
    note_positions(s)
        .into_iter()
        .find(|offset| offset[0] == x)
        .expect("the tracked upcoming note remains visible")[1] as f64
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
    let mut s = State::open(&p, 0).unwrap();
    transport_host::configure_projection(&mut s, &d.path().join("prefs.json"), false);
    transport_host::publish(&mut s);
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
            s.update_audio(transport_host::snapshot(&s, 0.125));
            assert_eq!(s.workspace().row(), 12);
            let a = arrow(&s);
            s.update_audio(transport_host::snapshot(&s, 0.126));
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
            s.update_audio(transport_host::snapshot(&s, 0.2));
            close(arrow(&s), stopped);
        }
    }
}
#[test]
fn time_view_advances_through_delay_while_edit_cursor_stays_at_same_row() {
    let (_d, mut s) = setup(true);
    mode(&mut s, 1);
    press(&mut s, KeyCode::Enter);
    s.update_audio(transport_host::snapshot(&s, 0.1));
    assert_eq!(s.workspace().row(), 0);
    close(arrow(&s), 280.0);
    s.update_audio(transport_host::snapshot(&s, 0.2));
    assert_eq!(s.workspace().row(), 0);
    close(arrow(&s), 250.0);
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn negative_lead_in_and_stale_audio_keep_display_independent_of_integer_edit_cursor() {
    for m in 0..3 {
        let (_d, mut s) = setup(false);
        mode(&mut s, m);
        let stale = transport_host::snapshot(&s, 9.0);
        press(&mut s, KeyCode::Enter);
        s.update_audio(transport_host::snapshot(&s, -0.125));
        assert_eq!(s.workspace().row(), 0);
        close(arrow(&s), [172.0, 197.5, 175.0][m]);
        s.update_audio(stale);
        close(arrow(&s), [172.0, 197.5, 175.0][m]);
        assert_eq!(s.workspace().row(), 0);
        assert!(!s.workspace().file().session.is_dirty());
    }
}

fn assert_play_start(s: &mut State, seconds: f64) {
    use deadsync_gameplay::GameplayAudioCommand;
    let commands = s.take_audio_commands();
    match commands.as_slice() {
        [
            GameplayAudioCommand::EditorStartMusic {
                seconds: start,
                rate,
                ..
            },
        ]
        | [
            GameplayAudioCommand::EditorResumeMusic {
                seconds: start,
                rate,
                ..
            },
        ] => {
            assert_eq!(*start, seconds);
            assert_eq!(*rate, 1.0);
        }
        other => panic!("expected one source start/resume command, got {other:?}"),
    }
}
#[test]
fn pause_resume_preserves_exact_position_and_audio_cut_in_every_view_and_direction() {
    use deadsync_gameplay::GameplayAudioCommand;
    for m in 0..3 {
        for reverse in [false, true] {
            let (d, mut s) = setup(false);
            let source = fs::read(d.path().join("song.ssc")).unwrap();
            mode(&mut s, m);
            if reverse {
                event(&mut s, KeyCode::ShiftLeft, true);
                press(&mut s, KeyCode::KeyV);
                event(&mut s, KeyCode::ShiftLeft, false);
            }
            press(&mut s, KeyCode::Enter);
            assert_play_start(&mut s, 0.0);
            s.update_audio(transport_host::snapshot(&s, 0.126));
            let [x, y] = *note_positions(&s).first().expect("upcoming note");
            let y = y as f64;
            let row = s.workspace().row();
            assert_eq!(row, 12);
            press(&mut s, KeyCode::Enter);
            assert!(!s.is_playing());
            close(same_arrow(&s, x), y);
            assert_eq!(
                s.take_audio_commands(),
                vec![GameplayAudioCommand::EditorPauseMusic {
                    owner: s.editor_asset_owner(),
                    seconds: s.chart_focus_time().unwrap()
                }]
            );
            s.update_audio(transport_host::snapshot(&s, 9.0));
            close(same_arrow(&s, x), y);
            assert_eq!(s.workspace().row(), row);
            press(&mut s, KeyCode::Enter);
            assert!(s.is_playing());
            assert_play_start(&mut s, 0.126);
            close(same_arrow(&s, x), y); // No snap while the audio engine installs the new mapping.
            press(&mut s, KeyCode::Enter);
            s.take_audio_commands();
            close(same_arrow(&s, x), y);
            press(&mut s, KeyCode::Enter);
            assert_play_start(&mut s, 0.126);
            s.update_audio(transport_host::snapshot(&s, 0.127));
            assert_ne!(same_arrow(&s, x), y);
            assert!(!s.workspace().file().session.is_dirty());
            assert_eq!(fs::read(d.path().join("song.ssc")).unwrap(), source);
        }
    }
}
#[test]
fn pause_resume_keeps_position_inside_delay_and_negative_lead_in() {
    for (delay, time, expected) in [(true, 0.2, 250.0), (false, -0.125, 197.5)] {
        let (_d, mut s) = setup(delay);
        mode(&mut s, 1);
        press(&mut s, KeyCode::Enter);
        s.take_audio_commands();
        s.update_audio(transport_host::snapshot(&s, time));
        assert_eq!(s.workspace().row(), 0);
        close(arrow(&s), expected);
        press(&mut s, KeyCode::Enter);
        s.take_audio_commands();
        close(arrow(&s), expected);
        press(&mut s, KeyCode::Enter);
        assert_play_start(&mut s, time);
        close(arrow(&s), expected);
    }
}
#[test]
fn navigation_and_edits_discard_paused_position_before_next_play() {
    for edit in [false, true] {
        let (_d, mut s) = setup(false);
        press(&mut s, KeyCode::Enter);
        s.take_audio_commands();
        s.update_audio(transport_host::snapshot(&s, 0.126));
        press(&mut s, KeyCode::Enter);
        s.take_audio_commands();
        if edit {
            press(&mut s, KeyCode::Digit2);
        } else {
            press(&mut s, KeyCode::ArrowDown);
        }
        assert_eq!(s.workspace().row(), if edit { 12 } else { 48 });
        assert_eq!(s.workspace().file().session.is_dirty(), edit);
        press(&mut s, KeyCode::Enter);
        assert_play_start(&mut s, if edit { 0.125 } else { 0.5 });
    }
}
#[test]
fn older_mapped_or_unmapped_snapshots_cannot_rewind_or_stop_current_transport() {
    let (_d, mut s) = setup(false);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    let mut stale = transport_host::snapshot(&s, 0.1);
    let latest = transport_host::snapshot(&s, 0.126);
    s.update_audio(latest);
    let [x, y] = *note_positions(&s).first().expect("upcoming note");
    let y = y as f64;
    s.update_audio(stale);
    close(same_arrow(&s, x), y);
    assert_eq!(s.workspace().row(), 12);
    stale.stream_clock.has_music_mapping = false;
    s.update_audio(stale);
    assert!(s.is_playing());
    assert!(s.take_audio_commands().is_empty());
    let mut ended = transport_host::snapshot(&s, 0.0);
    ended.stream_clock.has_music_mapping = false;
    s.update_audio(ended);
    assert!(!s.is_playing());
    close(same_arrow(&s, x), y);
    s.take_audio_commands();
    press(&mut s, KeyCode::Enter);
    assert_play_start(&mut s, 0.126);
}
#[test]
fn pausing_before_first_audio_mapping_resumes_from_requested_edit_row() {
    let (_d, mut s) = setup(false);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_play_start(&mut s, 0.5);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    press(&mut s, KeyCode::Enter);
    assert_play_start(&mut s, 0.5);
    assert_eq!(s.workspace().row(), 48);
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn resume_retains_original_audio_time_with_music_and_editor_offsets() {
    let (d, _) = setup(false);
    let p = d.path().join("song.ssc");
    let source = fs::read_to_string(&p)
        .unwrap()
        .replace("#OFFSET:0;", "#OFFSET:0.25;")
        .replace("\"SyncOffset\":0}", "\"SyncOffset\":0.009}");
    fs::write(&p, &source).unwrap();
    let mut s = State::open(&p, 0).unwrap();
    transport_host::configure_projection(&mut s, &d.path().join("prefs.json"), false);
    transport_host::publish(&mut s);
    press(&mut s, KeyCode::Enter);
    assert_play_start(&mut s, -0.25 + 0.009);
    let seconds = 0.123456789;
    s.update_audio(transport_host::snapshot(&s, seconds));
    let row = s.workspace().row();
    for _ in 0..3 {
        press(&mut s, KeyCode::Enter);
        s.take_audio_commands();
        press(&mut s, KeyCode::Enter);
        assert_play_start(&mut s, seconds);
    }
    assert_eq!(s.workspace().row(), row);
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(&p).unwrap(), source);
}
#[test]
fn display_changes_while_paused_use_same_fractional_focal_position() {
    let (_d, mut s) = setup(false);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    s.update_audio(transport_host::snapshot(&s, 0.126));
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    close(arrow(&s), 147.904);
    mode(&mut s, 1);
    close(arrow(&s), 122.2);
    mode(&mut s, 2);
    close(arrow(&s), 144.88);
    let x = note_positions(&s)
        .into_iter()
        .find(|point| point[1] > 160.0)
        .expect("upcoming note")[0];
    event(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::KeyV);
    event(&mut s, KeyCode::ShiftLeft, false);
    close(same_arrow(&s, x), 295.12);
    assert!(!s.is_playing());
    assert!(s.take_audio_commands().is_empty());
    press(&mut s, KeyCode::Enter);
    assert_play_start(&mut s, 0.126);
    close(same_arrow(&s, x), 295.12);
}
