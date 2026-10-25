use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
};
use std::{fs, sync::Arc};
const SSC: &[u8] = b"#VERSION:0.83;#TITLE:Original;#ARTIST:Author;#BPMS:0=120;#VENDOR:root-data;\n#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Source;#DIFFICULTY:Hard;#METER:9;#CREDIT:Original Author;#VENDOR_CHART:keep\\;exact;#BPMS:0=180;#STOPS:2=0.25;#NOTES:\n1000\n0000\n0000\n0000\n;\n// footer without newline";
fn position(lane: u16, row: u64) -> Position {
    Position {
        player: 0,
        lane,
        row,
    }
}
fn generated() -> Grid {
    let mut grid = Grid::new(8, 1).unwrap();
    for (lane, row, kind, end) in [
        (0, 0, Kind::Roll, Some(96)),
        (3, 48, Kind::Fake, None),
        (4, 72, Kind::Lift, None),
        (7, 120, Kind::Mine, None),
    ] {
        grid.set(
            position(lane, row),
            Some(Note {
                kind,
                end,
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    grid
}
#[test]
fn append_save_reopen_preserves_existing_bytes_and_effective_chart_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session
        .edit_notes(0, "Manual note", [(position(1, 48), Some(Note::tap()))])
        .unwrap();
    file.session
        .set_text(Some(0), "DESCRIPTION", "Updated: source")
        .unwrap();
    file.session
        .set_text(Some(0), "NEW_VENDOR", "value;#safe")
        .unwrap();
    let before = file.session.serialize().unwrap();
    let source_grid = file.session.grid(0).unwrap().clone();
    let grid = generated();
    let revision = file.session.revision();
    let added = file
        .session
        .append_chart(0, "dance-double", &grid, revision, "Generate chart")
        .unwrap();
    assert_eq!(added, 1);
    assert_eq!(file.session.undo_label(), Some("Generate chart"));
    let after = file.session.serialize().unwrap();
    assert!(after.starts_with(&before));
    assert_eq!(file.session.grid(0).unwrap(), &source_grid);
    assert_eq!(file.session.grid(1).unwrap(), &grid);
    assert_eq!(
        file.session
            .document()
            .text(Some(1), "DESCRIPTION")
            .as_deref(),
        Some("Updated: source")
    );
    assert_eq!(
        file.session.document().text(Some(1), "CREDIT").as_deref(),
        Some("Original Author")
    );
    assert_eq!(
        file.session
            .document()
            .text(Some(1), "NEW_VENDOR")
            .as_deref(),
        Some("value;#safe")
    );
    assert_eq!(
        file.session
            .document()
            .raw_property(Some(1), "VENDOR_CHART"),
        Some(b"keep\\;exact".as_slice())
    );
    let analyzed = analysis::chart(&file.session, 1, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(analyzed.timing_segments.bpms, [(0.0, 180.0)]);
    assert_eq!(
        fs::read(&path).unwrap(),
        SSC,
        "Applying preview is not saving"
    );
    file.save().unwrap();
    assert!(!file.session.is_dirty());
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.grid(1).unwrap(), &grid);
    assert_eq!(
        analysis::chart(&reopened.session, 1, &rssp::AnalysisOptions::default())
            .unwrap()
            .short_hash,
        analyzed.short_hash
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), before);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), after);
}
#[test]
fn structural_history_keeps_earlier_span_and_note_edits_valid_across_multiple_additions() {
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(SSC), Format::Ssc).unwrap());
    let original = session.serialize().unwrap();
    session
        .edit_notes(0, "Earlier note", [(position(1, 48), Some(Note::tap()))])
        .unwrap();
    let notes = session.serialize().unwrap();
    session
        .set_text(None, "TITLE", "A much longer title than before")
        .unwrap();
    let title = session.serialize().unwrap();
    let revision = session.revision();
    session
        .append_chart(0, "dance-double", &generated(), revision, "First chart")
        .unwrap();
    let first = session.serialize().unwrap();
    session
        .edit_notes(1, "Generated edit", [(position(6, 144), Some(Note::tap()))])
        .unwrap();
    session
        .set_text(Some(0), "CREDIT", "Updated author between additions")
        .unwrap();
    let edited = session.serialize().unwrap();
    let revision = session.revision();
    session
        .append_chart(1, "dance-double", session_grid(), revision, "Second chart")
        .unwrap();
    let second = session.serialize().unwrap();
    assert_eq!(session.document().charts().len(), 3);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), edited);
    session.undo().unwrap();
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), first);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), title);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), notes);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    for _ in 0..6 {
        session.redo().unwrap();
    }
    assert_eq!(session.serialize().unwrap(), second);
    fn session_grid() -> &'static Grid {
        static GRID: std::sync::OnceLock<Grid> = std::sync::OnceLock::new();
        GRID.get_or_init(generated)
    }
}
#[test]
fn sm_chart_addition_preserves_legacy_bytes_header_fields_and_global_timing() {
    let source = b"#TITLE:\xe9t\xe9;#BPMS:0=150;#VENDOR:untouched;\n#NOTES2:\n dance-single:\n Caf\xe9\\: name:\n Hard:\n 12:\n 0.1,0.2,0.3,0.4,0.5:\n1000\n0000\n0000\n0000\n;";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sm");
    fs::write(&path, source).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let grid = generated();
    file.session
        .append_chart(0, "dance-double", &grid, 0, "Generate")
        .unwrap();
    let bytes = file.session.serialize().unwrap();
    assert!(bytes.starts_with(source));
    assert_eq!(
        file.session.document().charts()[1].description,
        "Café: name"
    );
    assert_eq!(file.session.document().charts()[1].meter, "12");
    assert_eq!(file.session.grid(1).unwrap(), &grid);
    assert!(
        bytes
            .windows(b"Caf\xe9".len())
            .filter(|bytes| *bytes == b"Caf\xe9")
            .count()
            == 2
    );
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.grid(1).unwrap(), &grid);
    assert_eq!(
        analysis::chart(&reopened.session, 1, &rssp::AnalysisOptions::default())
            .unwrap()
            .timing_segments
            .bpms,
        [(0.0, 150.0)]
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), source);
}
#[test]
fn rejected_or_stale_chart_additions_preserve_history_and_redo() {
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(SSC), Format::Ssc).unwrap());
    session.set_text(None, "TITLE", "Changed").unwrap();
    session.undo().unwrap();
    let before = session.serialize().unwrap();
    let redo = session.redo_label().unwrap().to_owned();
    assert!(
        session
            .append_chart(
                0,
                "dance-single",
                &generated(),
                session.revision(),
                "Invalid width"
            )
            .is_err()
    );
    assert!(
        session
            .append_chart(
                9,
                "dance-double",
                &generated(),
                session.revision(),
                "Invalid source"
            )
            .is_err()
    );
    assert!(
        session
            .append_chart(0, "dance-double", &generated(), 999, "Stale")
            .is_err()
    );
    assert!(
        session
            .append_chart(
                0,
                "dance-double;#TITLE:injected",
                &generated(),
                session.revision(),
                "Invalid layout"
            )
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.redo_label(), Some(redo.as_str()));
    assert_eq!(session.document().charts().len(), 1);
}

#[test]
fn note_attributes_cannot_inject_simfile_tags_or_comments() {
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(SSC), Format::Ssc).unwrap());
    for attributes in [
        "[value;#TITLE:Injected]",
        "[value#TITLE:Injected]",
        "[value//comment]",
    ] {
        let before = session.serialize().unwrap();
        let revision = session.revision();
        let note = Note {
            attributes: attributes.into(),
            ..Note::tap()
        };
        assert!(
            session
                .edit_notes(0, "Unsafe attributes", [(position(1, 48), Some(note))])
                .is_err()
        );
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.revision(), revision);
    }
    let note = Note {
        attributes: r"[value\;\#safe\/\/text]".into(),
        ..Note::tap()
    };
    session
        .edit_notes(0, "Escaped attributes", [(position(1, 48), Some(note))])
        .unwrap();
    let grid = session.grid(0).unwrap().clone();
    let bytes = session.serialize().unwrap();
    let mut reopened =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    assert_eq!(reopened.grid(0).unwrap(), &grid);
}

#[test]
fn appending_preserves_unsupported_charts_as_opaque_bytes() {
    let mut bytes = SSC.to_vec();
    bytes.extend_from_slice(
        b"\n#NOTEDATA:;#STEPSTYPE:custom-opaque;#VENDOR:keep;#NOTES:opaque data;\n",
    );
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), Format::Ssc).unwrap());
    let index = session
        .append_chart(0, "dance-double", &generated(), 0, "Generate chart")
        .unwrap();
    assert_eq!(index, 2);
    assert!(session.serialize().unwrap().starts_with(&bytes));
    assert_eq!(session.grid(2).unwrap(), &generated());
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), bytes);
}
