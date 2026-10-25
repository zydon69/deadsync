use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    spacing::{Rate, Speeds},
    spacing_index::Index,
    waveform_spacing::{self, ScrollMode},
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path};
fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn close(a: f64, b: f64) {
    assert!(
        a == b || a.to_bits().abs_diff(b.to_bits()) <= 2,
        "{a:?} != {b:?}"
    );
}
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor")
}
#[test]
fn all_waveform_policies_match_unchanged_original_method_before_zoom_and_reverse() {
    for (name, mode) in [
        ("constanttime", SpacingMode::ConstantTime),
        ("constantrow", SpacingMode::ConstantRow),
        ("variable", SpacingMode::Variable),
    ] {
        for suffix in ["", "-reverse"] {
            let name = format!("spacing-waveform-{name}{suffix}");
            let req: Value = serde_json::from_slice(
                &fs::read(root().join(format!("{name}.request.json"))).unwrap(),
            )
            .unwrap();
            let expected: Value = serde_json::from_slice(
                &fs::read(root().join(format!("{name}.expected.json"))).unwrap(),
            )
            .unwrap();
            let p = &req["Speeds"];
            let speeds = Speeds {
                time: n(p, "Time"),
                row: n(p, "Row"),
                variable: n(p, "Variable"),
            };
            let queries = req["Queries"].as_array().unwrap();
            assert_eq!(queries.len(), 75);
            for (q, e) in queries.iter().zip(expected.as_array().unwrap()) {
                let policy = match q["Policy"].as_str().unwrap() {
                    "CurrentTempo" => ScrollMode::CurrentTempo,
                    "CurrentTempoAndRate" => ScrollMode::CurrentTempoAndRate,
                    "MostCommonTempo" => ScrollMode::MostCommonTempo,
                    _ => panic!(),
                };
                let rate = Rate {
                    row: 0.0,
                    time: 0.0,
                    tempo: n(q, "Tempo"),
                    rows_per_second: n(q, "RowsPerSecond"),
                    seconds_per_row: 1.0 / n(q, "RowsPerSecond"),
                    scroll: n(q, "Scroll"),
                };
                close(
                    waveform_spacing::pixels_per_second(
                        mode,
                        policy,
                        rate,
                        speeds,
                        n(&req, "CommonTempo"),
                        n(q, "Interpolated"),
                    )
                    .unwrap(),
                    e.as_f64().unwrap(),
                );
            }
        }
    }
}
#[test]
fn common_tempo_matches_original_refresh_accounting_including_ties_and_final_rate_boundary() {
    for name in [
        "single",
        "rounding-history",
        "tie",
        "repeat",
        "last-rate",
        "late-first",
        "warp-stop",
    ] {
        let e: Value = serde_json::from_slice(
            &fs::read(root().join(format!("waveform-tempo-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let sections = e["Sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| SpacingSection {
                order: r["Order"].as_u64().unwrap() as u8,
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
        close(
            waveform_spacing::most_common_tempo(&index)
                .unwrap()
                .unwrap(),
            n(&e, "CommonTempo"),
        );
        if name == "tie" {
            assert_eq!(n(&e, "CommonTempo"), 120.0);
        }
        if name == "last-rate" {
            assert_eq!(n(&e, "CommonTempo"), 240.0);
        }
    }
    assert_eq!(
        waveform_spacing::most_common_tempo(&Index::new(vec![]).unwrap()).unwrap(),
        None
    );
}
#[test]
fn ignored_fields_stay_ignored_and_singular_outputs_are_explicit_errors() {
    let rate = Rate {
        row: 0.0,
        time: 0.0,
        tempo: 120.0,
        rows_per_second: 96.0,
        seconds_per_row: 1.0 / 96.0,
        scroll: 0.0,
    };
    let speeds = Speeds::default();
    for factor in [-2.0, 0.0, 2.0] {
        assert_eq!(
            waveform_spacing::pixels_per_second(
                SpacingMode::Variable,
                ScrollMode::CurrentTempoAndRate,
                rate,
                speeds,
                0.0,
                factor
            )
            .unwrap(),
            120.0
        );
    }
    assert_eq!(
        waveform_spacing::pixels_per_second(
            SpacingMode::Variable,
            ScrollMode::MostCommonTempo,
            Rate {
                tempo: f64::NAN,
                scroll: f64::NAN,
                ..rate
            },
            speeds,
            180.0,
            f64::NAN
        )
        .unwrap(),
        180.0
    );
    assert_eq!(
        waveform_spacing::pixels_per_second(
            SpacingMode::ConstantTime,
            ScrollMode::MostCommonTempo,
            rate,
            Speeds {
                row: f64::NAN,
                variable: f64::NAN,
                ..speeds
            },
            f64::NAN,
            f64::NAN
        )
        .unwrap(),
        300.0
    );
    assert!(
        waveform_spacing::pixels_per_second(
            SpacingMode::Variable,
            ScrollMode::MostCommonTempo,
            rate,
            speeds,
            0.0,
            1.0
        )
        .is_err()
    );
    assert!(
        waveform_spacing::pixels_per_second(
            SpacingMode::Variable,
            ScrollMode::CurrentTempoAndRate,
            rate,
            speeds,
            0.0,
            f64::NAN
        )
        .is_err()
    );
    assert!(
        waveform_spacing::pixels_per_second(
            SpacingMode::ConstantRow,
            ScrollMode::CurrentTempo,
            rate,
            Speeds {
                row: f64::MAX,
                ..speeds
            },
            0.0,
            1.0
        )
        .is_err()
    );
    assert_eq!(
        waveform_spacing::pixels_per_second(
            SpacingMode::Variable,
            ScrollMode::CurrentTempoAndRate,
            Rate {
                scroll: -f64::MAX,
                ..rate
            },
            speeds,
            0.0,
            f64::MAX
        )
        .unwrap(),
        120.0
    );
}
