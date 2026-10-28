use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    key_binding_preferences::{
        Action, Preferences,
        inspector::{self, Draft},
        source_profile::{self, Platform},
    },
};
use std::{fs, sync::Arc};
#[test]
fn presets_use_executed_platform_defaults_and_neutralize_native_extensions_explicitly() {
    let native = Preferences::default();
    for (platform, physical) in [
        (Platform::Mac, "LeftWindows"),
        (Platform::Other, "LeftControl"),
    ] {
        let prepared = source_profile::preset(&native, platform).unwrap();
        let p = &prepared.preferences;
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/key_commands/source-defaults.json"))
                .unwrap();
        let expected_platform = if platform == Platform::Mac {
            "mac"
        } else {
            "other"
        };
        let archived: serde_json::Value =
            serde_json::from_str(p.source_import_raw.as_deref().unwrap()).unwrap();
        assert_eq!(archived, oracle["platforms"][expected_platform]);
        assert_eq!(p.commands["Save"], vec![vec![physical, "S"]]);
        assert_eq!(p.commands["SaveAs"], vec![vec![physical, "LeftShift", "S"]]);
        assert_eq!(p.commands["PlayPause"], vec![vec!["Space"]]);
        assert_eq!(p.commands["SelectRowRange"], vec![vec!["Tab"]]);
        assert_eq!(p.commands["TogglePlayer"], vec![vec!["OemQuestion"]]);
        assert_eq!(p.commands["New"], vec![vec![physical, "N"]]);
        assert_eq!(p.commands["Reload"], vec![vec![physical, "R"]]);
        for id in [
            "SaveRecoveryDraft",
            "RecoverDraft",
            "NextNoteTool",
            "PreviousNoteTool",
            "OpenWaveformSettings",
            "CreateEmptyChart",
        ] {
            assert_eq!(p.commands[id], vec![Vec::<String>::new()]);
        }
        assert_eq!(
            p.commands["OpenEditorMenu"],
            native.commands["OpenEditorMenu"]
        );
        assert!(
            prepared
                .report
                .neutralized_native_extensions
                .contains(&"SaveRecoveryDraft".to_owned())
        );
        assert!(
            !prepared
                .report
                .unavailable_commands
                .contains(&"New".to_owned())
        );
        assert!(
            !prepared
                .report
                .unavailable_commands
                .contains(&"Reload".to_owned())
        );
        let source_ids: std::collections::BTreeSet<_> =
            deadsync_editor::key_commands::source::COMMANDS
                .iter()
                .map(|c| c.id)
                .collect();
        let implemented = Action::ALL
            .iter()
            .filter(|a| source_ids.contains(a.id()))
            .count();
        assert_eq!(prepared.report.applied.len(), implemented);
        assert_eq!(
            prepared.report.unavailable_commands.len(),
            source_ids.len() - implemented
        );
        assert_eq!(prepared.report.retained_modifiers.len(), 6);
        assert!(
            prepared
                .report
                .rejected
                .iter()
                .all(|entry| entry.id == "Mirror"
                    && entry.reason == "Chord is owned by an unmigrated native context"),
            "{:?}",
            prepared.report.rejected
        );
    }
    assert_eq!(native.commands["Save"], vec![vec!["Ctrl", "S"]]);
}
#[test]
fn source_jsonc_import_retains_exact_unhandled_data_and_uses_physical_key_names() {
    let raw = "\u{feff}{\n// keep this exact source comment\n\"PreferencesKeyBinds\":{\"Save\":[[\"rightcontrol\",\"s\"]],\"Undo\":[],\"SelectAllNotes\":[[\"LeftWindows\",\"A\"]],\"ArrowModification\":[[\"RightShift\"]],\"Vendor\":{\"future\":[1,2,]},},\"OtherGroup\":17,}";
    let prepared = source_profile::import(&Preferences::default(), raw, Platform::Mac).unwrap();
    assert_eq!(prepared.preferences.source_import_raw.as_deref(), Some(raw));
    assert_eq!(
        prepared.preferences.commands["Save"],
        vec![vec!["LeftControl", "S"]]
    );
    assert_eq!(
        prepared.preferences.commands["Undo"],
        vec![Vec::<String>::new()]
    );
    assert_eq!(
        prepared.preferences.commands["PlayPause"],
        vec![vec!["Space"]]
    );
    assert!(
        prepared
            .report
            .unavailable_commands
            .contains(&"SelectAllNotes".to_owned())
    );
    assert!(
        prepared
            .report
            .retained_unknown
            .contains(&"Vendor".to_owned())
    );
    assert!(
        prepared
            .report
            .unavailable_commands
            .contains(&"ArrowModification".to_owned())
    );
    // Editor.cs registers ArrowModification with press/release callbacks.
    // ScrollZoom is one of the six modifier-only preferences, not a callback.
    assert!(
        prepared
            .report
            .retained_modifiers
            .contains(&"ScrollZoom".to_owned())
    );
    assert!(
        !prepared
            .report
            .retained_modifiers
            .contains(&"ArrowModification".to_owned())
    );
}
#[test]
fn unsupported_alternative_is_disabled_whole_instead_of_dropping_its_modifier() {
    let raw = r#"{"Save":[["Ctrl","S"],["LeftWindows","F13"]],"Undo":[["A","B"]],"Copy":[["Escape"]],"Cut":[["OemPlus","C"]]}"#;
    let prepared = source_profile::import(&Preferences::default(), raw, Platform::Mac).unwrap();
    assert_eq!(
        prepared.preferences.commands["Save"],
        vec![vec!["LeftWindows", "F13"]]
    );
    for id in ["Undo", "Copy", "Cut"] {
        assert_eq!(
            prepared.preferences.commands[id],
            vec![Vec::<String>::new()]
        );
    }
    assert_eq!(
        prepared
            .report
            .rejected
            .iter()
            .filter(|entry| matches!(entry.id.as_str(), "Save" | "Undo" | "Copy" | "Cut"))
            .count(),
        4
    );
    assert_eq!(prepared.preferences.source_import_raw.as_deref(), Some(raw));
}
#[test]
fn draft_cancel_and_archive_merge_preserve_ownership_atomically() {
    let baseline = Arc::new(Preferences::default());
    let prepared = source_profile::preset(&baseline, Platform::Mac).unwrap();
    let mut draft = Draft::capture(Arc::clone(&baseline)).unwrap();
    draft.stage_profile(prepared.preferences).unwrap();
    assert!(draft.changed());
    assert!(Arc::ptr_eq(&draft.baseline(), &baseline));
    let mut external = (*baseline).clone();
    external.source_import_raw = Some(Arc::from("other source"));
    let before = external.clone();
    assert!(inspector::merge(&external, &baseline, &draft.staged()).is_err());
    assert_eq!(external, before);
    let native = source_profile::native_defaults(&draft.staged());
    assert_eq!(native.source_import_raw, draft.staged().source_import_raw);
}
#[test]
fn global_profile_save_undo_redo_preserves_archive_unknown_native_fields_and_simfile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        br#"{"Version":1,"KeyBinds":{"Vendor":17,"UnportedSourceAction":[["OemPlus"]]}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let mut custom = (*file.key_bindings()).clone();
    custom
        .commands
        .insert("Save".into(), vec![vec!["F13".into()]]);
    let mut file = file;
    file.set_key_bindings(Arc::new(custom)).unwrap();
    file.save().unwrap();
    let before = file.key_bindings();
    let after = Arc::new(
        source_profile::preset(&before, Platform::Mac)
            .unwrap()
            .preferences,
    );
    let mut session = Session::new(
        Document::parse(
            b"#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let song = session.serialize().unwrap();
    session
        .set_key_binding_preferences(Arc::clone(&before), Arc::clone(&after), Some(&file))
        .unwrap();
    session.take_key_binding_preferences_update();
    assert_eq!(File::open(&path).unwrap().key_bindings(), after);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["KeyBinds"]["Vendor"], 17);
    assert_eq!(
        json["KeyBinds"]["UnportedSourceAction"],
        serde_json::json!([["OemPlus"]])
    );
    assert_eq!(
        json["KeyBinds"][source_profile::ARCHIVE_KEY].as_str(),
        after.source_import_raw.as_deref()
    );
    assert!(session.undo().unwrap());
    session.take_key_binding_preferences_update();
    assert_eq!(File::open(&path).unwrap().key_bindings(), before);
    assert!(session.redo().unwrap());
    session.take_key_binding_preferences_update();
    assert_eq!(File::open(&path).unwrap().key_bindings(), after);
    assert_eq!(session.serialize().unwrap(), song);
    assert!(!session.is_dirty());
}
#[test]
fn malformed_or_oversized_inputs_do_not_mutate_current_preferences() {
    let p = Preferences::default();
    let original = p.clone();
    for raw in [
        "[]",
        "{\"UnrelatedSettings\":1}",
        "{\"PreferencesKeyBinds\":false}",
        "{/* unclosed",
    ] {
        assert!(source_profile::import(&p, raw, Platform::Mac).is_err());
        assert_eq!(p, original);
    }
    assert!(
        source_profile::import(
            &p,
            &" ".repeat(source_profile::MAX_SOURCE_BYTES + 1),
            Platform::Mac
        )
        .is_err()
    );
}

#[test]
fn imported_equal_priority_commands_keep_original_callback_declaration_order() {
    use deadsync_editor::key_commands::{Effect, Key, Registry};
    let prepared = source_profile::import(
        &Preferences::default(),
        r#"{"New":[["F13"]],"Copy":[["F13"]]}"#,
        Platform::Mac,
    )
    .unwrap();
    assert!(prepared.preferences.source_order);
    let mut registry = Registry::new(prepared.preferences.bindings());
    assert_eq!(
        registry.update(0., vec![Key::named("F13")]),
        vec![Effect::Press("New".into()), Effect::Press("Copy".into())]
    );
    assert!(!source_profile::native_defaults(&prepared.preferences).source_order);
}

#[test]
fn unavailable_source_chords_block_shorter_callbacks_without_owning_native_contexts() {
    use deadsync_editor::key_commands::{Key, Registry};
    let prepared = source_profile::preset(&Preferences::default(), Platform::Other).unwrap();
    assert!(
        prepared
            .report
            .blocked_unavailable
            .iter()
            .any(|id| id == "Close")
    );
    assert!(
        prepared
            .report
            .retained_native_contexts
            .iter()
            .any(|id| id.starts_with("Arrow"))
    );
    for (terminal, available) in [("W", "CloseFocusedChart"), ("O", "Open")] {
        let mut registry = Registry::new(prepared.preferences.bindings());
        registry.update(0., ["LeftControl", "LeftShift"].map(Key::named));
        let effects = registry.update(0.1, ["LeftControl", "LeftShift", terminal].map(Key::named));
        assert!(!registry.is_active(available), "{effects:?}");
        assert!(registry.is_active_prefix(source_profile::BLOCKER_PREFIX));
    }
    for blocker in source_profile::blockers(&prepared.preferences) {
        for chord in blocker.alternatives {
            let names: Vec<_> = chord.iter().map(|key| key.name().to_owned()).collect();
            assert!(!deadsync_editor::key_binding_preferences::reservations::conflicts(&names));
            assert_ne!(names.last().map(String::as_str), Some("F1"));
        }
    }
}
#[test]
fn blockers_round_trip_and_are_owned_by_the_global_draft() {
    let native = Preferences::default();
    let source = source_profile::preset(&native, Platform::Other)
        .unwrap()
        .preferences;
    let mut fields = serde_json::Map::new();
    source.write(&mut fields).unwrap();
    assert_eq!(Preferences::read(&fields).unwrap(), source);
    let mut draft = Draft::capture(Arc::new(native.clone())).unwrap();
    draft.stage_profile(source.clone()).unwrap();
    let after = draft.staged();
    assert_eq!(
        inspector::merge(&native, &native, &after)
            .unwrap()
            .source_blockers,
        source.source_blockers
    );
    let mut concurrent = native.clone();
    concurrent.source_blockers = source.source_blockers.clone();
    assert!(inspector::merge(&concurrent, &native, &after).is_err());
    let reset = source_profile::native_defaults(&source);
    assert!(reset.source_blockers.is_empty());
    assert!(source_profile::blockers(&reset).is_empty());
}
#[test]
fn source_import_cannot_steal_f1_or_silently_narrow_unavailable_chords() {
    let prepared = source_profile::import(&Preferences::default(), r#"{"New":[["F1"]],"Close":[["LeftControl","LeftShift","F13"],["LeftControl","OemTilde"],["D1"]]}"#, Platform::Other).unwrap();
    assert_eq!(
        prepared.preferences.commands["New"],
        vec![Vec::<String>::new()]
    );
    assert_eq!(
        prepared.preferences.commands["OpenEditorMenu"],
        vec![vec!["F1"]]
    );
    assert_eq!(
        prepared.preferences.source_blockers["Close"],
        vec![vec!["LeftControl", "LeftShift", "F13"]]
    );
    assert!(
        prepared
            .report
            .rejected
            .iter()
            .any(|rejected| rejected.id == "New")
    );
    assert!(
        prepared
            .report
            .retained_native_contexts
            .iter()
            .any(|id| id == "Close")
    );
}
#[test]
fn malformed_persisted_blockers_fail_before_runtime_and_unknown_ids_never_activate() {
    let mut prefs = Preferences::default();
    prefs.source_order = true;
    prefs
        .source_blockers
        .insert("FutureCallback".into(), vec![vec!["F13".into()]]);
    prefs.validate().unwrap();
    assert!(source_profile::blockers(&prefs).is_empty());
    prefs
        .source_blockers
        .insert("Close".into(), vec![vec!["D1".into()]]);
    assert!(prefs.validate().is_err());
    prefs
        .source_blockers
        .insert("Close".into(), vec![vec!["F1".into()]]);
    assert!(prefs.validate().is_err());
}
