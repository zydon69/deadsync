use deadsync_editor::pack::{
    Data, MAX_BYTES,
    storage::{Durability, Error, FileSession, durability_guarantee},
};
use std::{fs, path::Path};
fn data(title: &str) -> Data {
    Data::new(title, "../shared/banner.png", 2026, None, 0.009)
}
fn existing(root: &Path) -> FileSession {
    fs::write(root.join("Pack.ini"),b";keep\r\n[Group]\r\nVersion=7\r\nDisplayTitle=Original\r\nYear=2020\r\nUnknown=opaque\r\n").unwrap();
    FileSession::open(root).unwrap().unwrap()
}
fn title(file: &mut FileSession, value: &str) {
    let mut next = file.draft().data().clone();
    next.title = Some(value.into());
    file.replace(next).unwrap();
}
#[test]
fn missing_discovery_is_read_only_and_creation_requires_explicit_save() {
    let dir = tempfile::tempdir().unwrap();
    assert!(FileSession::open(dir.path()).unwrap().is_none());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    let mut file = FileSession::create(dir.path(), data("New pack")).unwrap();
    assert!(!file.path().exists());
    assert!(file.draft().has_unsaved_changes());
    let guarantee = file.save().unwrap();
    assert_eq!(guarantee, durability_guarantee());
    #[cfg(unix)]
    assert_eq!(guarantee, Durability::FileAndDirectory);
    #[cfg(not(unix))]
    assert_eq!(guarantee, Durability::FileOnly);
    assert!(!file.draft().has_unsaved_changes());
    assert!(!file.durability_pending());
    let reopened = FileSession::open(dir.path()).unwrap().unwrap();
    assert_eq!(reopened.draft().data(), file.draft().data());
}
#[test]
fn existing_save_preserves_unknown_bytes_and_future_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    title(&mut file, "Été");
    file.save().unwrap();
    assert_eq!(fs::read(file.path()).unwrap(),b";keep\r\n[Group]\r\nVersion=7\r\nDisplayTitle=\xc3\x89t\xc3\xa9\r\nYear=2020\r\nUnknown=opaque\r\n");
    assert!(!file.has_external_change().unwrap());
    assert!(!file.draft().has_unsaved_changes());
}
#[test]
fn prepared_snapshot_saves_its_payload_and_preserves_later_live_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    title(&mut file, "First");
    let first = file.prepare_save().unwrap();
    title(&mut file, "Later");
    let stale = file.prepare_save().unwrap();
    file.publish(first).unwrap();
    assert!(
        String::from_utf8(fs::read(file.path()).unwrap())
            .unwrap()
            .contains("DisplayTitle=First")
    );
    assert_eq!(file.draft().data().title.as_deref(), Some("Later"));
    assert!(file.draft().has_unsaved_changes());
    assert!(matches!(file.publish(stale), Err(Error::StaleSnapshot)));
    file.save().unwrap();
    assert!(!file.draft().has_unsaved_changes());
}
#[test]
fn foreign_snapshot_never_writes_another_pack() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut first = existing(a.path());
    let mut second = existing(b.path());
    title(&mut first, "First");
    let before = fs::read(second.path()).unwrap();
    assert!(matches!(
        second.publish(first.prepare_save().unwrap()),
        Err(Error::StaleSnapshot)
    ));
    assert_eq!(fs::read(second.path()).unwrap(), before);
    assert!(first.draft().has_unsaved_changes());
}
#[test]
fn external_edit_and_delete_are_conflicts_with_dirty_state_retained() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    title(&mut file, "Edited");
    fs::write(file.path(), b"external").unwrap();
    assert!(file.has_external_change().unwrap());
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert_eq!(fs::read(file.path()).unwrap(), b"external");
    assert!(file.draft().has_unsaved_changes());
    fs::remove_file(file.path()).unwrap();
    assert!(file.has_external_change().unwrap());
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert!(!file.path().exists());
}
#[cfg(unix)]
#[test]
fn external_atomic_replacement_with_identical_bytes_is_still_a_binding_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    let before = fs::read(file.path()).unwrap();
    let replacement = dir.path().join("replacement.ini");
    fs::write(&replacement, &before).unwrap();
    fs::rename(replacement, file.path()).unwrap();
    title(&mut file, "Never written");
    assert!(file.has_external_change().unwrap());
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert_eq!(fs::read(file.path()).unwrap(), before);
}
#[test]
fn newly_appeared_file_is_never_overwritten_by_explicit_creation() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = FileSession::create(dir.path(), data("New")).unwrap();
    fs::write(file.path(), b"other writer").unwrap();
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert_eq!(fs::read(file.path()).unwrap(), b"other writer");
    assert!(file.draft().has_unsaved_changes());
    assert!(matches!(
        FileSession::create(dir.path(), data("Again")),
        Err(Error::ExternalChange)
    ));
}
#[test]
fn malformed_and_oversized_files_are_not_treated_as_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Pack.ini");
    for bytes in [
        b"[Group]\nDisplayTitle=broken\n".to_vec(),
        vec![b'x'; MAX_BYTES + 1],
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            FileSession::open(dir.path()),
            Err(Error::Metadata(_))
        ));
        assert!(FileSession::create(dir.path(), data("New")).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
#[test]
fn advisory_lock_blocks_second_writer_without_cleaning_its_draft() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    title(&mut file, "Edited");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join(".Pack.ini.deadsync-save-lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(file.save(), Err(Error::SaveLocked)));
    assert!(file.draft().has_unsaved_changes());
    drop(lock);
    file.save().unwrap();
    assert!(!file.draft().has_unsaved_changes());
}
#[cfg(unix)]
#[test]
fn permissions_survive_atomic_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut file = existing(dir.path());
    fs::set_permissions(file.path(), fs::Permissions::from_mode(0o640)).unwrap();
    title(&mut file, "Changed");
    file.save().unwrap();
    assert_eq!(
        fs::metadata(file.path()).unwrap().permissions().mode() & 0o7777,
        0o640
    );
}
#[cfg(unix)]
#[test]
fn existing_symlink_updates_target_and_retargeting_conflicts() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _ = existing(target.path());
    symlink(target.path().join("Pack.ini"), dir.path().join("Pack.ini")).unwrap();
    let mut file = FileSession::open(dir.path()).unwrap().unwrap();
    title(&mut file, "Through link");
    file.save().unwrap();
    assert!(
        fs::symlink_metadata(file.path())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        FileSession::open(target.path())
            .unwrap()
            .unwrap()
            .draft()
            .data()
            .title
            .as_deref(),
        Some("Through link")
    );
    fs::remove_file(file.path()).unwrap();
    let other = target.path().join("Other.ini");
    fs::write(&other, fs::read(target.path().join("Pack.ini")).unwrap()).unwrap();
    symlink(&other, file.path()).unwrap();
    title(&mut file, "Never written");
    assert!(file.has_external_change().unwrap());
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert!(
        !String::from_utf8(fs::read(other).unwrap())
            .unwrap()
            .contains("Never written")
    );
}
#[cfg(unix)]
#[test]
fn dangling_symlinks_and_symlink_locks_do_not_create_or_modify_their_targets() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let absent = dir.path().join("Absent.ini");
    symlink(&absent, dir.path().join("Pack.ini")).unwrap();
    assert!(FileSession::open(dir.path()).is_err());
    assert!(FileSession::create(dir.path(), data("New")).is_err());
    assert!(!absent.exists());
    fs::remove_file(dir.path().join("Pack.ini")).unwrap();
    let mut file = existing(dir.path());
    title(&mut file, "Changed");
    let untouched = dir.path().join("DoNotCreate");
    symlink(&untouched, dir.path().join(".Pack.ini.deadsync-save-lock")).unwrap();
    assert!(file.save().is_err());
    assert!(!untouched.exists());
    assert!(file.draft().has_unsaved_changes());
}
#[cfg(unix)]
#[test]
fn rebound_parent_directory_is_a_conflict_even_with_identical_file_bytes() {
    let tree = tempfile::tempdir().unwrap();
    let pack = tree.path().join("Pack");
    fs::create_dir(&pack).unwrap();
    let mut file = existing(&pack);
    title(&mut file, "Changed");
    let bytes = fs::read(file.path()).unwrap();
    fs::rename(&pack, tree.path().join("OldPack")).unwrap();
    fs::create_dir(&pack).unwrap();
    fs::write(pack.join("Pack.ini"), &bytes).unwrap();
    assert!(file.has_external_change().unwrap());
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), bytes);
}
