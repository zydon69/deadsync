use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::storage::FileSession;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    time::{Duration, Instant},
};
const SONG: &[u8] = b"#TITLE:Original;#BPMS:0=120;#NOTES:dance-single:Test:Hard:9:0,0,0,0,0:1000;";
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
#[test]
fn frame_updates_write_recoverable_drafts_and_preserve_foreign_drafts_in_both_languages() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    for language in ["en", "fr"] {
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.sm");
        fs::write(&path, SONG).unwrap();
        let mut screen = State::open(&path, 0).unwrap();
        let start = Instant::now();
        screen.update_with_time(start);
        press(&mut screen, KeyCode::Digit2);
        let expected = screen.workspace().file().session.serialize().unwrap();
        let recovery = screen.workspace().file().recovery_path();
        screen.update_with_time(start + Duration::from_secs(29));
        assert!(!recovery.exists());
        let now = start + Duration::from_secs(30);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            screen.update_with_time(now);
            if recovery.exists() && !screen.recovery_pending() {
                break;
            }
            assert!(Instant::now() < deadline, "Background draft did not appear");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(fs::read(&path).unwrap(), SONG);
        assert!(screen.workspace().file().session.is_dirty());
        let mut reopened = FileSession::open(&path).unwrap();
        reopened.recover().unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), expected);
        drop(screen);

        let previous = fs::read(&recovery).unwrap();
        let mut screen = State::open(&path, 0).unwrap();
        let start = Instant::now();
        screen.update_with_time(start);
        press(&mut screen, KeyCode::Digit3);
        let now = start + Duration::from_secs(30);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            screen.update_with_time(now);
            let mut actors = Vec::new();
            screen.actors(&mut actors);
            if actors.iter().any(|actor| matches!(actor, Actor::Text {content, ..} if content.as_ref() == i18n::tr("Edition", "RecoveryExists").as_ref())) { break; }
            assert!(
                Instant::now() < deadline,
                "Localized recovery conflict was not shown ({language}): {:?}",
                actors.iter().filter_map(|actor| match actor {
                    Actor::Text { content, z: 8006, .. } => Some(content.as_ref()),
                    _ => None,
                }).collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(fs::read(&recovery).unwrap(), previous);
        assert_eq!(fs::read(&path).unwrap(), SONG);
    }
}
