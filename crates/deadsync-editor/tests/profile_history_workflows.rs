use deadsync_editor::generation::profiles::ProfileFile;
use std::fs;

fn fixture() -> (tempfile::TempDir, ProfileFile, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ecc-history.json");
    let bytes = br#"{ "Guid":"12345678-9abc-4def-8123-456789abcdef", "Name":"Original", "Future":1e999, "Config":{} }
"#.to_vec();
    fs::write(&path, &bytes).unwrap();
    let file = ProfileFile::open(&path).unwrap();
    (dir, file, bytes)
}

#[test]
fn saved_profile_history_restores_exact_bytes_and_identity() {
    let (_dir, mut file, original) = fixture();
    assert!(!file.undo().unwrap());
    let guid = file.profile().guid().clone();
    let mut edited = file.profile().clone();
    edited.rename("First").unwrap();
    file.save(&edited).unwrap();
    let first = fs::read(file.path()).unwrap();
    edited.rename("Second").unwrap();
    file.save(&edited).unwrap();
    let second = fs::read(file.path()).unwrap();
    assert!(file.undo().unwrap());
    assert_eq!(fs::read(file.path()).unwrap(), first);
    assert!(file.undo().unwrap());
    assert_eq!(fs::read(file.path()).unwrap(), original);
    assert_eq!(file.profile().guid(), &guid);
    assert_eq!(file.profile().name(), Some("Original"));
    assert!(!file.can_undo());
    assert!(file.redo().unwrap());
    assert!(file.redo().unwrap());
    assert_eq!(fs::read(file.path()).unwrap(), second);
    assert!(!file.redo().unwrap());
}

#[test]
fn external_changes_leave_disk_and_history_intact_and_branching_drops_redo() {
    let (_dir, mut file, _) = fixture();
    let mut edited = file.profile().clone();
    edited.rename("Saved").unwrap();
    file.save(&edited).unwrap();
    let saved = fs::read(file.path()).unwrap();
    fs::write(file.path(), b"external edit").unwrap();
    let same = file.profile().clone();
    assert!(file.save(&same).is_err());
    assert!(file.undo().is_err());
    assert!(file.can_undo());
    assert!(!file.can_redo());
    assert_eq!(fs::read(file.path()).unwrap(), b"external edit");
    fs::write(file.path(), &saved).unwrap();
    assert!(file.undo().unwrap());
    let original = fs::read(file.path()).unwrap();
    fs::write(file.path(), b"external again").unwrap();
    assert!(file.redo().is_err());
    assert!(file.can_redo());
    fs::write(file.path(), original).unwrap();
    edited.rename("Branch").unwrap();
    file.save(&edited).unwrap();
    assert!(!file.can_redo());
    let same = file.profile().clone();
    file.save(&same).unwrap();
    assert!(file.undo().unwrap());
    assert_eq!(file.profile().name(), Some("Original"));
}
