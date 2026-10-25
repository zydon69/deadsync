use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    spacing::{Anchor, Speeds},
    spacing_search::{Settings, find_location},
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path};
fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn close(a: f64, b: f64) {
    assert!(
        a == b || (a.is_finite() && b.is_finite() && a.to_bits().abs_diff(b.to_bits()) <= 2),
        "{a:?} != {b:?}"
    );
}
#[test]
fn original_active_chart_search_matches_across_sections_modes_directions_and_speed_signs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for (name, mode) in [
        ("constanttime", SpacingMode::ConstantTime),
        ("constantrow", SpacingMode::ConstantRow),
        ("variable", SpacingMode::Variable),
    ] {
        for sign in ["normal", "negative", "zero"] {
            for suffix in ["", "-reverse"] {
                let name = format!("spacing-search-{name}-{sign}{suffix}");
                let request: Value = serde_json::from_slice(
                    &fs::read(root.join(format!("{name}.request.json"))).unwrap(),
                )
                .unwrap();
                let expected: Value = serde_json::from_slice(
                    &fs::read(root.join(format!("{name}.expected.json"))).unwrap(),
                )
                .unwrap();
                let rate_source: Value = serde_json::from_slice(
                    &fs::read(root.join(request["RateSourceFixture"].as_str().unwrap())).unwrap(),
                )
                .unwrap();
                assert_eq!(request["Rates"], rate_source);
                let sections: Vec<_> = request["Rates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| SpacingSection {
                        order: 0,
                        mapping: RateMapping::new(
                            n(r, "Row"),
                            n(r, "Time"),
                            n(r, "Warp"),
                            n(r, "Stop"),
                            n(r, "SecondsPerRow"),
                        )
                        .unwrap(),
                        tempo: n(r, "Tempo"),
                        scroll: n(r, "Scroll"),
                    })
                    .collect();
                let s = &request["Speeds"];
                let settings = Settings {
                    mode,
                    reverse: request["Reverse"].as_bool().unwrap(),
                    speeds: Speeds {
                        time: n(s, "Time"),
                        row: n(s, "Row"),
                        variable: n(s, "Variable"),
                    },
                    interpolated_scroll: n(&request, "Interpolated"),
                    zoom: n(&request, "Zoom"),
                };
                let queries = request["Queries"].as_array().unwrap();
                assert_eq!(queries.len(), expected.as_array().unwrap().len());
                for (q, e) in queries.iter().zip(expected.as_array().unwrap()) {
                    let f = &q["Focal"];
                    let focal = Anchor {
                        y: n(f, "Y"),
                        time: n(f, "Time"),
                        row: n(f, "Row"),
                    };
                    let result = find_location(
                        &sections,
                        q["Active"].as_u64().unwrap() as usize,
                        focal,
                        n(q, "Target"),
                        settings,
                    );
                    match (e["Time"].as_f64(), e["Row"].as_f64()) {
                        (Some(time), Some(row)) => {
                            let actual =
                                result.unwrap_or_else(|error| panic!("{name} query {q}: {error}"));
                            close(actual.0, time);
                            close(actual.1, row);
                        }
                        _ => assert!(result.is_err(), "{name} nonfinite source result {e}"),
                    }
                }
            }
        }
    }
}
#[test]
fn empty_focal_and_invalid_inputs_have_explicit_outcomes() {
    let settings = Settings {
        mode: SpacingMode::Variable,
        reverse: false,
        speeds: Speeds::default(),
        interpolated_scroll: 0.0,
        zoom: 1.0,
    };
    let focal = Anchor {
        y: 160.0,
        time: 2.0,
        row: 192.0,
    };
    assert_eq!(
        find_location(&[], 0, focal, 0.0, settings).unwrap(),
        (0.0, 0.0)
    );
    let section = SpacingSection {
        order: 1,
        mapping: RateMapping::new(0.0, 0.0, 0.0, 0.0, 1.0 / 96.0).unwrap(),
        tempo: 120.0,
        scroll: 1.0,
    };
    assert_eq!(
        find_location(&[section], 0, focal, 160.0, settings).unwrap(),
        (2.0, 192.0)
    );
    assert!(find_location(&[section], 1, focal, 160.0, settings).is_err());
    assert!(find_location(&[section], 0, focal, f64::NAN, settings).is_err());
    assert!(find_location(&[section], 0, focal, 161.0, settings).is_err());
    let invalid = Settings {
        zoom: f64::INFINITY,
        ..settings
    };
    assert!(find_location(&[section], 0, focal, 161.0, invalid).is_err());
}
