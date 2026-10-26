use deadsync_editor::generation::{
    metadata::DYNAMIC,
    profiles::{Loaded, ProfileFile},
};
use std::fs;
fn fixture() -> (tempfile::TempDir, ProfileFile, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ecc-custom.json");
    let bytes = br#"{ "Guid":"12345678-9abc-4def-8123-456789abcdef", "Name":"Original", "Future":1e999, "Config":{} }
"#.to_vec();
    fs::write(&path, &bytes).unwrap();
    let file = ProfileFile::open(&path).unwrap();
    (dir, file, bytes)
}
#[test]
fn deletion_restore_and_redo_preserve_exact_bytes_and_catalog_visibility() {
    let (dir, file, bytes) = fixture();
    let mut deleted = file.delete().unwrap();
    assert!(deleted.is_deleted());
    assert!(!file.path().exists());
    assert!(Loaded::directory(dir.path()).unwrap().profiles.is_empty());
    assert!(!deleted.delete_again().unwrap());
    assert!(deleted.restore().unwrap());
    assert!(!deleted.restore().unwrap());
    assert_eq!(fs::read(file.path()).unwrap(), bytes);
    assert_eq!(Loaded::directory(dir.path()).unwrap().profiles.len(), 1);
    assert!(deleted.delete_again().unwrap());
    assert!(!file.path().exists());
    assert!(deleted.restore().unwrap());
    drop(deleted);
    assert_eq!(fs::read(file.path()).unwrap(), bytes);
}
#[test]
fn conflicting_file_blocks_restore_and_external_edit_blocks_both_deletions() {
    let (_dir, file, bytes) = fixture();
    fs::write(file.path(), b"external").unwrap();
    assert!(file.delete().is_err());
    assert_eq!(fs::read(file.path()).unwrap(), b"external");
    fs::write(file.path(), &bytes).unwrap();
    let mut deleted = file.delete().unwrap();
    fs::write(file.path(), b"replacement").unwrap();
    assert!(deleted.restore().is_err());
    assert!(deleted.is_deleted());
    assert_eq!(fs::read(file.path()).unwrap(), b"replacement");
    fs::remove_file(file.path()).unwrap();
    assert!(deleted.restore().unwrap());
    fs::write(file.path(), b"edited restored file").unwrap();
    assert!(deleted.delete_again().is_err());
    assert!(!deleted.is_deleted());
    assert_eq!(fs::read(file.path()).unwrap(), b"edited restored file");
}
#[test]
fn builtin_identity_cannot_be_deleted_even_from_a_custom_filename() {
    let (_dir, file, bytes) = fixture();
    fs::write(
        file.path(),
        String::from_utf8(bytes)
            .unwrap()
            .replace("12345678-9abc-4def-8123-456789abcdef", DYNAMIC),
    )
    .unwrap();
    let builtin = ProfileFile::open(file.path()).unwrap();
    assert!(builtin.delete().is_err());
    assert!(file.path().exists());
}
#[cfg(unix)]
#[test]
fn restore_retains_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, file, _) = fixture();
    fs::set_permissions(file.path(), fs::Permissions::from_mode(0o640)).unwrap();
    let mut deleted = file.delete().unwrap();
    deleted.restore().unwrap();
    assert_eq!(
        fs::metadata(file.path()).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn combined_deletion_and_undo_restore_profile_and_all_chart_references() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{metadata::ChartData, profile_deletion::Action},
        history::Session,
    };
    use std::sync::Arc;
    let (_dir, file, original) = fixture();
    let mut session = Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    let mut data = ChartData::default();
    data.expressed_chart_config = file.profile().guid().clone();
    data.save(&mut session, 0).unwrap();
    let song = session.serialize().unwrap();
    let mut action = Action::apply(&file, &mut session).unwrap();
    assert_eq!(action.affected_charts(), 1);
    assert_eq!(action.affected_references(), 1);
    assert!(!file.path().exists());
    assert_eq!(
        ChartData::read(session.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config
            .to_string(),
        DYNAMIC
    );
    let repaired = session.serialize().unwrap();
    // Disk conflicts prevent both halves of undo from being consumed.
    fs::write(file.path(), b"replacement").unwrap();
    assert!(action.undo(&mut session).is_err());
    assert_eq!(session.serialize().unwrap(), repaired);
    assert!(action.is_deleted());
    fs::remove_file(file.path()).unwrap();
    assert!(action.undo(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), song);
    assert_eq!(fs::read(file.path()).unwrap(), original);
    assert!(!action.undo(&mut session).unwrap());
    assert!(action.redo(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), repaired);
    assert!(!file.path().exists());
    assert!(!action.redo(&mut session).unwrap());
}

#[test]
fn unreferenced_deletion_never_consumes_unrelated_song_history_and_rejects_stale_undo() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{metadata::ChartData, profile_deletion::Action},
        history::Session,
    };
    use std::sync::Arc;
    let (_dir, file, original) = fixture();
    let mut session = Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    ChartData::default().save(&mut session, 0).unwrap();
    let song = session.serialize().unwrap();
    let revision = session.revision();
    let mut action = Action::apply(&file, &mut session).unwrap();
    assert_eq!(action.affected_references(), 0);
    assert_eq!(session.revision(), revision);
    assert!(action.undo(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), song);
    assert_eq!(fs::read(file.path()).unwrap(), original);
    assert!(action.redo(&mut session).unwrap());
    let mut data = ChartData::default();
    data.music_offset = 1.0;
    data.save(&mut session, 0).unwrap();
    let edited = session.serialize().unwrap();
    assert!(action.undo(&mut session).is_err());
    assert!(!file.path().exists());
    assert_eq!(session.serialize().unwrap(), edited);
}

#[test]
fn replacing_the_song_redo_branch_cannot_redo_another_edit_as_a_deletion() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{metadata::ChartData, profile_deletion::Action},
        history::Session,
    };
    use std::sync::Arc;
    let (_dir, file, bytes) = fixture();
    let mut session = Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    let mut data = ChartData::default();
    data.expressed_chart_config = file.profile().guid().clone();
    data.save(&mut session, 0).unwrap();
    let mut action = Action::apply(&file, &mut session).unwrap();
    action.undo(&mut session).unwrap();
    data.music_offset = 3.0;
    data.save(&mut session, 0).unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    assert!(action.redo(&mut session).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(fs::read(file.path()).unwrap(), bytes);
    assert!(!action.is_deleted());
    session.redo().unwrap();
    assert_eq!(
        ChartData::read(session.document(), 0)
            .unwrap()
            .unwrap()
            .music_offset,
        3.0
    );
}

#[test]
fn invalid_song_metadata_prevents_disk_deletion() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::profile_deletion::Action,
        history::Session,
    };
    use std::sync::Arc;
    let (_dir, file, bytes) = fixture();
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#GAChartDataVersion:1;#NOTES:1000;".as_slice()), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    assert!(Action::apply(&file, &mut session).is_err());
    assert_eq!(fs::read(file.path()).unwrap(), bytes);
    assert_eq!(session.serialize().unwrap(), before);
    assert!(session.undo_label().is_none());
}
