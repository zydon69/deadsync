use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    minimap_palette::{Family, Palette},
    note_color_palette,
};
use serde_json::Value;
use std::sync::Arc;
fn cases() -> Value {
    serde_json::from_str(include_str!("fixtures/note-color-raw-source.json")).unwrap()
}
fn session() -> Session {
    Session::new(Document::parse(b"#TITLE:Palette;".as_slice(), Format::Sm).unwrap())
}
fn bytes(v: &Value) -> [u8; 4] {
    (v.as_u64().unwrap() as u32).to_le_bytes()
}
#[test]
fn real_raw_models_are_applied_and_both_cached_consumers_equal_original_source() {
    let cases = cases();
    for case in cases["cases"].as_array().unwrap() {
        let file = File::memory().unwrap();
        let mut s = session();
        let draft = Draft::prepare(
            &mut s,
            None,
            &file,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        assert!(
            draft
                .report()
                .items
                .iter()
                .filter(|r| r.path.starts_with("PreferencesMultiplayer."))
                .all(|r| r.disposition != Disposition::Retained),
            "{}",
            case["name"]
        );
        let p = draft.target().note_colors_arc();
        let (mut raw, mut options) = (serde_json::Map::new(), serde_json::Map::new());
        p.write(&mut raw, &mut options).unwrap();
        let expected = &case["value"]["PreferencesMultiplayer"];
        for (key, value) in raw.iter() {
            if let Some(want) = expected.get(key) {
                if want.is_number() {
                    assert_eq!(
                        value.as_f64().unwrap() as f32,
                        want.as_f64().unwrap() as f32,
                        "{key}"
                    );
                } else if let Some(v) = want.as_object() {
                    for (c, want) in v {
                        assert_eq!(
                            value[c].as_f64().unwrap() as f32,
                            want.as_f64().unwrap() as f32,
                            "{key}.{c}"
                        );
                    }
                }
            }
        }
        let ui = Palette::new(&p.colors).unwrap();
        let main = note_color_palette::Palette::capture(p).unwrap();
        for r in case["lookups"].as_array().unwrap() {
            let player = r["player"].as_i64().unwrap();
            let ordinary = player < 0;
            let player = player.max(0) as u32;
            let row = r["row"].as_u64().unwrap() as u8;
            let selected = r["selected"].as_bool().unwrap();
            let held = r["held"].as_bool().unwrap();
            let layout = if ordinary {
                "dance-single"
            } else {
                "dance-routine"
            };
            let f = main
                .fills(layout, row, 0, player, selected, false, held, true)
                .unwrap();
            if !held {
                assert_eq!(f.head, bytes(&r["color"]), "{}", case["name"]);
                assert_eq!(
                    main.mine(layout, player, selected).unwrap(),
                    bytes(&r["mine"])
                );
            }
            assert_eq!(f.start, bytes(&r["hold"]));
            assert_eq!(
                main.fills(layout, row, 0, player, selected, true, held, true)
                    .unwrap()
                    .start,
                bytes(&r["roll"])
            );
            assert_eq!(
                ui.colors(
                    if ordinary {
                        Family::Row
                    } else {
                        Family::RoutineRow
                    },
                    row,
                    0,
                    player,
                    selected
                )
                .unwrap()
                .head,
                r["ui"].as_u64().unwrap() as u32
            );
        }
        for r in case["pump"].as_array().unwrap() {
            let player = r["player"].as_i64().unwrap();
            let ordinary = player < 0;
            let player = player.max(0) as u32;
            let lane = r["lane"].as_u64().unwrap() as u16;
            let selected = r["selected"].as_bool().unwrap();
            let held = r["held"].as_bool().unwrap();
            let layout = if ordinary {
                "pump-single"
            } else {
                "pump-routine"
            };
            let f = main
                .fills(layout, 0, lane, player, selected, false, held, true)
                .unwrap();
            if !held {
                assert_eq!(f.head, bytes(&r["color"]));
            }
            assert_eq!(f.start, bytes(&r["hold"]));
            assert_eq!(
                ui.colors(
                    Family::PumpLane {
                        start_index: 0,
                        routine: !ordinary
                    },
                    0,
                    lane,
                    player,
                    selected
                )
                .unwrap()
                .head,
                r["ui"].as_u64().unwrap() as u32
            );
        }
    }
}
#[test]
fn imported_raw_palette_restore_undo_redo_keeps_clean_memory_and_full_raw_values() {
    for case in cases()["cases"].as_array().unwrap().iter().skip(1) {
        let mut s = session();
        let file = File::memory().unwrap();
        let draft = Draft::prepare(
            &mut s,
            None,
            &file,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        s.import_source_preferences(draft, &file, None, true)
            .unwrap();
        let mut file = s
            .take_source_preferences_update()
            .unwrap()
            .snapshot()
            .file()
            .clone();
        let before = file.note_colors_arc();
        let after = Arc::new(deadsync_editor::note_color_preferences::Preferences::default());
        let song = s.serialize().unwrap();
        assert!(
            s.set_note_color_preferences(before.clone(), after.clone(), Some(&file))
                .unwrap()
        );
        file = s
            .take_note_color_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.note_colors_arc(), after);
        s.capture_source_preferences_context(&file);
        s.undo().unwrap();
        file = s
            .take_note_color_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.note_colors_arc(), before);
        s.capture_source_preferences_context(&file);
        s.redo().unwrap();
        file = s
            .take_note_color_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.note_colors_arc(), after);
        assert!(!file.has_changes());
        assert_eq!(s.serialize().unwrap(), song);
    }
}

#[test]
fn raw_palette_disk_roundtrip_preserves_unknown_fields_and_ui_edits_only_touched_components() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Vendor":73,"NoteColor":{"Future":{"keep":7}}}"#,
    )
    .unwrap();
    for case in cases()["cases"].as_array().unwrap() {
        let mut s = session();
        let draft = Draft::prepare(
            &mut s,
            None,
            &File::memory().unwrap(),
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        let source = draft.target().note_colors_arc();
        let mut file = File::open(&path).unwrap();
        file.set_note_colors(source.clone()).unwrap();
        file.save().unwrap();
        assert_eq!(File::open(&path).unwrap().note_colors_arc(), source);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["Vendor"], 73);
        assert_eq!(raw["NoteColor"]["Future"]["keep"], 7);
        let ui = deadsync_editor::note_color_inspector::Draft::capture(source.clone()).unwrap();
        assert!(!ui.changed());
        assert_eq!(ui.staged(), source);
    }
    let mut raw = deadsync_editor::note_color_preferences::Preferences::default();
    raw.colors.subdivisions[0] = [1e20, -1e20, 2.];
    raw.selection_multiplier = -1.;
    raw.colors.routine_alpha = 12.;
    let mut ui = deadsync_editor::note_color_inspector::Draft::capture(Arc::new(raw)).unwrap();
    ui.select(0).unwrap();
    ui.adjust(0, true, false).unwrap();
    assert_eq!(ui.staged().colors.subdivisions[0], [1., -1e20, 2.]);
    assert_eq!(ui.staged().selection_multiplier, -1.);
    assert_eq!(ui.staged().colors.routine_alpha, 12.);
}
