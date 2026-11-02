use deadsync_editor::key_binding_preferences::{
    Action, Preferences,
    inspector::Draft,
    source_profile::{self, Platform},
};
use std::sync::Arc;

fn select(draft: &mut Draft, action: Action) {
    draft.action = Action::ALL.iter().position(|a| *a == action).unwrap();
}

#[test]
fn every_source_action_resets_to_the_host_source_defaults_without_replacing_the_profile() {
    let expected = source_profile::preset(&Preferences::default(), Platform::current())
        .unwrap()
        .preferences;
    for action in Action::ALL {
        let mut before = expected.clone();
        before
            .commands
            .insert(action.id().into(), vec![vec!["F12".into()]]);
        let mut draft = Draft::capture(Arc::new(before.clone())).unwrap();
        select(&mut draft, action);
        draft.reset_action().unwrap();
        let after = draft.staged();
        assert_eq!(
            after.commands[action.id()],
            expected.commands[action.id()],
            "{}",
            action.id()
        );
        for peer in Action::ALL {
            if peer != action {
                assert_eq!(
                    after.commands[peer.id()],
                    before.commands[peer.id()],
                    "{} reset changed {}",
                    action.id(),
                    peer.id()
                );
            }
        }
        assert!(after.source_order);
        assert_eq!(after.source_import_raw, before.source_import_raw);
        assert_eq!(after.source_blockers, before.source_blockers);
        assert_eq!(after.scroll_modifiers, before.scroll_modifiers);
        assert_eq!(*draft.baseline(), before);
    }
}

#[test]
fn native_mode_reset_keeps_native_accelerators_even_with_a_retained_source_archive() {
    let imported = source_profile::preset(&Preferences::default(), Platform::current()).unwrap();
    let native = source_profile::native_defaults(&imported.preferences);
    assert!(!native.source_order);
    assert!(native.source_import_raw.is_some());
    for action in Action::ALL {
        let mut draft = Draft::capture(Arc::new(native.clone())).unwrap();
        select(&mut draft, action);
        draft.replace(vec!["F12".into()]).unwrap();
        draft.reset_action().unwrap();
        assert_eq!(
            draft.staged().commands[action.id()],
            native.commands[action.id()]
        );
        assert!(!draft.changed());
        assert_eq!(draft.staged().source_import_raw, native.source_import_raw);
    }
}

#[test]
fn foreign_import_reset_uses_running_platform_and_round_trip_keeps_the_exact_archive() {
    let host = Platform::current();
    let foreign = if host == Platform::Mac {
        Platform::Other
    } else {
        Platform::Mac
    };
    let imported = source_profile::preset(&Preferences::default(), foreign)
        .unwrap()
        .preferences;
    let expected = source_profile::preset(&Preferences::default(), host)
        .unwrap()
        .preferences;
    assert_ne!(
        imported.commands[Action::Undo.id()],
        expected.commands[Action::Undo.id()]
    );
    let mut draft = Draft::capture(Arc::new(imported.clone())).unwrap();
    select(&mut draft, Action::Undo);
    draft.reset_action().unwrap();
    assert_eq!(
        draft.staged().commands[Action::Undo.id()],
        expected.commands[Action::Undo.id()]
    );
    assert_eq!(draft.staged().source_import_raw, imported.source_import_raw);
    let mut fields = serde_json::Map::new();
    fields.insert("VendorUnknown".into(), serde_json::json!({"keep": true}));
    draft.staged().write(&mut fields).unwrap();
    assert_eq!(fields["VendorUnknown"], serde_json::json!({"keep": true}));
    assert_eq!(Preferences::read(&fields).unwrap(), *draft.staged());
}

#[test]
fn reset_of_an_unchanged_source_action_or_a_reverted_edit_is_a_noop() {
    let original = Arc::new(
        source_profile::preset(&Preferences::default(), Platform::current())
            .unwrap()
            .preferences,
    );
    let mut draft = Draft::capture(Arc::clone(&original)).unwrap();
    select(&mut draft, Action::Undo);
    draft.reset_action().unwrap();
    assert!(!draft.changed());
    draft.replace(vec!["F12".into()]).unwrap();
    assert!(draft.changed());
    draft.reset_action().unwrap();
    assert!(!draft.changed());
    assert_eq!(*draft.staged(), *original);
}
