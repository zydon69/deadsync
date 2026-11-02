//! Source Editor.SupportedSinglePlayerChartTypes and InitStepGraphDataAsync,
//! GrooveAuthor 48e18f71655d0f0d86144c63239331833501fda2.
use deadsync_editor::{autogen_preferences::Preferences, generation::job::TARGET_LAYOUTS};
#[test]
fn generation_targets_match_actual_source_graph_families_not_all_editable_chart_types() {
    let expected = [
        "dance-single",
        "dance-double",
        "dance-solo",
        "dance-threepanel",
        "pump-single",
        "pump-halfdouble",
        "pump-double",
        "smx-beginner",
        "smx-single",
        "smx-dual",
        "smx-full",
    ];
    assert_eq!(TARGET_LAYOUTS, expected);
    for layout in expected {
        let root = serde_json::json!({"LastSelectedAutogenChartType":layout.replace('-',"_")})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            Preferences::read(&root).unwrap().target.layout().unwrap(),
            layout
        );
    }
    // Source loads pad geometry for these editable multiplayer types, but creates
    // neither generation StepGraphs nor root nodes; its action refuses them too.
    for layout in [
        "dance_routine",
        "pump_routine",
        "smx_team",
        "dance_couple",
        "pump_couple",
        "kb7_single",
    ] {
        let root = serde_json::json!({"LastSelectedAutogenChartType":layout})
            .as_object()
            .unwrap()
            .clone();
        let preferences = Preferences::read(&root).unwrap();
        assert_eq!(preferences.target.source_name(), layout);
        assert!(
            preferences.target.layout().is_err(),
            "No source generation graph for {layout}"
        );
    }
}
