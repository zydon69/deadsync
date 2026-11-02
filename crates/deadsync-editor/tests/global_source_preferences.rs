//! Prepared core integration tests; parent runs after integrating the lot.
use deadsync_editor::{
    audio_preferences::File,
    key_binding_preferences::source_profile::Platform,
    workspace::{Command, Workspace},
};
fn workspace(root: &std::path::Path) -> Workspace {
    let path = root.join("song.ssc");
    std::fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    Workspace::open(&path, 0).unwrap()
}
#[test]
fn raw_jsonc_unknown_huge_numbers_and_version_are_preserved_with_explicit_report() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let raw = br#"{// kept exactly
 "Version":77,"Vendor":{"number":1e999},"PreferencesAudio":{"MusicVolume":0.7,},}"#;
    let draft = ws
        .prepare_source_preferences(&file, raw, Platform::Mac)
        .unwrap();
    assert_eq!(draft.raw_source(), raw);
    assert!(draft.report().requires_acknowledgement());
    assert!(draft.report().items.iter().any(|i| i.path == "Version"));
    assert_eq!(
        draft.target().source_archive().unwrap()["RawUtf8"]
            .as_str()
            .unwrap()
            .as_bytes(),
        raw
    );
    assert!(ws.import_source_preferences(draft, &file, false).is_err());
    assert!(ws.take_source_preferences_update().is_none());
}
#[test]
fn edit_undo_and_undo_redo_do_not_revalidate_a_reviewed_draft() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let draft = ws
        .prepare_source_preferences(&file, b"{}", Platform::Mac)
        .unwrap();
    ws.execute(Command::Place(1)).unwrap();
    ws.execute(Command::Undo).unwrap();
    assert!(ws.import_source_preferences(draft, &file, true).is_err());
    ws.execute(Command::Place(1)).unwrap();
    let draft = ws
        .prepare_source_preferences(&file, b"{}", Platform::Mac)
        .unwrap();
    ws.execute(Command::Undo).unwrap();
    ws.execute(Command::Redo).unwrap();
    assert!(ws.import_source_preferences(draft, &file, true).is_err());
}
#[test]
fn one_runtime_snapshot_is_consumed_and_direct_competing_preferences_block_undo() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let draft = ws
        .prepare_source_preferences(
            &file,
            br#"{"PreferencesAudio":{"MusicVolume":0.7}}"#,
            Platform::Mac,
        )
        .unwrap();
    ws.import_source_preferences(draft, &file, true).unwrap();
    let mut current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    assert!(ws.execute(Command::Undo).is_err()); // A fresh runtime witness is mandatory.
    let original = current.audio();
    let mut competing = original;
    competing
        .set_field(
            deadsync_editor::audio_preferences::Field::MusicVolume,
            deadsync_editor::audio_preferences::FieldValue::Float(0.9),
        )
        .unwrap();
    current.set_audio(competing);
    ws.capture_source_preferences_context(&current);
    assert!(ws.execute(Command::Undo).is_err());
    assert!(ws.take_source_preferences_update().is_none());
    current.set_audio(original);
    ws.capture_source_preferences_context(&current);
    ws.execute(Command::Undo).unwrap();
    let restored = ws.take_source_preferences_update().unwrap();
    assert!(restored.snapshot().file().source_archive().is_none());
    assert!(ws.execute(Command::Redo).is_err());
}
#[test]
fn disk_unknown_metadata_survives_import_undo_redo_and_owned_conflict_is_atomic() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let path = root.path().join("prefs.json");
    std::fs::write(&path, r#"{"Version":1,"VendorNative":{"keep":7}}"#).unwrap();
    let file = File::open(&path).unwrap();
    let draft = ws
        .prepare_source_preferences(
            &file,
            br#"{"PreferencesAudio":{"MusicVolume":0.7}}"#,
            Platform::Mac,
        )
        .unwrap();
    ws.import_source_preferences(draft, &file, true).unwrap();
    let current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let mut disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    disk["GrooveAuthorSourceArchive"]["Vendor"] = 9.into();
    disk["VendorNative"]["new"] = 8.into();
    std::fs::write(&path, serde_json::to_vec(&disk).unwrap()).unwrap();
    ws.capture_source_preferences_context(&current);
    ws.execute(Command::Undo).unwrap();
    let current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk["VendorNative"]["new"], 8);
    assert_eq!(disk["GrooveAuthorSourceArchive"]["Vendor"], 9);
    assert!(disk["GrooveAuthorSourceArchive"].get("RawUtf8").is_none());
    ws.capture_source_preferences_context(&current);
    ws.execute(Command::Redo).unwrap();
    let current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let mut disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    disk["Audio"]["MusicVolume"] = 0.9.into();
    let bytes = serde_json::to_vec(&disk).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    ws.capture_source_preferences_context(&current);
    assert!(ws.execute(Command::Undo).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(ws.take_source_preferences_update().is_none());
}
#[test]
fn current_performance_ftue_and_scroll_modifier_owners_are_imported_with_defaults_and_raw_archive()
{
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let alternatives = vec![vec!["LeftControl"]; 24];
    let raw=serde_json::to_vec(&serde_json::json!({"PreferencesPerformance":{"ShowPerformanceWindow":true,"MaxFramesToDraw":123,"FrameMaxTime":"99","ExplicitFrameMaxTime":0.05,"PerformanceMonitorPaused":true},"LastCompletedFtueVersion":"0.1.0","FtueIndex":3,"PreferencesKeyBinds":{"ScrollZoom":alternatives}})).unwrap();
    let draft = ws
        .prepare_source_preferences(&file, &raw, Platform::Mac)
        .unwrap();
    assert_eq!(draft.target().performance().max_frames, 123);
    assert!(draft.target().performance().visible);
    assert_eq!(
        draft.target().performance().scale,
        deadsync_editor::performance::preferences::Scale::Unknown(99)
    );
    assert!(draft.target().ftue().completed());
    assert_eq!(
        draft
            .target()
            .key_bindings()
            .scroll_modifiers
            .get(deadsync_editor::key_binding_preferences::scroll_modifiers::Kind::Zoom)
            .alternatives()
            .len(),
        24
    );
    assert_eq!(draft.target().ftue().index, 3);
    assert!(
        draft
            .report()
            .items
            .iter()
            .any(|r| r.path == "PreferencesPerformance.MaxFramesToDraw")
    );
    ws.import_source_preferences(draft, &file, true).unwrap();
    let current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let draft = ws
        .prepare_source_preferences(&current, b"{}", Platform::Mac)
        .unwrap();
    assert_eq!(
        draft.target().performance(),
        deadsync_editor::performance::preferences::Preferences::default()
    );
    assert_eq!(
        draft.target().ftue(),
        &deadsync_editor::ftue::Preferences::default()
    );
}
#[test]
fn native_version_one_stays_strict_and_source_known_nonfinite_float_rejects_before_history() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let path = root.path().join("prefs.json");
    for raw in [
        b"{\"Version\":77}".as_slice(),
        b"{//comment\n\"Version\":1}",
    ] {
        std::fs::write(&path, raw).unwrap();
        assert!(File::open(&path).is_err());
    }
    let file = File::memory().unwrap();
    let revision = ws.file().session.revision();
    assert!(
        ws.prepare_source_preferences(
            &file,
            br#"{"PreferencesPerformance":{"ExplicitFrameMaxTime":1e999}}"#,
            Platform::Mac
        )
        .is_err()
    );
    assert_eq!(ws.file().session.revision(), revision);
}

#[test]
fn source_recent_postload_runs_once_at_import_and_native_reopen_keeps_chartless() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let path = root.path().join("recent.json");
    std::fs::write(&path, b"{\"Version\":1}").unwrap();
    let file = File::open(&path).unwrap();
    let raw=br#"{"RecentFiles":[{"FileName":"empty.ssc","ActiveCharts":[],"FocusedChartIndex":-1},{"FileName":"old.ssc","LastChartType":"dance_double","LastChartDifficultyType":"Hard"}]}"#;
    let draft = ws
        .prepare_source_preferences(&file, raw, Platform::Mac)
        .unwrap();
    assert_eq!(
        draft.target().recent_songs().entries[0]
            .song()
            .unwrap()
            .active
            .len(),
        1
    );
    assert_eq!(
        draft.target().recent_songs().entries[0]
            .song()
            .unwrap()
            .focused,
        Some(0)
    );
    assert_eq!(
        draft.target().recent_songs().entries[1]
            .song()
            .unwrap()
            .active[0]
            .steps_type,
        "dance-double"
    );
    ws.import_source_preferences(draft, &file, true).unwrap();
    let current = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    let mut song = current.recent_songs().entries[0].song().unwrap();
    song.active.clear();
    song.focused = None;
    let mut current = current;
    current.record_recent_song(&song).unwrap();
    current.save().unwrap();
    let reopened = File::open(&path).unwrap();
    let native = reopened.recent_songs().entries[0].song().unwrap();
    assert!(native.active.is_empty());
    assert_eq!(native.focused, None);
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["RecentFiles"][0]["ActiveCharts"], serde_json::json!([]));
    assert_eq!(raw["RecentFiles"][0]["FocusedChartIndex"], -1);
}

#[test]
fn imported_window_visibility_requires_explicit_native_panel_adaptation_review() {
    use deadsync_editor::audio_preferences::source_import::Disposition;
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let draft = ws
        .prepare_source_preferences(
            &file,
            br#"{"ShowLogWindow":true,"PreferencesPerformance":{"ShowPerformanceWindow":true}}"#,
            Platform::Mac,
        )
        .unwrap();
    for path in [
        "ShowLogWindow",
        "PreferencesPerformance.ShowPerformanceWindow",
    ] {
        let item = draft
            .report()
            .items
            .iter()
            .find(|item| item.path == path)
            .unwrap();
        assert_eq!(item.disposition, Disposition::Adapted);
        assert_eq!(item.effective, Some(serde_json::Value::Bool(true)));
        assert!(item.reason.starts_with("Source window visibility retained"));
    }
    assert!(draft.target().log_preferences().show);
    assert!(draft.report().requires_acknowledgement());
    assert!(ws.import_source_preferences(draft, &file, false).is_err());
}

#[test]
fn source_receptor_placement_is_consumed_and_coordinate_adaptation_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let mut ws = workspace(root.path());
    let file = File::memory().unwrap();
    let draft=ws.prepare_source_preferences(&file,br#"{"PreferencesReceptors":{"PositionX":900,"PositionY":123,"CenterHorizontally":false,"LockPositionX":true,"LockPositionY":true,"AllowChartMoveWhenPositionLocked":true}}"#,Platform::Mac).unwrap();
    let p = draft.target().receptors().placement;
    assert_eq!((p.x, p.y), (900, 123));
    assert!(p.lock_x && p.lock_y && p.allow_header);
    for field in ["PositionX", "PositionY", "AllowChartMoveWhenPositionLocked"] {
        assert!(draft.report().items.iter().any(|item| item.path
            == format!("PreferencesReceptors.{field}")
            && matches!(
                item.disposition,
                deadsync_editor::audio_preferences::source_import::Disposition::Adapted
            )));
    }
    let raw = draft.raw_source().to_vec();
    ws.import_source_preferences(draft, &file, true).unwrap();
    let installed = ws
        .take_source_preferences_update()
        .unwrap()
        .snapshot()
        .file()
        .clone();
    assert_eq!(installed.receptors().placement, p);
    assert_eq!(
        installed.source_archive().unwrap()["RawUtf8"]
            .as_str()
            .unwrap()
            .as_bytes(),
        raw
    );
}
