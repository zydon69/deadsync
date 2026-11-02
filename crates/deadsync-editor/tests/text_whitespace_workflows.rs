use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    song_assets::{Assets, ImageType},
    song_text::{Field, Text},
};
use serde_json::Value;
#[test]
fn literal_song_strings_match_ten_real_fumen_reader_cases() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/key_commands/text-whitespace-source.json"
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
        for (tag, key) in [
            ("TITLE", "title"),
            ("BANNER", "banner"),
            ("CREDIT", "credit"),
            ("JACKET", "jacket"),
        ] {
            assert_eq!(
                document.text(None, tag).as_deref(),
                case[key].as_str(),
                "{case} {tag}"
            );
        }
        assert_eq!(
            document.serialize(),
            input,
            "read-only decoding must remain lossless"
        );
    }
}
#[test]
fn all_song_text_fields_keep_authored_spaces_through_reload_undo_and_redo() {
    for format in [Format::Sm, Format::Ssc] {
        let notes = if format == Format::Sm {
            "#NOTES:dance-single::Easy:1::0000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
        };
        let input = format!("#TITLE:before;#UNKNOWN:  opaque  ;{notes}").into_bytes();
        for field in Field::ALL {
            let mut session = Session::new(Document::parse(input.clone(), format).unwrap());
            // Exercise the authoring boundary: raw SM PREVIEW is deliberately
            // opaque, while Song Properties records an explicit preview owner.
            let mut draft = deadsync_editor::song_properties::Draft::capture(&session).unwrap();
            draft.set_text(field, "  a/b;c  ").unwrap();
            assert!(draft.apply(&mut session).unwrap());
            assert_eq!(
                Text::read(session.document()).get(field),
                "  a/b;c  ",
                "{format:?} {field:?}"
            );
            let saved = session.document().serialize();
            assert_eq!(
                Text::read(&Document::parse(saved.clone(), format).unwrap()).get(field),
                "  a/b;c  "
            );
            assert!(session.undo().unwrap());
            assert_eq!(session.document().serialize(), input);
            assert!(session.redo().unwrap());
            assert_eq!(session.document().serialize(), saved);
        }
    }
}
#[test]
fn unset_assets_do_not_replace_whitespace_and_normalized_fields_stay_normalized() {
    for format in [Format::Sm, Format::Ssc] {
        let notes = if format == Format::Sm {
            "#NOTES: dance-single : description : Easy : 1 ::0000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE: dance-single ;#NOTES:0000;"
        };
        let input = format!(
            "#TITLE:source;#JACKET: ;#OFFSET: 0.25 ;#BPMS: 0=120 ;#UNKNOWN:  opaque  ;{notes}"
        )
        .into_bytes();
        let mut session = Session::new(Document::parse(input.clone(), format).unwrap());
        let assets = Assets {
            images: vec![(ImageType::Jacket, "replacement.png".into())],
            lyrics: None,
        };
        assert_eq!(assets.apply(&mut session, true).unwrap(), 0);
        assert_eq!(
            session.document().text(None, "JACKET").as_deref(),
            Some(" ")
        );
        assert_eq!(
            session.document().text(None, "OFFSET").as_deref(),
            Some("0.25")
        );
        assert_eq!(
            session.document().text(None, "BPMS").as_deref(),
            Some("0=120")
        );
        assert_eq!(session.document().serialize(), input);
        assert!(session.undo_label().is_none());
        assert_eq!(assets.apply(&mut session, false).unwrap(), 1);
        assert_eq!(
            session.document().text(None, "JACKET").as_deref(),
            Some("replacement.png")
        );
        assert!(session.undo().unwrap());
        assert_eq!(session.document().serialize(), input);
    }
}
