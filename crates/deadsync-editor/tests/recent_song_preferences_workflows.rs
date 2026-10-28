//! Install after recent models and common preferences patch.
use deadsync_editor::{audio_preferences::File, minimap_palette::ColorMethod, recent_songs::Song};
use std::sync::Arc;
fn song() -> Song {
    Song {
        file: "/Pack/Été/song.ssc".into(),
        title: Some("Été".into()),
        pack: None,
        spacing_zoom: 1.25,
        chart_position: 160.5,
        active: vec![deadsync_editor::recent_songs::SavedChart {
            steps_type: "dance-single".into(),
            difficulty: 4,
            rating: 9,
            name: Some(String::new()),
            description: Some(String::new()),
        }],
        focused: Some(0),
    }
}
#[test]
fn recent_transaction_merges_with_staged_colors_and_minimap_without_losing_arcs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"Unknown":42,"RecentFilesHistorySize":3},"RecentFiles":[]}"#,
    )
    .unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.has_changes());
    let mut colors = file.note_colors_arc().as_ref().clone();
    colors.step_color_method = ColorMethod::Beat;
    let colors = Arc::new(colors);
    file.set_note_colors(Arc::clone(&colors)).unwrap();
    let mut minimap = file.minimap_content().unwrap();
    minimap.max_notes = 731;
    file.set_minimap_content(minimap).unwrap();
    file.record_recent_song(&song()).unwrap();
    assert!(file.has_changes());
    let cloned = file.clone();
    assert_eq!(cloned.recent_songs(), file.recent_songs());
    assert!(Arc::ptr_eq(&cloned.note_colors_arc(), &colors));
    assert!(file.save().unwrap());
    assert!(!file.has_changes());
    assert!(!file.save().unwrap());
    let reopened = File::open(&path).unwrap();
    assert_eq!(reopened.recent_songs().entries.len(), 1);
    assert_eq!(reopened.recent_songs().entries[0].song().unwrap(), song());
    assert_eq!(
        reopened.note_colors_arc().step_color_method,
        ColorMethod::Beat
    );
    assert_eq!(reopened.minimap_content().unwrap(), minimap);
    assert!(Arc::ptr_eq(&file.note_colors_arc(), &colors));
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json["Options"]["Unknown"], 42);
    assert_eq!(json["Options"]["StepColorMethodValue"], "Beat");
    assert_eq!(json["Options"]["MiniMapMaxNotesToDraw"], 731);
    assert_eq!(json["Options"]["RecentFilesHistorySize"], 3);
}
#[test]
fn changed_preference_file_blocks_recent_save_and_keeps_staged_history_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let mut file = File::open(&path).unwrap();
    file.record_recent_song(&song()).unwrap();
    std::fs::write(&path, b"external preference change").unwrap();
    assert!(matches!(
        file.save(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert!(file.has_changes());
    assert_eq!(std::fs::read(path).unwrap(), b"external preference change");
    assert_eq!(file.recent_songs().entries.len(), 1);
}
#[test]
fn invalid_history_size_does_not_mutate_settings_or_create_preference_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let mut file = File::open(&path).unwrap();
    let before = file.recent_songs().clone();
    let mut invalid = before.clone();
    invalid.maximum = usize::MAX;
    assert!(file.set_recent_songs(invalid).is_err());
    assert_eq!(file.recent_songs(), &before);
    assert!(!file.has_changes());
    assert!(!path.exists());
}
