use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    minimap_viewport::Axis,
    visual_preferences::{MiniMap, Snapshot},
};
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Visual;".as_slice(), Format::Sm).unwrap())
}
#[test]
fn waveform_and_complete_minimap_preferences_are_non_document_undoable_in_memory_and_disk() {
    for disk in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        let mut file = if disk {
            std::fs::write(&path, b"{\"Version\":1,\"Vendor\":{\"x\":7}}").unwrap();
            File::open(&path).unwrap()
        } else {
            File::memory().unwrap()
        };
        let mut session = session();
        let bytes = session.serialize().unwrap();
        let before = file.waveform();
        let mut after = before;
        after.set_show(false);
        after.set_size_to_chart(false);
        session
            .set_visual_preferences(
                Snapshot::Waveform(before),
                Snapshot::Waveform(after),
                Some(&file),
            )
            .unwrap();
        let update = session.take_visual_preferences_update().unwrap();
        file = update.file.unwrap();
        assert_eq!(file.waveform(), after);
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), bytes);
        session.capture_source_preferences_context(&file);
        assert!(session.undo().unwrap());
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.waveform(), before);
        session.capture_source_preferences_context(&file);
        assert!(session.redo().unwrap());
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.waveform(), after);
        let before = MiniMap::capture(&file).unwrap();
        let mut changed = before;
        changed
            .viewport
            .set_visible_range(Axis::Time, 100.)
            .unwrap();
        changed.position.set_offset(changed.position.mount, 57);
        changed.content.width = 128;
        changed.content.note_width = 32;
        changed.content.max_notes = 7000;
        session
            .set_visual_preferences(
                Snapshot::MiniMap(before),
                Snapshot::MiniMap(changed),
                Some(&file),
            )
            .unwrap();
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        let restored = changed.restored();
        assert_eq!(restored.content.max_notes, 7000);
        session
            .set_visual_preferences(
                Snapshot::MiniMap(changed),
                Snapshot::MiniMap(restored),
                Some(&file),
            )
            .unwrap();
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(MiniMap::capture(&file).unwrap(), restored);
        session.capture_source_preferences_context(&file);
        assert!(session.undo().unwrap());
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(MiniMap::capture(&file).unwrap(), changed);
        session.capture_source_preferences_context(&file);
        assert!(session.redo().unwrap());
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(MiniMap::capture(&file).unwrap(), restored);
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), bytes);
        if disk {
            let reopened = File::open(&path).unwrap();
            assert_eq!(MiniMap::capture(&reopened).unwrap(), restored);
            assert_eq!(reopened.waveform(), after);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
                    ["Vendor"]["x"],
                7
            );
        }
    }
}
#[test]
fn undo_merges_disjoint_fields_and_rejects_changed_leaf_without_mutating_disk_or_history() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    std::fs::write(&path, b"{\"Version\":1}").unwrap();
    let file = File::open(&path).unwrap();
    let mut session = session();
    let before = file.waveform();
    let mut after = before;
    after.set_show(false);
    session
        .set_visual_preferences(
            Snapshot::Waveform(before),
            Snapshot::Waveform(after),
            Some(&file),
        )
        .unwrap();
    let mut live = session
        .take_visual_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let mut external = File::open(&path).unwrap();
    let mut p = external.waveform();
    p.set_dense_scale(4.).unwrap();
    external.set_waveform(p);
    external.save().unwrap();
    session.capture_source_preferences_context(&live);
    assert!(session.undo().unwrap());
    live = session
        .take_visual_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert!(live.waveform().show());
    assert_eq!(live.waveform().dense_scale(), 4.);
    session.capture_source_preferences_context(&live);
    assert!(session.redo().unwrap());
    live = session
        .take_visual_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let mut external = File::open(&path).unwrap();
    let mut p = external.waveform();
    p.set_show(true);
    external.set_waveform(p);
    external.save().unwrap();
    let disk = std::fs::read(&path).unwrap();
    let label = session.undo_label().map(str::to_owned);
    session.capture_source_preferences_context(&live);
    assert!(session.undo().is_err());
    assert_eq!(std::fs::read(path).unwrap(), disk);
    assert_eq!(session.undo_label(), label.as_deref());
}
#[test]
fn direct_live_conflict_is_distinct_from_disk_cas_and_is_atomic() {
    let mut session = session();
    let file = File::memory().unwrap();
    let before = file.waveform();
    let mut after = before;
    after.set_show(false);
    session
        .set_visual_preferences(
            Snapshot::Waveform(before),
            Snapshot::Waveform(after),
            Some(&file),
        )
        .unwrap();
    let update = session.take_visual_preferences_update().unwrap();
    let mut live = update.file.unwrap();
    let mut competing = live.waveform();
    competing.set_show(true);
    live.set_waveform(competing);
    let label = session.undo_label().map(str::to_owned);
    session.capture_source_preferences_context(&live);
    assert!(session.undo().is_err());
    assert_eq!(session.undo_label(), label.as_deref());
    assert!(session.take_visual_preferences_update().is_none());
    assert_eq!(live.waveform(), competing);
}
