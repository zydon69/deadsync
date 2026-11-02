use deadsync_editor::key_binding_preferences::{
    Action, Preferences,
    source_profile::{self, Platform},
};
#[test]
fn reviewed_source_callbacks_are_explicitly_available_without_native_default_assignments() {
    let native = Preferences::default();
    let source_ids: Vec<_> = deadsync_editor::key_commands::source::COMMANDS
        .iter()
        .map(|command| command.id)
        .collect();
    for platform in [Platform::Mac, Platform::Other] {
        let prepared = source_profile::preset(&native, platform).unwrap();
        let mut added = 0;
        for action in Action::ALL {
            if action.is_selected_review() || action.id().starts_with("AddEvent") {
                added += 1;
                assert!(source_ids.contains(&action.id()));
                assert_eq!(native.commands[action.id()], vec![Vec::<String>::new()]);
                assert!(prepared.report.applied.iter().any(|id| id == action.id()));
                assert!(
                    !prepared
                        .preferences
                        .source_blockers
                        .contains_key(action.id())
                );
                assert!(
                    !prepared
                        .report
                        .unavailable_commands
                        .iter()
                        .any(|id| id == action.id())
                );
            }
        }
        assert_eq!(added, 58);
        for id in [
            "ShiftEarlier",
            "ShiftLater",
        ] {
            let action = Action::from_id(id).expect("selected mixed-event callback is ported");
            assert!(action.is_selected_review());
            assert!(source_ids.contains(&id));
            assert!(prepared.report.applied.iter().any(|applied| applied == id));
            assert!(!prepared.preferences.source_blockers.contains_key(id));
        }
        assert_eq!(
            prepared.preferences.commands["Mirror"],
            vec![vec![
                if platform == Platform::Mac {
                    "LeftWindows"
                } else {
                    "LeftControl"
                },
                "LeftShift",
                "M"
            ]]
        );
        assert!(
            !prepared
                .report
                .rejected
                .iter()
                .any(|entry| entry.id == "Mirror")
        );
        for id in ["ShiftAllEarlier", "ShiftAllLater"] {
            assert!(!source_ids.contains(&id));
            assert_eq!(
                prepared.preferences.commands[id],
                vec![Vec::<String>::new()]
            );
        }
    }
    assert_eq!(
        Action::ALL[0],
        Action::Save,
        "Keep the existing settings cursor and action order stable"
    );
}
#[test]
fn previously_archived_unavailable_chords_cannot_block_a_new_real_handler() {
    let mut preferences = Preferences::default();
    preferences.source_order = true;
    preferences
        .source_blockers
        .insert("Flip".into(), vec![vec!["F13".into()]]);
    preferences.validate().unwrap();
    assert!(
        !source_profile::blockers(&preferences)
            .iter()
            .any(|binding| source_profile::blocker_source(&binding.id) == Some("Flip"))
    );
    assert!(preferences.commands["Flip"].iter().all(Vec::is_empty));
}
