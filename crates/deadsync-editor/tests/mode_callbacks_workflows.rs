use deadsync_editor::{
    audio_preferences,
    key_binding_preferences::{
        Action, Preferences,
        source_profile::{self, Platform},
    },
    lane_entry::Mode,
    workspace::Workspace,
    zoom::SpacingMode,
};
#[test]
fn original_mode_callbacks_match_all_six_initial_states_without_chart_history() {
    let traces: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/mode-callbacks-source.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("chart.ssc");
    std::fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
    )
    .unwrap();
    let mut count = 0;
    for trace in traces.as_array().unwrap() {
        let mut workspace = Workspace::open(&song, 0).unwrap();
        let frames = trace["Frames"].as_array().unwrap();
        workspace.set_auto_advance(frames[0]["Note"] == "AdvanceBySnap");
        let mut spacing = SpacingMode::read(Some(&frames[0]["Spacing"]));
        let before = workspace.file().session.serialize().unwrap();
        let revision = workspace.file().session.revision();
        for frame in frames {
            match frame["Operation"].as_str().unwrap() {
                "initial" => {}
                "note" => workspace.toggle_note_entry_mode(),
                "spacing" => spacing = spacing.next(),
                _ => panic!("invalid oracle"),
            }
            assert_eq!(workspace.auto_advance(), frame["Note"] == "AdvanceBySnap");
            assert_eq!(spacing.name(), frame["Spacing"].as_str().unwrap());
            assert_eq!(workspace.file().session.revision(), revision);
            assert_eq!(workspace.file().session.serialize().unwrap(), before);
            assert!(workspace.file().session.undo_label().is_none());
            count += 1;
        }
    }
    assert_eq!(count, 48);
}
#[test]
fn both_source_callbacks_have_physical_defaults_and_native_controls_keep_their_bindings() {
    let native = Preferences::default();
    assert_eq!(
        native.commands["ToggleNoteEntryMode"],
        vec![Vec::<String>::new()]
    );
    assert_eq!(
        native.commands["ToggleSpacingMode"],
        vec![Vec::<String>::new()]
    );
    assert_eq!(native.commands["ToggleAutoAdvance"], vec![vec!["A"]]);
    for platform in [Platform::Other, Platform::Mac] {
        let prepared = source_profile::preset(&native, platform).unwrap();
        for (id, key) in [("ToggleNoteEntryMode", "M"), ("ToggleSpacingMode", "S")] {
            assert_eq!(prepared.preferences.commands[id], vec![vec![key]]);
            assert!(prepared.report.applied.iter().any(|x| x == id));
            assert!(!prepared.report.unavailable_commands.iter().any(|x| x == id));
            assert!(!Action::from_id(id).unwrap().repeats());
        }
    }
}
#[test]
fn effective_mode_preferences_roundtrip_without_losing_unknown_fields_or_touching_song_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path,br#"{"Version":1,"NoteEntryMode":"Normal","Scroll":{"SpacingMode":"ConstantTime","Vendor":17},"Unknown":"keep"}"#).unwrap();
    let mut file = audio_preferences::File::open(&path).unwrap();
    file.set_note_entry_mode(Mode::AdvanceBySnap);
    file.set_spacing_mode(SpacingMode::Variable);
    assert!(file.save().unwrap());
    let file = audio_preferences::File::open(&path).unwrap();
    assert_eq!(file.note_entry_mode(), Mode::AdvanceBySnap);
    assert_eq!(file.spacing_mode(), SpacingMode::Variable);
    assert!(file.has_spacing_mode());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json["Unknown"], "keep");
    assert_eq!(json["Scroll"]["Vendor"], 17);
    assert_eq!(Mode::read(Some(&serde_json::json!("Future"))), Mode::Normal);
    assert_eq!(
        SpacingMode::read(Some(&serde_json::json!("Future"))),
        SpacingMode::ConstantTime
    );
    let mut chartless = Workspace::new_song(dir.path(), 0.0).unwrap();
    assert!(!chartless.has_chart());
    chartless.toggle_note_entry_mode();
    assert!(chartless.auto_advance());
    chartless.set_auto_advance(false);
    assert!(!chartless.auto_advance());
}
