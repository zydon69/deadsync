use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    event_clipboard::Selection,
    history::Session,
    selection_preferences::{Overlap, Preferences, RegionMode},
};
use serde_json::{Value, json};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#BPMS:0=120;#NOTES:dance-single::Easy:1::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    )
}
#[test]
fn source_valid_restore_histories_remain_nonfile_and_preserve_choices_and_anchor() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/selection-preferences-source.json"
    ))
    .unwrap();
    for case in fixture["restores"].as_array().unwrap() {
        let mut session = session();
        let mut selection = Selection::capture(&mut session, 0, 0, 192).unwrap();
        selection.select_all(false);
        let index = selection
            .events()
            .iter()
            .position(|event| matches!(event, deadsync_editor::event_clipboard::Event::Note(_)))
            .unwrap();
        selection.set_selected(index, true).unwrap();
        let anchor = selection.last_selected_event();
        let before_bytes = session.serialize().unwrap();
        let before = Preferences {
            overlap: Overlap::from_value(case["mode"].as_i64().unwrap() as i32),
            region: RegionMode::from_value(case["region"].as_i64().unwrap() as i32),
        };
        let changed = session
            .set_selection_preferences(before, Preferences::default(), None)
            .unwrap();
        assert_eq!(changed, case["undo"].as_bool().unwrap());
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), before_bytes);
        assert!(selection.copy(&mut session).unwrap().is_some());
        assert_eq!(selection.last_selected_event(), anchor);
        if changed {
            assert_eq!(
                session
                    .take_selection_preferences_update()
                    .unwrap()
                    .preferences,
                Preferences::default()
            );
            assert!(session.undo().unwrap());
            assert_eq!(
                session
                    .take_selection_preferences_update()
                    .unwrap()
                    .preferences,
                before
            );
            assert!(selection.copy(&mut session).unwrap().is_some());
            assert!(session.redo().unwrap());
            assert_eq!(
                session
                    .take_selection_preferences_update()
                    .unwrap()
                    .preferences,
                Preferences::default()
            );
            assert!(!session.is_dirty());
        }
    }
}
#[test]
fn explicit_source_field_decoder_matches_real_converter_without_claiming_root_import() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/selection-preferences-source.json"
    ))
    .unwrap();
    for case in fixture["reads"].as_array().unwrap() {
        let fields: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        let prefs = Preferences::read_source_fields(fields.as_object().unwrap()).unwrap();
        assert_eq!(prefs.overlap.value(), case["mode"].as_i64().unwrap() as i32);
        assert_eq!(
            prefs.region.value(),
            case["region"].as_i64().unwrap() as i32
        );
        let mut written = serde_json::Map::new();
        prefs.write(&mut written).unwrap();
        assert_eq!(Preferences::read(&written).unwrap(), prefs);
    }
    assert_eq!(
        Preferences::read(json!({"Mode":1}).as_object().unwrap())
            .unwrap()
            .overlap,
        Overlap::Center
    );
    assert_eq!(
        Preferences::read_source_fields(json!({"Mode":1}).as_object().unwrap())
            .unwrap()
            .overlap,
        Overlap::Any
    );
}
#[test]
fn preference_undo_owns_only_changed_leaves_and_content_guard_still_rejects_edits() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path,serde_json::to_vec(&json!({"Version":1,"Selection":{"Mode":"OverlapAny","RegionMode":"TimeOrPositionAndLane","Vendor":true},"Opaque":{"keep":3}})).unwrap()).unwrap();
    let file = File::open(&path).unwrap();
    let before = file.selection();
    let after = Preferences {
        overlap: Overlap::Center,
        ..before
    };
    let mut session = session();
    let selection = Selection::capture(&mut session, 0, 0, 192).unwrap();
    session
        .set_selection_preferences(before, after, Some(&file))
        .unwrap();
    let mut external: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    external["Selection"]["RegionMode"] = "TimeOrPosition".into();
    std::fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();
    assert!(session.undo().unwrap());
    let update = session.take_selection_preferences_update().unwrap();
    assert_eq!(
        update.preferences,
        Preferences {
            overlap: Overlap::Any,
            region: RegionMode::TimeOrPosition
        }
    );
    let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["Selection"]["Vendor"], true);
    assert_eq!(stored["Opaque"]["keep"], 3);
    assert!(selection.copy(&mut session).unwrap().is_some());
    session.set_text(None, "TITLE", "changed").unwrap();
    assert!(selection.copy(&mut session).is_err());
    let mut foreign =
        Session::new(Document::parse(session.serialize().unwrap(), Format::Sm).unwrap());
    assert!(selection.copy(&mut foreign).is_err());
}
#[test]
fn noop_restore_keeps_redo_and_owned_leaf_conflict_never_partially_writes() {
    let mut session = session();
    let defaults = Preferences::default();
    let changed = Preferences {
        overlap: Overlap::All,
        ..defaults
    };
    session
        .set_selection_preferences(defaults, changed, None)
        .unwrap();
    session.undo().unwrap();
    let redo = session.redo_label().unwrap().to_owned();
    assert!(
        !session
            .set_selection_preferences(defaults, defaults, None)
            .unwrap()
    );
    assert_eq!(session.redo_label(), Some(redo.as_str()));
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        b"{\"Version\":1,\"Selection\":{\"Mode\":\"OverlapCenter\"}}",
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    session
        .set_selection_preferences(defaults, changed, Some(&file))
        .unwrap();
    let external = b"{\"Version\":1,\"Selection\":{\"Mode\":\"OverlapAny\",\"Vendor\":7}}";
    std::fs::write(&path, external).unwrap();
    let label = session.undo_label().unwrap().to_owned();
    assert!(session.undo().is_err());
    assert_eq!(session.undo_label(), Some(label.as_str()));
    assert_eq!(std::fs::read(&path).unwrap(), external);
    assert!(!session.is_dirty());
}
