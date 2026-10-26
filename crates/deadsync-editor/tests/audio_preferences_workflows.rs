use deadsync_editor::{
    Error,
    audio_preferences::{Audio, File},
};
use std::fs;

#[test]
fn rate_shortcuts_match_pinned_original_including_ulp_boundaries_and_clamps() {
    let input: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/audio-music-rate.request.json"
    ))
    .unwrap();
    let expected: Vec<Vec<f64>> = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/audio-music-rate.expected.json"
    ))
    .unwrap();
    for (index, case) in input["Cases"].as_array().unwrap().iter().enumerate() {
        let mut audio = Audio::default();
        audio
            .set_music_rate(case["Rate"].as_f64().unwrap())
            .unwrap();
        for (step, direction) in case["Directions"].as_array().unwrap().iter().enumerate() {
            audio.step_music_rate(direction.as_bool().unwrap());
            assert_eq!(
                audio.music_rate(),
                expected[index][step],
                "case {index} step {step}"
            );
        }
    }
}

#[test]
fn audio_preferences_share_undo_history_without_dirtying_song_and_guard_disk_conflicts() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let mut session = Session::new(
        Document::parse(
            b"#TITLE:Original;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let original = session.serialize().unwrap();
    let mut rate = Audio::default();
    rate.set_music_rate(1.1).unwrap();
    session
        .set_audio_preferences(Audio::default(), rate, Some(&file))
        .unwrap();
    let committed = session.take_audio_preferences_update().unwrap();
    assert_eq!(committed.audio, rate);
    assert!(!session.is_dirty());
    assert_eq!(session.serialize().unwrap(), original);
    session.set_text(None, "TITLE", "Edited").unwrap();
    session.undo().unwrap();
    assert!(session.take_audio_preferences_update().is_none());
    session.undo().unwrap();
    assert_eq!(
        session.take_audio_preferences_update().unwrap().audio,
        Audio::default()
    );
    assert_eq!(File::open(&path).unwrap().audio(), Audio::default());
    session.redo().unwrap();
    assert_eq!(session.take_audio_preferences_update().unwrap().audio, rate);
    let saved = fs::read(&path).unwrap();
    fs::write(&path, b"External settings").unwrap();
    let revision = session.revision();
    assert!(matches!(session.undo(), Err(Error::InvalidFormat(_))));
    assert_eq!(session.revision(), revision);
    assert!(session.take_audio_preferences_update().is_none());
    assert_eq!(fs::read(&path).unwrap(), b"External settings");
    fs::write(&path, saved).unwrap();
    session.undo().unwrap();
    assert_eq!(
        session.take_audio_preferences_update().unwrap().audio,
        Audio::default()
    );
    assert!(!session.is_dirty());
}

#[test]
fn rate_validation_persistence_and_memory_history_preserve_noops_and_reject_staged_io() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let mut audio = Audio::default();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(audio.set_music_rate(bad).is_err());
        assert_eq!(audio.music_rate(), 1.0);
    }
    audio.set_music_rate(100.0).unwrap();
    assert_eq!(audio.music_rate(), 4.0);
    audio.set_music_rate(-1.0).unwrap();
    assert_eq!(audio.music_rate(), 0.25);
    let mut session = Session::new(
        Document::parse(
            b"#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    assert!(
        session
            .edit_chart_atomic(0, "Invalid", |staged| staged
                .set_audio_preferences(Audio::default(), audio, None)
                .map(|_| ()))
            .is_err()
    );
    assert_eq!(session.revision(), 0);
    session
        .set_audio_preferences(Audio::default(), audio, None)
        .unwrap();
    assert_eq!(
        session.take_audio_preferences_update().unwrap().audio,
        audio
    );
    assert!(!session.is_dirty());
    session.undo().unwrap();
    session.take_audio_preferences_update();
    assert!(
        !session
            .set_audio_preferences(Audio::default(), Audio::default(), None)
            .unwrap()
    );
    assert!(session.redo().unwrap());
    assert_eq!(
        session.take_audio_preferences_update().unwrap().audio,
        audio
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    assert_eq!(File::open(&path).unwrap().audio().music_rate(), 0.25);
    fs::write(&path, br#"{"Version":1,"Audio":{"MusicRate":"bad"}}"#).unwrap();
    assert!(File::open(&path).is_err());
}

#[test]
fn rate_history_merges_unrelated_preferences_and_refuses_changed_owned_fields() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let file = File::open(&path).unwrap();
    let mut session = Session::new(
        Document::parse(
            b"#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let mut audio = Audio::default();
    audio.set_music_rate(1.1).unwrap();
    session
        .set_audio_preferences(Audio::default(), audio, Some(&file))
        .unwrap();
    session.take_audio_preferences_update();
    let mut external: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    external["Audio"]["PreviewFadeOutTime"] = 2.5.into();
    external["Unrelated"] = true.into();
    fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();
    session.undo().unwrap();
    let restored = session.take_audio_preferences_update().unwrap();
    assert_eq!(restored.audio.music_rate(), 1.0);
    assert_eq!(restored.audio.fade_out(), 2.5);
    session.redo().unwrap();
    session.take_audio_preferences_update();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["Unrelated"], true);
    assert_eq!(saved["Audio"]["PreviewFadeOutTime"], 2.5);
    let mut changed = saved;
    changed["Audio"]["MusicRate"] = 1.7.into();
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let before = session.revision();
    assert!(matches!(session.undo(), Err(Error::ExternalChange)));
    assert_eq!(session.revision(), before);
    assert!(session.take_audio_preferences_update().is_none());
}

#[test]
fn song_drafts_follow_content_while_audio_actions_share_global_history() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
        song_properties::Draft,
    };
    let bytes = b"#BPMS:0=120;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;".as_slice();
    let mut session = Session::new(Document::parse(bytes, Format::Sm).unwrap());
    let draft = Draft::capture(&session).unwrap();
    let mut audio = Audio::default();
    audio.set_music_rate(1.2).unwrap();
    session
        .set_audio_preferences(Audio::default(), audio, None)
        .unwrap();
    assert!(draft.clock(&session, 0).is_ok());
    assert!(!draft.apply(&mut session).unwrap());
    let other = Session::new(Document::parse(bytes, Format::Sm).unwrap());
    assert!(draft.clock(&other, 0).is_err());
    session.set_text(None, "TITLE", "Changed").unwrap();
    assert!(draft.clock(&session, 0).is_err());
    session.undo().unwrap();
    assert!(draft.clock(&session, 0).is_ok());
    session.undo().unwrap();
    assert!(draft.clock(&session, 0).is_ok());
}

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
