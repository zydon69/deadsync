use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    key_binding_preferences::source_profile::Platform,
    dark_preferences::{Order, Preferences, Size},
    document::{Document, Format},
    history::Session,
};
use serde_json::{Value, json};
fn new_session() -> Session {
    Session::new(Document::parse(b"#TITLE:Dark;".as_slice(), Format::Sm).unwrap())
}
#[test]
fn real_dark_models_raw_enum_and_source_color_are_consumed_not_archived() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/dark-source.json")).unwrap();
    let current = File::memory().unwrap();
    let mut session = new_session();
    for case in cases {
        assert_eq!(case["accepted"], json!(true));
        let draft = Draft::prepare(
            &mut session,
            None,
            &current,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        let p = draft.target().dark();
        let expected = &case["value"]["PreferencesDark"];
        assert_eq!(p.size().name(), expected["Size"].as_str().unwrap());
        assert_eq!(
            p.order().name(),
            expected["DarkBgDrawOrder"].as_str().unwrap()
        );
        for (index, key) in ["X", "Y", "Z", "W"].into_iter().enumerate() {
            assert_eq!(
                p.color()[index],
                expected["Color"][key].as_f64().unwrap() as f32
            );
        }
        let packed = case["color"].as_u64().unwrap() as u32;
        assert_eq!(
            p.rendered_color(),
            std::array::from_fn(|i| ((packed >> (i * 8)) & 255) as f32 / 255.)
        );
        if case["name"].as_str().unwrap().ends_with("99") {
            let key = if case["name"].as_str().unwrap().starts_with("Size") {
                "Size"
            } else {
                "DarkBgDrawOrder"
            };
            assert!(
                !draft
                    .report()
                    .items
                    .iter()
                    .any(|row| row.path == format!("PreferencesDark.{key}")
                        && row.disposition == Disposition::Retained)
            );
            assert!(
                draft
                    .report()
                    .items
                    .iter()
                    .any(|row| row.path == format!("PreferencesDark.{key}")
                        && row.disposition == Disposition::Applied)
            );
        }
    }
}
#[test]
fn dark_memory_restore_undo_redo_and_live_component_cas_use_canonical_owner() {
    let mut file = File::memory().unwrap();
    let mut before = file.dark();
    before.set_show(true);
    before.set_size(Size::Unknown(99));
    before.set_order(Order::Unknown(-99));
    before.set_color([1e20, -1e20, 0.251, 0.]).unwrap();
    file.set_dark(before).unwrap();
    let mut session = new_session();
    let song = session.serialize().unwrap();
    session
        .set_dark_preferences(before, Preferences::default(), Some(&file))
        .unwrap();
    file = session
        .take_dark_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert!(!file.has_changes());
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    file = session
        .take_dark_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.dark(), before);
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    file = session
        .take_dark_preferences_update()
        .unwrap()
        .file
        .unwrap();
    assert_eq!(file.dark(), Preferences::default());
    assert!(file.path().as_os_str().is_empty());
    assert!(!session.is_dirty());
    assert_eq!(session.serialize().unwrap(), song);
    for conflict in [false, true] {
        let mut file = File::memory().unwrap();
        let before = file.dark();
        let mut after = before;
        after.set_color([0.2, 0., 0., 0.8]).unwrap();
        let mut s = new_session();
        s.set_dark_preferences(before, after, Some(&file)).unwrap();
        file = s.take_dark_preferences_update().unwrap().file.unwrap();
        let mut live = file.dark();
        let mut color = live.color();
        color[if conflict { 0 } else { 1 }] = 0.9;
        live.set_color(color).unwrap();
        file.set_dark(live).unwrap();
        s.capture_source_preferences_context(&file);
        if conflict {
            assert!(s.undo().is_err());
            assert!(s.take_dark_preferences_update().is_none());
        } else {
            assert!(s.undo().unwrap());
            let update = s.take_dark_preferences_update().unwrap();
            assert_eq!(update.preferences.color(), [0., 0.9, 0., 0.8]);
        }
    }
}

#[test]
fn raw_dark_enums_and_colors_restore_undo_redo_and_reopen_with_vendor_leaves() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/dark-source.json")).unwrap();
    for case in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let mut fields = case["value"]["PreferencesDark"].clone();
        fields["Vendor"] = json!({"preserved":7});
        fields["Color"]["Future"] = json!(9);
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({"Version":1,"Dark":fields})).unwrap(),
        )
        .unwrap();
        let mut file = File::open(&path).unwrap();
        let before = file.dark();
        let mut s = new_session();
        let song = s.serialize().unwrap();
        s.set_dark_preferences(before, Preferences::default(), Some(&file))
            .unwrap();
        file = s.take_dark_preferences_update().unwrap().file.unwrap();
        assert_eq!(file.dark(), Preferences::default());
        s.capture_source_preferences_context(&file);
        s.undo().unwrap();
        file = s.take_dark_preferences_update().unwrap().file.unwrap();
        assert_eq!(file.dark(), before);
        assert_eq!(File::open(&path).unwrap().dark(), before);
        s.capture_source_preferences_context(&file);
        s.redo().unwrap();
        file = s.take_dark_preferences_update().unwrap().file.unwrap();
        assert_eq!(file.dark(), Preferences::default());
        let reopened = File::open(&path).unwrap();
        assert_eq!(reopened.dark(), Preferences::default());
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["Dark"]["Vendor"], json!({"preserved":7}));
        assert_eq!(raw["Dark"]["Color"]["Future"], 9);
        assert!(!s.is_dirty());
        assert_eq!(s.serialize().unwrap(), song);
    }
}
