use deadsync_editor::{
    key_binding_preferences::{
        Action, Preferences,
        inspector::{Capture, Draft},
        source_profile::{self, Platform},
    },
    key_commands::{Binding, Effect, Key, Registry},
};
#[test]
fn arbitrary_key_sets_match_175_frames_of_the_original_complete_command_manager() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/multi-key-source.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/key_commands/multi-key-source.json")).unwrap();
    assert_eq!(
        expected["commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let mut frames = 0;
    for (input, case) in request
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["cases"].as_array().unwrap())
    {
        assert_eq!(input["name"], case["name"]);
        let bindings = input["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|binding| Binding {
                id: binding["id"].as_str().unwrap().into(),
                alternatives: binding["alternatives"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|chord| {
                        chord
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|key| Key::named(key.as_str().unwrap()))
                            .collect()
                    })
                    .collect(),
                repeat: binding["repeat"].as_bool().unwrap(),
                blocks: binding["blocks"].as_bool().unwrap(),
            });
        let mut registry = Registry::new(bindings);
        for query in case["conflicts"].as_array().unwrap() {
            let keys: Vec<_> = query["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| Key::named(key.as_str().unwrap()))
                .collect();
            let mut expected: Vec<_> = query["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_owned())
                .collect();
            expected.sort();
            expected.dedup();
            assert_eq!(
                registry.conflicts_with_policy(
                    query["id"].as_str().unwrap(),
                    &keys,
                    query["blocksInput"].as_bool().unwrap()
                ),
                expected,
                "{} advisory conflict set",
                case["name"]
            );
        }
        for frame in case["frames"].as_array().unwrap() {
            let keys = frame["down"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| Key::named(key.as_str().unwrap()));
            let actual: Vec<_> = registry
                .update(frame["time"].as_f64().unwrap(), keys)
                .into_iter()
                .map(|effect| match effect {
                    Effect::Press(id) | Effect::Repeat(id) => format!("activate:{id}"),
                    Effect::Release(id) => format!("release:{id}"),
                })
                .collect();
            let expected: Vec<_> = frame["effects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|effect| effect.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(actual, expected, "{} time{}", case["name"], frame["time"]);
            frames += 1;
        }
    }
    assert_eq!(frames, 175);
}
#[test]
fn configuration_import_storage_and_capture_preserve_all_ordinary_prefixes_and_modifier_finals() {
    let mut large = Preferences::default();
    large.commands.insert(
        "SnapToEighths".into(),
        vec![('A'..='J').map(|key| key.to_string()).collect()],
    );
    large.validate().unwrap();
    let raw = r#"{"SnapToEighths":[["A","B"]],"SnapToQuarters":[["F13","RightShift"]],"Arrow0":[["D1","F14"]],"Vendor":{"sequence":[1,2]}}"#;
    let prepared = source_profile::import(&Preferences::default(), raw, Platform::Other).unwrap();
    assert!(
        prepared.report.rejected.is_empty(),
        "{:?}",
        prepared.report.rejected
    );
    assert_eq!(
        prepared.preferences.commands["SnapToEighths"],
        vec![vec!["A", "B"]]
    );
    assert_eq!(
        prepared.preferences.commands["SnapToQuarters"],
        vec![vec!["F13", "LeftShift"]]
    );
    assert_eq!(
        prepared.preferences.commands["Arrow0"],
        vec![vec!["D1", "F14"]]
    );
    assert_eq!(prepared.preferences.source_import_raw.as_deref(), Some(raw));
    let mut fields = serde_json::Map::new();
    prepared.preferences.write(&mut fields).unwrap();
    assert_eq!(Preferences::read(&fields).unwrap(), prepared.preferences);
    let mut capture = Capture::new();
    capture.observe(&[]);
    capture.observe(&["B".into()]);
    capture.observe(&["A".into(), "B".into()]);
    capture.observe(&["A".into(), "B".into(), "LeftShift".into()]);
    capture.observe(&[]);
    assert_eq!(capture.keys(), ["B", "A", "LeftShift"]);
    let mut draft = Draft::capture(std::sync::Arc::new(Preferences::default())).unwrap();
    draft.replace(capture.keys().to_vec()).unwrap();
    assert_eq!(
        draft.staged().commands[Action::Save.id()],
        vec![vec!["B", "A", "LeftShift"]]
    );
}
#[test]
fn unsupported_keys_and_imported_f1_prefixes_are_rejected_whole_and_archived() {
    let raw = r#"{"SnapToEighths":[["A","OemPlus","B"],["F1","F13"],["A","A"]],"ToggleNoteEntryMode":[["F1","F14"]]}"#;
    let prepared = source_profile::import(&Preferences::default(), raw, Platform::Other).unwrap();
    assert_eq!(
        prepared.preferences.commands["SnapToEighths"],
        vec![Vec::<String>::new()]
    );
    assert_eq!(prepared.preferences.source_import_raw.as_deref(), Some(raw));
    assert!(
        !prepared
            .preferences
            .source_blockers
            .contains_key("ToggleNoteEntryMode")
    );
    assert_eq!(
        prepared
            .report
            .rejected
            .iter()
            .filter(|entry| entry.id == "SnapToEighths")
            .count(),
        3
    );
    assert!(
        prepared
            .report
            .applied
            .iter()
            .any(|id| id == "ToggleNoteEntryMode")
    );
    assert_eq!(
        prepared.preferences.commands["ToggleNoteEntryMode"],
        vec![Vec::<String>::new()]
    );
    assert!(
        prepared
            .report
            .rejected
            .iter()
            .any(|entry| entry.id == "ToggleNoteEntryMode")
    );
    assert!(!deadsync_editor::key_binding_preferences::supported_key(
        "F01"
    ));
    assert!(!deadsync_editor::key_binding_preferences::supported_key(
        "F0001"
    ));
    let mut prefs = Preferences::default();
    prefs.source_blockers.insert(
        "ToggleNoteEntryMode".into(),
        vec![vec!["F1".into(), "F13".into()]],
    );
    assert!(prefs.validate().is_err());
}
