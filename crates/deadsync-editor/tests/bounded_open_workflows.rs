use deadsync_editor::{document::Format, storage::FileSession};
use std::{fs, sync::atomic::AtomicBool};
const SSC: &[u8] =
    b"#VERSION:0.83;#TITLE:Bounded;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
#[test]
fn bounded_open_at_exact_cap_matches_strict_original_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let bounded = FileSession::open_bounded(&path, SSC.len(), &AtomicBool::new(false)).unwrap();
    let original = FileSession::open(&path).unwrap();
    assert_eq!(bounded.session.document().format, Format::Ssc);
    assert_eq!(
        bounded.session.serialize().unwrap(),
        original.session.serialize().unwrap()
    );
    assert_eq!(fs::read(path).unwrap(), SSC);
}
#[test]
fn too_large_sparse_file_is_rejected_and_kept_intact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let file = fs::File::create(&path).unwrap();
    file.set_len(64 * 1024 * 1024).unwrap();
    let result = FileSession::open_bounded(&path, 1024, &AtomicBool::new(false));
    assert!(
        matches!(result,Err(deadsync_editor::Error::Unsupported(message)) if message=="Song byte limit exceeded")
    );
    assert_eq!(fs::metadata(path).unwrap().len(), 64 * 1024 * 1024);
}
#[test]
fn cancelled_open_keeps_the_original_file_and_exposes_no_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let result = FileSession::open_bounded(&path, SSC.len(), &AtomicBool::new(true));
    assert!(
        matches!(result,Err(deadsync_editor::Error::InvalidEdit(message)) if message=="Song opening cancelled")
    );
    assert_eq!(fs::read(path).unwrap(), SSC);
}
#[test]
fn bounded_open_keeps_strict_missing_notes_validation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes = b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;";
    fs::write(&path, bytes).unwrap();
    let original = FileSession::open(&path).err().unwrap().to_string();
    let bounded = FileSession::open_bounded(&path, bytes.len(), &AtomicBool::new(false))
        .err()
        .unwrap()
        .to_string();
    assert_eq!(bounded, original);
    assert_eq!(fs::read(path).unwrap(), bytes);
}
