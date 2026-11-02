use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{playback::Clock, song_properties};
use std::time::Instant;
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
use deadsync_theme_simply_love::{i18n, screens::edition::State};
#[test]
fn native_entry_captures_open_sync_before_clocks_and_later_preferences_do_not_reinterpret_song() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;";
    std::fs::write(&song, source).unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(
        &prefs,
        r#"{"Version":1,"Options":{"OpenSongSyncOffset":0.375},"KeyBinds":{"Save":[["F16"]]}}"#,
    )
    .unwrap();
    let mut s = State::open_with_preferences(&song, 0, &prefs).unwrap();
    assert_eq!(
        song_properties::sync_offset(s.workspace().file().session.document()).unwrap(),
        0.375
    );
    let original_clock = Clock::capture(&s.workspace().file().session, 0)
        .unwrap()
        .music_time(48)
        .unwrap();
    assert_eq!(original_clock, 0.875);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    std::fs::write(
        &prefs,
        r#"{"Version":1,"Options":{"OpenSongSyncOffset":0.75},"KeyBinds":{"Save":[["F16"]]}}"#,
    )
    .unwrap();
    s.configure_audio_preferences(&prefs).unwrap();
    assert_eq!(
        Clock::capture(&s.workspace().file().session, 0)
            .unwrap()
            .music_time(48)
            .unwrap(),
        original_clock
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::F16);
    assert!(!s.workspace().file().session.is_dirty());
    let reopened = State::open_with_preferences(&song, 0, &prefs).unwrap();
    assert_eq!(
        song_properties::sync_offset(reopened.workspace().file().session.document()).unwrap(),
        0.375
    );
    assert_eq!(
        Clock::capture(&reopened.workspace().file().session, 0)
            .unwrap()
            .music_time(48)
            .unwrap(),
        original_clock
    );
}
