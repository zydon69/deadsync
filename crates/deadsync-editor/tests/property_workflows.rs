use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
};
use std::sync::Arc;

#[test]
fn adding_song_and_chart_properties_keeps_their_scopes_and_original_bytes() {
    let source = b"#TITLE:Test;\n#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:\n1000\n0000\n0000\n0000\n;\n#NOTEDATA:;#STEPSTYPE:pump-single;#NOTES:\n00001\n00000\n00000\n00000\n;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    session.set_text(None, "ARTIST", "Someone").unwrap();
    session.set_text(Some(0), "CREDIT", "First author").unwrap();
    session
        .set_text(Some(1), "CREDIT", "Second author")
        .unwrap();
    session
        .set_text(Some(0), "DESCRIPTION", "A new chart")
        .unwrap();
    session
        .edit_notes(
            0,
            "Place note",
            [(
                Position {
                    player: 0,
                    lane: 3,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let reopened =
        Document::parse(Arc::<[u8]>::from(session.serialize().unwrap()), Format::Ssc).unwrap();
    assert_eq!(reopened.text(None, "ARTIST").as_deref(), Some("Someone"));
    assert_eq!(
        reopened.text(Some(0), "CREDIT").as_deref(),
        Some("First author")
    );
    assert_eq!(
        reopened.text(Some(1), "CREDIT").as_deref(),
        Some("Second author")
    );
    assert_eq!(reopened.charts()[0].description, "A new chart");
    assert!(reopened.text(None, "CREDIT").is_none());
    for _ in 0..5 {
        session.undo().unwrap();
    }
    assert_eq!(session.serialize().unwrap(), source);
}

#[test]
fn chart_properties_cannot_be_silently_added_to_sm() {
    let source = b"#TITLE:Test;#NOTES:dance-single::Hard:9::\n1000\n0000\n0000\n0000\n;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Sm).unwrap());
    assert!(session.set_text(Some(0), "BPMS", "0=150").is_err());
    assert_eq!(session.serialize().unwrap(), source);
    assert!(!session.is_dirty());
}
