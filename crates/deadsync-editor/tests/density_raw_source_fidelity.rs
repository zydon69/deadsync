use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    density::{
        colors,
        layout::{self, Focus, Input, Orientation, Rect},
    },
    density_preferences::{Position, Preferences},
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
use serde_json::{Value, json};
fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/density-raw-source.json")).unwrap()
}
fn array(v: &Value) -> [u8; 4] {
    std::array::from_fn(|i| v[i].as_u64().unwrap() as u8)
}
#[test]
fn real_density_models_raw_import_and_unmodified_source_bounds_match() {
    let current = File::memory().unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Density;".as_slice(), Format::Sm).unwrap());
    for case in cases() {
        assert_eq!(case["accepted"], json!(true));
        let draft = Draft::prepare(
            &mut session,
            None,
            &current,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        )
        .unwrap();
        let p = draft.target().density();
        let expected = Preferences::read(
            case["value"]["PreferencesDensityGraph"]
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(p, expected, "{}", case["name"]);
        let input: Value = serde_json::from_str(case["raw"].as_str().unwrap()).unwrap();
        for key in input["PreferencesDensityGraph"].as_object().unwrap().keys() {
            assert!(
                draft
                    .report()
                    .items
                    .iter()
                    .any(|row| row.path == format!("PreferencesDensityGraph.{key}")
                        && row.disposition == Disposition::Applied),
                "{} {key}",
                case["name"]
            );
        }
        for geometry in case["bounds"].as_array().unwrap() {
            let mut p = p;
            p.position = Position::ALL
                .into_iter()
                .find(|p| p.name() == geometry["position"])
                .unwrap();
            let b = layout::bounds(
                p,
                Input {
                    chart_area: Rect {
                        x: 10,
                        y: 20,
                        width: 1280,
                        height: 720,
                    },
                    header: 40,
                    focus: geometry["focused"].as_bool().unwrap().then_some(Focus {
                        lanes_start: 100,
                        lanes_end: 500,
                        misc_end: 600,
                        misc_scaled_end: 650,
                    }),
                    dpi_scale: geometry["dpi"].as_f64().unwrap(),
                },
            )
            .unwrap();
            assert_eq!(
                b.rect,
                Rect {
                    x: geometry["x"].as_i64().unwrap() as i32,
                    y: geometry["y"].as_i64().unwrap() as i32,
                    width: geometry["width"].as_i64().unwrap() as i32,
                    height: geometry["height"].as_i64().unwrap() as i32
                },
                "{} {:?}",
                case["name"],
                geometry
            );
            assert_eq!(
                b.orientation,
                if geometry["orientation"] == "Vertical" {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                }
            );
        }
    }
}
#[test]
fn real_monogame_vector4_constructor_and_fumen_byte_interpolation_match_exactly() {
    for case in cases() {
        let p = Preferences::read(
            case["value"]["PreferencesDensityGraph"]
                .as_object()
                .unwrap(),
        )
        .unwrap();
        let palette = colors::Palette::new(p.low, p.high, p.background);
        assert_eq!(palette.low, array(&case["low"]));
        assert_eq!(palette.high, array(&case["high"]));
        assert_eq!(palette.background, array(&case["background"]));
        for c in case["colors"].as_array().unwrap() {
            let t = c["t"].as_f64().unwrap() as f32;
            assert_eq!(
                colors::interpolate(palette.low, palette.high, t),
                array(&c["bytes"])
            );
            assert_eq!(palette.color(t), colors::rgba(array(&c["bytes"])));
        }
    }
}
#[test]
fn raw_density_values_save_restore_undo_redo_and_reopen_without_slider_clamping() {
    for case in cases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let before = Preferences::read(
            case["value"]["PreferencesDensityGraph"]
                .as_object()
                .unwrap(),
        )
        .unwrap();
        let mut fields = serde_json::Map::new();
        before.write(&mut fields).unwrap();
        fields.insert("Vendor".into(), json!({"preserved":7}));
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({"Version":1,"DensityGraph":fields})).unwrap(),
        )
        .unwrap();
        let mut file = File::open(&path).unwrap();
        assert_eq!(file.density(), before);
        let mut session =
            Session::new(Document::parse(b"#TITLE:raw;".as_slice(), Format::Sm).unwrap());
        let bytes = session.serialize().unwrap();
        if before == Preferences::default() {
            assert!(
                !session
                    .set_density_preferences(before, before, Some(&file))
                    .unwrap()
            );
            continue;
        }
        session
            .set_density_preferences(before, Preferences::default(), Some(&file))
            .unwrap();
        file = session
            .take_density_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.density(), Preferences::default());
        session.capture_source_preferences_context(&file);
        session.undo().unwrap();
        file = session
            .take_density_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.density(), before);
        assert_eq!(File::open(&path).unwrap().density(), before);
        session.capture_source_preferences_context(&file);
        session.redo().unwrap();
        file = session
            .take_density_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.density(), Preferences::default());
        assert_eq!(File::open(&path).unwrap().density(), Preferences::default());
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["DensityGraph"]["Vendor"], json!({"preserved":7}));
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), bytes);
    }
}
