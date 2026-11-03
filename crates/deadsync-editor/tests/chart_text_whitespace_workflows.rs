use deadsync_editor::{
    chart_properties::Draft,
    chart_text::{Field, Text},
    document::{Document, Format},
    history::Session,
};
use serde_json::Value;
#[test]
fn ssc_chart_strings_and_sm_header_description_match_original_readers() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/key_commands/chart-text-whitespace-source.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let format = if case["format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let input = case["input"].as_str().unwrap().as_bytes().to_vec();
        let document = Document::parse(input.clone(), format).unwrap();
        let text = Text::read(&document, 0).unwrap();
        for (field, key) in [
            (Field::Description, "description"),
            (Field::Credit, "credit"),
        ] {
            assert_eq!(text.get(field), case[key].as_str().unwrap(), "{case} {key}");
        }
        if format == Format::Ssc {
            for (field, key) in [
                (Field::Name, "name"),
                (Field::Style, "style"),
                (Field::Music, "music"),
            ] {
                assert_eq!(text.get(field), case[key].as_str().unwrap(), "{case} {key}");
            }
        }
        assert_eq!(
            document.charts()[0].description,
            case["description"].as_str().unwrap()
        );
        assert_eq!(document.charts()[0].steps_type, "dance-single");
        assert_eq!(document.serialize(), input);
    }
}
#[test]
fn ssc_authoring_whitespace_survives_inspector_reload_and_undo_without_credit_fallback() {
    let original=b"#TITLE:source;#CREDIT:global;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:before;#CREDIT: ;#NOTES:0000;".to_vec();
    for field in Field::ALL {
        let mut session = Session::new(Document::parse(original.clone(), Format::Ssc).unwrap());
        assert_eq!(
            Text::read(session.document(), 0)
                .unwrap()
                .get(Field::Credit),
            " "
        );
        let mut draft = Draft::capture(&session, 0).unwrap();
        draft.set_text(field, "  preserved  ").unwrap();
        assert!(draft.apply(&mut session, 0).unwrap());
        let saved = session.document().serialize();
        let reloaded = Document::parse(saved.clone(), Format::Ssc).unwrap();
        assert_eq!(
            Text::read(&reloaded, 0).unwrap().get(field),
            "  preserved  "
        );
        assert!(session.undo().unwrap());
        assert_eq!(session.document().serialize(), original);
        assert!(session.redo().unwrap());
        assert_eq!(session.document().serialize(), saved);
    }
}
