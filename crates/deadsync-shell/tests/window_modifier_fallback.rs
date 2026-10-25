#[path = "../src/window_modifier_fallback.rs"]
mod window_modifier_fallback;

#[test]
fn command_snapshot_drives_editor_paste_when_no_physical_modifier_arrives() {
    use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
    use deadsync_editor::clipboard::{Error, TextClipboard};
    use deadsync_theme_simply_love::{i18n, screens::edition::State};
    use std::time::Instant;
    use winit::keyboard::ModifiersState;
    struct Board;
    impl TextClipboard for Board {
        fn read_text(&mut self) -> Result<String, Error> {
            Ok("Édition clavier QA".into())
        }
        fn write_text(&mut self, _: &str) -> Result<(), Error> {
            unreachable!()
        }
    }
    fn key(state: &mut State, code: KeyCode, pressed: bool) {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    std::fs::write(
        &path,
        "#VERSION:0.83;#TITLE:QA;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut state = State::open(&path, 0).unwrap();
    for code in [KeyCode::F7, KeyCode::ArrowUp, KeyCode::Enter] {
        key(&mut state, code, true);
        key(&mut state, code, false);
    }
    for (code, pressed) in window_modifier_fallback::changes([false; 4], ModifiersState::SUPER) {
        key(&mut state, code, pressed);
    }
    key(&mut state, KeyCode::KeyV, true);
    key(&mut state, KeyCode::KeyV, false);
    assert!(state.process_text_clipboard(&mut Board));
    let mut actors = Vec::new();
    state.actors(&mut actors);
    assert!(actors.iter().any(
        |a| matches!(a,deadlib_present::actors::Actor::Text {content,..}
        if content.to_string().contains("Édition clavier QA"))
    ));
}
