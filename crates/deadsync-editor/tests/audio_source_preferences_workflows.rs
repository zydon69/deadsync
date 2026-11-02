use deadsync_editor::{
    audio_preferences::{Audio, Field, FieldValue, File},
    document::{Document, Format},
    history::Session,
};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#TITLE:Audio;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    )
}
fn project(p: Audio, v: &serde_json::Value) {
    for f in Field::ALL {
        match p.value(f) {
            FieldValue::Float(n) => assert!(
                (n - v[f.key()].as_f64().unwrap()).abs() < 1e-6,
                "{}",
                f.key()
            ),
            FieldValue::Bool(n) => assert_eq!(n, v[f.key()].as_bool().unwrap()),
            FieldValue::Int(n) => assert_eq!(i64::from(n), v[f.key()].as_i64().unwrap()),
        }
    }
}
fn changed(mut p: Audio) -> Audio {
    for (f, v) in [
        (Field::MusicRate, 1.25),
        (Field::AudioOffset, -0.0125),
        (Field::MainVolume, 0.4),
        (Field::MusicVolume, 0.3),
        (Field::AssistTickVolume, 0.2),
        (Field::AssistTickAttackTime, 0.005),
        (Field::BeatTickVolume, 0.1),
        (Field::BeatTickAttackTime, 0.007),
        (Field::PreviewFadeInTime, 0.25),
        (Field::PreviewFadeOutTime, 0.75),
    ] {
        p.set_field(f, FieldValue::Float(v)).unwrap();
    }
    for f in [
        Field::UseAssistTick,
        Field::UseBeatTick,
        Field::SkipAssistTickOnBeatTick,
        Field::SkipBeatTickOnAssistTick,
    ] {
        p.set_field(f, FieldValue::Bool(true)).unwrap();
    }
    p.set_field(Field::DspBufferSize, FieldValue::Int(256))
        .unwrap();
    p.set_field(Field::DspNumBuffers, FieldValue::Int(3))
        .unwrap();
    p
}
#[test]
fn all_sixteen_fields_and_restore_history_match_whole_original_preferences_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/audio.expected.json")).unwrap();
    for c in oracle.as_array().unwrap() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("prefs.json");
        let input =
            serde_json::to_vec(&serde_json::json!({"Version":1,"Audio":c["Input"]})).unwrap();
        std::fs::write(&path, &input).unwrap();
        let mut f = File::open(&path).unwrap();
        project(f.audio(), &c["Normalized"]);
        assert_eq!(std::fs::read(&path).unwrap(), input);
        let p = changed(f.audio());
        project(p, &c["Changed"]);
        let mut s = session();
        s.set_audio_preferences(p, Audio::default(), None).unwrap();
        project(
            s.take_audio_preferences_update().unwrap().audio,
            &c["Restored"],
        );
        s.undo().unwrap();
        project(
            s.take_audio_preferences_update().unwrap().audio,
            &c["Undone"],
        );
        s.redo().unwrap();
        project(
            s.take_audio_preferences_update().unwrap().audio,
            &c["Redone"],
        );
        f.set_audio(p);
        f.save().unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            json["Audio"]["ShowAudioPreferencesWindow"],
            c["Input"]["ShowAudioPreferencesWindow"]
        );
        assert_eq!(c["Notifications"].as_array().unwrap().len(), 12);
        assert!(c["DuplicateNotifications"].as_array().unwrap().is_empty());
        assert!(c["NoOpNotifications"].as_array().unwrap().is_empty());
    }
}
#[test]
fn audio_history_owns_only_changed_leaf_across_external_volume_and_other_groups() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("prefs.json");
    let f = File::open(&path).unwrap();
    let before = f.audio();
    let mut after = before;
    after
        .set_field(Field::UseAssistTick, FieldValue::Bool(true))
        .unwrap();
    let mut s = session();
    let bytes = s.serialize().unwrap();
    s.set_audio_preferences(before, after, Some(&f)).unwrap();
    s.take_audio_preferences_update();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Audio"]["BeatTickVolume"] = 0.25.into();
    json["Audio"]["Vendor"] = serde_json::json!({"keep":true});
    json["Other"] = serde_json::json!({"keep":42});
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    s.undo().unwrap();
    let update = s.take_audio_preferences_update().unwrap();
    assert_eq!(
        update.audio.value(Field::UseAssistTick),
        FieldValue::Bool(false)
    );
    assert_eq!(
        update.audio.value(Field::BeatTickVolume),
        FieldValue::Float(0.25)
    );
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["Audio"]["Vendor"], json["Audio"]["Vendor"]);
    assert_eq!(written["Other"], json["Other"]);
}
#[test]
fn raw_import_is_preserved_but_effective_runtime_values_follow_music_manager_clamps() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("prefs.json");
    std::fs::write(&path,br#"{"Version":1,"Audio":{"MusicRate":8,"MainVolume":-2,"MusicVolume":3,"AudioOffset":-4,"AssistTickAttackTime":-1,"PreviewFadeInTime":-0.5,"PreviewFadeOutTime":-2}}"#).unwrap();
    let p = File::open(&path).unwrap().audio();
    assert_eq!(p.value(Field::MusicRate), FieldValue::Float(8.));
    assert_eq!(p.value(Field::PreviewFadeOutTime), FieldValue::Float(-2.));
    let e = p.effective();
    assert_eq!(e.rate, 4.);
    assert_eq!(e.music_gain, 0.);
    assert_eq!(e.offset, -4.);
    assert_eq!(e.assist_attack, 0.);
    // MusicManager.SetPreviewParameters converts imported fade durations to
    // sample indexes directly; only the interactive drag enforces nonnegative
    // edits. Effective playback must retain the imported signed durations.
    assert_eq!(e.fade_in, -0.5);
    assert_eq!(e.fade_out, -2.);
    assert_eq!(p.mix_gains().main, 0.);
    assert_eq!(p.mix_gains().music, 1.);
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["Audio"]["MainVolume"], -2);
    assert_eq!(raw["Audio"]["PreviewFadeInTime"], -0.5);
    assert_eq!(raw["Audio"]["PreviewFadeOutTime"], -2);
}
