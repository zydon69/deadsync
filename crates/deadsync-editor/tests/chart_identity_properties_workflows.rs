use deadsync_editor::{
    chart_identity::{Difficulty, Identity},
    chart_properties::Draft,
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn source(format: Format) -> Vec<u8> {
    let mut bytes = b"#TITLE:CP\xe9;#VERSION:0.83;#OFFSET:0.25;#BPMS:0=120;#VENDOR:root;".to_vec();
    for _ in 0..2 {
        let chart: &[u8] = if format == Format::Ssc {
            b"#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Desc\\:more;#DIFFICULTY:Hard;#METER:9;#X:keep;#NOTES:1000;"
        } else {
            b"#NOTES2:dance-single:Desc\\:more:Hard:9:0,0,0,0,0:1000;"
        };
        bytes.extend_from_slice(chart);
    }
    bytes
}
#[test]
fn actual_fumen_difficulty_rating_projection_matches_source_without_rewriting_legacy_values() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("chart-identity-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if request["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let mut s = Session::new(
            Document::parse(request["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
        );
        let before = s.serialize().unwrap();
        let identity = Identity::read(s.document(), 0).unwrap();
        assert_eq!(
            identity.difficulty().map(|v| v.as_str()),
            expected[0]["Difficulty"].as_str(),
            "{name}"
        );
        assert_eq!(
            i64::from(identity.rating()),
            expected[0]["Rating"].as_i64().unwrap(),
            "{name}"
        );
        assert!(!Draft::capture(&s, 0).unwrap().apply(&mut s, 0).unwrap());
        assert_eq!(s.serialize().unwrap(), before);
        assert!(!s.is_dirty());
    }
    assert_eq!(count, 7);
}
#[test]
fn identity_offset_and_metadata_apply_as_one_gesture_in_sm_ssc_and_survive_save_reopen() {
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        fs::write(&path, source(format)).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session
            .edit_notes(
                1,
                "Other chart",
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
        let before = file.session.serialize().unwrap();
        let original = file.session.grid(0).unwrap().clone();
        let mut draft = Draft::capture(&file.session, 0).unwrap();
        draft.set_difficulty(Difficulty::Challenge);
        draft.set_rating(14).unwrap();
        draft.set_music_offset(-0.125).unwrap();
        draft.set_use_chart_offset(true);
        assert!(draft.apply(&mut file.session, 0).unwrap());
        assert_eq!(file.session.undo_label(), Some("Change chart properties"));
        let id = Identity::read(file.session.document(), 0).unwrap();
        assert_eq!(id.difficulty(), Some(Difficulty::Challenge));
        assert_eq!(id.rating(), 14);
        assert_eq!(file.session.grid(0).unwrap(), &original);
        let other = Identity::read(file.session.document(), 1).unwrap();
        assert_eq!(other.rating(), 9);
        assert_eq!(other.difficulty(), Some(Difficulty::Hard));
        let after = file.session.serialize().unwrap();
        if format == Format::Sm {
            assert!(
                String::from_utf8_lossy(&after)
                    .contains("#NOTES2:dance-single:Desc\\:more:Challenge:14:0,0,0,0,0:")
            );
        }
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), before);
        file.session.redo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        let reopened = FileSession::open(&path).unwrap();
        let id = Identity::read(reopened.session.document(), 0).unwrap();
        assert_eq!(id.rating(), 14);
        assert_eq!(id.difficulty(), Some(Difficulty::Challenge));
        assert_eq!(
            Draft::capture(&reopened.session, 0)
                .unwrap()
                .effective_music_offset(),
            -0.125
        );
        assert!(!reopened.session.is_dirty());
    }
}
#[test]
fn identity_only_avoids_unrelated_custom_data_and_preserves_notes_unknown_tags_and_redo() {
    for format in [Format::Sm, Format::Ssc] {
        let mut s = Session::new(Document::parse(source(format), format).unwrap());
        let before = s.serialize().unwrap();
        let mut draft = Draft::capture(&s, 0).unwrap();
        draft.set_difficulty(Difficulty::Edit);
        draft.set_rating(i32::MAX).unwrap();
        assert!(draft.apply(&mut s, 0).unwrap());
        assert!(!String::from_utf8_lossy(&s.serialize().unwrap()).contains("GAChartData"));
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), before);
        let mut invalid = Draft::capture(&s, 0).unwrap();
        assert!(invalid.set_rating(0).is_err());
        assert!(invalid.set_rating(-1).is_err());
        assert!(!invalid.has_changes());
        assert!(!invalid.apply(&mut s, 0).unwrap());
        assert!(s.redo().unwrap());
        assert_eq!(Identity::read(s.document(), 0).unwrap().rating(), i32::MAX);
    }
    for value in Difficulty::ALL {
        assert_eq!(value.step(true).step(false), value);
    }
}
#[test]
fn stale_wrong_chart_foreign_and_legacy_signed_rating_keep_history_atomic() {
    let mut s = Session::new(Document::parse(source(Format::Sm), Format::Sm).unwrap());
    let mut draft = Draft::capture(&s, 0).unwrap();
    draft.set_rating(12).unwrap();
    let before = s.serialize().unwrap();
    assert!(draft.apply(&mut s, 1).is_err());
    let mut foreign = Session::new(Document::parse(source(Format::Sm), Format::Sm).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    s.set_text(None, "TITLE", "Changed").unwrap();
    let changed = s.serialize().unwrap();
    assert!(draft.apply(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), changed);
    assert_eq!(foreign.serialize().unwrap(), before);
    let raw = b"#BPMS:0=120;#NOTES:dance-single:Keep:Hard:-7:0,0,0,0,0:1000;";
    let mut s = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let mut draft = Draft::capture(&s, 0).unwrap();
    assert_eq!(draft.rating(), -7);
    draft.set_difficulty(Difficulty::Easy);
    draft.apply(&mut s, 0).unwrap();
    assert_eq!(Identity::read(s.document(), 0).unwrap().rating(), -7);
    assert!(String::from_utf8_lossy(&s.serialize().unwrap()).contains(":Easy:-7:0,0,0,0,0:"));
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), raw);
}
