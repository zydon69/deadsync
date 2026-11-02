use super::*;
use deadsync_editor::key_binding_preferences::{
    Action, Preferences,
    source_profile::{self, Platform},
};

fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
}

#[test]
fn physical_source_reset_cancel_save_and_global_history_preserve_the_profile_en_fr() {
    for locale in ["en", "fr"] {
        i18n::init(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        let expected = source_profile::preset(&Preferences::default(), Platform::current())
            .unwrap()
            .preferences;
        let mut original = expected.clone();
        original
            .commands
            .insert(Action::Undo.id().into(), vec![vec!["F12".into()]]);
        state.configure_key_binding_preferences(Arc::new(original.clone()));
        let document = state.workspace.file().session.serialize().unwrap();
        for save in [false, true] {
            state.open_key_binding_settings();
            state.key_binding_draft.as_mut().unwrap().draft.action =
                Action::ALL.iter().position(|a| *a == Action::Undo).unwrap();
            // Navigate through the actual fixed keyboard rows, then activate Reset.
            for _ in 0..5 {
                press(&mut state, KeyCode::ArrowDown);
            }
            assert!(matches!(state.panel, Some(Panel::KeyBindingSettings(5))));
            press(&mut state, KeyCode::Enter);
            let staged = state.key_binding_draft.as_ref().unwrap().draft.staged();
            assert_eq!(
                staged.commands[Action::Undo.id()],
                expected.commands[Action::Undo.id()]
            );
            assert_eq!(*state.command_bindings.preferences, original);
            if save {
                press(&mut state, KeyCode::ArrowDown);
                press(&mut state, KeyCode::Enter);
            } else {
                press(&mut state, KeyCode::Escape);
            }
            assert!(state.panel.is_none());
            assert_eq!(
                state.workspace.file().session.serialize().unwrap(),
                document
            );
            assert!(!state.has_song());
            if !save {
                assert_eq!(*state.command_bindings.preferences, original);
            }
        }
        let committed = state.command_bindings.preferences.clone();
        assert_eq!(
            committed.commands[Action::Undo.id()],
            expected.commands[Action::Undo.id()]
        );
        assert_eq!(committed.source_import_raw, original.source_import_raw);
        assert!(committed.source_order);
        state.command(Command::Undo);
        assert_eq!(*state.command_bindings.preferences, original);
        state.command(Command::Redo);
        assert_eq!(*state.command_bindings.preferences, *committed);
        state.open_key_binding_settings();
        assert_eq!(
            *state.key_binding_draft.as_ref().unwrap().draft.baseline(),
            *committed
        );
        press(&mut state, KeyCode::Escape);
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            document
        );
        assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    }
}
