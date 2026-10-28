//! First-save lifecycle tests for the proposed Untitled adapter patch.
use super::*;
use crate::{
    document::{Document, Format},
    file_origin::{NewSong, Origin},
    history::Session,
};
fn empty(base: &Path) -> FileSession {
    FileSession::from_new(NewSong::prepare(base, 0.009).unwrap()).unwrap()
}
#[test]
fn untouched_new_song_requires_save_as_without_creating_original_or_recovery() {
    let root = tempfile::tempdir().unwrap();
    let mut file = empty(root.path());
    assert!(file.existing_path().is_none());
    assert!(file.persisted().is_none());
    assert!(file.requires_save_as());
    assert!(file.has_unsaved_document());
    assert!(file.recovery_path().is_none());
    assert!(file.save().is_err());
    assert!(file.recover().is_err());
    assert!(file.save_recovery().is_err());
    file.session.set_text(None, "TITLE", "A new title").unwrap();
    let mut recovery = super::super::AutoRecovery::default();
    recovery.request(&file, std::time::Instant::now()).unwrap();
    assert!(!recovery.is_pending());
    assert!(file.session.undo().unwrap());
    assert!(file.session.redo().unwrap());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
#[test]
#[cfg(unix)]
fn first_save_relocates_media_in_original_and_redo_states_without_losing_guids_or_grid() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().canonicalize().unwrap();
    let output = base.join("Published");
    fs::create_dir(&output).unwrap();
    fs::write(base.join("audio.ogg"), b"original audio").unwrap();
    let document=Document::parse(b"#VERSION:0.83;#MUSIC:audio.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.ogg;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;".as_slice(),Format::Ssc).unwrap();
    let mut file = FileSession::from_new(NewSong {
        origin: Origin::Untitled {
            resource_base: base.clone(),
        },
        session: Session::new(document),
    })
    .unwrap();
    file.session
        .set_text(None, "MUSIC", "audio-v2.ogg")
        .unwrap();
    let revision = file.session.revision();
    let duplicate = file.session.duplicate_chart(0, revision).unwrap();
    let duplicate_id = file.session.chart_ids()[duplicate].clone();
    file.session.undo().unwrap();
    let ids = file.session.chart_ids().to_vec();
    let pointer = file.session.grid(0).unwrap() as *const crate::notes::Grid;
    let destination = output.join("song.ssc");
    let review = file.prepare_save_as(&destination).unwrap();
    match file.save_as(review).unwrap() {
        SaveAsOutcome::Durable {
            previous_recovery, ..
        } => assert!(previous_recovery.is_none()),
        _ => panic!("Unix directory sync should succeed in test tree"),
    }
    assert_eq!(file.existing_path(), Some(destination.as_path()));
    assert_eq!(file.resource_base(), output);
    assert!(!file.has_unsaved_document());
    assert_eq!(file.session.chart_ids(), ids);
    assert_eq!(
        file.session.loaded_grid(0).unwrap() as *const crate::notes::Grid,
        pointer
    );
    assert_eq!(
        file.session.document().text(None, "MUSIC").as_deref(),
        Some("../audio-v2.ogg")
    );
    assert_eq!(
        file.session.document().text(Some(0), "MUSIC").as_deref(),
        Some("../chart.ogg")
    );
    file.session.redo().unwrap();
    assert_eq!(file.session.chart_ids()[1], duplicate_id);
    assert_eq!(
        file.session.document().text(Some(1), "MUSIC").as_deref(),
        Some("../chart.ogg")
    );
    file.session.undo().unwrap();
    file.session.undo().unwrap();
    assert_eq!(
        file.session.document().text(None, "MUSIC").as_deref(),
        Some("../audio.ogg")
    );
    assert_eq!(fs::read(base.join("audio.ogg")).unwrap(), b"original audio");
    assert!(!base.join("Untitled.ssc").exists());
}
#[test]
#[cfg(unix)]
fn stale_first_save_after_edit_then_undo_writes_nothing_and_keeps_untitled_origin() {
    let root = tempfile::tempdir().unwrap();
    let mut file = empty(root.path());
    let destination = root.path().join("song.ssc");
    let review = file.prepare_save_as(&destination).unwrap();
    file.session
        .set_text(None, "TITLE", "intervening edit")
        .unwrap();
    file.session.undo().unwrap();
    assert!(file.save_as(review).is_err());
    assert!(file.requires_save_as());
    assert!(file.existing_path().is_none());
    assert!(!destination.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
#[test]
#[cfg(unix)]
fn first_save_late_sync_error_has_real_path_but_no_false_clean_or_original_receipt() {
    let root = tempfile::tempdir().unwrap();
    let mut file = empty(root.path());
    let destination = root.path().join("song.ssc");
    let review = file.prepare_save_as(&destination).unwrap();
    let outcome = file
        .save_as_with_sync(review, |_| {
            Err(Error::Io(std::io::Error::other("late sync")))
        })
        .unwrap();
    let receipt = match outcome {
        SaveAsOutcome::WrittenPendingDurability {
            receipt,
            previous_recovery,
            ..
        } => {
            assert!(previous_recovery.is_none());
            receipt
        }
        _ => panic!("expected pending outcome"),
    };
    assert!(file.existing_path().is_some());
    assert!(!file.requires_save_as());
    assert!(file.has_unsaved_document());
    let bytes = fs::read(&destination).unwrap();
    file.session.set_text(None, "TITLE", "new edits").unwrap();
    assert!(receipt.retry(&mut file).is_err());
    assert!(file.has_unsaved_document());
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    file.session.undo().unwrap();
    receipt.retry(&mut file).unwrap();
    assert!(!file.has_unsaved_document());
    assert_eq!(fs::read(&destination).unwrap(), bytes);
}
