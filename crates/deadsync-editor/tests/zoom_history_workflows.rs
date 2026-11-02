use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    zoom_preferences::Preferences,
};
fn session() -> Session {
    Session::new(
        Document::parse(b"#TITLE:No chart;#VENDOR:opaque;".as_slice(), Format::Sm).unwrap(),
    )
}
fn install(s: &mut Session, file: &mut File, before: Preferences, target: Preferences) {
    assert!(s.apply_zoom_preferences(before, target, file).unwrap());
    let update = s.take_restore_scroll_update().unwrap();
    assert_eq!(update.preferences.zoom, target);
    *file = update.file.unwrap();
    assert!(!file.has_changes());
}
#[test]
fn three_original_undoable_fields_and_direct_cap_publish_once_then_preserve_later_cap_on_history() {
    for disk in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut file = if disk {
            let path = root.path().join("prefs.json");
            std::fs::write(
                &path,
                br#"{"Version":1,"Zoom":{"Future":99},"Vendor":{"keep":true}}"#,
            )
            .unwrap();
            File::open(&path).unwrap()
        } else {
            File::memory().unwrap()
        };
        let mut s = session();
        let song = s.serialize().unwrap();
        let initial = file.zoom();
        let mut mixed = initial;
        mixed.set_duration(0.25).unwrap();
        mixed.set_multiplier(1.7).unwrap();
        mixed.set_size_cap(0.75).unwrap();
        mixed.set_limited(true);
        install(&mut s, &mut file, initial, mixed);
        assert!(s.can_undo_application_change());
        assert!(!s.is_dirty());
        let revision = s.revision();
        let label = s.undo_label().map(str::to_owned);
        let mut cap = mixed;
        cap.set_size_cap(0.5).unwrap();
        install(&mut s, &mut file, mixed, cap);
        assert_eq!(s.revision(), revision);
        assert_eq!(s.undo_label(), label.as_deref());
        for redo in [false, true] {
            s.capture_source_preferences_context(&file);
            if redo {
                s.redo().unwrap();
            } else {
                s.undo().unwrap();
            }
            let update = s.take_restore_scroll_update().unwrap();
            let p = update.preferences.zoom;
            assert_eq!(p.size_cap(), 0.5);
            assert_eq!(
                (p.duration(), p.multiplier(), p.limited()),
                if redo {
                    (0.25, 1.7, true)
                } else {
                    (initial.duration(), initial.multiplier(), initial.limited())
                }
            );
            file = update.file.unwrap();
            assert_eq!(file.zoom(), p);
            assert_eq!(s.serialize().unwrap(), song);
            assert!(!s.is_dirty());
        }
        // A cap-only publication must not discard an existing real redo action.
        s.capture_source_preferences_context(&file);
        s.undo().unwrap();
        file = s.take_restore_scroll_update().unwrap().file.unwrap();
        let redo = s.redo_label().map(str::to_owned);
        let old = file.zoom();
        let mut next = old;
        next.set_size_cap(0.375).unwrap();
        install(&mut s, &mut file, old, next);
        assert_eq!(s.redo_label(), redo.as_deref());
        s.capture_source_preferences_context(&file);
        s.redo().unwrap();
        let p = s.take_restore_scroll_update().unwrap().preferences.zoom;
        assert_eq!(p.size_cap(), 0.375);
        assert!(p.limited());
        if disk {
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(file.path()).unwrap()).unwrap();
            assert_eq!(raw["Zoom"]["Future"], 99);
            assert_eq!(raw["Vendor"]["keep"], true);
        }
    }
}
#[test]
fn changed_leaf_runtime_and_disk_conflicts_refuse_the_entire_mixed_publication_and_noop_keeps_history()
 {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    let bytes = br#"{"Version":1,"Zoom":{"ScrollInterpolationDuration":0.1,"Future":99}}"#;
    std::fs::write(&path, bytes).unwrap();
    let file = File::open(&path).unwrap();
    let before = file.zoom();
    let mut target = before;
    target.set_duration(0.25).unwrap();
    target.set_size_cap(0.75).unwrap();
    target.set_limited(true);
    let mut s = session();
    let song = s.serialize().unwrap();
    let revision = s.revision();
    let mut runtime = file.clone();
    let mut competing = before;
    competing.set_duration(0.3).unwrap();
    runtime.set_zoom(competing);
    assert!(s.apply_zoom_preferences(before, target, &runtime).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(s.revision(), revision);
    assert!(s.undo_label().is_none());
    assert!(s.take_restore_scroll_update().is_none());
    let external = br#"{"Version":1,"Zoom":{"ScrollInterpolationDuration":0.4,"Future":99}}"#;
    std::fs::write(&path, external).unwrap();
    assert!(s.apply_zoom_preferences(before, target, &file).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), external);
    assert_eq!(s.revision(), revision);
    assert!(s.undo_label().is_none());
    assert!(s.take_restore_scroll_update().is_none());
    assert_eq!(s.serialize().unwrap(), song);
    std::fs::write(&path, bytes).unwrap();
    let mut file = File::open(&path).unwrap();
    install(&mut s, &mut file, before, target);
    s.capture_source_preferences_context(&file);
    s.undo().unwrap();
    file = s.take_restore_scroll_update().unwrap().file.unwrap();
    let redo = s.redo_label().map(str::to_owned);
    let revision = s.revision();
    assert!(
        !s.apply_zoom_preferences(file.zoom(), file.zoom(), &file)
            .unwrap()
    );
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), redo.as_deref());
    assert!(s.take_restore_scroll_update().is_none());
}
#[test]
fn loaded_source_raw_cap_survives_numeric_history_without_ui_clamping() {
    use deadsync_editor::{
        audio_preferences::source_import::Draft, key_binding_preferences::source_profile::Platform,
    };
    let mut s = session();
    let file = File::memory().unwrap();
    let draft=Draft::prepare(&mut s,None,&file,br#"{"PreferencesScroll":{"SizeCap":4,"ScrollInterpolationDuration":3,"ZoomMultiplier":0.3}}"#,Platform::Mac).unwrap();
    s.import_source_preferences(draft, &file, None, true)
        .unwrap();
    let mut file = s
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let before = file.zoom();
    assert_eq!(
        (before.duration(), before.multiplier(), before.size_cap()),
        (3., 0.3, 4.)
    );
    let mut target = before;
    target.set_duration(0.25).unwrap();
    install(&mut s, &mut file, before, target);
    s.capture_source_preferences_context(&file);
    s.undo().unwrap();
    let p = s.take_restore_scroll_update().unwrap().preferences.zoom;
    assert_eq!(p, before);
}
#[test]
fn open_draft_merges_a_disjoint_cap_change_and_undo_leaves_that_direct_value_intact() {
    let mut s = session();
    let mut file = File::memory().unwrap();
    let opened = file.zoom();
    let mut later = opened;
    later.set_size_cap(0.625).unwrap();
    install(&mut s, &mut file, opened, later);
    assert!(s.undo_label().is_none());
    let mut staged = opened;
    staged.set_duration(0.25).unwrap();
    staged.set_limited(true);
    assert!(s.apply_zoom_preferences(opened, staged, &file).unwrap());
    let update = s.take_restore_scroll_update().unwrap();
    assert_eq!(update.preferences.zoom.size_cap(), 0.625);
    assert_eq!(update.preferences.zoom.duration(), 0.25);
    file = update.file.unwrap();
    s.capture_source_preferences_context(&file);
    s.undo().unwrap();
    let p = s.take_restore_scroll_update().unwrap().preferences.zoom;
    assert_eq!(p.size_cap(), 0.625);
    assert_eq!(p.duration(), opened.duration());
    assert_eq!(p.limited(), opened.limited());
}
