use deadsync_editor::{
    chart_identity::Difficulty,
    chart_properties::Draft,
    chart_text::{Field, Text},
    document::{Document, Format},
    history::Session,
    storage::FileSession,
};
use serde_json::Value;
use std::{fs, path::PathBuf};
#[test]
fn actual_fumen_chart_text_and_song_credit_inheritance_match_without_source_rewrites() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy();
        if !name.starts_with("chart-text-") || !name.ends_with(".request.json") {
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
        let bytes = request["Content"].as_str().unwrap().as_bytes();
        let mut s = Session::new(Document::parse(bytes, format).unwrap());
        let text = Text::read(s.document(), 0).unwrap();
        for (field, key) in [
            (Field::Name, "Name"),
            (Field::Description, "Description"),
            (Field::Credit, "Credit"),
            (Field::Style, "Style"),
        ] {
            assert_eq!(
                text.get(field),
                expected[0][key].as_str().unwrap(),
                "{name}: {key}"
            );
        }
        assert!(!Draft::capture(&s, 0).unwrap().apply(&mut s, 0).unwrap());
        assert_eq!(s.serialize().unwrap(), bytes);
    }
    assert_eq!(count, 4);
}
#[test]
fn ssc_all_text_fields_identity_offset_apply_atomically_with_unicode_escaping_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let bytes=b"#VERSION:0.83;#TITLE:CP\xe9;#BPMS:0=120;#CREDIT:Song;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#CREDIT:Other;#NOTES:0001;";
    fs::write(&path, bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let mut draft = Draft::capture(&file.session, 0).unwrap();
    let values = ["Été 日本語 🎵", "A:B;C\\D // E=F,#G", "Zydon", "Pad"];
    for (field, value) in Field::ALL.into_iter().zip(values) {
        draft.set_text(field, value).unwrap();
    }
    draft.set_rating(12).unwrap();
    draft.set_difficulty(Difficulty::Challenge);
    draft.set_music_offset(0.125).unwrap();
    draft.set_use_chart_offset(true);
    assert!(draft.apply(&mut file.session, 0).unwrap());
    assert_eq!(file.session.undo_label(), Some("Change chart properties"));
    let after = file.session.serialize().unwrap();
    assert!(
        after
            .windows(b"#TITLE:CP\xe9;".len())
            .any(|v| v == b"#TITLE:CP\xe9;")
    );
    assert_eq!(
        Text::read(file.session.document(), 1)
            .unwrap()
            .get(Field::Credit),
        "Other"
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), bytes);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), after);
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    let text = Text::read(reopened.session.document(), 0).unwrap();
    for (field, value) in Field::ALL.into_iter().zip(values) {
        assert_eq!(text.get(field), value);
    }
    assert_eq!(
        reopened.session.grid(0).unwrap(),
        file.session.grid(0).unwrap()
    );
}
#[test]
fn sm_description_edit_preserves_positional_headers_and_rejects_nonpersistent_fields() {
    let bytes=b"#BPMS:0=120;#CREDIT:Song;#VENDOR:keep;#NOTES2:dance-single:Old\\:desc:Hard:9:0,1,2,3,4:1000;#NOTES:dance-single:Other:Easy:3::0001;";
    let mut s = Session::new(Document::parse(&bytes[..], Format::Sm).unwrap());
    let mut draft = Draft::capture(&s, 0).unwrap();
    assert_eq!(draft.text(Field::Description), "Old:desc");
    for field in [Field::Name, Field::Credit, Field::Style] {
        assert!(!draft.supports_text(field));
        assert!(draft.set_text(field, "lost").is_err());
    }
    assert!(!draft.has_changes());
    draft
        .set_text(Field::Description, "Été : ; // \\ 日本語")
        .unwrap();
    draft.set_rating(13).unwrap();
    assert!(draft.apply(&mut s, 0).unwrap());
    let after = s.serialize().unwrap();
    let read = Text::read(&Document::parse(after.clone(), Format::Sm).unwrap(), 0).unwrap();
    assert_eq!(read.get(Field::Description), "Été : ; // \\ 日本語");
    assert!(
        String::from_utf8(after.clone())
            .unwrap()
            .contains(":Hard:13:0,1,2,3,4:1000;#NOTES:dance-single:Other:Easy:3::0001;")
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), bytes);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn text_draft_rejects_controls_stale_foreign_and_wrong_chart_without_changing_redo() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0001;";
    let mut s = Session::new(Document::parse(&bytes[..], Format::Ssc).unwrap());
    let mut d = Draft::capture(&s, 0).unwrap();
    for value in ["bad\ntext", "bad\0text", "bad\ttext"] {
        assert!(d.set_text(Field::Name, value).is_err());
    }
    d.set_text(Field::Name, "Name").unwrap();
    assert!(d.apply(&mut s, 1).is_err());
    let mut other = Session::new(Document::parse(&bytes[..], Format::Ssc).unwrap());
    assert!(d.apply(&mut other, 0).is_err());
    d.apply(&mut s, 0).unwrap();
    let changed = s.serialize().unwrap();
    assert!(d.apply(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), changed);
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let clean = Draft::capture(&s, 0).unwrap();
    assert!(!clean.apply(&mut s, 0).unwrap());
    assert!(s.redo_label().is_some());
    assert_eq!(s.serialize().unwrap(), before);
}
