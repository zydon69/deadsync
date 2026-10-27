//! Apply the accompanying transaction patch before moving this target into tests.
use deadsync_editor::{
    audio_preferences::File,
    minimap_position_preferences::{Mount, SelectMode},
};
#[test]
fn mount_edits_are_dirty_atomic_and_preserve_other_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(&path, r#"{"Version":1,"MiniMap":{"Unknown":7,"PositionOffsets":{"Future":93}},"Other":{"Keep":true}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.has_changes());
    let original = file.minimap_position();
    let mut draft = original;
    draft.mount = Mount::FocusedChartWithScaling;
    draft.select_mode = SelectMode::MoveToSelectedArea;
    draft.set_offset(draft.mount, -17);
    // Cancelling an uncommitted panel draft does not mutate File.
    assert_eq!(file.minimap_position(), original);
    assert!(!file.has_changes());
    file.set_minimap_position(draft);
    assert!(file.has_changes());
    assert!(file.save().unwrap());
    assert!(!file.save().unwrap());
    assert_eq!(File::open(&path).unwrap().minimap_position(), draft);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["MiniMap"]["Unknown"], 7);
    assert_eq!(json["MiniMap"]["PositionOffsets"]["Future"], 93);
    assert_eq!(json["Other"]["Keep"], true);
    assert_eq!(
        json["MiniMap"]["PositionOffsets"]["FocusedChartWithScaling"].as_i64(),
        Some(-17)
    );
}
#[test]
fn external_change_blocks_mount_save_without_overwriting_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    let mut file = File::open(&path).unwrap();
    let mut draft = file.minimap_position();
    draft.mount = Mount::RightSideOfWindow;
    file.set_minimap_position(draft);
    let external = b"external change";
    std::fs::write(&path, external).unwrap();
    assert!(matches!(
        file.save(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(std::fs::read(path).unwrap(), external);
    assert!(file.has_changes());
}
#[test]
fn content_and_mount_share_one_save_and_invalid_drafts_never_mutate_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"MiniMap":{"Unknown":7},"Options":{"Keep":42}}"#,
    )
    .unwrap();
    let mut file = File::open(&path).unwrap();
    let original = file.minimap_content().unwrap();
    let mut invalid = original;
    invalid.width = 2;
    assert!(file.set_minimap_content(invalid).is_err());
    assert_eq!(file.minimap_content().unwrap(), original);
    assert!(!file.has_changes());
    let mut content = original;
    content.width = 120;
    content.max_notes = 2048;
    content.visible = false;
    file.set_minimap_content(content).unwrap();
    let mut mount = file.minimap_position();
    mount.mount = Mount::RightSideOfWindow;
    file.set_minimap_position(mount);
    file.save().unwrap();
    let reopened = File::open(&path).unwrap();
    assert_eq!(reopened.minimap_content().unwrap(), content);
    assert_eq!(reopened.minimap_position(), mount);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json["MiniMap"]["Unknown"], 7);
    assert_eq!(json["Options"]["Keep"], 42);
    assert_eq!(
        json["Options"]["MiniMapMaxNotesToDraw"].as_u64(),
        Some(2048)
    );
    assert!(json["MiniMap"]["MiniMapWidth"].is_u64());
}
#[test]
fn shared_options_merge_preserves_staged_color_arc_snap_and_receptors() {
    use deadsync_editor::minimap_palette::ColorMethod;
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(&path, r#"{"Version":1,"Options":{"Keep":42,"StepColorMethodValue":"Stepmania"},"NoteColor":{"Future":19}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut colors = file.note_colors_arc().as_ref().clone();
    colors.step_color_method = ColorMethod::Beat;
    let colors = Arc::new(colors);
    file.set_note_colors(Arc::clone(&colors)).unwrap();
    let mut content = file.minimap_content().unwrap();
    content.max_notes = 731;
    file.set_minimap_content(content).unwrap();
    let mut mount = file.minimap_position();
    mount.mount = Mount::FocusedChartWithScaling;
    file.set_minimap_position(mount);
    let receptors = file.receptors();
    let snap = file.snap();
    let before_save = file.clone();
    assert!(Arc::ptr_eq(&before_save.note_colors_arc(), &colors));
    file.save().unwrap();
    assert!(!file.has_changes());
    assert!(Arc::ptr_eq(&file.note_colors_arc(), &colors));
    let reopened = File::open(&path).unwrap();
    assert_eq!(
        reopened.note_colors_arc().step_color_method,
        ColorMethod::Beat
    );
    assert_eq!(reopened.minimap_content().unwrap(), content);
    assert_eq!(reopened.minimap_position(), mount);
    assert_eq!(reopened.receptors(), receptors);
    assert_eq!(reopened.snap(), snap);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json["Options"]["StepColorMethodValue"], "Beat");
    assert_eq!(json["Options"]["MiniMapMaxNotesToDraw"], 731);
    assert_eq!(json["Options"]["Keep"], 42);
    assert_eq!(json["NoteColor"]["Future"], 19);
}
