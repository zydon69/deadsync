pub use deadsync_editor::{Error, minimap_navigation, minimap_projection, playback};
use deadsync_editor::minimap_viewport as viewport;
use viewport::{Axis, Geometry, Viewport, area_start};

#[test]
fn viewport_start_and_quantization_match_unchanged_pinned_csharp_methods() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-viewport.reference.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 160);
    for (index, case) in cases.iter().enumerate() {
        let input = &case["input"];
        let number = |key: &str| input[key].as_f64().unwrap();
        let start = area_start(
            number("FullStart"),
            number("FullEnd"),
            number("EditorStart"),
            number("EditorEnd"),
            number("MiniRange"),
        )
        .unwrap();
        let projection = minimap_projection::Projection {
            full_start: number("FullStart"),
            full_end: number("FullEnd"),
            editor_start: number("EditorStart"),
            editor_end: number("EditorEnd"),
            mini_start: start,
            mini_unquantized: start,
            mini_range: number("MiniRange"),
            bounds_y: 0,
            visible_height: input["VisibleHeight"].as_i64().unwrap() as i32,
            grabbed_percentage: 0.0,
            reverse: false,
            quantize: input["Quantize"].as_bool().unwrap(),
        }
        .initialized()
        .unwrap();
        for (key, actual) in [
            ("miniStart", projection.mini_start),
            ("unquantized", projection.mini_unquantized),
        ] {
            let expected = case["expected"][key].as_f64().unwrap();
            assert!(
                (actual - expected).abs() < 1e-10,
                "case {index} {key}: {actual} != {expected}"
            );
        }
    }
}
#[test]
fn real_viewport_bounds_and_reverse_preserve_exact_focal_coordinate() {
    let view = Viewport {
        top: 80.0,
        bottom: 400.0,
        focal: 180.0,
    };
    for reverse in [false, true] {
        let geometry = Geometry::build(
            Axis::Time,
            view,
            120.0,
            3.23456789,
            (0.0, 100.0),
            240.0,
            50,
            200,
            reverse,
            false,
        )
        .unwrap();
        let range = 320.0 / 120.0;
        assert!(
            (geometry.projection.editor_end - geometry.projection.editor_start - range).abs()
                < 1e-12
        );
        assert!((geometry.focused_coordinate() - 3.23456789).abs() < 1e-12);
        assert_eq!(geometry.projection.full_start, -range);
    }
    assert!(
        Viewport {
            top: 0.0,
            bottom: 0.0,
            focal: 0.0
        }
        .range(1.0)
        .is_err()
    );
}
#[test]
fn constant_row_uses_fractional_clock_and_does_not_invert_away_warp_positions() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let source=b"#VERSION:0.83;#BPMS:0=120;#WARPS:1=2;#OFFSET:0.126;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;";
    let session = Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap());
    let clock = playback::Clock::capture(&session, 0).unwrap();
    for row in [-0.125, 47.75, 48.5, 143.75, 160.125] {
        let geometry = Geometry::build(
            Axis::Row,
            Viewport {
                top: 80.0,
                bottom: 400.0,
                focal: 180.0,
            },
            2.0,
            row,
            (0.0, 192.0),
            24576.0,
            50,
            200,
            false,
            false,
        )
        .unwrap();
        let position = geometry.position(&clock).unwrap();
        assert!((position.row - row).abs() < 1e-12);
        assert!((position.chart_time - clock.fractional_chart_time(row).unwrap()).abs() < 1e-12);
        assert!((position.music_time - clock.fractional_music_time(row).unwrap()).abs() < 1e-12);
    }
}
