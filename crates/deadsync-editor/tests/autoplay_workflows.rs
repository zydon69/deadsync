use deadsync_editor::{
    autoplay::{Action, ActionKind, Input, State, Timeline},
    document::{Document, Format},
    history::Session,
    notes::Position,
};
fn session(timing: &str, notes: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:{notes};").into_bytes(),Format::Ssc).unwrap())
}
#[test]
fn transition_actions_and_held_state_match_unchanged_csharp_update_and_stop() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../reference/autoplay-transitions.json")).unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    for case in fixture["cases"].as_array().unwrap() {
        let steps = case["steps"].as_array().unwrap();
        let mut state = State::new(steps[0]["next"].as_array().unwrap().len() as u16).unwrap();
        for (step, expected) in steps.iter().zip(case["expected"].as_array().unwrap()) {
            let actions = if step["reset"].as_bool() == Some(true) {
                state.reset()
            } else {
                let next: Vec<Option<Input>> =
                    serde_json::from_value(step["next"].clone()).unwrap();
                state.update(step["time"].as_f64().unwrap(), &next).unwrap()
            };
            let actual: Vec<Action> = serde_json::from_value(expected["actions"].clone()).unwrap();
            assert_eq!(actions, actual, "{} step {step}", case["name"]);
            for (lane, held) in expected["held"].as_array().unwrap().iter().enumerate() {
                assert_eq!(state.held(lane as u16), held.as_bool());
            }
        }
    }
}
#[test]
fn index_uses_fake_regions_owners_lifts_and_overlapping_hold_tail_in_chart_time() {
    let session = session(
        "#FAKES:1=1;",
        "20000000\n00000000\n30000000\n00000000\n&\n00000000\n01000000\n00L00000\n00000000",
    );
    let index = Timeline::capture(&session, 0).unwrap();
    assert_eq!(index.columns(), 8);
    let next = index.next_inputs(&session, 0, 0.5).unwrap();
    assert_eq!(
        next[0].unwrap().kind,
        deadsync_editor::autoplay::InputKind::HoldEnd
    );
    assert_eq!(next[0].unwrap().time, 1.0);
    assert!(next[1].is_none());
    assert_eq!(next[2].unwrap().time, 1.0);
    let mut state = State::new(8).unwrap();
    let actions = state.update(0.5, &next).unwrap();
    assert_eq!(
        actions,
        vec![Action {
            lane: 0,
            kind: ActionKind::Down,
            time_delta: 0.0
        }]
    );
    let fake = Position {
        player: 1,
        lane: 1,
        row: 48,
    };
    assert!(
        !index
            .presentation(&session, 0, fake, 1.1, true, true, true)
            .unwrap()
            .unwrap()
            .hidden
    );
    let hold = Position {
        player: 0,
        lane: 0,
        row: 0,
    };
    let active = index
        .presentation(&session, 0, hold, 0.5, true, true, true)
        .unwrap()
        .unwrap();
    assert!(active.active_hold);
    assert!(active.clip_hold);
    let unlit = index
        .presentation(&session, 0, hold, 0.5, true, true, false)
        .unwrap()
        .unwrap();
    assert!(!unlit.active_hold);
    assert!(unlit.clip_hold);
    assert!(!active.hidden);
    assert!(
        !index
            .presentation(&session, 0, hold, 1.0, true, true, true)
            .unwrap()
            .unwrap()
            .hidden
    );
    assert!(
        index
            .presentation(&session, 0, hold, 1.01, true, true, true)
            .unwrap()
            .unwrap()
            .hidden
    );
    assert!(
        !index
            .presentation(&session, 0, hold, 1.01, false, true, true)
            .unwrap()
            .unwrap()
            .hidden
    );
}
#[test]
fn warp_and_negative_stop_fake_dependencies_do_not_trigger_autoplay() {
    for timing in ["#WARPS:1=1;", "#STOPS:1=-0.5;"] {
        let session = session(timing, "00000000\n10000000\n01000000\n00000000");
        let index = Timeline::capture(&session, 0).unwrap();
        assert!(
            index.next_inputs(&session, 0, 0.0).unwrap()[0].is_none(),
            "{timing}"
        );
        assert!(
            !index
                .presentation(
                    &session,
                    0,
                    Position {
                        player: 0,
                        lane: 0,
                        row: 48
                    },
                    10.0,
                    true,
                    true,
                    true
                )
                .unwrap()
                .unwrap()
                .hidden
        );
    }
}
#[test]
fn content_and_session_bindings_reject_stale_queries_and_invalid_updates_are_atomic() {
    let mut song = session("", "10000000");
    let index = Timeline::capture(&song, 0).unwrap();
    let other = session("", "10000000");
    assert!(index.next_inputs(&other, 0, 0.0).is_err());
    song.set_text(None, "BPMS", "0=60").unwrap();
    assert!(index.next_inputs(&song, 0, 0.0).is_err());
    assert!(
        index
            .presentation(
                &song,
                0,
                Position {
                    player: 0,
                    lane: 0,
                    row: 0
                },
                0.0,
                true,
                true,
                true
            )
            .is_err()
    );
    let mut state = State::new(1).unwrap();
    let input = Some(Input {
        kind: deadsync_editor::autoplay::InputKind::HoldEnd,
        time: 1.0,
    });
    state.update(0.5, &[input]).unwrap();
    assert_eq!(state.held(0), Some(true));
    assert!(state.update(f64::NAN, &[None]).is_err());
    assert!(state.update(2.0, &[]).is_err());
    assert_eq!(state.held(0), Some(true));
    assert!(
        state
            .update(
                2.0,
                &[Some(Input {
                    time: f64::INFINITY,
                    ..input.unwrap()
                })]
            )
            .is_err()
    );
    assert_eq!(state.held(0), Some(true));
    assert_eq!(state.reset()[0].kind, ActionKind::Cancel);
    assert_eq!(state.held(0), Some(false));
}

#[test]
fn candidate_windows_match_original_tree_search_at_equal_time_blockers_and_nonlane_widgets() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../reference/autoplay-candidates.json")).unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    for case in fixture["cases"].as_array().unwrap() {
        let song = session(
            case["timing"].as_str().unwrap(),
            case["notes"].as_str().unwrap(),
        );
        let index = Timeline::capture(&song, 0).unwrap();
        for (time, expected) in case["queries"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            let actual = index.next_inputs(&song, 0, time.as_f64().unwrap()).unwrap();
            let expected: Vec<Option<Input>> = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(actual, expected, "{} at {time}", case["name"]);
        }
    }
}
