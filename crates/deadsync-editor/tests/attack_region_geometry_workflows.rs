use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    spacing::{Anchor, Speeds},
    spacing_geometry::{Cursor, Frame},
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};
fn index() -> Index {
    Index::new(
        [(0.0, 1.0), (96.0, -1.0), (192.0, 0.0), (288.0, 1.0)]
            .into_iter()
            .map(|(row, scroll)| SpacingSection {
                order: 13,
                mapping: RateMapping::new(row, row / 96.0, 0.0, 0.0, 1.0 / 96.0).unwrap(),
                tempo: 120.0,
                scroll,
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn duration_path_retains_scroll_turns_frozen_pieces_reverse_and_viewport_clipping() {
    let index = index();
    for reverse in [false, true] {
        let settings = Settings {
            mode: SpacingMode::Variable,
            speeds: Speeds {
                time: 100.0,
                row: 1.0,
                variable: 240.0,
            },
            reverse,
            interpolated_scroll: 1.0,
            zoom: 1.0,
        };
        let cursor = Cursor::begin(
            &index,
            Anchor {
                row: 0.0,
                time: 0.0,
                y: 150.0,
            },
            settings,
        )
        .unwrap()
        .unwrap();
        let frame = Frame::capture(cursor).unwrap();
        let visible = frame.visible_rows(100.0, 200.0, 480).unwrap();
        let bands = frame
            .region_segments(0.0, 384.0, &visible, 100.0, 200.0, 16)
            .unwrap();
        assert!(bands.len() >= 3, "{bands:?}");
        assert!(
            bands
                .iter()
                .all(|&(a, b)| a >= 100.0 && a <= b && b <= 200.0)
        );
        assert!(bands.iter().any(|&(a, b)| a == b));
        // The whole path bends away from and back to the same endpoint height.
        let endpoints = (frame.row_y(0.0, 6).unwrap(), frame.row_y(192.0, 6).unwrap());
        assert_eq!(endpoints.0, endpoints.1);
        assert!(bands.iter().any(|&(a, b)| b - a > 1.0));
        assert_eq!(
            frame
                .region_segments(0.0, 384.0, &visible, 100.0, 200.0, 1)
                .unwrap()
                .len(),
            1
        );
        assert!(
            frame
                .region_segments(f64::NAN, 1.0, &visible, 100.0, 200.0, 1)
                .is_err()
        );
        assert!(
            frame
                .region_segments(20.0, 1.0, &visible, 100.0, 200.0, 1)
                .is_err()
        );
    }
}

#[test]
fn duration_can_begin_before_the_first_timing_rate_without_losing_its_first_segment() {
    let index = Index::new(vec![SpacingSection {
        order: 1,
        mapping: RateMapping::new(48.0, 0.0, 0.0, 0.0, 1.0 / 96.0).unwrap(),
        tempo: 120.0,
        scroll: 1.0,
    }])
    .unwrap();
    let settings = Settings {
        mode: SpacingMode::Variable,
        speeds: Speeds {
            time: 100.0,
            row: 1.0,
            variable: 240.0,
        },
        reverse: false,
        interpolated_scroll: 1.0,
        zoom: 1.0,
    };
    let cursor = Cursor::begin(
        &index,
        Anchor {
            row: 0.0,
            time: -0.5,
            y: 150.0,
        },
        settings,
    )
    .unwrap()
    .unwrap();
    let frame = Frame::capture(cursor).unwrap();
    let visible = frame.visible_rows(100.0, 300.0, 192).unwrap();
    let bands = frame
        .region_segments(0.0, 96.0, &visible, 100.0, 300.0, 16)
        .unwrap();
    assert_eq!(bands, vec![(150.0, 246.0)]);
}
