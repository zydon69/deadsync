use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    song_properties::Draft,
    song_text::{Field, Text, resident},
    storage::FileSession,
};
use std::fs;
const RAW: &str = "#TITLE:Preview;#MUSIC:main.ogg;#PREVIEW:legacy\\;preview.ogg;#SAMPLESTART:0.25;#SAMPLELENGTH:1.5;#BPMS:0=120;#VENDOR:raw\\:keep;#NOTES:dance-single:Peer:Hard:9::1000;";
fn session() -> Session {
    Session::new(Document::parse(RAW.as_bytes(), Format::Sm).unwrap())
}
fn edit(session: &mut Session, path: &str) {
    let mut draft = Draft::capture(session).unwrap();
    draft.set_text(Field::PreviewMusic, path).unwrap();
    assert!(draft.apply(session).unwrap());
}
#[test]
fn raw_sm_preview_is_opaque_but_ssc_preview_is_effective() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.ogg"), b"media path fixture").unwrap();
    let mut session = session();
    assert_eq!(
        Text::try_read(session.document())
            .unwrap()
            .get(Field::PreviewMusic),
        ""
    );
    assert_eq!(
        session.document().text(None, "PREVIEW").as_deref(),
        Some("legacy;preview.ogg")
    );
    let draft = Draft::capture(&session).unwrap();
    let sample = draft
        .sample_audio_from_directory(&session, 0, dir.path())
        .unwrap();
    assert_eq!(sample.path, dir.path().join("main.ogg"));
    assert_eq!(sample.start, 0.25);
    assert_eq!(sample.length, Some(1.5));
    assert!(!draft.apply(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), RAW.as_bytes());
    let ssc = Document::parse(
        b"#PREVIEW:dedicated.ogg;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),
        Format::Ssc,
    )
    .unwrap();
    assert_eq!(
        Text::try_read(&ssc).unwrap().get(Field::PreviewMusic),
        "dedicated.ogg"
    );
}
#[test]
fn authored_sm_media_and_clear_survive_reparse_exact_undo_redo_and_sample_resolution() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("audio")).unwrap();
    fs::write(dir.path().join("main.ogg"), b"media path fixture").unwrap();
    fs::write(
        dir.path().join("audio/Été;mix#x.ogg"),
        b"dedicated path fixture",
    )
    .unwrap();
    let mut session = session();
    let before = session.serialize().unwrap();
    let peer = session.grid(0).unwrap().clone();
    edit(&mut session, "audio/Été;mix#x.ogg");
    let authored = session.serialize().unwrap();
    assert_eq!(
        resident::read(session.document()).unwrap().as_deref(),
        Some("audio/Été;mix#x.ogg")
    );
    assert_eq!(
        session.document().text(None, "PREVIEW").as_deref(),
        Some("legacy;preview.ogg")
    );
    let draft = Draft::capture(&session).unwrap();
    let sample = draft
        .sample_audio_from_directory(&session, 0, dir.path())
        .unwrap();
    assert_eq!(sample.path, dir.path().join("audio/Été;mix#x.ogg"));
    assert_eq!(sample.start, 0.0);
    assert_eq!(sample.length, None);
    let reparsed = session.document().reparse(authored.clone()).unwrap();
    assert_eq!(
        resident::effective(&reparsed).unwrap(),
        "audio/Été;mix#x.ogg"
    );
    edit(&mut session, "");
    let cleared = session.serialize().unwrap();
    assert_eq!(
        resident::read(session.document()).unwrap(),
        Some(String::new())
    );
    assert_eq!(
        Draft::capture(&session)
            .unwrap()
            .sample_audio_from_directory(&session, 0, dir.path())
            .unwrap()
            .length,
        Some(1.5)
    );
    assert!(session.undo().unwrap());
    assert_eq!(session.serialize().unwrap(), authored);
    assert!(session.undo().unwrap());
    assert_eq!(session.serialize().unwrap(), before);
    assert!(session.redo().unwrap());
    assert_eq!(session.serialize().unwrap(), authored);
    assert!(session.redo().unwrap());
    assert_eq!(session.serialize().unwrap(), cleared);
    assert_eq!(session.grid(0).unwrap(), &peer);
}
#[test]
fn clearing_ignored_raw_preview_records_empty_owner_and_repeated_clear_is_noop() {
    let mut session = session();
    edit(&mut session, "");
    let cleared = session.serialize().unwrap();
    assert_eq!(
        resident::read(session.document()).unwrap(),
        Some(String::new())
    );
    assert_eq!(
        session.document().text(None, "PREVIEW").as_deref(),
        Some("legacy;preview.ogg")
    );
    let mut again = Draft::capture(&session).unwrap();
    again.set_text(Field::PreviewMusic, "").unwrap();
    assert!(!again.apply(&mut session).unwrap());
    assert_eq!(session.serialize().unwrap(), cleared);
    assert!(session.undo().unwrap());
    assert_eq!(session.serialize().unwrap(), RAW.as_bytes());
}
#[test]
fn native_save_reopen_and_recovery_restore_shadow_without_reactivating_raw_media() {
    for value in ["audio/Été;mix.ogg", ""] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.sm");
        fs::write(&path, RAW).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        edit(&mut file.session, value);
        let dirty = file.session.serialize().unwrap();
        file.save_recovery().unwrap();
        assert_eq!(fs::read(&path).unwrap(), RAW.as_bytes());
        drop(file);
        let mut recovered = FileSession::open(&path).unwrap();
        recovered.recover().unwrap();
        assert_eq!(recovered.session.serialize().unwrap(), dirty);
        assert_eq!(
            Draft::capture(&recovered.session)
                .unwrap()
                .text(Field::PreviewMusic),
            value
        );
        recovered.save().unwrap();
        assert_eq!(recovered.session.serialize().unwrap(), dirty);
        drop(recovered);
        let reopened = FileSession::open(&path).unwrap();
        assert_eq!(
            Draft::capture(&reopened.session)
                .unwrap()
                .text(Field::PreviewMusic),
            value
        );
        assert_eq!(
            reopened.session.document().text(None, "PREVIEW").as_deref(),
            Some("legacy;preview.ogg")
        );
    }
}
#[test]
fn malformed_unknown_duplicate_or_control_owner_blocks_drafts_without_fallback_or_mutation() {
    for raw in [
        r#"{"version":2,"path":"x"}"#,
        r#"{"version":1}"#,
        r#"{"version":1,"path":null}"#,
        r#"{"version":1,"path":"x","future":0}"#,
        r#"{"version":1,"version":1,"path":"x"}"#,
        r#"{"version":1,"path":"a\nb"}"#,
    ] {
        let payload = deadsync_editor::document::escape_text(raw);
        let bytes = [RAW.as_bytes(), b"#DSSongPreview:", &payload, b";"].concat();
        let session = Session::new(Document::parse(bytes.clone(), Format::Sm).unwrap());
        let baseline = (
            session.revision(),
            session.content_revision(),
            session.chart_ids().to_vec(),
        );
        assert!(Text::try_read(session.document()).is_err());
        assert!(Draft::capture(&session).is_err());
        assert_eq!(Text::read(session.document()).get(Field::PreviewMusic), "");
        assert_eq!(session.serialize().unwrap(), bytes);
        assert_eq!(
            (
                session.revision(),
                session.content_revision(),
                session.chart_ids().to_vec()
            ),
            baseline
        );
        assert!(session.undo_label().is_none());
        assert!(session.redo_label().is_none());
    }
    let payload = resident::encode("a.ogg").unwrap();
    let bytes = [
        RAW.as_bytes(),
        b"#DSSongPreview:",
        &payload,
        b";#DSSongPreview:",
        &payload,
        b";",
    ]
    .concat();
    let session = Session::new(Document::parse(bytes.clone(), Format::Sm).unwrap());
    assert!(Draft::capture(&session).is_err());
    assert_eq!(session.serialize().unwrap(), bytes);
    assert_eq!(
        resident::decode(&resident::encode("Été/quote\";#\\.ogg").unwrap()).unwrap(),
        "Été/quote\";#\\.ogg"
    );
}
#[test]
fn output_preparation_does_not_resurrect_opaque_raw_preview() {
    let raw = session();
    let output = resident::materialize_for_output(raw.document(), Format::Ssc).unwrap();
    assert_eq!(output.text(None, "PREVIEW").as_deref(), Some(""));
    let mut authored = session();
    edit(&mut authored, "new.ogg");
    let before = authored.serialize().unwrap();
    let output = resident::materialize_for_output(authored.document(), Format::Ssc).unwrap();
    assert_eq!(output.text(None, "PREVIEW").as_deref(), Some("new.ogg"));
    assert_eq!(authored.serialize().unwrap(), before);
}
