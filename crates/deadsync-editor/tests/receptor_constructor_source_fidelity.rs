use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Receptors;".as_slice(), Format::Sm).unwrap())
}
#[test]
fn original_constructor_clamps_before_adapted_projection_and_rejects_true_int32_overflow() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/receptor-constructor-source.json")).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let mut s = session();
        let file = File::memory().unwrap();
        let raw = case["raw"].as_str().unwrap();
        let draft = Draft::prepare(&mut s, None, &file, raw.as_bytes(), Platform::Mac);
        if !case["accepted"].as_bool().unwrap() {
            assert!(draft.is_err(), "{}", case["name"]);
            continue;
        }
        let draft = draft.unwrap();
        let p = draft.target().receptors();
        let expected = &case["value"]["PreferencesReceptors"];
        assert_eq!(
            p.placement.x,
            expected["PositionX"].as_i64().unwrap() as i32
        );
        assert_eq!(
            p.placement.y,
            expected["PositionY"].as_i64().unwrap() as i32
        );
        if case["name"].as_str().unwrap().contains("--") {
            assert!(
                draft
                    .report()
                    .items
                    .iter()
                    .any(|r| r.disposition == Disposition::Normalized
                        && r.path.starts_with("PreferencesReceptors.Position")
                        && r.effective == Some(serde_json::json!(0)))
            );
        }
        s.import_source_preferences(draft, &file, None, true)
            .unwrap();
        let mut file = s
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        assert_eq!(
            file.source_archive().unwrap()["RawUtf8"].as_str().unwrap(),
            raw
        );
        s.capture_source_preferences_context(&file);
        s.undo().unwrap();
        file = s
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        assert_eq!(file.receptors(), File::memory().unwrap().receptors());
        s.capture_source_preferences_context(&file);
        s.redo().unwrap();
        file = s
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        assert_eq!(file.receptors(), p);
    }
}
#[test]
fn aliased_hold_flag_report_has_actual_source_raw_value() {
    let mut s = session();
    let file = File::memory().unwrap();
    let draft = Draft::prepare(
        &mut s,
        None,
        &file,
        br#"{"PreferencesMultiplayer":{"ColorHoldsAndRolls":false}}"#,
        Platform::Mac,
    )
    .unwrap();
    let row = draft
        .report()
        .items
        .iter()
        .find(|r| r.path == "PreferencesMultiplayer.ColorHoldsAndRolls")
        .unwrap();
    assert_eq!(row.raw, Some(serde_json::json!(false)));
    assert_eq!(row.effective, Some(serde_json::json!(false)));
    assert_eq!(row.disposition, Disposition::Applied);
    assert!(
        !draft
            .target()
            .note_colors_arc()
            .colors
            .color_multiplayer_holds
    );
}
#[test]
fn version_one_native_receptor_raw_coordinates_are_not_source_constructor_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Receptors":{"PositionX":-12,"PositionY":-20},"Vendor":7}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    assert_eq!(file.receptors().placement.x, -12);
    assert_eq!(file.receptors().placement.y, -20);
}
