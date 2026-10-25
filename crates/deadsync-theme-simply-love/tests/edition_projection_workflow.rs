use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::fs;

#[test]
fn detailed_projection_keeps_adjacent_labels_and_tiny_viewports_do_not_panic() {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("projection.ssc");
    let labels = (0..200)
        .map(|row| format!("{}=label_{row}", row as f64 / 48.0))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "#VERSION:0.83;#TITLE:Projection;#BPMS:0=120;#LABELS:{labels};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
    );
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 0).unwrap();
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let labels: Vec<_> = actors
        .iter()
        .filter_map(|actor| match actor {
            Actor::Text { content, .. } if content.starts_with("label_") => {
                Some(content.to_string())
            }
            _ => None,
        })
        .collect();
    assert_eq!(labels.len(), 200);
    for row in 0..200 {
        assert!(labels.contains(&format!("label_{row}")));
    }
    set_current_metrics(Metrics::centered(854.0, 180.0));
    actors.clear();
    state.actors(&mut actors);
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}
