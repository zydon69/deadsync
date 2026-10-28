use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
};
use std::{fs, sync::Arc};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#TITLE:Keep;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    )
}
#[test]
fn typed_bindings_track_changes_and_preserve_unknown_source_actions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        br#"{"Version":1,"KeyBinds":{"UnportedSourceAction":[["OemQuestion"]],"Vendor":17}}"#,
    )
    .unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.has_changes());
    let mut changed = (*file.key_bindings()).clone();
    changed
        .commands
        .insert("Save".into(), vec![vec!["F13".into()]]);
    file.set_key_bindings(Arc::new(changed)).unwrap();
    assert!(file.has_changes());
    file.save().unwrap();
    assert!(!file.has_changes());
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["KeyBinds"]["Vendor"], 17);
    assert_eq!(
        json["KeyBinds"]["UnportedSourceAction"],
        serde_json::json!([["OemQuestion"]])
    );
    let before = file.key_bindings();
    let mut invalid = (*before).clone();
    invalid
        .commands
        .insert("Save".into(), vec![vec!["NoSuchKey".into()]]);
    assert!(file.set_key_bindings(Arc::new(invalid)).is_err());
    assert_eq!(file.key_bindings(), before);
    assert!(!file.has_changes());
}
#[test]
fn global_save_is_one_undo_action_and_never_modifies_simfile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let before = file.key_bindings();
    let mut after = (*before).clone();
    after
        .commands
        .insert("Save".into(), vec![vec!["F13".into()]]);
    let after = Arc::new(after);
    let mut s = session();
    let bytes = s.serialize().unwrap();
    assert!(
        s.set_key_binding_preferences(Arc::clone(&before), Arc::clone(&after), Some(&file))
            .unwrap()
    );
    assert_eq!(
        s.take_key_binding_preferences_update().unwrap().preferences,
        after
    );
    assert!(s.undo_changes_key_binding_preferences());
    assert!(
        !s.set_key_binding_preferences(Arc::clone(&after), Arc::clone(&after), None)
            .unwrap()
    );
    assert!(s.undo().unwrap());
    assert_eq!(
        s.take_key_binding_preferences_update().unwrap().preferences,
        before
    );
    assert!(!s.undo().unwrap());
    assert!(s.redo().unwrap());
    assert_eq!(
        s.take_key_binding_preferences_update().unwrap().preferences,
        after
    );
    assert_eq!(s.serialize().unwrap(), bytes);
    assert!(!s.is_dirty());
}
#[test]
fn undo_merges_unrelated_actions_but_rejects_owned_action_conflicts_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let before = file.key_bindings();
    let mut after = (*before).clone();
    after
        .commands
        .insert("Save".into(), vec![vec!["F13".into()]]);
    let mut s = session();
    s.set_key_binding_preferences(Arc::clone(&before), Arc::new(after), Some(&file))
        .unwrap();
    s.take_key_binding_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["KeyBinds"]["Copy"] = serde_json::json!([["F15"]]);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(s.undo().unwrap());
    let update = s.take_key_binding_preferences_update().unwrap();
    assert_eq!(update.preferences.commands["Save"], before.commands["Save"]);
    assert_eq!(update.preferences.commands["Copy"], vec![vec!["F15"]]);
    assert!(s.redo().unwrap());
    s.take_key_binding_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["KeyBinds"]["Save"] = serde_json::json!([["F16"]]);
    let bytes = serde_json::to_vec(&json).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(s.undo().is_err());
    assert!(s.undo_changes_key_binding_preferences());
    assert!(s.take_key_binding_preferences_update().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!s.is_dirty());
}
