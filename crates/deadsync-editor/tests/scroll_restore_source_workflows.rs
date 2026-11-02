use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    scroll_preferences::{restore::Snapshot, spacing::Preference},
};
use serde_json::{Value, json};
fn session() -> Session {
    Session::new(
        Document::parse(b"#VERSION:0.83;#TITLE:Unchanged;".as_slice(), Format::Ssc).unwrap(),
    )
}
fn prefs(fields: &Value) -> Value {
    let mut scroll = fields.as_object().unwrap().clone();
    let mut zoom = serde_json::Map::new();
    for key in [
        "ScrollInterpolationDuration",
        "ZoomMultiplier",
        "SizeCap",
        "LimitZoomToSize",
    ] {
        if let Some(value) = scroll.remove(key) {
            zoom.insert(key.into(), value);
        }
    }
    let waveform_mode = scroll
        .remove("RowBasedWaveFormScrollMode")
        .unwrap_or(json!(2));
    scroll.insert("Future".into(), json!(17));
    json!({"Version":1,"Scroll":scroll,"Zoom":zoom,"Waveform":{"RowBasedWaveFormScrollMode":waveform_mode,"EnableWaveForm":false,"Future":23},"Options":{"Future":29}})
}
fn assert_source(actual: Snapshot, expected: &Value) {
    let mut fields = serde_json::Map::new();
    actual.write(&mut fields);
    for (key, value) in fields {
        let expected = &expected[&key];
        if key == "SpacingMode" {
            assert_eq!(
                value,
                Preference::from_raw(expected.as_i64().unwrap() as i32).source_name()
            );
        } else if key == "RowBasedWaveFormScrollMode" {
            assert_eq!(
                value,
                ["CurrentTempo", "CurrentTempoAndRate", "MostCommonTempo"]
                    [expected.as_u64().unwrap() as usize]
            );
        } else if value.is_number() {
            assert_eq!(value.as_f64(), expected.as_f64(), "{key}");
        } else {
            assert_eq!(&value, expected, "{key}");
        }
    }
}
#[test]
fn original_restore_all_fields_undo_redo_preserves_visibility_other_groups_and_song() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/scroll-preferences.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        std::fs::write(&path, serde_json::to_vec(&prefs(&case["Changed"])).unwrap()).unwrap();
        let file = File::open(&path).unwrap();
        let before = Snapshot::capture(&file);
        assert_source(before, &case["Changed"]);
        let mut session = session();
        let bytes = session.serialize().unwrap();
        assert!(session.restore_scroll(before, Some(&file)).unwrap());
        let update = session.take_restore_scroll_update().unwrap();
        assert_source(update.preferences, &case["Restored"]);
        assert_eq!(session.undo_label(), Some("Restore Scroll defaults"));
        assert!(session.can_undo_application_change());
        assert!(session.undo().unwrap());
        assert_source(
            session.take_restore_scroll_update().unwrap().preferences,
            &case["Undone"],
        );
        assert!(session.redo().unwrap());
        assert_source(
            session.take_restore_scroll_update().unwrap().preferences,
            &case["Redone"],
        );
        let disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            disk["Scroll"]["ShowScrollControlPreferencesWindow"],
            case["Changed"]["ShowScrollControlPreferencesWindow"]
        );
        assert_eq!(disk["Scroll"]["Future"], 17);
        assert_eq!(disk["Waveform"]["Future"], 23);
        assert_eq!(disk["Waveform"]["EnableWaveForm"], false);
        assert_eq!(disk["Options"]["Future"], 29);
        assert_eq!(session.serialize().unwrap(), bytes);
        assert!(!session.is_dirty());
    }
}
#[test]
fn cap_only_source_noop_and_unknown_raw_real_restore_are_distinct() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    let bytes=br#"{"Version":1,"Zoom":{"SizeCap":0.5},"Scroll":{"ShowScrollControlPreferencesWindow":true}}"#;
    std::fs::write(&path, bytes).unwrap();
    let file = File::open(&path).unwrap();
    let mut history = session();
    assert!(
        !history
            .restore_scroll(Snapshot::capture(&file), Some(&file))
            .unwrap()
    );
    assert!(history.undo_label().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut file = file;
    file.set_spacing_preference(Preference::Unknown(99));
    file.save().unwrap();
    assert!(
        history
            .restore_scroll(Snapshot::capture(&file), Some(&file))
            .unwrap()
    );
    let update = history.take_restore_scroll_update().unwrap();
    assert_eq!(update.preferences.spacing, Preference::ConstantTime);
    assert_eq!(update.preferences.zoom.size_cap(), 1.);
    assert!(history.undo().unwrap());
    let update = history.take_restore_scroll_update().unwrap();
    assert_eq!(update.preferences.spacing, Preference::Unknown(99));
    assert_eq!(update.preferences.zoom.size_cap(), 0.5);
    assert_eq!(
        File::open(&path).unwrap().spacing_preference(),
        Preference::Unknown(99)
    );
}
#[test]
fn owned_external_conflict_refuses_without_popping_history_or_overwriting_disk() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Scroll":{"Reverse":true},"Zoom":{"ZoomMultiplier":2}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let mut history = session();
    history
        .restore_scroll(Snapshot::capture(&file), Some(&file))
        .unwrap();
    history.take_restore_scroll_update();
    let mut external: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    external["Zoom"]["ZoomMultiplier"] = json!(3.);
    let bytes = serde_json::to_vec(&external).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        history.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(history.undo_label(), Some("Restore Scroll defaults"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(history.take_restore_scroll_update().is_none());
}
#[test]
fn absent_preferences_action_is_application_only_and_unrelated_disk_changes_merge() {
    let mut history = session();
    let mut before = Snapshot::default();
    before.spacing = Preference::Unknown(-2);
    assert!(history.restore_scroll(before, None).unwrap());
    assert_eq!(
        history
            .take_restore_scroll_update()
            .unwrap()
            .preferences
            .spacing,
        Preference::ConstantTime
    );
    assert!(history.undo().unwrap());
    assert_eq!(
        history
            .take_restore_scroll_update()
            .unwrap()
            .preferences
            .spacing,
        Preference::Unknown(-2)
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Scroll":{"Reverse":true},"Options":{"Future":1}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let mut history = session();
    history
        .restore_scroll(Snapshot::capture(&file), Some(&file))
        .unwrap();
    history.take_restore_scroll_update();
    let mut external: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    external["Options"]["Future"] = json!(7);
    let bytes = serde_json::to_vec(&external).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(history.undo().unwrap());
    assert!(
        history
            .take_restore_scroll_update()
            .unwrap()
            .preferences
            .scroll
            .reverse()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap()["Options"]["Future"],
        7
    );
}
