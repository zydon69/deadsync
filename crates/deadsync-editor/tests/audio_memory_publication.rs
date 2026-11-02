use deadsync_editor::{
    audio_preferences::{Audio, Field, FieldValue, File, source_import::Draft},
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
fn changed(mut audio: Audio, field: Field) -> Audio {
    let value = match audio.value(field) {
        FieldValue::Bool(v) => FieldValue::Bool(!v),
        FieldValue::Int(v) => FieldValue::Int(v + 1),
        FieldValue::Float(v) => FieldValue::Float(if v > 0.5 { v * 0.75 } else { v + 0.1 }),
    };
    audio.set_field(field, value).unwrap();
    audio
}
#[test]
fn every_audio_field_after_source_memory_import_saves_and_undo_redoes() {
    for field in Field::ALL {
        let mut session =
            Session::new(Document::parse(b"#TITLE:AudioMemory;".as_slice(), Format::Sm).unwrap());
        let file = File::memory().unwrap();
        let draft = Draft::prepare(
            &mut session,
            None,
            &file,
            br#"{"VendorPreserved":{"x":1}}"#,
            Platform::Mac,
        )
        .unwrap();
        session
            .import_source_preferences(draft, &file, None, true)
            .unwrap();
        let mut file = session
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        let before = file.audio();
        let after = changed(before, field);
        let song = session.serialize().unwrap();
        assert!(
            session
                .set_audio_preferences(before, after, Some(&file))
                .unwrap(),
            "{field:?}"
        );
        let update = session.take_audio_preferences_update().unwrap();
        assert_eq!(update.audio, after);
        file = update.file.unwrap();
        assert!(file.path().as_os_str().is_empty());
        assert!(!file.has_changes());
        session.capture_source_preferences_context(&file);
        session.undo().unwrap();
        let update = session.take_audio_preferences_update().unwrap();
        assert_eq!(update.audio, before);
        file = update.file.unwrap();
        session.capture_source_preferences_context(&file);
        session.redo().unwrap();
        let update = session.take_audio_preferences_update().unwrap();
        assert_eq!(update.audio, after);
        file = update.file.unwrap();
        assert!(file.source_archive().is_some());
        assert!(!file.has_changes());
        assert_eq!(session.serialize().unwrap(), song);
    }
}
#[test]
fn memory_untracked_tick_keeps_rate_history_and_canonical_file() {
    let mut session =
        Session::new(Document::parse(b"#TITLE:Tick;".as_slice(), Format::Sm).unwrap());
    let mut file = File::memory().unwrap();
    let before = file.audio();
    let after = changed(before, Field::MusicRate);
    session
        .set_audio_preferences(before, after, Some(&file))
        .unwrap();
    file = session
        .take_audio_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let tick = changed(after, Field::UseBeatTick);
    session
        .set_audio_preferences_untracked(after, tick, Some(&file))
        .unwrap();
    file = session
        .take_audio_preferences_update()
        .unwrap()
        .file
        .unwrap();
    let label = session.undo_label().unwrap().to_owned();
    session.capture_source_preferences_context(&file);
    session.undo().unwrap();
    let update = session.take_audio_preferences_update().unwrap();
    assert_eq!(update.audio, changed(before, Field::UseBeatTick));
    file = update.file.unwrap();
    session.capture_source_preferences_context(&file);
    session.redo().unwrap();
    let update = session.take_audio_preferences_update().unwrap();
    assert_eq!(update.audio, tick);
    assert_eq!(session.undo_label(), Some(label.as_str()));
    assert!(!update.file.unwrap().has_changes());
}
