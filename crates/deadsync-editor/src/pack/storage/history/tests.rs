use super::*;
use crate::{
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
};
use std::fs;
fn fixture() -> (tempfile::TempDir, Owner, Session, Vec<u8>) {
    let root = tempfile::tempdir().unwrap();
    let data = Data::new("Original", "", 2026, None, 0.009);
    let mut bytes = data.canonical().unwrap();
    bytes.extend_from_slice(b"FutureOpaque=keep-this\n");
    fs::write(root.path().join("Pack.ini"), &bytes).unwrap();
    let owner = Owner::new(FileSession::open(root.path()).unwrap().unwrap());
    let session = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    (root, owner, session, bytes)
}
fn commit(session: &mut Session, owner: &Owner, after: Data) -> Result<bool, crate::Error> {
    session.edit_pack_properties_reviewed(&owner.review()?, after)
}
#[test]
fn note_then_pack_fields_are_one_global_order_without_song_dirty_or_implicit_disk_writes() {
    let (root, owner, mut session, disk) = fixture();
    let empty = session.serialize().unwrap();
    session
        .edit_notes(
            0,
            "Note",
            [(
                Position {
                    row: 48,
                    lane: 0,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let notes = session.serialize().unwrap();
    let content = session.content_revision();
    let before = owner.data().unwrap();
    let mut after = before.clone();
    after.sync_offset = 0;
    assert!(commit(&mut session, &owner, after.clone()).unwrap());
    assert_eq!(session.content_revision(), content);
    assert!(session.can_undo_application_change());
    assert_eq!(session.serialize().unwrap(), notes);
    assert_eq!(fs::read(root.path().join("Pack.ini")).unwrap(), disk);
    assert!(session.undo().unwrap());
    assert_eq!(owner.data().unwrap(), before);
    assert_eq!(session.serialize().unwrap(), notes);
    assert!(session.undo().unwrap());
    assert_eq!(session.serialize().unwrap(), empty);
    assert!(session.redo().unwrap());
    assert_eq!(session.serialize().unwrap(), notes);
    assert!(session.redo().unwrap());
    assert_eq!(owner.data().unwrap(), after);
    assert_eq!(fs::read(root.path().join("Pack.ini")).unwrap(), disk);
}
#[test]
fn cached_field_publishes_one_action_at_deactivation_and_net_zero_preserves_redo() {
    let (_root, owner, mut session, _) = fixture();
    let original = owner.data().unwrap();
    session
        .edit_notes(
            0,
            "Note",
            [(
                Position {
                    row: 48,
                    lane: 0,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    session.undo().unwrap();
    let revision = session.revision();
    let redo = session.redo_label().map(str::to_owned);
    let review = owner.review().unwrap();
    let mut staged = review.data().clone();
    for value in ["A", "Ab", "Abc"] {
        staged.title = Some(value.into());
        assert_eq!(owner.data().unwrap(), original);
        assert_eq!(session.revision(), revision);
        assert_eq!(session.redo_label(), redo.as_deref());
    }
    staged.title = original.title.clone();
    assert!(
        !session
            .edit_pack_properties_reviewed(&review, staged)
            .unwrap()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.redo_label(), redo.as_deref());
    assert!(session.undo_label().is_none());
    session.redo().unwrap();
    let review = owner.review().unwrap();
    let mut staged = review.data().clone();
    staged.title = Some("Final".into());
    session
        .edit_pack_properties_reviewed(&review, staged.clone())
        .unwrap();
    assert_eq!(owner.data().unwrap(), staged);
    session.undo().unwrap();
    assert_eq!(owner.data().unwrap(), original);
    assert!(!session.history_changes_pack_properties(false));
    session.redo().unwrap();
    assert_eq!(owner.data().unwrap(), staged);
}
#[test]
fn untracked_change_refuses_stale_field_and_separate_fresh_commit_preserves_it_on_undo() {
    let (_root, owner, mut session, _) = fixture();
    let mut a = owner.data().unwrap();
    a.title = Some("A".into());
    commit(&mut session, &owner, a).unwrap();
    let stale = owner.review().unwrap();
    let mut untracked = owner.data().unwrap();
    untracked.series = Some("Outside history".into());
    owner.replace_untracked(untracked.clone()).unwrap();
    let mut b = stale.data().clone();
    b.title = Some("B".into());
    let revision = session.revision();
    assert!(matches!(
        session.edit_pack_properties_reviewed(&stale, b),
        Err(crate::Error::ExternalChange)
    ));
    assert_eq!(session.revision(), revision);
    assert_eq!(owner.data().unwrap(), untracked);
    let fresh = owner.review().unwrap();
    let mut b = fresh.data().clone();
    b.title = Some("B".into());
    session
        .edit_pack_properties_reviewed(&fresh, b.clone())
        .unwrap();
    session.undo().unwrap();
    assert_eq!(
        owner.data().unwrap(),
        untracked,
        "Undo B preserves Series from outside history"
    );
    let revision = session.revision();
    let undo = session.undo_label().map(str::to_owned);
    assert!(matches!(session.undo(), Err(crate::Error::ExternalChange)));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.undo_label(), undo.as_deref());
    assert_eq!(
        owner.data().unwrap(),
        untracked,
        "Undo A cannot overwrite the external logical token"
    );
    session.redo().unwrap();
    assert_eq!(owner.data().unwrap(), b);
}
#[test]
fn explicit_save_updates_baseline_but_preserves_memory_undo_and_unknown_bytes() {
    let (root, owner, mut session, _) = fixture();
    let path = root.path().join("Pack.ini");
    let original = owner.data().unwrap();
    let mut after = original.clone();
    after.title = Some("Saved".into());
    commit(&mut session, &owner, after.clone()).unwrap();
    owner.with(FileSession::save).unwrap();
    let saved = fs::read(&path).unwrap();
    assert!(String::from_utf8_lossy(&saved).contains("FutureOpaque=keep-this"));
    assert!(
        !owner
            .with(|file| Ok(file.draft().has_unsaved_changes()))
            .unwrap()
    );
    session.undo().unwrap();
    assert_eq!(owner.data().unwrap(), original);
    assert!(
        owner
            .with(|file| Ok(file.draft().has_unsaved_changes()))
            .unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), saved);
    session.redo().unwrap();
    assert_eq!(owner.data().unwrap(), after);
    assert!(
        !owner
            .with(|file| Ok(file.draft().has_unsaved_changes()))
            .unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), saved);
}
#[test]
fn external_change_retirement_and_untracked_edit_refuse_without_popping_history() {
    for scenario in 0..3 {
        let (root, owner, mut session, _) = fixture();
        let before = owner.data().unwrap();
        let mut after = before.clone();
        after.sync_offset = 0;
        commit(&mut session, &owner, after.clone()).unwrap();
        match scenario {
            0 => fs::write(root.path().join("Pack.ini"), b"external writer").unwrap(),
            1 => owner.retire(),
            _ => {
                let mut other = after.clone();
                other.series = Some("Outside history".into());
                owner.replace_untracked(other).unwrap();
            }
        }
        let revision = session.revision();
        let label = session.undo_label().map(str::to_owned);
        let bytes = session.serialize().unwrap();
        let error = session.undo().unwrap_err();
        if scenario == 1 {
            assert!(
                matches!(error,crate::Error::InvalidEdit(ref message) if message=="Pack property history owner is no longer current")
            );
        } else {
            assert!(matches!(error, crate::Error::ExternalChange));
        }
        assert_eq!(session.revision(), revision);
        assert_eq!(session.undo_label(), label.as_deref());
        assert_eq!(session.serialize().unwrap(), bytes);
    }
}
#[test]
fn published_sync_failure_retains_receipt_and_blocks_undo_until_retry_without_rewrite() {
    let (root, owner, mut session, _) = fixture();
    let path = root.path().join("Pack.ini");
    let before = owner.data().unwrap();
    let mut after = before.clone();
    after.sync_offset = 0;
    commit(&mut session, &owner, after.clone()).unwrap();
    assert!(matches!(
        owner.with(|file| {
            let snapshot = file.prepare_save()?;
            file.publish_with(
                snapshot,
                |_| Ok(()),
                |_| Err(std::io::Error::other("post-publication sync failure")),
            )
        }),
        Err(Error::DurabilityPending(_))
    ));
    let published = fs::read(&path).unwrap();
    let identity = fs::metadata(&path).unwrap().modified().unwrap();
    let revision = session.revision();
    assert!(session.undo().is_err());
    assert_eq!(session.revision(), revision);
    assert_eq!(owner.data().unwrap(), after);
    owner.with(FileSession::retry_durability).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), identity);
    session.undo().unwrap();
    assert_eq!(owner.data().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(owner.data().unwrap(), after);
    assert_eq!(fs::read(&path).unwrap(), published);
}
#[test]
fn invalid_and_noop_pack_edits_keep_history_and_send_owner_can_move_to_worker() {
    fn send<T: Send>() {}
    send::<Owner>();
    send::<Session>();
    let (_root, owner, mut session, _) = fixture();
    let data = owner.data().unwrap();
    let revision = session.revision();
    assert!(!commit(&mut session, &owner, data.clone()).unwrap());
    let mut bad = data;
    bad.title = Some("bad\nline".into());
    assert!(commit(&mut session, &owner, bad).is_err());
    assert_eq!(session.revision(), revision);
    assert!(session.undo_label().is_none());
    assert!(!session.is_dirty());
}

#[test]
fn deleted_owner_cannot_recreate_pack_through_property_history() {
    let (root, owner, mut session, _) = fixture();
    let mut data = owner.data().unwrap();
    data.series = Some("Edited".into());
    commit(&mut session, &owner, data).unwrap();
    owner.delete(owner.prepare_delete().unwrap()).unwrap();
    let revision = session.revision();
    assert!(
        matches!(session.undo(),Err(crate::Error::InvalidEdit(ref message)) if message=="Pack property history owner is no longer current")
    );
    assert_eq!(session.revision(), revision);
    assert!(!root.path().join("Pack.ini").exists());
}

#[test]
fn external_change_allows_local_review_but_refuses_commit_and_save_without_history_loss() {
    let (root, owner, mut session, _) = fixture();
    let before = owner.data().unwrap();
    let external = b"[Group]\nDisplayTitle=External\n";
    fs::write(root.path().join("Pack.ini"), external).unwrap();
    let review = owner.review().expect("local cached field may start after Ignore");
    assert_eq!(review.data(), &before);
    let mut after = before.clone(); after.title = Some("Local draft".into());
    let label = session.undo_label().map(str::to_owned);
    assert!(matches!(session.edit_pack_properties_reviewed(&review, after), Err(crate::Error::ExternalChange)));
    assert!(matches!(owner.save(), Err(Error::ExternalChange)));
    assert_eq!(owner.data().unwrap(), before);
    assert_eq!(session.undo_label(), label.as_deref());
    assert_eq!(fs::read(root.path().join("Pack.ini")).unwrap(), external);
}
