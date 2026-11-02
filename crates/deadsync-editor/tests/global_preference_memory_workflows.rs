//! Real public source import, typed global actions, and memory publication.
use deadsync_editor::{
    audio_preferences::{File, source_import::Draft},
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
use std::sync::Arc;
fn imported() -> (Session, File) {
    let mut session =
        Session::new(Document::parse(b"#TITLE:Memory;".as_slice(), Format::Sm).unwrap());
    let file = File::memory().unwrap();
    let draft=Draft::prepare(&mut session,None,&file,br#"{"VendorPreserved":{"opaque":[1,2,3]},"PreferencesScroll":{"Reverse":true},"PreferencesOptions":{"OpenLastOpenedFileOnLaunch":false}}"#,Platform::Mac).unwrap();
    session
        .import_source_preferences(draft, &file, None, true)
        .unwrap();
    let file = session
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    assert!(file.source_archive().is_some());
    (session, file)
}
#[test]
fn receptor_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.receptors();
    let mut after = before;
    after.hide_arrows = !after.hide_arrows;
    assert!(
        session
            .set_receptor_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.receptors(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.receptors(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.receptors(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn selection_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.selection();
    let mut after = before;
    after.overlap = deadsync_editor::selection_preferences::Overlap::All;
    assert!(
        session
            .set_selection_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_selection_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.selection(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_selection_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.selection(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_selection_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.selection(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn stream_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.streams();
    let mut after = before;
    after.show_break_lengths = !after.show_break_lengths;
    assert!(
        session
            .set_stream_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.streams(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.streams(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_stream_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.streams(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn scroll_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.scroll();
    let mut after = before;
    after.set_reverse(!before.reverse());
    assert!(
        session
            .set_scroll_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_scroll_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.scroll(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_scroll_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.scroll(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_scroll_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.scroll(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn performance_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.performance();
    let mut after = before;
    after.paused = !after.paused;
    assert!(
        session
            .set_performance_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_performance_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.performance(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_performance_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.performance(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_performance_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.performance(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn display_options_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.display_options();
    let mut after = before;
    after.render.notes = !after.render.notes;
    assert!(
        session
            .set_display_options(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session.take_display_options_update().unwrap().file.unwrap();
    assert_eq!(file.display_options(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session.take_display_options_update().unwrap().file.unwrap();
    assert_eq!(file.display_options(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session.take_display_options_update().unwrap().file.unwrap();
    assert_eq!(file.display_options(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn general_options_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.general_options().clone();
    let mut after = before.clone();
    after.open_last = !after.open_last;
    assert!(
        session
            .set_general_options(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session.take_general_options_update().unwrap().file.unwrap();
    assert_eq!(file.general_options(), &after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session.take_general_options_update().unwrap().file.unwrap();
    assert_eq!(file.general_options(), &before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session.take_general_options_update().unwrap().file.unwrap();
    assert_eq!(file.general_options(), &after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn note_color_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.note_colors_arc();
    let mut after = before.clone();
    Arc::make_mut(&mut after).selection_multiplier += 0.25;
    assert!(
        session
            .set_note_color_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_note_color_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.note_colors_arc(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_note_color_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.note_colors_arc(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_note_color_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.note_colors_arc(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn key_binding_preferences_source_import_save_undo_redo_keeps_memory_and_song_clean() {
    let (mut session, mut file) = imported();
    let song = session.serialize().unwrap();
    let before = file.key_bindings();
    let mut after = before.clone();
    Arc::make_mut(&mut after).commands.insert(
        deadsync_editor::key_binding_preferences::Action::SelectAllNotes
            .id()
            .into(),
        vec![vec![]], // Canonical typed representation of an unbound command.
    );
    assert!(
        session
            .set_key_binding_preferences(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    file = session
        .take_key_binding_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.key_bindings(), after);
    assert!(file.path().as_os_str().is_empty());
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_key_binding_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.key_bindings(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_key_binding_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.key_bindings(), after);
    assert!(file.source_archive().is_some());
    assert!(!file.has_changes());
    assert_eq!(session.serialize().unwrap(), song);
}
#[test]
fn restore_options_source_import_undo_redo_is_memory_publication() {
    let (mut session, mut file) = imported();
    let before = deadsync_editor::options_preferences::restore::Snapshot::capture(&file);
    assert!(
        session
            .restore_options(before.clone(), Some(&file))
            .unwrap()
    );
    file = session.take_restore_options_update().unwrap().file.unwrap();
    let restored = deadsync_editor::options_preferences::restore::Snapshot::capture(&file);
    assert_ne!(restored, before);
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session.take_restore_options_update().unwrap().file.unwrap();
    assert_eq!(
        deadsync_editor::options_preferences::restore::Snapshot::capture(&file),
        before
    );
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session.take_restore_options_update().unwrap().file.unwrap();
    assert_eq!(
        deadsync_editor::options_preferences::restore::Snapshot::capture(&file),
        restored
    );
    assert!(!file.has_changes());
}
#[test]
fn restore_scroll_source_import_undo_redo_is_memory_publication() {
    let (mut session, mut file) = imported();
    let before = deadsync_editor::scroll_preferences::restore::Snapshot::capture(&file);
    assert!(session.restore_scroll(before.clone(), Some(&file)).unwrap());
    file = session.take_restore_scroll_update().unwrap().file.unwrap();
    let restored = deadsync_editor::scroll_preferences::restore::Snapshot::capture(&file);
    assert_ne!(restored, before);
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session.take_restore_scroll_update().unwrap().file.unwrap();
    assert_eq!(
        deadsync_editor::scroll_preferences::restore::Snapshot::capture(&file),
        before
    );
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session.take_restore_scroll_update().unwrap().file.unwrap();
    assert_eq!(
        deadsync_editor::scroll_preferences::restore::Snapshot::capture(&file),
        restored
    );
    assert!(!file.has_changes());
}
#[test]
fn receptor_live_cas_preserves_disjoint_leaf_and_blocks_overlapping_undo() {
    let (mut session, mut file) = imported();
    let before = file.receptors();
    let mut after = before;
    after.hide_arrows = !before.hide_arrows;
    session
        .set_receptor_preferences(before, after, Some(&file))
        .unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let mut live = file.receptors();
    live.placement.x += 17;
    file.set_receptors(live);
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.receptors().placement.x, live.placement.x);
    assert_eq!(file.receptors().hide_arrows, before.hide_arrows);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let mut competing = file.receptors();
    competing.hide_arrows = before.hide_arrows;
    file.set_receptors(competing);
    session.capture_source_preferences_context(&file);
    assert!(session.undo().is_err());
    assert!(session.take_receptor_preferences_update().is_none());
}
#[test]
fn receptor_disk_and_runtime_cas_are_independent_and_fail_before_publication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let mut file = File::open(&path).unwrap();
    file.save().unwrap();
    let mut session = Session::new(Document::parse(b"#TITLE:CAS;".as_slice(), Format::Sm).unwrap());
    let before = file.receptors();
    let mut after = before;
    after.hide_arrows = !after.hide_arrows;
    let mut disk = File::open(&path).unwrap();
    let mut disjoint = disk.receptors();
    disjoint.placement.x += 23;
    disk.set_receptors(disjoint);
    disk.save().unwrap();
    session
        .set_receptor_preferences(before, after, Some(&file))
        .unwrap();
    file = session
        .take_receptor_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.receptors().placement.x, disjoint.placement.x);
    assert_eq!(File::open(&path).unwrap().receptors(), file.receptors());
    // Conflicting live field: even an otherwise valid disk must remain byte-identical.
    let mut live = file.clone();
    let mut p = live.receptors();
    p.hide_arrows = before.hide_arrows;
    live.set_receptors(p);
    let bytes = std::fs::read(&path).unwrap();
    session.capture_source_preferences_context(&live);
    assert!(session.undo().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    // Conflicting disk field: a matching runtime must not bypass persisted CAS.
    let mut disk = File::open(&path).unwrap();
    let mut p = disk.receptors();
    p.hide_arrows = before.hide_arrows;
    disk.set_receptors(p);
    disk.save().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    session.capture_source_preferences_context(&file);
    assert!(session.undo().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(session.take_receptor_preferences_update().is_none());
}
