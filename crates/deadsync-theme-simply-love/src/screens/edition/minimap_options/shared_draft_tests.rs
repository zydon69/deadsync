use super::*;
use deadsync_editor::{
    audio_preferences::File, minimap_viewport::Axis, visual_preferences::MiniMap,
};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
}
fn row(s: &mut State, target: usize) {
    let (mut current, count) = match s.panel {
        Some(Panel::MinimapSettings(i)) => (i, 8),
        Some(Panel::MinimapOptions(i)) => (i, 17),
        _ => panic!("minimap panel required"),
    };
    while current != target {
        press(s, KeyCode::ArrowDown);
        current = (current + 1) % count;
    }
}
fn fixture(disk: bool) -> (tempfile::TempDir, State, std::path::PathBuf) {
    crate::i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open(&song, 0).unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path, b"{\"Version\":1,\"UnknownOwner\":{\"Keep\":true}}").unwrap();
    let mut file = if disk {
        File::open(&path).unwrap()
    } else {
        File::memory().unwrap()
    };
    let mut viewport = file.minimap();
    viewport.set_visible_range(Axis::Time, 120.).unwrap();
    file.set_minimap(viewport);
    let mut content = file.minimap_content().unwrap();
    content.width = 88;
    content.max_notes = 4321;
    file.set_minimap_content(content).unwrap();
    if disk {
        file.save().unwrap();
        s.configure_audio_preferences(&path).unwrap();
    } else {
        s.audio_preferences_file = None;
        s.audio_preferences_path = None;
        s.configure_minimap_preferences(file.minimap());
        s.configure_minimap_position_preferences(file.minimap_position());
        s.configure_minimap_content_preferences(file.minimap_content().unwrap());
        s.source_preferences_import.memory = Some(file);
    }
    s.take_audio_commands();
    (root, s, path)
}
fn owned(s: &State) -> MiniMap {
    MiniMap::capture(s.application_preferences_owner().unwrap()).unwrap()
}
#[test]
fn physical_basic_advanced_basic_applies_one_complete_snapshot_and_global_undo_in_both_owner_modes()
{
    for disk in [false, true] {
        let (_root, mut s, path) = fixture(disk);
        let before = s.visual_minimap();
        let song = s.workspace.file().session.serialize().unwrap();
        let prefs = std::fs::read(&path).unwrap();
        s.open_minimap_settings();
        row(&mut s, 1);
        press(&mut s, KeyCode::ArrowLeft);
        row(&mut s, 3);
        press(&mut s, KeyCode::Enter);
        let viewport = s.minimap.draft.unwrap();
        row(&mut s, 7);
        press(&mut s, KeyCode::Enter);
        assert!(matches!(s.panel, Some(Panel::MinimapOptions(0))));
        assert_eq!(s.pending_minimap_visual().viewport, viewport);
        row(&mut s, 4);
        press(&mut s, KeyCode::ArrowRight);
        let content = s.minimap.options_draft.unwrap().content;
        row(&mut s, 16);
        press(&mut s, KeyCode::Enter);
        assert!(matches!(s.panel, Some(Panel::MinimapSettings(0))));
        assert_eq!(s.minimap.draft.unwrap(), viewport);
        assert_eq!(s.pending_minimap_visual().content, content);
        assert_eq!(s.visual_minimap(), before);
        assert_eq!(owned(&s), before);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            prefs,
            "page navigation cannot publish"
        );
        let expected = MiniMap {
            viewport,
            content,
            ..before
        };
        row(&mut s, 5);
        press(&mut s, KeyCode::Enter);
        assert!(s.panel.is_none(), "{}", s.status);
        assert_eq!(s.visual_minimap(), expected);
        assert_eq!(owned(&s), expected);
        assert!(!s.workspace.file().session.is_dirty());
        assert_eq!(s.workspace.file().session.serialize().unwrap(), song);
        if disk {
            assert_eq!(
                MiniMap::capture(&File::open(&path).unwrap()).unwrap(),
                expected
            );
            let json: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(json["UnknownOwner"]["Keep"], true);
        }
        s.command(Command::Undo);
        assert_eq!(s.visual_minimap(), before);
        assert_eq!(owned(&s), before);
        s.command(Command::Redo);
        assert_eq!(s.visual_minimap(), expected);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), song);
        assert!(s.take_audio_commands().is_empty());
    }
}
#[test]
fn physical_restore_across_pages_preserves_options_max_notes_and_cancel_discards_all_pending_groups()
 {
    for disk in [false, true] {
        for cancel_advanced in [false, true] {
            let (_root, mut s, path) = fixture(disk);
            let before = s.visual_minimap();
            let revision = s.workspace.file().session.revision();
            let bytes = std::fs::read(&path).unwrap();
            s.open_minimap_settings();
            row(&mut s, 1);
            press(&mut s, KeyCode::ArrowLeft);
            row(&mut s, 7);
            press(&mut s, KeyCode::Enter);
            row(&mut s, 12);
            press(&mut s, KeyCode::ArrowRight);
            let max = s.minimap.options_draft.unwrap().content.max_notes;
            row(&mut s, 13);
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.pending_minimap_visual().content.max_notes, max);
            row(&mut s, 16);
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.minimap.draft.unwrap(), Default::default());
            assert_eq!(s.pending_minimap_visual().content.max_notes, max);
            row(&mut s, 4);
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.pending_minimap_visual().content.max_notes, max);
            if cancel_advanced {
                row(&mut s, 7);
                press(&mut s, KeyCode::Enter);
                row(&mut s, 15);
            } else {
                row(&mut s, 6);
            }
            press(&mut s, KeyCode::Enter);
            assert!(s.panel.is_none());
            assert!(
                s.minimap.visual_draft.is_none()
                    && s.minimap.draft.is_none()
                    && s.minimap.options_draft.is_none()
            );
            assert_eq!(s.visual_minimap(), before);
            assert_eq!(owned(&s), before);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            // Restore then Apply from the advanced page publishes viewport and content together.
            s.open_minimap_settings();
            row(&mut s, 4);
            press(&mut s, KeyCode::Enter);
            row(&mut s, 7);
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.pending_minimap_visual().viewport, Default::default());
            row(&mut s, 14);
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.visual_minimap(), before.restored());
            s.command(Command::Undo);
            assert_eq!(s.visual_minimap(), before);
        }
    }
}
