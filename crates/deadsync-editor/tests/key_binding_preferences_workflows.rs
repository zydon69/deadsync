use deadsync_editor::{key_binding_preferences, key_commands};
use key_binding_preferences::{Action, Preferences};
use key_commands::{Effect, Key, Registry};
#[test]
fn native_defaults_keep_playback_selection_and_player_shortcuts() {
    let p = Preferences::default();
    p.validate().unwrap();
    assert_eq!(Action::ALL.len(), 136);
    assert_eq!(
        p.commands["PlayPause"],
        vec![vec!["Enter"], vec!["NumPadEnter"]]
    );
    assert_eq!(p.commands["SelectRowRange"], vec![vec!["Space"]]);
    assert_eq!(p.commands["TogglePlayer"], vec![vec!["P"]]);
    assert_eq!(Action::from_id("Save"), Some(Action::Save));
    assert!(Action::from_id("MouseSelectionAltBehavior").is_none());
}
#[test]
fn persisted_remap_drives_real_registry_and_roundtrips_unknown_source_commands() {
    let mut fields=serde_json::json!({"Save":[["Ctrl","F13"]],"UnportedSourceAction":[["OemQuestion"]],"Vendor":17}).as_object().unwrap().clone();
    let p = Preferences::read(&fields).unwrap();
    let mut registry = Registry::new(p.bindings());
    registry.update(0., vec![Key::named("LeftControl")]);
    assert_eq!(
        registry.update(0.1, vec![Key::named("LeftControl"), Key::named("F13")]),
        vec![Effect::Press("Save".into())]
    );
    p.write(&mut fields).unwrap();
    assert_eq!(fields["Vendor"], 17);
    assert_eq!(
        fields["UnportedSourceAction"],
        serde_json::json!([["OemQuestion"]])
    );
    assert_eq!(Preferences::read(&fields).unwrap(), p);
}
#[test]
fn unbinding_is_explicit_and_unsupported_or_duplicate_keys_are_rejected() {
    let mut p = Preferences::default();
    p.commands.insert("Save".into(), vec![vec![]]);
    p.validate().unwrap();
    let mut registry = Registry::new(p.bindings());
    registry.update(0., vec![Key::named("LeftControl")]);
    assert!(
        registry
            .update(0.1, vec![Key::named("LeftControl"), Key::named("S")])
            .is_empty()
    );
    for keys in [vec!["Imaginary"], vec!["LeftShift", "RightShift"]] {
        let mut invalid = p.clone();
        invalid.commands.insert(
            "Save".into(),
            vec![keys.into_iter().map(str::to_owned).collect()],
        );
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn capture_preserves_bindable_enter_escape_and_waits_for_opening_release() {
    use key_binding_preferences::inspector::{Capture, CapturePhase};
    for name in ["Enter", "Escape"] {
        let mut capture = Capture::new();
        capture.observe(&["Enter".into()]);
        assert_eq!(capture.phase(), CapturePhase::WaitingOpeningRelease);
        capture.observe(&[]);
        capture.observe(&[name.into()]);
        assert_eq!(capture.phase(), CapturePhase::Recording);
        capture.observe(&[]);
        assert_eq!(capture.phase(), CapturePhase::Review);
        assert_eq!(capture.keys(), &[name.to_owned()]);
    }
    let mut capture = Capture::new();
    capture.observe(&[]);
    capture.observe(&["RightShift".into()]);
    capture.observe(&["LeftShift".into(), "K".into()]);
    capture.observe(&[]);
    assert_eq!(capture.keys(), &["LeftShift".to_owned(), "K".to_owned()]);
}
#[test]
fn inspector_cancel_baseline_alternatives_and_owned_command_merge_are_atomic() {
    use key_binding_preferences::inspector::{Draft, merge};
    use std::sync::Arc;
    let baseline = Arc::new(Preferences::default());
    let mut draft = Draft::capture(Arc::clone(&baseline)).unwrap();
    assert!(!draft.changed());
    draft.add().unwrap();
    draft.replace(vec!["F13".into()]).unwrap();
    assert!(draft.changed());
    assert_eq!(draft.baseline(), baseline);
    draft.delete().unwrap();
    assert!(!draft.changed());
    draft.replace(vec!["F14".into()]).unwrap();
    let mut external = (*baseline).clone();
    external
        .commands
        .insert("Copy".into(), vec![vec!["F15".into()]]);
    let merged = merge(&external, &baseline, &draft.staged()).unwrap();
    assert_eq!(merged.commands["Copy"], vec![vec!["F15"]]);
    assert_eq!(merged.commands["Save"], vec![vec!["F14"]]);
    external
        .commands
        .insert("Save".into(), vec![vec!["F16".into()]]);
    let before = external.clone();
    assert!(merge(&external, &baseline, &draft.staged()).is_err());
    assert_eq!(external, before);
    draft.delete().unwrap();
    assert_eq!(draft.alternatives(), &[Vec::<String>::new()]);
    draft.reset_action().unwrap();
    assert!(!draft.changed());
}

#[test]
fn reservations_follow_native_modifier_predicates_and_protect_unmigrated_callbacks() {
    use key_binding_preferences::reservations;
    let chord = |names: &[&str]| {
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    };
    for names in [vec!["Escape"],vec!["F2"],vec!["Ctrl","P"],vec!["Ctrl","M"]] {
        assert!(reservations::conflicts(&chord(&names)), "{names:?}");
    }
    assert_eq!(Preferences::default().commands["OpenScrollSettings"],vec![vec!["LeftShift","S"]]);
    for names in [
        vec!["LeftShift","S"],
        vec!["Ctrl", "S"],
        vec!["Ctrl", "LeftShift", "Z"],
        vec!["P"],
        vec!["PageUp"],
        vec!["Ctrl", "Left"],
        vec!["F13"],
    ] {
        assert!(!reservations::conflicts(&chord(&names)), "{names:?}");
    }
    assert!(reservations::conflicts(&chord(&["LeftAlt", "M"])));
    assert!(!reservations::simple_chord(&chord(&["A", "F13"])));
}

#[test]
fn second_increment_retains_native_chords_and_source_repeat_policies() {
    let preferences = Preferences::default();
    preferences.validate().unwrap();
    for (id, keys) in [
        ("DecreaseSnap", vec!["Left"]),
        ("IncreaseSnap", vec!["Right"]),
        ("MoveToChartEnd", vec!["End"]),
        ("MoveToPreviousLabel", vec!["Ctrl", "LeftShift", "L"]),
        ("FocusNextChart", vec!["Ctrl", "PageDown"]),
        ("OpenPreviousChart", vec!["Ctrl", "LeftShift", "Tab"]),
        ("ToggleWaveForm", vec!["W"]),
        ("OpenWaveformSettings", vec!["LeftShift", "W"]),
        ("IncreaseMusicRate", vec!["LeftShift", "Right"]),
    ] {
        assert_eq!(preferences.commands[id], vec![keys]);
    }
    for id in [
        "DecreaseSnap",
        "IncreaseSnap",
        "MoveToPreviousLabel",
        "MoveToNextLabel",
        "FocusPreviousChart",
        "FocusNextChart",
        "DecreaseMusicRate",
        "IncreaseMusicRate",
    ] {
        assert!(Action::from_id(id).unwrap().repeats());
    }
    for id in [
        "OpenPreviousChart",
        "OpenNextChart",
        "MoveToChartEnd",
        "ToggleWaveForm",
        "OpenWaveformSettings",
    ] {
        assert!(!Action::from_id(id).unwrap().repeats());
    }
    let mut registry = Registry::new(preferences.bindings());
    registry.update(0., vec![Key::named("LeftControl"), Key::named("LeftShift")]);
    assert_eq!(
        registry.update(
            0.1,
            vec![
                Key::named("LeftControl"),
                Key::named("LeftShift"),
                Key::named("L")
            ]
        ),
        vec![Effect::Press("MoveToPreviousLabel".into())]
    );
}

#[test]
fn third_increment_names_file_and_chart_operations_without_aliasing_new_or_reload() {
    let preferences = Preferences::default();
    preferences.validate().unwrap();
    assert_eq!(preferences.commands["SaveAs"], vec![vec!["Ctrl", "F12"]]);
    assert_eq!(
        preferences.commands["CreateEmptyChart"],
        vec![vec!["Ctrl", "N"]]
    );
    assert_eq!(
        preferences.commands["RecoverDraft"],
        vec![vec!["Ctrl", "R"]]
    );
    assert_eq!(Action::from_id("New"), Some(Action::FileNew));
    assert_eq!(Action::from_id("Reload"), Some(Action::FileReload));
    for action in Action::ALL {
        assert_eq!(Action::from_id(action.id()), Some(action));
    }
}

#[test]
fn fourth_increment_uses_real_handlers_keeps_physical_lane_modifiers_reserved() {
    let p = Preferences::default();
    p.validate().unwrap();
    assert_eq!(
        p.commands["ResetKeyboardZoom"],
        vec![vec!["Ctrl", "D0"], vec!["Ctrl", "NumPad0"]]
    );
    for id in [
        "OpenNoteConversion",
        "OpenNoteOwners",
        "OpenNoteAttributes",
        "OpenTransforms",
    ] {
        assert_eq!(p.commands[id], vec![Vec::<String>::new()]);
    }
    for names in [
        vec!["D1"],
        vec!["LeftShift", "D1"],
        vec!["Ctrl", "P"],
        vec!["Ctrl", "M"],
        vec!["Escape"],
        vec!["F2"],
    ] {
        assert!(key_binding_preferences::reservations::conflicts(
            &names.into_iter().map(str::to_owned).collect::<Vec<_>>()
        ));
    }
    assert!(key_binding_preferences::supported_key("NumPad0"));
    assert!(Action::from_id("ArrowModification").is_none());
    assert_eq!(Action::from_id("New"), Some(Action::FileNew));
}
