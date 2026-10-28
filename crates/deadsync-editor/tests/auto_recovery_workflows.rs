use deadsync_editor::{
    Error,
    storage::{AutoRecovery, FileSession},
};
use std::{
    fs,
    time::{Duration, Instant},
};
const SONG: &[u8] = b"#TITLE:Original;#BPMS:0=120;#NOTES:dance-single:Test:Hard:9:0,0,0,0,0:1000;";
fn file() -> (tempfile::TempDir, FileSession) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    fs::write(&path, SONG).unwrap();
    let file = FileSession::open(&path).unwrap();
    (dir, file)
}
fn finish(auto: &mut AutoRecovery, file: &FileSession, now: Instant) -> Result<(), Error> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while auto.is_pending() {
        auto.update(file, now)?;
        assert!(Instant::now() < deadline, "Recovery worker timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
#[test]
fn interval_snapshot_survives_close_without_saving_or_clearing_history() {
    let (_dir, mut file) = file();
    let mut auto = AutoRecovery::default();
    let start = Instant::now();
    auto.update(&file, start).unwrap();
    file.session.set_text(None, "TITLE", "Draft").unwrap();
    let dirty = file.session.serialize().unwrap();
    auto.update(&file, start + Duration::from_secs(29)).unwrap();
    assert!(!auto.is_pending());
    assert!(!file.recovery_path().unwrap().exists());
    let now = start + Duration::from_secs(30);
    auto.update(&file, now).unwrap();
    assert!(auto.is_pending());
    finish(&mut auto, &file, now).unwrap();
    assert_eq!(fs::read(file.require_existing_path().unwrap()).unwrap(), SONG);
    assert!(file.session.is_dirty());
    file.session.undo().unwrap();
    assert!(!file.session.is_dirty());
    let path = file.require_existing_path().unwrap().to_owned();
    drop(file);
    let mut recovered = FileSession::open(&path).unwrap();
    recovered.recover().unwrap();
    assert_eq!(recovered.session.serialize().unwrap(), dirty);
    assert!(recovered.session.is_dirty());
}
#[test]
fn unchanged_revisions_are_skipped_but_recovered_session_identity_is_not() {
    let (_dir, mut file) = file();
    let mut auto = AutoRecovery::default();
    let now = Instant::now();
    file.session.set_text(None, "TITLE", "Draft").unwrap();
    auto.request(&file, now).unwrap();
    finish(&mut auto, &file, now).unwrap();
    auto.request(&file, now).unwrap();
    assert!(!auto.is_pending());
    file.session.undo().unwrap();
    file.recover().unwrap();
    auto.request(&file, now).unwrap();
    assert!(auto.is_pending());
    finish(&mut auto, &file, now).unwrap();
}
#[test]
fn existing_draft_is_preserved_until_explicit_recovery_or_manual_replacement() {
    let (_dir, mut owner) = file();
    owner
        .session
        .set_text(None, "TITLE", "Earlier draft")
        .unwrap();
    owner.save_recovery().unwrap();
    let previous = fs::read(owner.recovery_path().unwrap()).unwrap();
    let mut other = FileSession::open(owner.require_existing_path().unwrap()).unwrap();
    other.session.set_text(None, "TITLE", "New edits").unwrap();
    let mut auto = AutoRecovery::default();
    let now = Instant::now();
    auto.request(&other, now).unwrap();
    assert!(matches!(
        finish(&mut auto, &other, now),
        Err(Error::RecoveryConflict)
    ));
    assert_eq!(fs::read(owner.recovery_path().unwrap()).unwrap(), previous);
    other.save_recovery().unwrap();
    other.session.set_text(None, "TITLE", "More edits").unwrap();
    auto.request(&other, now).unwrap();
    finish(&mut auto, &other, now).unwrap();
    let mut reopened = FileSession::open(other.require_existing_path().unwrap()).unwrap();
    reopened.recover().unwrap();
    assert_eq!(
        reopened.session.serialize().unwrap(),
        other.session.serialize().unwrap()
    );
}
#[test]
fn external_changes_preserve_the_last_draft_and_retry_after_resolution() {
    let (_dir, mut file) = file();
    let mut auto = AutoRecovery::default();
    let now = Instant::now();
    file.session.set_text(None, "TITLE", "Draft").unwrap();
    auto.request(&file, now).unwrap();
    finish(&mut auto, &file, now).unwrap();
    let previous = fs::read(file.recovery_path().unwrap()).unwrap();
    file.session.set_text(None, "TITLE", "New draft").unwrap();
    fs::write(file.require_existing_path().unwrap(), b"#TITLE:External;").unwrap();
    auto.request(&file, now).unwrap();
    assert!(matches!(
        finish(&mut auto, &file, now),
        Err(Error::ExternalChange)
    ));
    assert_eq!(fs::read(file.recovery_path().unwrap()).unwrap(), previous);
    fs::write(file.require_existing_path().unwrap(), SONG).unwrap();
    auto.update(&file, now + Duration::from_secs(29)).unwrap();
    assert!(!auto.is_pending());
    let retry = now + Duration::from_secs(30);
    auto.update(&file, retry).unwrap();
    finish(&mut auto, &file, retry).unwrap();
    let mut reopened = FileSession::open(file.require_existing_path().unwrap()).unwrap();
    reopened.recover().unwrap();
    assert_eq!(
        reopened.session.serialize().unwrap(),
        file.session.serialize().unwrap()
    );
}
#[test]
fn save_then_undo_rebases_even_a_previously_saved_recovery_revision() {
    let (_dir, mut file) = file();
    let mut auto = AutoRecovery::default();
    let now = Instant::now();
    file.session.set_text(None, "TITLE", "First").unwrap();
    auto.request(&file, now).unwrap();
    finish(&mut auto, &file, now).unwrap();
    let first = file.session.serialize().unwrap();
    file.session.set_text(None, "TITLE", "Second").unwrap();
    file.save().unwrap();
    file.session.undo().unwrap();
    auto.request(&file, now).unwrap();
    assert!(auto.is_pending());
    finish(&mut auto, &file, now).unwrap();
    let mut reopened = FileSession::open(file.require_existing_path().unwrap()).unwrap();
    reopened.recover().unwrap();
    assert_eq!(reopened.session.serialize().unwrap(), first);
}
#[test]
fn busy_save_lock_does_not_publish_a_partial_draft_or_mark_the_revision_saved() {
    let (dir, mut file) = file();
    let lock = fs::File::create(dir.path().join(".song.sm.deadsync-save-lock")).unwrap();
    lock.try_lock().unwrap();
    file.session.set_text(None, "TITLE", "Draft").unwrap();
    let mut auto = AutoRecovery::default();
    let now = Instant::now();
    auto.request(&file, now).unwrap();
    assert!(finish(&mut auto, &file, now).is_err());
    assert!(!file.recovery_path().unwrap().exists());
    drop(lock);
    auto.request(&file, now).unwrap();
    finish(&mut auto, &file, now).unwrap();
    assert!(file.recovery_path().unwrap().exists());
}
