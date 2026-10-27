use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    minimap_palette::ColorMethod,
    note_color_preferences::Preferences,
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
fn combined_color_and_options_save_is_one_undo_effect_without_song_edits() {
    let mut s = session();
    let bytes = s.serialize().unwrap();
    let before = Arc::new(Preferences::default());
    let mut after = before.as_ref().clone();
    after.colors.hold[0] = 0.8;
    after.step_color_method = ColorMethod::Beat;
    let after = Arc::new(after);
    assert!(
        s.set_note_color_preferences(Arc::clone(&before), Arc::clone(&after), None)
            .unwrap()
    );
    assert_eq!(
        s.take_note_color_preferences_update().unwrap().preferences,
        after
    );
    assert!(s.undo_changes_note_color_preferences());
    let rev = s.revision();
    assert!(
        !s.set_note_color_preferences(Arc::clone(&after), Arc::clone(&after), None)
            .unwrap()
    );
    assert_eq!(s.revision(), rev);
    assert!(s.undo().unwrap());
    assert_eq!(
        s.take_note_color_preferences_update().unwrap().preferences,
        before
    );
    assert!(!s.undo().unwrap());
    assert!(s.redo().unwrap());
    assert_eq!(
        s.take_note_color_preferences_update().unwrap().preferences,
        after
    );
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
}
#[test]
fn rgb_component_undo_preserves_external_sibling_and_unknowns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let mut s = session();
    let before = file.note_colors_arc();
    let mut after = before.as_ref().clone();
    after.colors.hold[0] = 0.8;
    let after = Arc::new(after);
    s.set_note_color_preferences(Arc::clone(&before), after, Some(&file))
        .unwrap();
    s.take_note_color_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["NoteColor"]["HoldColor"]["Y"] = 0.9.into();
    json["Options"]["Vendor"] = 42.into();
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(s.undo().unwrap());
    let update = s.take_note_color_preferences_update().unwrap();
    assert_eq!(update.preferences.colors.hold[0], before.colors.hold[0]);
    assert_eq!(update.preferences.colors.hold[1], 0.9);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["Options"]["Vendor"], 42);
    assert!(!s.is_dirty());
}
#[test]
fn owned_component_conflict_keeps_history_runtime_update_and_disk_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let mut s = session();
    let before = file.note_colors_arc();
    let mut after = before.as_ref().clone();
    after.colors.players[2][1] = 0.8;
    s.set_note_color_preferences(before, Arc::new(after), Some(&file))
        .unwrap();
    s.take_note_color_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["NoteColor"]["Player2Color"]["Y"] = 0.9.into();
    let bytes = serde_json::to_vec(&json).unwrap();
    fs::write(&path, &bytes).unwrap();
    let rev = s.revision();
    assert!(matches!(
        s.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(s.revision(), rev);
    assert!(s.undo_changes_note_color_preferences());
    assert!(s.take_note_color_preferences_update().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
