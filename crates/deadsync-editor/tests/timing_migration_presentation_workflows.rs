use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    timing_edit::migration::Draft,
};
#[test]
fn migration_receipts_preserve_original_editor_clocks_and_are_bound_to_exact_history() {
    for text in [
        include_str!("fixtures/grooveauthor/legacy-timing-fractional.request.json"),
        include_str!("fixtures/grooveauthor/legacy-timing-missing-version.request.json"),
        include_str!("fixtures/grooveauthor/legacy-timing-mixed.request.json"),
    ] {
        let input: serde_json::Value = serde_json::from_str(text).unwrap();
        let mut session = Session::new(
            Document::parse(input["Content"].as_str().unwrap().as_bytes(), Format::Ssc).unwrap(),
        );
        let original = session.serialize().unwrap();
        let token = deadsync_editor::timing_edit::migration::PresentationOrigin::capture(&session);
        let draft = Draft::capture(&session).unwrap();
        let application = draft.apply_with_presentation_receipt(&mut session).unwrap();
        assert!(application.changed());
        let receipt = application.presentation_receipt().unwrap();
        assert!(receipt.preserves_audio());
        receipt.validate_transition(&token, &session).unwrap();
        let migrated = session.serialize().unwrap();
        assert_ne!(migrated, original);
        assert!(session.history_preserves_timing_migration(false));
        let before = deadsync_editor::timing_edit::migration::PresentationOrigin::capture(&session);
        let undo = session.apply_timing_migration_history(false).unwrap();
        undo.presentation_receipt()
            .unwrap()
            .validate_transition(&before, &session)
            .unwrap();
        assert_eq!(session.serialize().unwrap(), original);
        let before = deadsync_editor::timing_edit::migration::PresentationOrigin::capture(&session);
        let redo = session.apply_timing_migration_history(true).unwrap();
        redo.presentation_receipt()
            .unwrap()
            .validate_transition(&before, &session)
            .unwrap();
        assert_eq!(session.serialize().unwrap(), migrated);
        session.set_text(None, "TITLE", "Later user edit").unwrap();
        let edited = session.serialize().unwrap();
        assert!(!session.history_preserves_timing_migration(false));
        assert!(session.apply_timing_migration_history(false).is_err());
        assert_eq!(session.serialize().unwrap(), edited);
        assert!(receipt.validate_transition(&token, &session).is_err());
    }
}
#[test]
fn stale_migration_never_mints_a_receipt_or_replaces_existing_redo() {
    let mut session=Session::new(Document::parse(b"#VERSION:0.60;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180;#NOTES:1000;".as_slice(),Format::Ssc).unwrap());
    session.set_text(None, "TITLE", "Before review").unwrap();
    let draft = Draft::capture(&session).unwrap();
    session.set_text(None, "TITLE", "User edit").unwrap();
    session.set_text(None, "TITLE", "Redo candidate").unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    let revision = session.revision();
    assert!(draft.apply_with_presentation_receipt(&mut session).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.revision(), revision);
    assert!(!session.history_preserves_timing_migration(false));
    assert!(session.redo_label().is_some());
}
