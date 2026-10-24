use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{fs, sync::Arc};

const SONG: &[u8] = b"#VERSION:0.83;#TITLE:Test;#ARTIST:Author;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:\n1000\n0000\n0000\n0000\n;";

#[test]
fn rssp_observes_unsaved_edits_and_saved_reopened_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let options = rssp::AnalysisOptions::default();
    assert_eq!(
        analysis::chart(&file.session, 0, &options)
            .unwrap()
            .stats
            .total_arrows,
        1
    );
    file.session
        .edit_notes(
            0,
            "Place",
            [(
                Position {
                    player: 0,
                    lane: 2,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let edited = analysis::chart(&file.session, 0, &options).unwrap();
    assert_eq!(edited.stats.total_arrows, 2);
    assert!(edited.max_nps.is_finite());
    assert_eq!(
        fs::read(&path).unwrap(),
        SONG,
        "Analysis must not save edits"
    );
    file.save().unwrap();
    let reopened = FileSession::open(&path).unwrap();
    let analyzed = analysis::chart(&reopened.session, 0, &options).unwrap();
    assert_eq!(analyzed.short_hash, edited.short_hash);
    assert_eq!(analyzed.stats, edited.stats);
    assert_eq!(analyzed.timing_segments.bpms, [(0.0, 120.0)]);
}

#[test]
fn cp1252_metadata_uses_the_same_decoder_as_deadsync_without_rewriting_bytes() {
    let source = b"#TITLE:\xe9t\xe9 \x80;#NOTES:dance-single::Hard:9::\n1000\n0000\n0000\n0000\n;";
    let session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Sm).unwrap());
    assert_eq!(
        session.document().text(None, "TITLE").as_deref(),
        Some("été €")
    );
    assert_eq!(session.serialize().unwrap(), source);
}
