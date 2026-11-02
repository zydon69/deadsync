use deadsync_editor::{
    audio_preferences::File,
    density_preferences::Accumulation,
    document::{Document, Format},
    history::Session,
    stream_preferences::{NoteType, Preferences},
};
fn session() -> Session {
    Session::new(
        Document::parse(
            b"#TITLE:Keep;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    )
}
fn project(p: Preferences, v: &serde_json::Value) {
    assert_eq!(p.note_type.name(), v["NoteType"].as_str().unwrap());
    assert_eq!(
        if p.accumulation == Accumulation::Row {
            "Row"
        } else {
            "Step"
        },
        v["AccumulationType"].as_str().unwrap()
    );
    assert_eq!(
        p.show_break_lengths,
        v["ShowBreakLengths"].as_bool().unwrap()
    );
    assert_eq!(
        i64::from(p.minimum_length),
        v["MinimumLengthToConsiderStream"].as_i64().unwrap()
    );
    assert_eq!(
        i64::from(p.short_break_cutoff),
        v["ShortBreakCutoff"].as_i64().unwrap()
    );
    assert_eq!(
        p.short_break.to_string(),
        v["ShortBreakCharacter"].as_str().unwrap()
    );
    assert_eq!(
        p.long_break.to_string(),
        v["LongBreakCharacter"].as_str().unwrap()
    );
}
#[test]
fn whole_source_defaults_import_and_restore_undo_redo_match_ten_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/stream.expected.json")).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 10);
    for c in cases.as_array().unwrap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let bytes = serde_json::to_vec(&serde_json::json!({"Version":1,"Stream":c["Input"]})).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let f = File::open(&path).unwrap();
        project(f.streams(), &c["Normalized"]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let mut changed = f.streams();
        changed.note_type = NoteType::EighthTriplet;
        changed.accumulation = Accumulation::Row;
        changed.show_break_lengths = true;
        changed.minimum_length = 3;
        changed.short_break_cutoff = 11;
        changed.short_break = 'é';
        changed.long_break = ':';
        project(changed, &c["Changed"]);
        let mut s = session();
        s.set_stream_preferences(changed, Preferences::default(), None)
            .unwrap();
        project(
            s.take_stream_preferences_update().unwrap().preferences,
            &c["Restored"],
        );
        s.undo().unwrap();
        project(
            s.take_stream_preferences_update().unwrap().preferences,
            &c["Undone"],
        );
        s.redo().unwrap();
        project(
            s.take_stream_preferences_update().unwrap().preferences,
            &c["Redone"],
        );
    }
}
#[test]
fn per_field_history_merges_unowned_stream_changes_and_preserves_other_owners() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path,br#"{"Version":1,"Stream":{"Vendor":{"x":1},"ShowStreamPreferencesWindow":true},"Other":{"keep":true}}"#).unwrap();
    let file = File::open(&path).unwrap();
    let before = file.streams();
    let mut after = before;
    after.note_type = NoteType::Quarter;
    let mut s = session();
    s.set_stream_preferences(before, after, Some(&file))
        .unwrap();
    s.take_stream_preferences_update();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Stream"]["LongBreakCharacter"] = "é".into();
    json["Audio"]["Vendor"] = 42.into();
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    s.undo().unwrap();
    let update = s.take_stream_preferences_update().unwrap();
    assert_eq!(update.preferences.note_type, NoteType::Sixteenth);
    assert_eq!(update.preferences.long_break, 'é');
    s.redo().unwrap();
    assert_eq!(
        s.take_stream_preferences_update()
            .unwrap()
            .preferences
            .long_break,
        'é'
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["Stream"]["Vendor"], json["Stream"]["Vendor"]);
    assert_eq!(written["Stream"]["ShowStreamPreferencesWindow"], true);
    assert_eq!(written["Audio"]["Vendor"], 42);
    assert_eq!(written["Other"], json["Other"]);
    assert!(!s.is_dirty());
}
#[test]
fn owned_conflict_refuses_without_file_history_or_density_update_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let f = File::open(&path).unwrap();
    let before = f.streams();
    let mut after = before;
    after.minimum_length = 3;
    let mut s = session();
    s.set_stream_preferences(before, after, Some(&f)).unwrap();
    s.take_stream_preferences_update();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    json["Stream"]["MinimumLengthToConsiderStream"] = 8.into();
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let revision = s.revision();
    assert!(matches!(
        s.undo(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(s.revision(), revision);
    assert!(s.undo_changes_stream_preferences());
    assert!(s.take_stream_preferences_update().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
#[test]
fn no_op_preserves_redo_and_stream_settings_never_dirty_simfile() {
    let mut s = session();
    let bytes = s.serialize().unwrap();
    let before = Preferences::default();
    let mut after = before;
    after.show_break_lengths = true;
    s.set_stream_preferences(before, after, None).unwrap();
    s.take_stream_preferences_update();
    s.undo().unwrap();
    s.take_stream_preferences_update();
    let revision = s.revision();
    assert!(!s.set_stream_preferences(before, before, None).unwrap());
    assert_eq!(s.revision(), revision);
    assert!(s.redo_changes_stream_preferences());
    s.redo().unwrap();
    assert_eq!(
        s.take_stream_preferences_update().unwrap().preferences,
        after
    );
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
}
#[test]
fn signed_source_thresholds_preserve_storage_and_equivalent_density_semantics() {
    let f = serde_json::json!({"MinimumLengthToConsiderStream":-3,"ShortBreakCutoff":-4,"ShortBreakCharacter":"\t"});
    let p = Preferences::read(f.as_object().unwrap()).unwrap();
    assert_eq!(p.minimum_length, -3);
    assert_eq!(p.short_break_cutoff, -4);
    assert_eq!(p.density_settings().minimum_length, 0);
    assert_eq!(p.density_settings().short_break_cutoff, 0);
    assert_eq!(p.short_break, '\t');
    assert_eq!(
        deadsync_editor::density::StreamSettings::read(f.as_object().unwrap()).unwrap(),
        p.density_settings()
    );
}
#[test]
fn malformed_marks_and_source_crashing_enum_indices_are_rejected() {
    for v in [
        serde_json::json!({"ShortBreakCharacter":""}),
        serde_json::json!({"ShortBreakCharacter":"ab"}),
        serde_json::json!({"LongBreakCharacter":"😀"}),
        serde_json::json!({"NoteType":"999"}),
        serde_json::json!({"MinimumLengthToConsiderStream":2147483648i64}),
        serde_json::json!({"ShowBreakLengths":3}),
    ] {
        assert!(Preferences::read(v.as_object().unwrap()).is_err());
    }
    let mut p = Preferences::default();
    p.long_break = '😀';
    assert!(p.validate().is_err());
}
