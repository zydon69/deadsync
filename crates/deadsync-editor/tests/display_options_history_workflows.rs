use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
};
fn session() -> Session {
    Session::new(Document::parse(&b"#VERSION:0.83;#TITLE:Display;"[..], Format::Ssc).unwrap())
}
#[test]
fn display_undo_merges_unowned_options_and_preserves_unknown_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"Future":"keep","MaxEventsToDraw":37}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let before = file.display_options();
    let mut after = before;
    after.render.notes = false;
    after.background.hide = true;
    let mut s = session();
    let bytes = s.serialize().unwrap();
    assert!(s.set_display_options(before, after, Some(&file)).unwrap());
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert_eq!(s.take_display_options_update().unwrap().preferences, after);
    let mut external = File::open(&path).unwrap();
    let mut other = external.display_options();
    other.misc_width = 121;
    external.set_display_options(other);
    external.save().unwrap();
    assert!(s.undo().unwrap());
    let actual = s.take_display_options_update().unwrap().preferences;
    assert!(actual.render.notes);
    assert!(!actual.background.hide);
    assert_eq!(actual.misc_width, 121);
    assert_eq!(actual.limits.max_events, 37);
    assert!(s.redo().unwrap());
    let actual = s.take_display_options_update().unwrap().preferences;
    assert!(!actual.render.notes);
    assert!(actual.background.hide);
    assert_eq!(actual.misc_width, 121);
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value["Options"]["Future"], "keep");
    assert_eq!(s.serialize().unwrap(), bytes);
}
#[test]
fn conflicting_display_undo_is_atomic_and_remains_retryable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let before = file.display_options();
    let mut after = before;
    after.misc_width = 120;
    let mut s = session();
    assert!(s.set_display_options(before, after, Some(&file)).unwrap());
    s.take_display_options_update();
    let mut external = File::open(&path).unwrap();
    let mut other = external.display_options();
    other.misc_width = 150;
    external.set_display_options(other);
    external.save().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(
        s.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(s.take_display_options_update().is_none());
    assert_eq!(s.undo_label(), Some("Display options"));
    external.set_display_options(after);
    external.save().unwrap();
    assert!(s.undo().unwrap());
    assert_eq!(s.take_display_options_update().unwrap().preferences, before);
}
