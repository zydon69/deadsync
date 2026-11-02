use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    options_preferences::Display,
};
#[test]
fn application_history_uses_content_stamps_not_labels_or_chart_presence() {
    let mut s =
        Session::new(Document::parse(&b"#VERSION:0.83;#TITLE:No chart;"[..], Format::Ssc).unwrap());
    assert!(!s.can_undo_application_change());
    assert!(!s.can_redo_application_change());
    s.set_text(None, "TITLE", "Song edit").unwrap();
    assert!(!s.can_undo_application_change());
    let before = Display::default();
    let mut after = before;
    after.render.notes = false;
    let bytes = s.serialize().unwrap();
    s.set_display_options(before, after, None).unwrap();
    assert!(s.can_undo_application_change());
    assert!(!s.can_redo_application_change());
    assert!(s.undo().unwrap());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert!(!s.can_undo_application_change());
    assert!(s.can_redo_application_change());
    assert!(s.redo().unwrap());
    assert!(s.can_undo_application_change());
    assert!(s.undo().unwrap());
    assert!(s.undo().unwrap());
    assert!(
        !s.can_redo_application_change(),
        "a song-changing redo cannot masquerade as application history"
    );
}
