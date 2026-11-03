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
fn arrow(s: &State) -> f64 {
    transport_host::heads(s)
        .first()
        .expect("visible source head")[1] as f64
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
fn arrows_snap_in_screen_direction_and_seek_playing_audio_with_original_default_policy() {
    use deadsync_gameplay::GameplayAudioCommand;
    for m in 0..3 {
        for reverse in [false, true] {
            for down in [false, true] {
                for stop in [false, true] {
                    let (d, mut s) = setup(false);
                    let prefs = d.path().join("prefs.json");
                    fs::write(
                        &prefs,
                        format!(
                            "{{\"Version\":1,\"Scroll\":{{\"StopPlaybackWhenScrolling\":{stop}}}}}"
                        ),
                    )
                    .unwrap();
                    s.configure_audio_preferences(&prefs).unwrap();
                    s.configure_spacing_preference(
                        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
                    );
                    mode(&mut s, m);
                    if reverse {
                        event(&mut s, KeyCode::ShiftLeft, true);
                        press(&mut s, KeyCode::KeyV);
                        event(&mut s, KeyCode::ShiftLeft, false);
                    }
                    let source = fs::read(d.path().join("song.ssc")).unwrap();
                    press(&mut s, KeyCode::Enter);
                    s.take_audio_commands();
                    s.update_audio(transport_host::snapshot(&s, 0.126));
                    assert_eq!(s.workspace().row(), 12);
                    let stale = transport_host::snapshot(&s, 9.0);
                    press(
                        &mut s,
                        if down {
                            KeyCode::ArrowDown
                        } else {
                            KeyCode::ArrowUp
                        },
                    );
                    let row = if down != reverse { 48 } else { 0 };
                    assert_eq!(s.workspace().row(), row);
                    assert_eq!(s.is_playing(), !stop);
                    let commands = s.take_audio_commands();
                    if stop {
                        assert_eq!(commands, vec![GameplayAudioCommand::EditorPauseMusic {
                                owner: s.editor_asset_owner(),
                                seconds: 0.126
                            }]);
                    } else {
                        assert_eq!(
                            commands,
                            vec![GameplayAudioCommand::EditorSeekMusic {
                                owner: s.editor_asset_owner(),
                                seconds: row as f64 / 96.0
                            }]
                        );
                        let before = arrow(&s);
                        s.update_audio(stale);
                        close(arrow(&s), before);
                        assert_eq!(s.workspace().row(), row);
                    }
                    assert!(!s.workspace().file().session.is_dirty());
                    assert_eq!(fs::read(d.path().join("song.ssc")).unwrap(), source);
                }
            }
        }
    }
}
#[test]
fn repeated_keys_continue_from_snapped_destination_and_modal_panels_keep_arrow_focus() {
    let (_d, mut s) = setup(false);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    s.update_audio(transport_host::snapshot(&s, 0.126));
    let start = Instant::now();
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowDown,
        pressed: true,
        repeat: false,
        timestamp: start,
        host_nanos: 0,
    });
    assert_eq!(s.workspace().row(), 48);
    for (elapsed, row) in [(501, 96), (1002, 144)] {
        s.update_with_time(start + std::time::Duration::from_millis(elapsed));
        assert_eq!(s.workspace().row(), row);
        assert!(s.is_playing());
        s.take_audio_commands();
    }
    s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ArrowDown,
            pressed: false,
            repeat: false,
            timestamp: start + std::time::Duration::from_millis(1003),
            host_nanos: 0,
        });
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    press(&mut s, KeyCode::ArrowUp);
    assert_eq!(s.workspace().row(), 96);
    assert!(!s.is_playing());
    press(&mut s, KeyCode::F10);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 96);
}
#[test]
fn navigation_keeps_open_track_after_its_pathname_is_removed() {
    let (d, mut s) = setup(false);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    s.update_audio(transport_host::snapshot(&s, 0.126));
    fs::remove_file(d.path().join("music.ogg")).unwrap();
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.is_playing());
    assert_eq!(s.workspace().row(), 48);
    assert_eq!(
        s.take_audio_commands(),
        vec![deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic {
            owner: s.editor_asset_owner(),
            seconds: 0.5
        }]
    );
    assert!(!s.workspace().file().session.is_dirty());
}
