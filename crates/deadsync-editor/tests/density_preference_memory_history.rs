use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
};
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Density;".as_slice(), Format::Sm).unwrap())
}
#[test]
fn density_restore_save_undo_redo_uses_memory_owner_without_empty_path_io() {
    let mut session = session();
    let bytes = session.serialize().unwrap();
    let mut file = File::memory().unwrap();
    let defaults = file.density();
    let mut changed = defaults;
    changed.height = 123;
    changed.position_offsets[0] = 501;
    file.set_density(changed).unwrap();
    assert!(
        session
            .set_density_preferences(changed, defaults, Some(&file))
            .unwrap()
    );
    file = session
        .take_density_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.density(), defaults);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    assert!(session.undo().unwrap());
    file = session
        .take_density_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.density(), changed);
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    assert!(session.redo().unwrap());
    file = session
        .take_density_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.density(), defaults);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    assert!(!session.is_dirty());
    assert_eq!(session.serialize().unwrap(), bytes);
}
#[test]
fn density_live_conflict_blocks_before_disk_publication_and_disjoint_leaf_is_preserved() {
    for conflict in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, b"{\"Version\":1}").unwrap();
        let mut file = File::open(&path).unwrap();
        let before = file.density();
        let mut after = before;
        after.height = 123;
        let mut session = session();
        session
            .set_density_preferences(before, after, Some(&file))
            .unwrap();
        file = session
            .take_density_preferences_update()
            .unwrap()
            .file
            .unwrap();
        let mut live = file.density();
        if conflict {
            live.height = 456;
        } else {
            live.position_offsets[0] = 456;
        }
        file.set_density(live).unwrap();
        let disk = std::fs::read(&path).unwrap();
        let label = session.undo_label().map(str::to_owned);
        session.capture_source_preferences_context(&file);
        if conflict {
            assert!(session.undo().is_err());
            assert_eq!(std::fs::read(&path).unwrap(), disk);
            assert_eq!(session.undo_label(), label.as_deref());
            assert!(session.take_density_preferences_update().is_none());
        } else {
            assert!(session.undo().unwrap());
            let update = session.take_density_preferences_update().unwrap();
            assert_eq!(update.preferences.height, before.height);
            assert_eq!(update.preferences.position_offsets[0], 456);
            assert_eq!(File::open(&path).unwrap().density(), before);
        }
    }
}
