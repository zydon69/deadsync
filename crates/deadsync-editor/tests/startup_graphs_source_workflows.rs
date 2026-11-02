use deadsync_editor::{
    document::chart_type,
    generation::graph::StepGraph,
    options_preferences::{General, startup::Task},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
};
#[test]
fn original_startup_selection_matches_all_configurations_and_missing_pad_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/startup-graphs.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let mut p = General::default();
        p.startup_graphs = case["Selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let missing: BTreeSet<i32> = case["MissingPads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let available: BTreeSet<&str> = chart_type::ALL
            .iter()
            .enumerate()
            .filter(|(i, _)| !missing.contains(&(*i as i32)))
            .map(|(_, s)| *s)
            .collect();
        let actual = Task::capture(&p).available_layouts(&available);
        let expected: Vec<_> = case["Layouts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(actual, expected);
    }
}
#[test]
fn checkbox_preserves_unknown_members_but_select_all_replaces_the_set_like_source() {
    let mut p = General::default();
    p.startup_graphs.insert(999);
    assert!(p.startup_graph_enabled(0));
    p.toggle_startup_graph(0).unwrap();
    assert!(!p.startup_graph_enabled(0));
    assert!(p.startup_graphs.contains(&999));
    p.select_all_startup_graphs();
    assert_eq!(p.startup_graphs.len(), 11);
    assert!(!p.startup_graphs.contains(&999));
    assert_eq!(Task::capture(&p).layouts().len(), 11);
    let before = p.clone();
    assert!(p.toggle_startup_graph(11).is_err());
    assert_eq!(p, before);
}
#[test]
fn cancellation_before_start_never_publishes_partial_success() {
    let task = Task::capture(&General::default());
    let mut progressed = false;
    assert!(
        task.run(&AtomicBool::new(true), &mut |_| progressed = true)
            .is_err()
    );
    assert!(!progressed);
}
#[test]
fn real_native_graph_and_root_validation_populates_shared_builtin_cache() {
    let mut p = General::default();
    p.startup_graphs = [38].into_iter().collect();
    let report = Task::capture(&p)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert_eq!(report.loaded, ["smx-beginner"]);
    let one = StepGraph::builtin("smx-beginner").unwrap();
    let two = StepGraph::builtin("smx-beginner").unwrap();
    assert!(Arc::ptr_eq(&one, &two));
    assert!(!one.nodes.is_empty());
}
