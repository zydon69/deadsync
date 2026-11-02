//! Source action fidelity: unknown numeric enum dictionary entries are not reset.
use deadsync_editor::{
    audio_preferences::File,
    density_preferences,
    document::{Document, Format},
    history::Session,
    visual_preferences::{MiniMap, Snapshot},
};
use serde_json::{Value, json};
fn fixture() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/preference-restore-source.json")).unwrap()
}
fn stage<'a>(cases: &'a [Value], name: &str) -> &'a Value {
    cases.iter().find(|c| c["name"] == name).unwrap()
}
fn assert_file(file: &File, path: &std::path::Path, expected: &Value, density: bool) {
    let reopened = File::open(path).unwrap();
    if density {
        let p = density_preferences::Preferences::read(expected["density"].as_object().unwrap())
            .unwrap();
        assert_eq!(file.density(), p);
        assert_eq!(reopened.density(), p);
    } else {
        // Exercise the public persisted preference boundary rather than a
        // crate-private parser from this external workflow test.
        let expected_directory = tempfile::tempdir().unwrap();
        let expected_path = expected_directory.path().join("expected.json");
        std::fs::write(
            &expected_path,
            serde_json::to_vec(&json!({"Version":1,"MiniMap":expected["minimap"]})).unwrap(),
        )
        .unwrap();
        let expected_file = File::open(&expected_path).unwrap();
        assert_eq!(file.minimap(), expected_file.minimap());
        assert_eq!(reopened.minimap(), expected_file.minimap());
        assert_eq!(
            file.minimap_position(),
            deadsync_editor::minimap_position_preferences::Preferences::read(
                expected["minimap"].as_object().unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            file.minimap_content().unwrap(),
            reopened.minimap_content().unwrap()
        );
    }
    let raw: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(raw["Vendor"], json!({"preserved":7}));
    let (group, fields) = if density {
        (
            "DensityGraph",
            vec!["DensityGraphPositionOffsets", "DensityGraphWidthOffsets"],
        )
    } else {
        ("MiniMap", vec!["PositionOffsets"])
    };
    for field in fields {
        let source = if density {
            &expected["density"]
        } else {
            &expected["minimap"]
        };
        assert_eq!(raw[group][field]["99"], source[field]["99"]);
        assert_eq!(raw[group][field]["Vendor"], json!(55));
    }
}
#[test]
fn original_restore_actions_apply_undo_redo_and_reopen_keep_numeric_enum_entries() {
    let cases = fixture();
    for density in [false, true] {
        let prefix = if density { "density" } else { "minimap" };
        let initial = stage(&cases, &format!("{prefix}-before"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let mut raw = json!({"Version":1,"MiniMap":initial["minimap"],"DensityGraph":initial["density"],"Vendor":{"preserved":7}});
        for (group, field) in [
            ("MiniMap", "PositionOffsets"),
            ("DensityGraph", "DensityGraphPositionOffsets"),
            ("DensityGraph", "DensityGraphWidthOffsets"),
        ] {
            raw[group][field]["Vendor"] = json!(55);
        }
        std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let mut file = File::open(&path).unwrap();
        let mut session =
            Session::new(Document::parse(b"#TITLE:Restore;".as_slice(), Format::Sm).unwrap());
        let bytes = session.serialize().unwrap();
        if density {
            session
                .set_density_preferences(
                    file.density(),
                    density_preferences::Preferences::default(),
                    Some(&file),
                )
                .unwrap();
            file = session
                .take_density_preferences_update()
                .unwrap()
                .file
                .unwrap();
        } else {
            let before = MiniMap::capture(&file).unwrap();
            session
                .set_visual_preferences(
                    Snapshot::MiniMap(before),
                    Snapshot::MiniMap(before.restored()),
                    Some(&file),
                )
                .unwrap();
            file = session
                .take_visual_preferences_update()
                .unwrap()
                .file
                .unwrap();
        }
        assert_file(
            &file,
            &path,
            stage(&cases, &format!("{prefix}-apply")),
            density,
        );
        session.capture_source_preferences_context(&file);
        assert!(session.undo().unwrap());
        file = if density {
            session
                .take_density_preferences_update()
                .unwrap()
                .file
                .unwrap()
        } else {
            session
                .take_visual_preferences_update()
                .unwrap()
                .file
                .unwrap()
        };
        assert_file(
            &file,
            &path,
            stage(&cases, &format!("{prefix}-undo")),
            density,
        );
        session.capture_source_preferences_context(&file);
        assert!(session.redo().unwrap());
        file = if density {
            session
                .take_density_preferences_update()
                .unwrap()
                .file
                .unwrap()
        } else {
            session
                .take_visual_preferences_update()
                .unwrap()
                .file
                .unwrap()
        };
        assert_file(
            &file,
            &path,
            stage(&cases, &format!("{prefix}-reopen")),
            density,
        );
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), bytes);
    }
}
#[test]
fn original_is_using_defaults_ignores_numeric_unknown_dictionary_entries() {
    let cases = fixture();
    for prefix in ["minimap", "density"] {
        let after = stage(&cases, &format!("{prefix}-apply"));
        assert_eq!(after[&format!("{prefix}_defaults")], json!(true));
        if prefix == "minimap" {
            let p = deadsync_editor::minimap_position_preferences::Preferences::read(
                after["minimap"].as_object().unwrap(),
            )
            .unwrap();
            assert_eq!(p, Default::default());
        } else {
            assert_eq!(
                density_preferences::Preferences::read(after["density"].as_object().unwrap())
                    .unwrap(),
                Default::default()
            );
        }
    }
}
