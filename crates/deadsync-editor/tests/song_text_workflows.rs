use deadsync_editor::{
    chart_text::{self, Field as ChartField},
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    playback::Preview,
    song_properties::Draft,
    song_text::{Field, Text},
    storage::FileSession,
};
use serde_json::Value;
use std::{fs, path::PathBuf};
#[test]
fn actual_fumen_song_fields_and_initial_chart_authors_match_without_rewriting_files() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy();
        if !name.starts_with("song-text-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if request["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let source = request["Content"].as_str().unwrap().as_bytes();
        let mut session = Session::new(Document::parse(source, format).unwrap());
        let fields = Text::read(session.document());
        for field in Field::ALL {
            assert_eq!(
                fields.get(field),
                expected["Fields"][field.tag()].as_str().unwrap(),
                "{name} {}",
                field.tag()
            );
        }
        let draft = Draft::capture(&session).unwrap();
        assert_eq!(
            draft.sample_start(),
            expected["SampleStart"].as_f64().unwrap()
        );
        assert_eq!(
            draft.sample_length(),
            expected["SampleLength"].as_f64().unwrap()
        );
        for (index, credit) in expected["ChartCredits"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                chart_text::Text::read(session.document(), index)
                    .unwrap()
                    .get(ChartField::Credit),
                credit.as_str().unwrap()
            );
        }
        assert!(
            !Draft::capture(&session)
                .unwrap()
                .apply(&mut session)
                .unwrap()
        );
        assert_eq!(session.serialize().unwrap(), source);
    }
    assert_eq!(count, 6);
}
#[test]
fn all_song_fields_and_sync_commit_once_preserve_chart_authors_pending_notes_and_unknown_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes=b"#VERSION:0.83;#TITLE:CP\xe9;#CREDIT:Old author;#MUSIC:song.ogg;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0,\"Future\":{\"keep\":true},\"DefaultChartIndex\":1};#VENDOR:CP\xe9;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#CREDIT:Explicit;#MUSIC:chart.ogg;#NOTES:0001;";
    fs::write(&path, bytes).unwrap();
    fs::write(dir.path().join("new.ogg"), b"song").unwrap();
    fs::write(dir.path().join("chart.ogg"), b"chart").unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session
        .edit_notes(
            1,
            "Other chart",
            [(
                Position {
                    row: 48,
                    lane: 2,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let before = file.session.serialize().unwrap();
    let grid = file.session.grid(1).unwrap().clone();
    let mut draft = Draft::capture(&file.session).unwrap();
    let values = [
        "Été 日本語 : ; \\ / #",
        "Subtitle",
        "Composer",
        "New author",
        "Dance",
        "Arcade",
        "Summer",
        "Mix",
        "Latin",
        "new.ogg",
    ];
    for (field, value) in Field::ALL.into_iter().zip(values) {
        draft.set_text(field, value).unwrap();
    }
    draft.set_sync_offset(0.125).unwrap();
    assert!(draft.apply(&mut file.session).unwrap());
    assert_eq!(file.session.undo_label(), Some("Change song properties"));
    for (field, value) in Field::ALL.into_iter().zip(values) {
        assert_eq!(Text::read(file.session.document()).get(field), value);
    }
    assert_eq!(
        chart_text::Text::read(file.session.document(), 0)
            .unwrap()
            .get(ChartField::Credit),
        "Old author"
    );
    assert_eq!(
        chart_text::Text::read(file.session.document(), 1)
            .unwrap()
            .get(ChartField::Credit),
        "Explicit"
    );
    assert_eq!(file.session.grid(1).unwrap(), &grid);
    assert_eq!(
        Preview::capture(&file.session, 0, &path).unwrap().music(),
        dir.path().join("new.ogg")
    );
    assert_eq!(
        Preview::capture(&file.session, 1, &path).unwrap().music(),
        dir.path().join("chart.ogg")
    );
    let json: Value =
        serde_json::from_str(&file.session.document().text(None, "GASongData").unwrap()).unwrap();
    assert_eq!(json["Future"]["keep"], true);
    assert_eq!(json["DefaultChartIndex"], 1);
    let after = file.session.serialize().unwrap();
    assert!(
        after
            .windows(b"#VENDOR:CP\xe9;".len())
            .any(|p| p == b"#VENDOR:CP\xe9;")
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), before);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), after);
    file.save().unwrap();
    let reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        Text::read(reopened.session.document()).get(Field::Title),
        values[0]
    );
    assert_eq!(
        chart_text::Text::read(reopened.session.document(), 0)
            .unwrap()
            .get(ChartField::Credit),
        "Old author"
    );
}
#[test]
fn text_only_edits_work_in_sm_ssc_and_empty_files_without_creating_editor_metadata() {
    for (format, notes) in [
        (Format::Sm, "#NOTES:dance-single::Hard:9::1000;"),
        (
            Format::Ssc,
            "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        ),
        (Format::Ssc, ""),
    ] {
        let source =
            format!("#VERSION:0.83;#TITLE:Old;#ARTIST:A;#CREDIT:Old author;#BPMS:0=120;{notes}");
        let mut s = Session::new(Document::parse(source.as_bytes(), format).unwrap());
        let mut d = Draft::capture(&s).unwrap();
        d.set_text(Field::Title, "New").unwrap();
        d.set_text(Field::Credit, "New author").unwrap();
        d.apply(&mut s).unwrap();
        let after = s.serialize().unwrap();
        assert!(
            !String::from_utf8(after.clone())
                .unwrap()
                .contains("GASongData")
        );
        let doc = Document::parse(after.clone(), format).unwrap();
        assert_eq!(Text::read(&doc).get(Field::Title), "New");
        if format == Format::Sm {
            assert_eq!(
                chart_text::Text::read(&doc, 0)
                    .unwrap()
                    .get(ChartField::Credit),
                "New author"
            );
        } else if !notes.is_empty() {
            assert_eq!(
                chart_text::Text::read(&doc, 0)
                    .unwrap()
                    .get(ChartField::Credit),
                "Old author"
            );
        }
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), source.as_bytes());
        s.redo().unwrap();
        assert_eq!(s.serialize().unwrap(), after);
    }
}
#[test]
fn invalid_stale_foreign_and_noop_drafts_preserve_source_and_redo_and_restore_legacy_text() {
    let bytes=b"#VERSION:0.83;#TITLE:Line1\nLine2;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut s = Session::new(Document::parse(&bytes[..], Format::Ssc).unwrap());
    let mut d = Draft::capture(&s).unwrap();
    assert!(d.set_text(Field::Title, "Bad\0value").is_err());
    d.set_text(Field::Title, "Good").unwrap();
    d.set_text(Field::Title, "Line1\nLine2").unwrap();
    assert!(!d.has_changes());
    assert!(!d.apply(&mut s).unwrap());
    d.set_text(Field::Title, "New").unwrap();
    let mut other = Session::new(Document::parse(&bytes[..], Format::Ssc).unwrap());
    assert!(d.apply(&mut other).is_err());
    d.apply(&mut s).unwrap();
    assert!(d.apply(&mut s).is_err());
    s.undo().unwrap();
    assert!(!Draft::capture(&s).unwrap().apply(&mut s).unwrap());
    assert!(s.redo_label().is_some());
    assert_eq!(s.serialize().unwrap(), bytes);
}
