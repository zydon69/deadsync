use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    minimap_palette::ColorMethod,
    note_color_palette::Palette,
    note_color_preferences::Preferences,
};
use serde_json::{Map, Value};
use std::sync::Arc;
#[test]
fn original_converter_and_unchanged_row_method_match_all_eighteen_cases() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/step-color-method.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let parsed = Preferences::read(&Map::new(), case["Input"].as_object().unwrap());
        if case["Error"] == true {
            assert!(parsed.is_err(), "{case}");
            continue;
        }
        let parsed = parsed.unwrap();
        assert_eq!(
            parsed.step_color_method.raw(),
            case["Raw"].as_i64().unwrap() as i32,
            "{case}"
        );
        let mut colors = Map::new();
        let mut options = Map::new();
        parsed.write(&mut colors, &mut options).unwrap();
        assert_eq!(
            options["StepColorMethodValue"], case["Serialized"],
            "{case}"
        );
        assert_eq!(
            u64::from(parsed.step_color_method.color_row(300, 32, 8).unwrap()),
            case["Row"].as_u64().unwrap(),
            "{case}"
        );
    }
}
#[test]
fn unknown_raw_value_uses_stepmania_without_signatures_and_is_preserved_by_save_undo_redo() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"StepColorMethodValue":"999","Future":17}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let before = file.note_colors_arc();
    assert_eq!(before.step_color_method, ColorMethod::Unknown(999));
    assert_eq!(
        Palette::capture(Arc::clone(&before))
            .unwrap()
            .color_row(300, None)
            .unwrap(),
        12
    );
    let mut after = before.as_ref().clone();
    after.step_color_method = ColorMethod::Note;
    let after = Arc::new(after);
    let mut session = Session::new(
        Document::parse(b"#TITLE:Colors;#BPMS:0=120;".as_slice(), Format::Sm).unwrap(),
    );
    let bytes = session.serialize().unwrap();
    assert!(
        session
            .set_note_color_preferences(Arc::clone(&before), Arc::clone(&after), Some(&file))
            .unwrap()
    );
    let update = session.take_note_color_preferences_update().unwrap();
    assert_eq!(
        update.file.unwrap().note_colors_arc().step_color_method,
        ColorMethod::Note
    );
    assert!(session.undo().unwrap());
    let update = session.take_note_color_preferences_update().unwrap();
    assert_eq!(
        update.file.unwrap().note_colors_arc().step_color_method,
        ColorMethod::Unknown(999)
    );
    let disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk["Options"]["StepColorMethodValue"], "999");
    assert_eq!(disk["Options"]["Future"], 17);
    assert!(session.redo().unwrap());
    assert_eq!(
        session
            .take_note_color_preferences_update()
            .unwrap()
            .file
            .unwrap()
            .note_colors_arc()
            .step_color_method,
        ColorMethod::Note
    );
    assert_eq!(session.serialize().unwrap(), bytes);
    assert!(!session.is_dirty());
}
