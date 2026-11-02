use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    visual_preferences::{MiniMap, Snapshot},
};
#[test]
fn both_visual_owners_publish_clean_memory_baselines_across_save_undo_redo() {
    for mini in [false, true] {
        let mut session =
            Session::new(Document::parse(b"#TITLE:Unchanged;".as_slice(), Format::Sm).unwrap());
        let file = File::memory().unwrap();
        let (before, after) = if mini {
            let p = MiniMap {
                viewport: file.minimap(),
                position: file.minimap_position(),
                content: file.minimap_content().unwrap(),
            };
            let mut changed = p;
            changed.content.width += 1;
            (Snapshot::MiniMap(p), Snapshot::MiniMap(changed))
        } else {
            let p = file.waveform();
            let mut changed = p;
            changed.set_channel_width(0.123456).unwrap();
            (Snapshot::Waveform(p), Snapshot::Waveform(changed))
        };
        assert!(
            session
                .set_visual_preferences(before, after, Some(&file))
                .unwrap()
        );
        let update = session.take_visual_preferences_update().unwrap();
        assert_eq!(update.preferences, after);
        let mut file = update.file.unwrap();
        assert!(!file.has_changes());
        session.capture_source_preferences_context(&file);
        session.undo().unwrap();
        let update = session.take_visual_preferences_update().unwrap();
        assert_eq!(update.preferences, before);
        file = update.file.unwrap();
        assert!(!file.has_changes());
        session.capture_source_preferences_context(&file);
        session.redo().unwrap();
        let update = session.take_visual_preferences_update().unwrap();
        assert_eq!(update.preferences, after);
        file = update.file.unwrap();
        assert!(!file.has_changes());
        assert_eq!(session.serialize().unwrap(), b"#TITLE:Unchanged;");
    }
}
