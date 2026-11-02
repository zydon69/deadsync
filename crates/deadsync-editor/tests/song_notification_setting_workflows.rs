use deadsync_editor::{
    audio_preferences::{File, source_import::Draft},
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
#[test]
fn original_song_notification_checkbox_matches_public_memory_action_undo_redo() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/song-notification-setting-source.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
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
        let before = file.suppress_external_song_notifications();
        assert_eq!(before, stages[0]["suppressed"].as_bool().unwrap());
        session
            .set_suppress_external_song_notifications(before, !before, &file)
            .unwrap();
        file = session.take_restore_options_update().unwrap().file.unwrap();
        assert_eq!(
            file.suppress_external_song_notifications(),
            stages[1]["suppressed"].as_bool().unwrap()
        );
        assert!(!file.has_changes());
        session.capture_source_preferences_context(&file);
        session.undo().unwrap();
        file = session.take_restore_options_update().unwrap().file.unwrap();
        assert_eq!(
            file.suppress_external_song_notifications(),
            stages[2]["suppressed"].as_bool().unwrap()
        );
        session.capture_source_preferences_context(&file);
        session.redo().unwrap();
        file = session.take_restore_options_update().unwrap().file.unwrap();
        assert_eq!(
            file.suppress_external_song_notifications(),
            stages[3]["suppressed"].as_bool().unwrap()
        );
        assert_eq!(
            file.suppress_external_song_notifications(),
            stages[4]["suppressed"].as_bool().unwrap()
        );
        assert!(!file.has_changes());
        assert!(file.source_archive().is_some());
        assert_eq!(session.serialize().unwrap(), b"#TITLE:Keep;");
    }
}
