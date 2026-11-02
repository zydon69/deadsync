use deadsync_editor::{
    audio_preferences::{File, source_import::Draft},
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    selection_preferences::Preferences,
};
use serde_json::Value;
fn equal(p: Preferences, case: &Value) {
    let original = &case["value"]["PreferencesSelection"];
    assert_eq!(p.overlap.name(), original["Mode"].as_str().unwrap());
    assert_eq!(p.region.name(), original["RegionMode"].as_str().unwrap());
    assert_eq!(
        p == Preferences::default(),
        case["using_defaults"].as_bool().unwrap()
    );
}
#[test]
fn original_selection_restore_undo_redo_and_reopen_transcripts_match_public_memory_history() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/selection-restore.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 30);
    for stages in cases.chunks_exact(5) {
        let mut session =
            Session::new(Document::parse(b"#TITLE:Keep;".as_slice(), Format::Sm).unwrap());
        let original = File::memory().unwrap();
        let draft = Draft::prepare(
            &mut session,
            None,
            &original,
            stages[0]["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        session
            .import_source_preferences(draft, &original, None, true)
            .unwrap();
        let mut file = session
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        let before = file.selection();
        equal(before, &stages[0]);
        let changed = session
            .set_selection_preferences(before, Preferences::default(), Some(&file))
            .unwrap();
        if changed {
            file = session
                .take_selection_preferences_update()
                .unwrap()
                .file
                .unwrap();
            equal(file.selection(), &stages[1]);
            session.capture_source_preferences_context(&file);
            session.undo().unwrap();
            file = session
                .take_selection_preferences_update()
                .unwrap()
                .file
                .unwrap();
            equal(file.selection(), &stages[2]);
            session.capture_source_preferences_context(&file);
            session.redo().unwrap();
            file = session
                .take_selection_preferences_update()
                .unwrap()
                .file
                .unwrap();
        }
        equal(file.selection(), &stages[3]);
        equal(file.selection(), &stages[4]);
        assert!(file.source_archive().is_some());
        assert!(!file.has_changes());
        assert_eq!(session.serialize().unwrap(), b"#TITLE:Keep;");
    }
}
