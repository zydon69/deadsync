use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    spacing::{Anchor, Speeds},
    spacing_index::Index,
    spacing_search::{self, Settings},
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path};
fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn section(row: f64, time: f64, order: u8) -> SpacingSection {
    SpacingSection {
        order,
        mapping: RateMapping::new(row, time, 0.0, 0.0, 1.0 / 96.0).unwrap(),
        tempo: 120.0,
        scroll: 1.0,
    }
}
#[test]
fn strict_active_selection_matches_original_findbest_and_real_fumen_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "baseline",
        "late-tempo-and-scroll",
        "late-negative-scroll",
        "late-tempo",
        "zero-scroll",
        "overlapping-warp-stop",
        "empty",
    ] {
        let request: Value = serde_json::from_slice(
            &fs::read(root.join(format!("spacing-index-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("spacing-index-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        if let Some(source) = request["RateSourceFixture"].as_str() {
            let source: Value =
                serde_json::from_slice(&fs::read(root.join(source)).unwrap()).unwrap();
            assert_eq!(request["Rates"], source);
        }
        let sections = request["Rates"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, r)| SpacingSection {
                order: i as u8,
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
        let index = Index::new(sections).unwrap();
        let queries = request["Queries"].as_array().unwrap();
        assert_eq!(queries.len(), expected.as_array().unwrap().len());
        for (q, e) in queries.iter().zip(expected.as_array().unwrap()) {
            let mode = match q["Mode"].as_str().unwrap() {
                "ConstantTime" => SpacingMode::ConstantTime,
                "ConstantRow" => SpacingMode::ConstantRow,
                "Variable" => SpacingMode::Variable,
                _ => panic!("Unknown mode"),
            };
            assert_eq!(
                index.active(mode, n(q, "Time"), n(q, "Row")).unwrap(),
                e["Index"].as_u64().map(|i| i as usize),
                "{name}: {q}"
            );
        }
    }
}
#[test]
fn automatic_lookup_composes_with_directional_search_and_preserves_exact_ties() {
    let source = b"#VERSION:0.83;#BPMS:0=120,2=240;#OFFSET:1234567.123456789;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let document = deadsync_editor::document::Document::parse(
        std::sync::Arc::<[u8]>::from(source.as_slice()),
        deadsync_editor::document::Format::Ssc,
    )
    .unwrap();
    let session = deadsync_editor::history::Session::new(document);
    let clock = deadsync_editor::playback::Clock::capture(&session, 0).unwrap();
    let captured = Index::new(clock.spacing_sections().collect()).unwrap();
    assert_eq!(clock.chart_time(96).unwrap(), 1.0);
    let selected = captured
        .active(
            SpacingMode::ConstantTime,
            clock.chart_time(96).unwrap(),
            96.0,
        )
        .unwrap()
        .unwrap();
    assert!(captured.sections()[selected].mapping.row() < 96.0);

    let index = Index::new(vec![
        section(0.0, 0.0, 0),
        section(48.0, 0.5, 1),
        section(48.0, 0.5, 2),
        section(96.0, 1.0, 1),
    ])
    .unwrap();
    assert_eq!(
        index.active(SpacingMode::ConstantRow, 0.5, 48.0).unwrap(),
        Some(0)
    );
    assert_eq!(
        index.active(SpacingMode::ConstantTime, 0.5, 48.0).unwrap(),
        Some(0)
    );
    assert_eq!(
        index.active(SpacingMode::ConstantRow, 0.5, 48.5).unwrap(),
        Some(2)
    );
    assert_eq!(
        index
            .active(SpacingMode::ConstantTime, 0.5001, 48.0)
            .unwrap(),
        Some(2)
    );
    let focal = Anchor {
        y: 160.0,
        time: 0.75,
        row: 72.0,
    };
    for mode in [
        SpacingMode::ConstantTime,
        SpacingMode::ConstantRow,
        SpacingMode::Variable,
    ] {
        for reverse in [false, true] {
            let settings = Settings {
                mode,
                reverse,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.2,
            };
            for target in [105.0, 160.0, 320.0] {
                assert_eq!(
                    index.find_location(focal, target, settings).unwrap(),
                    spacing_search::find_location(index.sections(), 2, focal, target, settings)
                        .unwrap()
                );
            }
        }
    }
}
#[test]
fn index_validates_original_clock_order_and_empty_or_nonfinite_queries() {
    assert!(Index::new(vec![section(48.0, 1.0, 1), section(0.0, 0.0, 0)]).is_err());
    assert!(Index::new(vec![section(0.0, 1.0, 0), section(48.0, 0.0, 1)]).is_err());
    let mut invalid = section(0.0, 0.0, 0);
    invalid.scroll = f64::NAN;
    assert!(Index::new(vec![invalid]).is_err());
    let index = Index::new(vec![]).unwrap();
    assert_eq!(
        index.active(SpacingMode::ConstantTime, 0.0, 0.0).unwrap(),
        None
    );
    assert!(
        index
            .active(SpacingMode::ConstantTime, f64::NAN, 0.0)
            .is_err()
    );
    let settings = Settings {
        mode: SpacingMode::ConstantRow,
        reverse: false,
        speeds: Speeds::default(),
        interpolated_scroll: 1.0,
        zoom: 1.0,
    };
    assert_eq!(
        index
            .find_location(
                Anchor {
                    y: 160.0,
                    time: 0.0,
                    row: 0.0
                },
                0.0,
                settings
            )
            .unwrap(),
        (0.0, 0.0)
    );
}
