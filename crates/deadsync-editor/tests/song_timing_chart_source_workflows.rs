use deadsync_editor::{
    chart_order::Snapshot,
    document::{Document, Format},
    generation::metadata::Guid,
    history::Session,
    open_preferences::Preferences,
    song_timing_chart::Choice,
};
use serde_json::Value;

fn session(charts: &Value) -> Session {
    let mut source = String::from("#VERSION:0.83;#TITLE:Timing;#BPMS:0=120;");
    for chart in charts.as_array().unwrap() {
        let kind = chart["Type"].as_str().unwrap().replace('_', "-");
        let columns = deadsync_editor::notes::layout_columns(&kind).unwrap();
        let difficulty = ["Beginner", "Easy", "Medium", "Hard", "Challenge", "Edit"]
            [chart["Difficulty"].as_u64().unwrap() as usize];
        source.push_str(&format!(
            "#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:{};#NAME:{};#NOTES:1{};",
            chart["Rating"],
            chart["Name"].as_str().unwrap(),
            "0".repeat(usize::from(columns - 1)),
        ));
    }
    Session::new(Document::parse(source.into_bytes(), Format::Ssc).unwrap())
}

fn source_identity(choice: &Choice, ids: &[Guid]) -> Option<usize> {
    choice
        .identity()
        .map(|id| ids.iter().position(|candidate| candidate == id).unwrap())
}

#[test]
fn load_set_delete_and_output_index_match_original_scenarios_in_two_cultures() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-chart.request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-chart.expected.json"
    ))
    .unwrap();
    let preferences = Preferences::default();
    for culture in expected.as_array().unwrap() {
        for (case, result) in input["Cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(culture["Cases"].as_array().unwrap())
        {
            assert_eq!(case["Name"], result["Name"]);
            let name = case["Name"].as_str().unwrap();
            let mut session = session(&case["Charts"]);
            let ids = session.chart_ids().to_vec();
            let original = session.serialize().unwrap();
            let snapshot = Snapshot::resident(&mut session).unwrap();
            let mut choice =
                Choice::load(&snapshot, case["Custom"].as_object(), &preferences).unwrap();
            assert_eq!(
                serde_json::json!(source_identity(&choice, &ids)),
                result["Loaded"],
                "{name}"
            );
            assert_eq!(session.serialize().unwrap(), original);
            assert!(!session.is_dirty());
            if let Some(set) = case.get("Set") {
                let set = set.as_i64().unwrap();
                if set == 999 {
                    let before = choice.clone();
                    assert!(choice.set(&snapshot, Some(999)).is_err());
                    assert_eq!(choice, before);
                } else {
                    choice
                        .set(&snapshot, (set >= 0).then_some(set as usize))
                        .unwrap();
                }
            }
            if let Some(deleted) = case.get("Delete").and_then(Value::as_u64) {
                let mut draft =
                    deadsync_editor::chart_deletion::Draft::capture(&session, deleted as usize)
                        .unwrap();
                let mut before_order = current_order(&session, &ids, &result["Order"]);
                if !before_order.contains(&(deleted as usize)) {
                    before_order.push(deleted as usize);
                }
                draft
                    .prepare_song_timing(&session, &preferences, &before_order)
                    .unwrap();
                draft.apply(&mut session).unwrap();
                let remaining = Snapshot::resident(&mut session).unwrap();
                let order = current_order(&session, &ids, &result["Order"]);
                choice
                    .reconcile_after_deletion(&remaining, &preferences, &order)
                    .unwrap();
            }
            assert_eq!(
                serde_json::json!(source_identity(&choice, &ids)),
                result["Selected"],
                "{name}"
            );
            let remaining = Snapshot::resident(&mut session).unwrap();
            let order = current_order(&session, &ids, &result["Order"]);
            let fields = choice.output_fields(&remaining, &order).unwrap();
            for field in ["DefaultChartType", "DefaultChartIndex"] {
                assert_eq!(fields[field], result["Metadata"][field], "{name}/{field}");
            }
        }
    }
}

fn current_order(session: &Session, original: &[Guid], order: &Value) -> Vec<usize> {
    order
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|index| {
            let id = &original[index.as_u64().unwrap() as usize];
            session
                .chart_ids()
                .iter()
                .position(|candidate| candidate == id)
        })
        .collect()
}

#[test]
fn file_order_and_presentation_order_encode_the_same_resident_choice_differently() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-chart.request.json"
    ))
    .unwrap();
    let mut s = session(&input["Cases"][0]["Charts"]);
    let bytes = s.serialize().unwrap();
    let snapshot = Snapshot::resident(&mut s).unwrap();
    let choice = Choice::load(&snapshot, None, &Preferences::default()).unwrap();
    let file_order = snapshot.source_indices();
    assert_eq!(
        choice.output_fields(&snapshot, file_order).unwrap()["DefaultChartIndex"],
        0
    );
    assert_eq!(
        choice.output_fields(&snapshot, &[2, 1, 0, 3]).unwrap()["DefaultChartIndex"],
        2
    );
    assert!(choice.output_fields(&snapshot, &[0, 0, 2, 3]).is_err());
    assert!(choice.output_fields(&snapshot, &[0]).is_err());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert!(!s.is_dirty());
}

#[test]
fn foreign_identity_is_refused_and_deleting_a_peer_keeps_the_reference() {
    let input: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-chart.request.json"
    ))
    .unwrap();
    let mut s = session(&input["Cases"][0]["Charts"]);
    let snapshot = Snapshot::resident(&mut s).unwrap();
    let mut choice = Choice::load(&snapshot, None, &Preferences::default()).unwrap();
    let selected = choice.identity().cloned();
    let draft = deadsync_editor::chart_deletion::Draft::capture(&s, 1).unwrap();
    draft.apply(&mut s).unwrap();
    let remaining = Snapshot::resident(&mut s).unwrap();
    choice
        .reconcile_after_deletion(
            &remaining,
            &Preferences::default(),
            remaining.source_indices(),
        )
        .unwrap();
    assert_eq!(choice.identity().cloned(), selected);
    let mut other = session(&input["Cases"][0]["Charts"]);
    let foreign = Snapshot::resident(&mut other).unwrap();
    assert!(
        choice
            .output_fields(&foreign, foreign.source_indices())
            .is_err()
    );
}
