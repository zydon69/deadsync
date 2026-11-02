use deadsync_editor::{audio_preferences::File, dark_preferences::{Order, Preferences, Size}, document::{Document, Format}, history::Session};
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Keep;#NOTES:dance-single::Hard:9::1000;".as_slice(), Format::Sm).unwrap())
}
fn assert_projection(actual: Preferences, expected: &serde_json::Value) {
    assert_eq!(actual.show(), expected["ShowDarkBg"]);
    assert_eq!(actual.size().name(), expected["Size"]);
    assert_eq!(actual.order().name(), expected["DarkBgDrawOrder"]);
    for (value, key) in actual.color().into_iter().zip(["X", "Y", "Z", "W"]) {
        assert!((f64::from(value) - expected["Color"][key].as_f64().unwrap()).abs() < 1e-6);
    }
}
#[test]
fn actual_source_preferences_and_restore_history_match_all_five_cases() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!("fixtures/dark.expected.json")).unwrap();
    for case in oracle.as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        let mut input = case["Input"].clone();
        input["Vendor"] = 17.into();
        let bytes = serde_json::to_vec(&serde_json::json!({"Version":1,"Dark":input,"Other":{"keep":true}})).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let file = File::open(&path).unwrap();
        assert_projection(file.dark(), &case["Normalized"]);
        assert!(!file.has_changes());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let mut changed = file.dark();
        changed.set_show(true); changed.set_size(Size::Charts); changed.set_order(Order::AfterWaveForm);
        changed.set_color([0.2, 0.4, 0.6, 0.3]).unwrap();
        assert_projection(changed, &case["Changed"]);
        let mut s = session();
        s.set_dark_preferences(changed, Preferences::default(), None).unwrap();
        assert_projection(s.take_dark_preferences_update().unwrap().preferences, &case["Restored"]);
        s.undo().unwrap();
        assert_projection(s.take_dark_preferences_update().unwrap().preferences, &case["Undone"]);
        s.redo().unwrap();
        assert_projection(s.take_dark_preferences_update().unwrap().preferences, &case["Redone"]);
    }
}
#[test]
fn dark_history_is_nondirty_and_noop_keeps_redo() {
    let mut s = session(); let bytes = s.serialize().unwrap();
    let before = Preferences::default(); let mut after = before; after.set_show(true);
    assert!(s.set_dark_preferences(before, after, None).unwrap());
    assert!(s.undo_changes_dark_preferences());
    assert_eq!(s.take_dark_preferences_update().unwrap().preferences, after);
    s.undo().unwrap();
    let revision = s.revision();
    assert!(!s.set_dark_preferences(before, before, None).unwrap());
    assert_eq!(s.revision(), revision);
    assert!(s.redo_changes_dark_preferences());
    s.redo().unwrap();
    assert_eq!(s.take_dark_preferences_update().unwrap().preferences, after);
    assert_eq!(s.serialize().unwrap(), bytes); assert!(!s.is_dirty());
}
#[test]
fn undo_merges_unowned_dark_fields_and_preserves_unknown_nested_values() {
    let root = tempfile::tempdir().unwrap(); let path = root.path().join("preferences.json");
    let file = File::open(&path).unwrap(); let before = file.dark(); let mut after = before; after.set_show(true);
    let mut s = session(); s.set_dark_preferences(before, after, Some(&file)).unwrap(); s.take_dark_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Dark"]["Size"] = "Charts".into(); json["Dark"]["Color"]["Vendor"] = 42.into(); json["OtherOwner"] = serde_json::json!({"keep":true});
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    s.undo().unwrap(); let update = s.take_dark_preferences_update().unwrap();
    assert!(!update.preferences.show()); assert_eq!(update.preferences.size(), Size::Charts);
    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["Dark"]["Color"]["Vendor"], 42); assert_eq!(written["OtherOwner"], json["OtherOwner"]);
    assert!(!s.is_dirty());
}
#[test]
fn owned_conflict_preserves_history_file_and_pending_update() {
    let root = tempfile::tempdir().unwrap(); let path = root.path().join("preferences.json");
    let file = File::open(&path).unwrap(); let before = file.dark(); let mut after = before; after.set_show(true);
    let mut s = session(); s.set_dark_preferences(before, after, Some(&file)).unwrap(); s.take_dark_preferences_update();
    let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap(); json["Dark"]["ShowDarkBg"] = false.into();
    let bytes = serde_json::to_vec(&json).unwrap(); std::fs::write(&path, &bytes).unwrap(); let revision = s.revision();
    assert!(matches!(s.undo(), Err(deadsync_editor::Error::ExternalChange)));
    assert_eq!(s.revision(), revision); assert!(s.undo_changes_dark_preferences()); assert!(s.take_dark_preferences_update().is_none()); assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
#[test]
fn nonfinite_colors_fail_atomically_while_imported_finite_values_remain_lossless() {
    let mut p = Preferences::default(); let original = p;
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(p.set_color([value,0.0,0.0,1.0]).is_err()); assert_eq!(p,original);
    }
    p.set_color([-2.0,3.0,0.5,0.8]).unwrap(); assert_eq!(p.color(),[-2.0,3.0,0.5,0.8]); assert_eq!(p.rendered_color(),[0.0,1.0,127.0/255.0,204.0/255.0]);
}
