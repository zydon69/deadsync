use deadsync_editor::{
    Error,
    audio_preferences::{Audio, Field, FieldValue, File},
    document::{Document, Format},
    history::Session,
};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#TITLE:Unchanged;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    )
}
fn rate(mut a: Audio, r: f64) -> Audio {
    a.set_music_rate(r).unwrap();
    a
}
fn beat(mut a: Audio) -> Audio {
    a.set_field(Field::UseBeatTick, FieldValue::Bool(true))
        .unwrap();
    a
}
#[test]
fn direct_tick_preserves_rate_history_and_redo_in_memory_and_on_disk() {
    for disk in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.json");
        let mut file = disk.then(|| File::open(&path).unwrap());
        let mut s = session();
        let bytes = s.serialize().unwrap();
        let base = Audio::default();
        let fast = rate(base, 1.1);
        s.set_audio_preferences(base, fast, file.as_ref()).unwrap();
        s.take_audio_preferences_update().unwrap();
        s.undo().unwrap();
        let update = s.take_audio_preferences_update().unwrap();
        file = update.file;
        assert_eq!(update.audio, base);
        s.set_audio_preferences_untracked(base, beat(base), file.as_ref())
            .unwrap();
        let update = s.take_audio_preferences_update().unwrap();
        assert_eq!(update.audio, beat(base));
        s.redo().unwrap();
        let update = s.take_audio_preferences_update().unwrap();
        assert_eq!(update.audio, beat(fast));
        s.undo().unwrap();
        assert_eq!(s.take_audio_preferences_update().unwrap().audio, beat(base));
        // The direct toggle itself was never put into the action queue.
        assert!(!s.undo().unwrap());
        assert_eq!(s.serialize().unwrap(), bytes);
        assert!(!s.is_dirty());
        if disk {
            assert_eq!(File::open(&path).unwrap().audio(), beat(base));
        }
    }
}
#[test]
fn direct_tick_atomic_refusal_keeps_file_and_rate_redo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audio.json");
    let file = File::open(&path).unwrap();
    let mut s = session();
    let base = Audio::default();
    s.set_audio_preferences(base, rate(base, 1.1), Some(&file))
        .unwrap();
    s.take_audio_preferences_update();
    s.undo().unwrap();
    let file = s.take_audio_preferences_update().unwrap().file.unwrap();
    let mut external = File::open(&path).unwrap();
    external.set_audio(beat(base));
    external.save().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(
        s.set_audio_preferences_untracked(base, beat(base), Some(&file)),
        Err(Error::ExternalChange)
    ));
    assert!(s.take_audio_preferences_update().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    s.redo().unwrap();
    assert_eq!(
        s.take_audio_preferences_update().unwrap().audio,
        beat(rate(base, 1.1))
    );
}
#[test]
fn stale_context_and_refused_memory_action_cannot_overwrite_untracked_audio() {
    let mut s = session();
    let base = Audio::default();
    let fast = rate(base, 1.1);
    s.set_audio_preferences(base, fast, None).unwrap();
    s.take_audio_preferences_update();
    s.set_audio_preferences_untracked(fast, beat(fast), None)
        .unwrap();
    s.take_audio_preferences_update();
    // Capturing an obsolete File is not an audio assignment.
    let stale = File::memory().unwrap();
    s.capture_source_preferences_context(&stale);
    let revision = s.revision();
    let label = s.undo_label().map(str::to_owned);
    assert!(matches!(
        s.set_audio_preferences(base, rate(base, 1.2), None),
        Err(Error::ExternalChange)
    ));
    assert_eq!(s.revision(), revision);
    assert_eq!(s.undo_label(), label.as_deref());
    assert!(s.take_audio_preferences_update().is_none());
    s.undo().unwrap();
    assert_eq!(s.take_audio_preferences_update().unwrap().audio, beat(base));
    s.redo().unwrap();
    assert_eq!(s.take_audio_preferences_update().unwrap().audio, beat(fast));
}
