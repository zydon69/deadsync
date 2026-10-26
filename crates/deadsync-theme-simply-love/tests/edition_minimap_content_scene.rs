#[path = "../src/screens/edition/minimap_content_scene.rs"]
mod minimap_content_scene;
use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    minimap_event_policy::Pattern,
    minimap_palette::{Family, Palette, Preferences},
    minimap_projection::Projection,
    minimap_viewport::Axis,
    notes::{Grid, Kind, Note, Position},
    playback::Clock,
};
use minimap_content_scene::*;
use std::sync::Arc;
fn clock() -> Clock {
    let source =
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#WARPS:3=1;#NOTES:1000;";
    Clock::capture(
        &Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap()),
        0,
    )
    .unwrap()
}
fn projection() -> Projection {
    Projection {
        full_start: 0.0,
        full_end: 400.0,
        mini_start: 48.0,
        mini_unquantized: 48.0,
        mini_range: 192.0,
        editor_start: 48.0,
        editor_end: 96.0,
        bounds_y: 0,
        visible_height: 64,
        grabbed_percentage: 0.25,
        reverse: false,
        quantize: false,
    }
}
fn overlay<'a>(patterns: &'a [Pattern], labels: &'a [Point]) -> Overlays<'a> {
    Overlays {
        patterns,
        labels,
        preview: Some((
            Point {
                row: 96.0,
                chart_time: 1.0,
            },
            Point {
                row: 144.0,
                chart_time: 1.5,
            },
        )),
        show_patterns: true,
        show_preview: true,
        show_labels: true,
        pattern_width: 3,
        preview_width: 3,
        pattern_color: 0xff112233,
        preview_color: 0xff445566,
        label_color: 0xff778899,
    }
}
fn limits() -> Limits {
    Limits {
        source_notes: 6144,
        query_heads: 6145,
        tracks: 64,
        markers: 256,
    }
}
#[test]
fn crossing_hold_roll_lane_colors_and_markers_compose_without_dirtying_source() {
    let mut grid = Grid::new(4, 2).unwrap();
    for (player, lane, row, end, kind) in [
        (0, 0, 0, Some(120), Kind::Hold),
        (1, 2, 96, Some(192), Kind::Roll),
        (0, 1, 144, None, Kind::Mine),
    ] {
        grid.set(
            Position { player, lane, row },
            Some(Note {
                kind,
                end,
                attributes: Arc::from(""),
                tail_attributes: Arc::from(""),
            }),
        )
        .unwrap();
    }
    let source = grid.serialize().unwrap();
    let palette = Palette::new(&Preferences::default()).unwrap();
    let patterns = [
        Pattern {
            start_row: 48,
            end_row: 96,
            start_time: 0.5,
            end_time: 1.0,
        },
        Pattern {
            start_row: 96,
            end_row: 144,
            start_time: 1.0,
            end_time: 1.5,
        },
    ];
    let labels = [
        Point {
            row: 0.0,
            chart_time: 0.0,
        },
        Point {
            row: 192.0,
            chart_time: 2.0,
        },
    ];
    let scene = build(
        &grid,
        &clock(),
        &projection(),
        Axis::Row,
        32,
        2,
        3,
        0xff1e1e1e,
        overlay(&patterns, &labels),
        limits(),
        |p, _| {
            palette.colors(
                Family::RoutineRow,
                (p.row % 48) as u8,
                p.lane,
                p.player,
                false,
            )
        },
    )
    .unwrap();
    assert!(scene.notes_complete);
    assert!(scene.markers_complete);
    assert_eq!(scene.note_count, 3);
    assert!(scene.tree_queries <= 12);
    assert_eq!(grid.serialize().unwrap(), source);
    assert!(scene.raster.pixels().iter().any(|p| *p == 0xff112233));
    assert!(scene.raster.pixels().iter().any(|p| *p == 0xff445566));
}
#[test]
fn note_and_marker_resource_limits_remain_explicitly_partial() {
    let mut grid = Grid::new(4, 1).unwrap();
    for row in 48..96 {
        grid.set(
            Position {
                player: 0,
                lane: 0,
                row,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    let palette = Palette::new(&Preferences::default()).unwrap();
    let labels = (48..100)
        .map(|row| Point {
            row: row as f64,
            chart_time: row as f64 / 96.0,
        })
        .collect::<Vec<_>>();
    let mut budget = limits();
    budget.query_heads = 2;
    budget.markers = 2;
    let scene = build(
        &grid,
        &clock(),
        &projection(),
        Axis::Row,
        32,
        2,
        3,
        0xff1e1e1e,
        overlay(&[], &labels),
        budget,
        |p, _| palette.colors(Family::Row, (p.row % 48) as u8, p.lane, p.player, false),
    )
    .unwrap();
    assert!(!scene.notes_complete);
    assert!(!scene.markers_complete);
    assert_eq!(scene.note_count, 2);
}
#[test]
fn time_axis_query_keeps_all_heads_sharing_a_warp_boundary() {
    let mut grid = Grid::new(4, 1).unwrap();
    for (lane, row) in [(0, 144), (1, 160), (2, 180), (3, 192)] {
        grid.set(
            Position {
                player: 0,
                lane,
                row,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    let clock = clock();
    let time = clock.note_time(160).unwrap();
    let mut p = projection();
    p.mini_start = time - 0.1;
    p.mini_unquantized = p.mini_start;
    p.mini_range = 0.2;
    let palette = Palette::new(&Preferences::default()).unwrap();
    let scene = build(
        &grid,
        &clock,
        &p,
        Axis::Time,
        32,
        2,
        3,
        0xff1e1e1e,
        overlay(&[], &[]),
        limits(),
        |p, _| palette.colors(Family::Row, (p.row % 48) as u8, p.lane, p.player, false),
    )
    .unwrap();
    assert!(scene.notes_complete);
    assert_eq!(scene.note_count, 4);
}
