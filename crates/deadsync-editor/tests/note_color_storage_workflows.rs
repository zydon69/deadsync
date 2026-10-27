use deadsync_editor::{audio_preferences::File, minimap_palette::ColorMethod};
use std::{fs, sync::Arc};
#[test]
fn typed_color_snapshots_track_both_groups_and_preserve_unknown_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(&path,b"{\"Version\":1,\"Vendor\":42,\"NoteColor\":{\"HoldColor\":{\"X\":0.1,\"Y\":0.2,\"Z\":0.3,\"Extra\":9},\"Unknown\":true},\"Options\":{\"UnknownOption\":17}}").unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.has_changes());
    let original = file.note_colors_arc();
    assert!(Arc::ptr_eq(&original, &file.note_colors_arc()));
    let clone = file.clone();
    file.set_note_colors(Arc::new(original.as_ref().clone()))
        .unwrap();
    assert!(
        !file.has_changes(),
        "equal values in another Arc are not edits"
    );
    let mut changed = original.as_ref().clone();
    changed.colors.hold[0] = 0.75;
    changed.step_color_method = ColorMethod::Beat;
    file.set_note_colors(Arc::new(changed.clone())).unwrap();
    assert!(file.has_changes());
    assert_eq!(
        clone.note_colors().unwrap(),
        *original,
        "cloned snapshot cannot share mutable settings"
    );
    assert!(file.save().unwrap());
    assert!(!file.has_changes());
    let reloaded = File::open(&path).unwrap();
    assert_eq!(reloaded.note_colors().unwrap(), changed);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["Vendor"], 42);
    assert_eq!(json["NoteColor"]["HoldColor"]["Extra"], 9);
    assert_eq!(json["NoteColor"]["Unknown"], true);
    assert_eq!(json["Options"]["UnknownOption"], 17);
}
#[test]
fn options_only_edits_are_dirty_invalid_dto_is_atomic_and_conflicts_do_not_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    let base = file.note_colors_arc();
    let mut bad = base.as_ref().clone();
    bad.colors.players.pop();
    assert!(file.set_note_colors(Arc::new(bad)).is_err());
    assert!(!file.has_changes());
    assert!(Arc::ptr_eq(&base, &file.note_colors_arc()));
    let mut changed = base.as_ref().clone();
    changed.step_color_method = ColorMethod::Note;
    file.set_note_colors(Arc::new(changed)).unwrap();
    assert!(file.has_changes());
    fs::write(&path, b"{\"Version\":1,\"External\":true}").unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(matches!(
        file.save(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(file.has_changes());
}
