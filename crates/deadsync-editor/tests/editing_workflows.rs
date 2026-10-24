//! Functional workflows through the public editor and native file adapter.
//! Fixtures are synthetic, require no user songs, and exercise reopening output.
use deadsync_editor::{
    Error,
    document::{Document, Format},
    history::{Clipboard, Session},
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
};
use std::{fs, sync::Arc};

const SSC: &[u8] = b"// author comment\r\n#VERSION:0.83;\r\n#TITLE:Example;\r\n#ARTIST:Someone;\r\n#BPMS:0=120;\r\n#UNKNOWN_VENDOR_DATA:retain\\;this;\r\n#NOTEDATA:;\r\n#STEPSTYPE:dance-single;\r\n#DIFFICULTY:Hard;\r\n#METER:9;\r\n#NOTES:\r\n1000\r\n0000\r\n0000\r\n0000\r\n;\r\n#NOTEDATA:;\r\n#STEPSTYPE:pump-single;\r\n#DIFFICULTY:Easy;\r\n#METER:3;\r\n#NOTES:\r\n00001\r\n00000\r\n00000\r\n00000\r\n;\r\n";

fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn session() -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(SSC), Format::Ssc).unwrap())
}

#[test]
fn opening_and_viewing_a_chart_preserves_every_byte() {
    let mut session = session();
    assert_eq!(session.document().charts().len(), 2);
    assert_eq!(session.grid(0).unwrap().iter().count(), 1);
    assert_eq!(session.grid(1).unwrap().columns(), 5);
    assert_eq!(session.serialize().unwrap(), SSC);
    assert!(!session.is_dirty());
}

#[test]
fn edit_undo_redo_save_and_reopen_preserves_other_charts_and_unknown_tags() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session
        .edit_notes(0, "Place note", [(pos(0, 2, 24), Some(Note::tap()))])
        .unwrap();
    assert!(file.session.is_dirty());
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), SSC);
    assert!(!file.session.is_dirty());
    file.session.redo().unwrap();
    file.save().unwrap();
    assert!(!file.session.is_dirty());
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        reopened.session.grid(0).unwrap().get(pos(0, 2, 24)),
        Some(&Note::tap())
    );
    let doc = reopened.session.document();
    assert_eq!(
        doc.value(doc.charts()[1].notes.clone()),
        b"\r\n00001\r\n00000\r\n00000\r\n00000\r\n"
    );
    assert!(
        fs::read(&path)
            .unwrap()
            .windows(b"#UNKNOWN_VENDOR_DATA:retain\\;this;".len())
            .any(|s| s == b"#UNKNOWN_VENDOR_DATA:retain\\;this;")
    );
    file.session.undo().unwrap();
    assert!(file.session.is_dirty());
    file.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), SSC);
}

#[test]
fn failed_multi_note_gesture_rolls_back_and_keeps_redo() {
    let mut session = session();
    session
        .edit_notes(0, "Place note", [(pos(0, 2, 24), Some(Note::tap()))])
        .unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    let hold = Note {
        kind: Kind::Hold,
        end: Some(96),
        ..Note::tap()
    };
    assert!(
        session
            .edit_notes(
                0,
                "Invalid gesture",
                [
                    (pos(0, 1, 0), Some(hold)),
                    (pos(0, 1, 48), Some(Note::tap()))
                ]
            )
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.redo_label(), Some("Place note"));
    session.redo().unwrap();
    assert!(session.grid(0).unwrap().get(pos(0, 2, 24)).is_some());
}

#[test]
fn deletion_with_invalid_address_is_atomic() {
    let mut session = session();
    assert!(
        session
            .edit_notes(0, "Delete", [(pos(0, 0, 0), None), (pos(0, 12, 0), None)])
            .is_err()
    );
    assert!(session.grid(0).unwrap().get(pos(0, 0, 0)).is_some());
    assert_eq!(session.serialize().unwrap(), SSC);
}

#[test]
fn metadata_escape_prevents_tag_injection_and_undo_restores_format() {
    let mut session = session();
    let title = "Été: live; #NOTES: // remix \\ final";
    session.set_text(None, "TITLE", title).unwrap();
    let reopened =
        Document::parse(Arc::<[u8]>::from(session.serialize().unwrap()), Format::Ssc).unwrap();
    assert_eq!(reopened.text(None, "TITLE").as_deref(), Some(title));
    assert_eq!(reopened.charts().len(), 2);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), SSC);
}

#[test]
fn external_modification_refuses_save_and_keeps_user_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session.set_text(None, "TITLE", "My draft").unwrap();
    fs::write(&path, b"changed elsewhere").unwrap();
    assert!(matches!(file.save(), Err(Error::ExternalChange)));
    assert!(file.session.is_dirty());
    assert_eq!(fs::read(&path).unwrap(), b"changed elsewhere");
    assert_eq!(
        file.session.document().text(None, "TITLE").as_deref(),
        Some("My draft")
    );
}

#[test]
fn durable_draft_survives_session_close_and_can_be_saved_after_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    {
        let mut file = FileSession::open(&path).unwrap();
        file.session
            .edit_notes(1, "Place note", [(pos(0, 0, 96), Some(Note::tap()))])
            .unwrap();
        file.save_recovery().unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), SSC);
    let mut file = FileSession::open(&path).unwrap();
    file.recover().unwrap();
    assert!(file.session.is_dirty());
    assert!(file.session.grid(1).unwrap().get(pos(0, 0, 96)).is_some());
    file.save().unwrap();
    assert!(
        FileSession::open(&path)
            .unwrap()
            .session
            .grid(1)
            .unwrap()
            .get(pos(0, 0, 96))
            .is_some()
    );
}

#[test]
fn recovery_refuses_when_original_changed_or_current_document_is_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session.set_text(None, "TITLE", "Draft").unwrap();
    file.save_recovery().unwrap();
    let before = file.session.serialize().unwrap();
    assert!(file.recover().is_err());
    assert_eq!(file.session.serialize().unwrap(), before);
    fs::write(&path, SSC.replace_bytes(b"Example", b"Changed")).unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert!(matches!(reopened.recover(), Err(Error::ExternalChange)));
    assert!(!reopened.session.is_dirty());
}

trait ReplaceBytes {
    fn replace_bytes(&self, needle: &[u8], replacement: &[u8]) -> Vec<u8>;
}
impl ReplaceBytes for [u8] {
    fn replace_bytes(&self, needle: &[u8], replacement: &[u8]) -> Vec<u8> {
        let at = self
            .windows(needle.len())
            .position(|part| part == needle)
            .unwrap();
        [&self[..at], replacement, &self[at + needle.len()..]].concat()
    }
}

#[test]
fn routine_owners_holds_rolls_keysounds_and_attack_attributes_round_trip() {
    let notes = b"2000\n0000\n3000\n0000\n&\n0400\n0000\n0300\nA{mod,1}0K[12]0\n&\n0001\n0000\n0000\n0000\n";
    let grid = Grid::parse(notes, 4).unwrap();
    assert_eq!(grid.players(), 3);
    assert_eq!(grid.get(pos(0, 0, 0)).unwrap().end, Some(96));
    assert_eq!(grid.get(pos(1, 1, 0)).unwrap().kind, Kind::Roll);
    assert_eq!(
        grid.get(pos(1, 2, 144)).unwrap().attributes.as_ref(),
        "[12]"
    );
    assert_eq!(
        grid.get(pos(1, 0, 144)).unwrap().attributes.as_ref(),
        "{mod,1}"
    );
    assert_eq!(Grid::parse(&grid.serialize().unwrap(), 4).unwrap(), grid);
}

#[test]
fn paste_single_owner_uses_active_player_and_mixed_owner_preserves_assignments() {
    let source = b"#TITLE:Routine;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:\n10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000\n;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    let single = Clipboard::copy(session.grid(0).unwrap(), 0, 0, Some(0)).unwrap();
    single.paste(&mut session, 0, 48, 1).unwrap();
    assert!(session.grid(0).unwrap().get(pos(1, 0, 48)).is_some());
    let mixed = Clipboard::copy(session.grid(0).unwrap(), 0, 0, None).unwrap();
    mixed.paste(&mut session, 0, 96, 0).unwrap();
    assert!(session.grid(0).unwrap().get(pos(0, 0, 96)).is_some());
    assert!(session.grid(0).unwrap().get(pos(1, 4, 96)).is_some());
    session.undo().unwrap();
    assert!(session.grid(0).unwrap().get(pos(1, 4, 96)).is_none());
}

#[test]
fn unsupported_symbols_and_precision_never_rewrite_source() {
    let bytes = SSC.replace_bytes(b"1000", b"X000");
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), Format::Ssc).unwrap());
    assert!(matches!(session.grid(0), Err(Error::Unsupported(_))));
    assert_eq!(session.serialize().unwrap(), bytes);
    assert!(Grid::parse(b"0000\n1000\n0000\n0000\n0000\n0000\n0000", 4).is_err());
}

#[test]
fn sm_note_edit_keeps_header_radar_and_legacy_metadata_bytes() {
    let source = b"#TITLE:\xe9t\xe9;\n#NOTES:dance-single:credit:Hard:9:0.5,0.4,0.3,0.2,0.1:\n1000\n0000\n0000\n0000\n;\n";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Sm).unwrap());
    session
        .edit_notes(0, "Place", [(pos(0, 2, 12), Some(Note::tap()))])
        .unwrap();
    let output = session.serialize().unwrap();
    assert!(
        output.starts_with(
            b"#TITLE:\xe9t\xe9;\n#NOTES:dance-single:credit:Hard:9:0.5,0.4,0.3,0.2,0.1:"
        )
    );
    let mut reopened =
        Session::new(Document::parse(Arc::<[u8]>::from(output), Format::Sm).unwrap());
    assert!(reopened.grid(0).unwrap().get(pos(0, 2, 12)).is_some());
}

#[test]
fn visible_query_includes_a_sustain_entering_the_viewport() {
    let grid = Grid::parse(b"2000\n0000\n3000\n0000", 4).unwrap();
    assert_eq!(grid.visible(24, 48).count(), 1);
    assert_eq!(grid.visible(100, 144).count(), 0);
}
