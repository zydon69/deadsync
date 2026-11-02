use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    minimap_preferences::VariableSpacing,
    minimap_viewport::Axis,
    visual_preferences::{MiniMap, Snapshot},
};
#[test]
fn real_original_models_zero_uint_ranges_and_raw_spacing_are_consumed() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/minimap_raw_source.expected.json")).unwrap();
    let current = File::memory().unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:minimap;".as_slice(), Format::Sm).unwrap());
    for case in cases.as_array().unwrap() {
        assert_eq!(case["accepted"], true);
        let draft = Draft::prepare(
            &mut session,
            None,
            &current,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        let p = draft.target().minimap();
        let expected = &case["minimap"];
        let spacing = match expected["MiniMapSpacingModeForVariable"].as_str().unwrap() {
            "ConstantTime" => VariableSpacing::ConstantTime,
            "ConstantRow" => VariableSpacing::ConstantRow,
            "Variable" => VariableSpacing::Variable,
            "99" => VariableSpacing::Unknown(99),
            other => panic!("unexpected source mode {other}"),
        };
        assert_eq!(p.variable_spacing(), spacing, "{}", case["name"]);
        assert_eq!(p.variable_axis(), spacing.axis());
        for (axis, key) in [
            (Axis::Time, "MiniMapVisibleTimeRange"),
            (Axis::Row, "MiniMapVisibleRowRange"),
        ] {
            assert_eq!(
                p.visible_range(axis),
                expected[key].as_u64().unwrap() as f64
            );
        }
        assert!(!draft.report().items.iter().any(|row| row.path
            == "PreferencesMiniMap.MiniMapSpacingModeForVariable"
            && row.disposition == Disposition::Retained));
        let mut file = File::memory().unwrap();
        file.set_minimap(p);
        assert_eq!(file.minimap(), p);
    }
}
#[test]
fn global_restore_changes_raw_variable_even_when_effective_axis_was_already_time_and_undo_reopens()
{
    for raw in ["Variable", "99"] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("minimap.json");
        std::fs::write(&path,format!("{{\"Version\":1,\"Vendor\":{{\"keep\":7}},\"MiniMap\":{{\"MiniMapSpacingModeForVariable\":\"{raw}\",\"MiniMapVisibleTimeRange\":0,\"MiniMapVisibleRowRange\":0}}}}" )).unwrap();
        let mut file = File::open(&path).unwrap();
        let before = MiniMap::capture(&file).unwrap();
        assert_eq!(before.viewport.variable_axis(), Axis::Time);
        let after = before.restored();
        assert_ne!(before, after);
        let mut session =
            Session::new(Document::parse(b"#TITLE:minimap;".as_slice(), Format::Sm).unwrap());
        let bytes = session.serialize().unwrap();
        session
            .set_visual_preferences(
                Snapshot::MiniMap(before),
                Snapshot::MiniMap(after),
                Some(&file),
            )
            .unwrap();
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(
            file.minimap().variable_spacing(),
            VariableSpacing::ConstantTime
        );
        assert_eq!(File::open(&path).unwrap().minimap(), after.viewport);
        session.capture_source_preferences_context(&file);
        assert!(session.undo().unwrap());
        file = session
            .take_visual_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(File::open(&path).unwrap().minimap(), before.viewport);
        session.capture_source_preferences_context(&file);
        assert!(session.redo().unwrap());
        assert_eq!(
            session
                .take_visual_preferences_update()
                .unwrap()
                .file
                .unwrap()
                .minimap(),
            after.viewport
        );
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), bytes);
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted["Vendor"]["keep"], 7);
    }
}
