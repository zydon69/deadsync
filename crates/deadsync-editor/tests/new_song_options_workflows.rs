use deadsync_editor::audio_preferences::File;

#[test]
fn source_new_song_offset_defaults_and_explicit_values_are_read_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let absent = root.path().join("absent.json");
    let defaults = File::open(&absent).unwrap();
    assert_eq!(defaults.new_song_sync_offset().unwrap(), 0.009);
    assert!(!absent.exists());

    for (json, expected) in [
        (r#"{"Version":1}"#, 0.009),
        (r#"{"Version":1,"Options":{"NewSongSyncOffset":-0.002}}"#, -0.002),
        (r#"{"Version":1,"Options":{"NewSongSyncOffset":0}}"#, 0.0),
    ] {
        let path = root.path().join("preferences.json");
        std::fs::write(&path, json).unwrap();
        let file = File::open(&path).unwrap();
        assert_eq!(file.new_song_sync_offset().unwrap(), expected);
        assert!(!file.has_changes());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
    }
}

#[test]
fn invalid_new_song_offset_is_an_error_without_output() {
    for json in [
        r#"{"Version":1,"Options":{"NewSongSyncOffset":"bad"}}"#,
        r#"{"Version":1,"Options":{"NewSongSyncOffset":null}}"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        std::fs::write(&path, json).unwrap();
        let file = File::open(&path).unwrap();
        assert!(file.new_song_sync_offset().is_err());
        assert!(!file.has_changes());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
    }
}
