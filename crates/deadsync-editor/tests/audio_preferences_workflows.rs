use deadsync_editor::{
    Error,
    audio_preferences::{Audio, File},
};
use std::fs;

#[test]
fn preview_fades_load_edit_save_and_reopen_independently_of_songs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("editor-preferences.json");
    let song = dir.path().join("song.ssc");
    fs::write(&song, b"original song").unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.audio().fade_in(), 0.0);
    assert_eq!(file.audio().fade_out(), 1.5);
    assert!(!file.save().unwrap());
    assert!(!path.exists());
    let mut audio = file.audio();
    audio.set_fade_in(0.125).unwrap();
    audio.set_fade_out(2.75).unwrap();
    file.set_audio(audio);
    assert!(file.has_changes());
    assert!(file.save().unwrap());
    assert!(!file.has_changes());
    assert!(!file.save().unwrap());
    assert_eq!(File::open(&path).unwrap().audio(), audio);
    assert_eq!(fs::read(&song).unwrap(), b"original song");
}
#[test]
fn preferences_keep_unknown_fields_and_original_bytes_on_noop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let source = br#"{ "Version":1,"Future":{"x":true},"Audio":{"PreviewFadeOutTime":2,"FutureAudio":[1,2]}}"#;
    fs::write(&path, source).unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.save().unwrap());
    assert_eq!(fs::read(&path).unwrap(), source);
    let mut audio = file.audio();
    audio.set_fade_in(0.5).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["Future"]["x"], true);
    assert_eq!(saved["Audio"]["FutureAudio"], serde_json::json!([1, 2]));
}
#[test]
fn competing_preferences_sessions_refuse_stale_writes_without_marking_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut first = File::open(&path).unwrap();
    let mut second = File::open(&path).unwrap();
    let mut audio = Audio::default();
    audio.set_fade_out(3.0).unwrap();
    first.set_audio(audio);
    first.save().unwrap();
    let saved = fs::read(&path).unwrap();
    assert!(matches!(second.save(), Err(Error::ExternalChange)));
    audio.set_fade_out(4.0).unwrap();
    second.set_audio(audio);
    assert!(matches!(second.save(), Err(Error::ExternalChange)));
    assert!(second.has_changes());
    assert_eq!(fs::read(&path).unwrap(), saved);
}
#[test]
fn invalid_preferences_and_fade_values_are_rejected_without_rewriting_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    for source in [
        "{broken",
        "[]",
        r#"{"Version":2}"#,
        r#"{"Version":1,"Audio":null}"#,
        r#"{"Version":1,"Audio":{"PreviewFadeOutTime":-1}}"#,
        r#"{"Version":1,"Audio":{"PreviewFadeInTime":"bad"}}"#,
    ] {
        fs::write(&path, source).unwrap();
        assert!(File::open(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
    let mut audio = Audio::default();
    for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(audio.set_fade_in(value).is_err());
        assert!(audio.set_fade_out(value).is_err());
        assert_eq!(audio, Audio::default());
    }
}
