#[path = "support/row_geometry.rs"]
mod row_geometry;
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::fs;

#[test]
fn detailed_projection_stacks_adjacent_labels_with_visible_hit_targets_and_tiny_viewports_do_not_panic()
 {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("projection.ssc");
    let labels = (0..200)
        .map(|row| format!("{}=label_{row}", (row + 48) as f64 / 48.0))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "#VERSION:0.83;#TITLE:Projection;#BPMS:0=120;#LABELS:{labels};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
    );
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    row_geometry::configure(&mut state, directory.path());
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let labels: Vec<_> = actors
        .iter()
        .filter_map(|actor| match actor {
            Actor::Text {
                content,
                clip: Some(bounds),
                ..
            } => content
                .find("label_")
                .map(|start| (content[start..].to_owned(), *bounds)),
            _ => None,
        })
        .collect();
    // Start at row48 so these labels do not share the initial signature
    // widget at row0. Their 20px boxes start at projected y208 minus10.
    // Source widgets are 20px high with 2px padding. Dense events retain
    // distinct draw/pick rectangles and are culled once the viewport is full.
    let expected_visible = ((395.0_f32 - 198.0) / 22.0).ceil() as usize;
    assert_eq!(labels.len(), expected_visible);
    for (index, (label, bounds)) in labels.iter().enumerate() {
        assert_eq!(label, &format!("label_{index}"));
        assert!((bounds[1] - (198.0 + index as f32 * 22.0)).abs() < 0.01);
        assert!(bounds[1] >= 105.0 && bounds[1] + bounds[3] <= 395.0);
        if index > 0 {
            let previous = labels[index - 1].1;
            assert!(bounds[1] >= previous[1] + previous[3] + 2.0);
        }
    }
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    set_current_metrics(Metrics::centered(854.0, 180.0));
    actors.clear();
    state.actors(&mut actors);
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}
