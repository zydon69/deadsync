use deadsync_editor::{
    audio_preferences::{Audio, Field, FieldValue, File},
    document::{Document, Format},
    history::Session,
};
#[test]
fn owned_rate_undo_preserves_scalar_tick_with_obsolete_file_context() {
    for disk in [false, true] {
        for supply_fresh in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("audio.json");
            let mut file = if disk {
                File::open(&path).unwrap()
            } else {
                File::memory().unwrap()
            };
            let mut session = Session::new(
                Document::parse(b"#TITLE:OwnedAudio;".as_slice(), Format::Sm).unwrap(),
            );
            let before = Audio::default();
            let mut fast = before;
            fast.set_music_rate(1.1).unwrap();
            session
                .set_audio_preferences(before, fast, Some(&file))
                .unwrap();
            file = session
                .take_audio_preferences_update()
                .unwrap()
                .file
                .unwrap();
            let mut tick = fast;
            tick.set_field(Field::UseBeatTick, FieldValue::Bool(true))
                .unwrap();
            session
                .set_audio_preferences_untracked(fast, tick, None)
                .unwrap();
            session.take_audio_preferences_update();
            if supply_fresh {
                session.capture_source_preferences_context(&file);
            }
            session.undo().unwrap();
            let update = session.take_audio_preferences_update().unwrap();
            let mut expected = before;
            expected
                .set_field(Field::UseBeatTick, FieldValue::Bool(true))
                .unwrap();
            // With no fresh canonical File, disk's independently edited Audio is the
            // legacy authoritative merged value; the explicit fresh case proves runtime.
            if !disk || supply_fresh {
                assert_eq!(update.audio, expected);
            }
            file = update.file.unwrap();
            if !disk {
                assert_eq!(file.audio(), expected);
                assert!(!file.has_changes());
            }
            if supply_fresh {
                session.capture_source_preferences_context(&file);
                session.redo().unwrap();
                assert_eq!(session.take_audio_preferences_update().unwrap().audio, tick);
            }
        }
    }
}
