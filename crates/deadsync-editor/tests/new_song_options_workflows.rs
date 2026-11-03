use deadsync_editor::{Error, audio_preferences::File, workspace::Workspace};

#[test]
fn source_new_song_offset_defaults_and_explicit_values_are_read_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let absent = root.path().join("absent.json");
    let defaults = File::open(&absent).unwrap();
    assert_eq!(defaults.new_song_sync_offset().unwrap(), 0.009);
    assert!(!absent.exists());

    for (json, expected) in [
        (r#"{"Version":1}"#, 0.009),
        (
            r#"{"Version":1,"Options":{"NewSongSyncOffset":-0.002}}"#,
            -0.002,
        ),
        (r#"{"Version":1,"Options":{"NewSongSyncOffset":0}}"#, 0.0),
    ] {
        let path = root.path().join("preferences.json");
        std::fs::write(&path, json).unwrap();
        let file = File::open(&path).unwrap();
        assert_eq!(file.new_song_sync_offset().unwrap(), expected);
        assert!(!file.has_changes());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
        let candidate =
            Workspace::new_song(root.path(), file.new_song_sync_offset().unwrap()).unwrap();
        assert!(candidate.file().requires_save_as());
        assert_eq!(
            candidate.song_properties().unwrap().sync_offset().to_bits(),
            expected.to_bits()
        );
        assert_eq!(
            std::fs::read_dir(root.path()).unwrap().count(),
            1,
            "preparing a new song creates no simfile"
        );
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
        // The complete General owner validates at File::open. The invalid
        // value must never become an installed File with a fallible lazy getter.
        assert!(
            matches!(File::open(&path), Err(Error::InvalidFormat(message))
            if message.contains("NewSongSyncOffset"))
        );
        let baseline = File::open(&root.path().join("valid-preferences.json")).unwrap();
        let mut workspace = Workspace::new_song(root.path(), 0.009).unwrap();
        let before = workspace.file().session.serialize().unwrap();
        let revision = workspace.file().session.revision();
        let native: serde_json::Value = serde_json::from_str(json).unwrap();
        let source =
            serde_json::to_vec(&serde_json::json!({"PreferencesOptions": native["Options"]}))
                .unwrap();
        assert!(
            workspace
                .prepare_source_preferences(
                    &baseline,
                    &source,
                    deadsync_editor::key_binding_preferences::source_profile::Platform::Mac
                )
                .is_err()
        );
        assert_eq!(workspace.file().session.serialize().unwrap(), before);
        assert_eq!(workspace.file().session.revision(), revision);
        assert!(baseline.source_archive().is_none());
        assert!(!baseline.has_changes());
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Workspace::new_song(root.path(), value).is_err());
        }
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
    }
}
