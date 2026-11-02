use deadsync_editor::{
    song_properties::{Draft, Selectable},
    song_text::Field,
    storage::FileSession,
};
#[test]
fn source_enum_parse_fidelity_and_all_five_song_selection_states_round_trip() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/source_song_selectable.expected.json"
    ))
    .unwrap();
    for case in reference.as_array().unwrap() {
        assert_eq!(
            Selectable::read(case["raw"].as_str()).source_name(),
            case["expected"].as_str().unwrap()
        );
    }
    for extension in ["sm", "ssc"] {
        for selected in Selectable::ALL {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(format!("song.{extension}"));
            std::fs::write(
                &path,
                "#TITLE:Keep;#VENDOR:raw\\;unchanged;#SELECTABLE:future;#BPMS:0=120;",
            )
            .unwrap();
            let mut file = FileSession::open(&path).unwrap();
            let before = file.session.serialize().unwrap();
            let mut draft = Draft::capture(&file.session).unwrap();
            assert_eq!(draft.selectable(), Selectable::default());
            assert!(!draft.has_changes());
            draft.set_text(Field::Artist, "New author").unwrap();
            assert!(draft.apply(&mut file.session).unwrap());
            assert_eq!(
                file.session.document().text(None, "SELECTABLE").as_deref(),
                Some("future")
            );
            assert!(file.session.undo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), before);
            let mut draft = Draft::capture(&file.session).unwrap();
            draft.set_selectable(selected);
            assert!(draft.has_changes());
            assert!(draft.apply(&mut file.session).unwrap());
            assert_eq!(
                file.session.document().text(None, "SELECTABLE").unwrap(),
                selected.source_name()
            );
            assert!(file.session.undo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), before);
            assert!(file.session.redo().unwrap());
            file.save().unwrap();
            let reopened = FileSession::open(&path).unwrap();
            assert_eq!(
                Draft::capture(&reopened.session).unwrap().selectable(),
                selected
            );
            assert_eq!(
                reopened.session.document().text(None, "VENDOR").as_deref(),
                Some("raw;unchanged")
            );
        }
    }
}
