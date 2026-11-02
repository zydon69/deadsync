use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    options_preferences::restore::Snapshot,
};
use serde_json::Value;
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Restore;#BPMS:0=120;".as_slice(), Format::Sm).unwrap())
}
fn options(file: &File) -> Value {
    let mut fields = serde_json::Map::new();
    Snapshot::capture(file).write(&mut fields).unwrap();
    Value::Object(fields)
}
#[test]
fn all_twenty_two_source_defaults_restore_as_one_atomic_history_action() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/options.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        let mut changed = case["Changed"].clone();
        changed["Future"] = 7.into();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"Version":1,"Options":changed})).unwrap(),
        )
        .unwrap();
        let file = File::open(&path).unwrap();
        let before = Snapshot::capture(&file);
        let mut s = session();
        let bytes = s.serialize().unwrap();
        assert!(s.restore_options(before.clone(), Some(&file)).unwrap());
        let update = s.take_restore_options_update().unwrap();
        let file = update.file.unwrap();
        let restored = options(&file);
        for (key, value) in restored.as_object().unwrap() {
            if matches!(key.as_str(), "NewSongSyncOffset" | "OpenSongSyncOffset" | "DpiScale") {
                // These are source double fields; .NET writes whole doubles
                // without a decimal point while serde retains `1.0`.
                assert_eq!(value.as_f64().unwrap(), case["Restored"][key].as_f64().unwrap(), "{key}");
            } else {
                assert_eq!(value, &case["Restored"][key], "{key}");
            }
        }
        let disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for key in [
            "ResetLayout",
            "MaxMarkersToDraw",
            "MaxEventsToDraw",
            "MaxRateAlteringEventsToProcessPerFrame",
            "MiniMapMaxNotesToDraw",
            "Future",
        ] {
            assert_eq!(disk["Options"][key], changed[key], "preserve {key}");
        }
        assert!(s.undo().unwrap());
        assert_eq!(s.take_restore_options_update().unwrap().preferences, before);
        assert!(!s.undo().unwrap(), "single action");
        assert!(s.redo().unwrap());
        assert_eq!(
            s.take_restore_options_update().unwrap().preferences,
            before.restored()
        );
        assert_eq!(s.serialize().unwrap(), bytes);
        assert!(!s.is_dirty());
    }
}
#[test]
fn owned_conflict_refuses_without_partial_disk_history_or_capacity_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path,r#"{"Version":1,"Options":{"UndoHistorySize":32,"OpenSongSyncOffset":0.25,"RenderNotes":false}}"#).unwrap();
    let file = File::open(&path).unwrap();
    let before = Snapshot::capture(&file);
    let mut external = File::open(&path).unwrap();
    let mut p = external.general_options().clone();
    p.open_sync = 0.5;
    external.set_general_options(p).unwrap();
    external.save().unwrap();
    let disk = std::fs::read(&path).unwrap();
    let mut s = session();
    assert!(s.restore_options(before, Some(&file)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    assert_eq!(s.history_capacity(), 1024);
    assert!(!s.undo().unwrap());
    assert!(s.take_restore_options_update().is_none());
}
#[test]
fn unrelated_concurrent_options_and_note_palette_changes_survive_restore_and_undo() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"RenderNotes":false,"OpenSongSyncOffset":0.25,"Future":1}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let before = Snapshot::capture(&file);
    let mut external = File::open(&path).unwrap();
    let mut colors = external.note_colors_arc().as_ref().clone();
    colors.selection_multiplier = 2.5;
    external
        .set_note_colors(std::sync::Arc::new(colors))
        .unwrap();
    external.save().unwrap();
    let mut s = session();
    assert!(s.restore_options(before, Some(&file)).unwrap());
    let update = s.take_restore_options_update().unwrap();
    assert_eq!(
        update.file.unwrap().note_colors_arc().selection_multiplier,
        2.5
    );
    assert!(s.undo().unwrap());
    let file = s.take_restore_options_update().unwrap().file.unwrap();
    assert_eq!(file.note_colors_arc().selection_multiplier, 2.5);
    assert_eq!(options(&file)["OpenSongSyncOffset"], 0.25);
}
