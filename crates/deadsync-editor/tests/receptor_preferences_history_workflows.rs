use deadsync_editor::receptor_preferences::Preferences;

#[test]
fn global_receptor_history_is_atomic_nondirty_and_noop_does_not_enqueue() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let mut session = Session::new(
        Document::parse(
            b"#TITLE:Keep;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let source = session.serialize().unwrap();
    let before = Preferences::default();
    let mut after = before;
    after.hide_arrows = false;
    after.animation.pulse = false;
    assert!(
        session
            .set_receptor_preferences(before, after, None)
            .unwrap()
    );
    assert!(session.undo_changes_receptor_preferences());
    assert_eq!(
        session
            .take_receptor_preferences_update()
            .unwrap()
            .preferences,
        after
    );
    let revision = session.revision();
    assert!(
        !session
            .set_receptor_preferences(after, after, None)
            .unwrap()
    );
    assert_eq!(session.revision(), revision);
    assert!(session.undo().unwrap());
    assert_eq!(
        session
            .take_receptor_preferences_update()
            .unwrap()
            .preferences,
        before
    );
    assert!(!session.undo().unwrap());
    assert!(session.redo().unwrap());
    assert_eq!(
        session
            .take_receptor_preferences_update()
            .unwrap()
            .preferences,
        after
    );
    assert_eq!(session.serialize().unwrap(), source);
    assert!(!session.is_dirty());
}

#[test]
fn undo_merges_an_external_unowned_flag_and_preserves_unknown_fields() {
    use deadsync_editor::{
        audio_preferences::File,
        document::{Document, Format},
        history::Session,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let mut session = Session::new(
        Document::parse(
            b"#TITLE:Keep;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let file = File::open(&path).unwrap();
    let before = file.receptors();
    let mut after = before;
    after.hide_arrows = false;
    session
        .set_receptor_preferences(before, after, Some(&file))
        .unwrap();
    session.take_receptor_preferences_update();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Receptors"]["PulseReceptorsWithTempo"] = false.into();
    json["Receptors"]["Vendor"] = 42.into();
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(session.undo().unwrap());
    let update = session.take_receptor_preferences_update().unwrap();
    assert!(update.preferences.hide_arrows);
    assert!(!update.preferences.animation.pulse);
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["Receptors"]["Vendor"], 42);
    assert!(!session.is_dirty());
}

#[test]
fn owned_flag_conflict_leaves_history_and_file_unchanged() {
    use deadsync_editor::{
        audio_preferences::File,
        document::{Document, Format},
        history::Session,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let file = File::open(&path).unwrap();
    let mut session = Session::new(
        Document::parse(
            b"#TITLE:Keep;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let before = file.receptors();
    let mut after = before;
    after.hide_arrows = false;
    session
        .set_receptor_preferences(before, after, Some(&file))
        .unwrap();
    session.take_receptor_preferences_update();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Receptors"]["AutoPlayHideArrows"] = true.into();
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let revision = session.revision();
    assert!(matches!(
        session.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(session.revision(), revision);
    assert!(session.undo_changes_receptor_preferences());
    assert!(session.take_receptor_preferences_update().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
