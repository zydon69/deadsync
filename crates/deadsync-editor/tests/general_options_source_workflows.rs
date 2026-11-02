use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    options_preferences::General,
};
use serde_json::{Map, Value};
fn session() -> Session {
    Session::new(Document::parse(&b"#VERSION:0.83;#TITLE:General;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;"[..],Format::Ssc).unwrap())
}
#[test]
fn all_twelve_fields_and_source_restore_match_original_options_oracle() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/options.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        for (input, expected) in [
            (&case["Input"], &case["Normalized"]),
            (&case["Changed"], &case["Changed"]),
            (&case["Restored"], &case["Restored"]),
        ] {
            let p = General::read(input.as_object().unwrap()).unwrap();
            let mut written = Map::new();
            p.write(&mut written).unwrap();
            for name in General::NAMES {
                if matches!(
                    name,
                    "NewSongSyncOffset" | "OpenSongSyncOffset" | "DpiScale"
                ) {
                    assert_eq!(written[name].as_f64(), expected[name].as_f64(), "{name}");
                } else if name == "StartupStepGraphs" {
                    let mut actual: Vec<_> = written[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect();
                    let mut wanted: Vec<_> = expected[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect();
                    actual.sort();
                    wanted.sort();
                    assert_eq!(actual, wanted);
                } else {
                    assert_eq!(written[name], expected[name], "{name}");
                }
            }
        }
    }
}
#[test]
fn changed_leaf_history_updates_real_new_open_consumers_without_dirtying_song() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"Future":"keep","RenderNotes":false}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let before = file.general_options().clone();
    let mut after = before.clone();
    after.new_sync = 0.125;
    after.open_sync = 0.375;
    after.open_last = false;
    let mut s = session();
    let bytes = s.serialize().unwrap();
    assert!(
        s.set_general_options(before.clone(), after.clone(), Some(&file))
            .unwrap()
    );
    let u = s.take_general_options_update().unwrap();
    let file = u.file.unwrap();
    assert_eq!(file.new_song_sync_offset().unwrap(), 0.125);
    assert_eq!(
        file.opening().unwrap().load_context().open_sync_offset(),
        0.375
    );
    assert!(!file.open_last_on_launch().unwrap());
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
    let mut external = File::open(&path).unwrap();
    let mut p = external.general_options().clone();
    p.dpi_scale = 1.75;
    external.set_general_options(p).unwrap();
    external.save().unwrap();
    assert!(s.undo().unwrap());
    let undo = s.take_general_options_update().unwrap();
    assert_eq!(undo.preferences.new_sync, 0.009);
    assert_eq!(undo.preferences.open_sync, 0.009);
    assert_eq!(undo.preferences.dpi_scale, 1.75);
    assert!(s.redo().unwrap());
    let redo = s.take_general_options_update().unwrap();
    assert_eq!(redo.preferences.open_sync, 0.375);
    assert_eq!(redo.preferences.dpi_scale, 1.75);
    let saved: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(saved["Options"]["Future"], "keep");
    assert_eq!(saved["Options"]["RenderNotes"], false);
    assert_eq!(s.serialize().unwrap(), bytes);
}
#[test]
fn resize_action_applies_before_publication_and_survives_undo_redo_with_bounded_history() {
    let mut s = session();
    let before = General::default();
    let mut after = before.clone();
    after.undo_history_size = 2;
    for i in 0..5 {
        s.set_text(None, "TITLE", &i.to_string()).unwrap();
    }
    assert!(s.set_general_options(before.clone(), after, None).unwrap());
    assert_eq!(s.history_capacity(), 2);
    assert_eq!(s.undo_label(), Some("General options"));
    assert!(s.undo().unwrap());
    assert_eq!(s.history_capacity(), 1024);
    s.take_general_options_update();
    assert!(s.redo().unwrap());
    assert_eq!(s.history_capacity(), 2);
    s.take_general_options_update();
    let mut count = 0;
    while s.undo().unwrap() {
        count += 1;
        s.take_general_options_update();
    }
    assert_eq!(count, 2);
    let mut count = 0;
    while s.redo().unwrap() {
        count += 1;
        s.take_general_options_update();
    }
    assert_eq!(count, 2);
    assert_eq!(s.history_capacity(), 2);
}
#[test]
fn raw_import_values_remain_separate_from_safe_native_editing_ranges() {
    let mut fields = Map::new();
    fields.insert("UndoHistorySize".into(), (-5).into());
    fields.insert("NewSongSyncOffset".into(), (-0.25).into());
    fields.insert("OpenSongSyncOffset".into(), (-0.5).into());
    fields.insert("DpiScale".into(), (-2.).into());
    fields.insert("DefaultStepsType".into(), "999".into());
    let mut p = General::read(&fields).unwrap();
    assert_eq!(p.undo_history_size, -5);
    assert_eq!(p.effective_history_size(), 1);
    assert_eq!(p.new_sync, -0.25);
    assert_eq!(p.open_sync, -0.5);
    assert_eq!(p.dpi_scale, -2.);
    assert_eq!(p.default_steps_type, 999);
    p.set_history_size(0);
    assert_eq!(p.undo_history_size, 1);
    p.set_new_sync(-0.5).unwrap();
    assert_eq!(p.new_sync, 0.);
    p.step_default_type(true);
    assert_eq!(p.default_steps_type, 1);
    assert!(p.set_open_sync(f64::INFINITY).is_err());
}
