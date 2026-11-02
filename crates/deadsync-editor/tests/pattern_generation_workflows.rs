use deadsync_editor::{
    generation::{
        config::{PatternConfig, PerformedConfig},
        pattern::{self, PatternRequest},
        random::SeededRandom,
        search::RootContext,
    },
    notes::Position,
    storage::FileSession,
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

fn fixture(name: &str, suffix: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grooveauthor")
        .join(if name.starts_with("parity-")||name.starts_with("policy-parity-"){format!("generation-parity/{name}.{suffix}.json")}else{format!("{name}.{suffix}.json")});
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn request(value: &Value) -> PatternRequest {
    let layout = value["Layout"].as_str().unwrap().to_owned();
    let columns = deadsync_editor::notes::layout_columns(&layout).unwrap();
    PatternRequest {
        layout,
        start: value["Start"].as_u64().unwrap(),
        end: value["End"].as_u64().unwrap(),
        seed: value["Seed"].as_i64().unwrap() as i32,
        previous_foot: value["PreviousFoot"].as_i64().unwrap() as i32,
        previous_footing: serde_json::from_value(value["PreviousFooting"].clone()).unwrap(),
        following_footing: serde_json::from_value(value["FollowingFooting"].clone()).unwrap(),
        context: RootContext {
            lane_counts: serde_json::from_value(
                value
                    .get("LaneCounts")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(vec![0; usize::from(columns)])),
            )
            .unwrap(),
            previous_times: serde_json::from_value(
                value
                    .get("PreviousTimes")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([0.0, 0.0])),
            )
            .unwrap(),
            previous_same_counts: serde_json::from_value(
                value
                    .get("PreviousSameCounts")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([0, 0])),
            )
            .unwrap(),
            total_steps: value["TotalSteps"].as_i64().unwrap_or(0),
            last_transition: value["LastTransitionSteps"]
                .as_i64()
                .filter(|&steps| steps != 0)
                .map(|steps| (value["LastTransitionLeft"].as_bool(), steps)),
        },
        config: serde_json::from_value(value["PerformedConfig"].clone()).unwrap(),
        pattern: serde_json::from_value(value["PatternConfig"].clone()).unwrap(),
        average_nps: value["Nps"].as_f64().unwrap_or(0.0),
    }
}

fn timing(value: &Value) -> deadsync_editor::generation::timing::GenerationTiming {
    use deadsync_editor::generation::timing::{Event, EventKind, GenerationTiming};
    let mut events = Vec::new();
    for name in ["Bpms", "Stops", "Delays", "Warps"] {
        if let Some(segments) = value[name].as_array() {
            for pair in segments {
                let amount = pair[1].as_f64().unwrap();
                events.push(Event {
                    row: pair[0].as_i64().unwrap(),
                    kind: match name {
                        "Bpms" => EventKind::Tempo(amount),
                        "Stops" => EventKind::Stop(amount),
                        "Delays" => EventKind::Delay(amount),
                        _ => EventKind::Warp(pair[1].as_i64().unwrap()),
                    },
                });
            }
        }
    }
    GenerationTiming::new(events).unwrap()
}

fn compare_original(name: &str) {
    let input = fixture(name, "request");
    let expected = fixture(name, "expected");
    let request = request(&input);
    let generated = pattern::generate(
        &request,
        &timing(&input),
        &AtomicBool::new(false),
        |_, _| {},
    );
    if expected["Success"] == false {
        assert!(generated.is_err(), "{name}");
        return;
    }
    let generated = generated.unwrap();
    for (actual, expected) in generated
        .steps
        .iter()
        .zip(expected["Nodes"].as_array().unwrap())
    {
        assert!(
            (actual.time - expected["Time"].as_f64().unwrap()).abs() < 1e-8,
            "{name}: timing diverged at row {}: Rust {}, original {}",
            actual.row,
            actual.time,
            expected["Time"]
        );
    }
    let expected_notes = expected["Notes"].as_array().unwrap();
    let mut notes = Vec::new();
    for step in &generated.steps {
        for foot in 0..2 {
            for portion in 0..2 {
                if step.link.feet[foot][portion].valid {
                    notes.push((step.row, u64::from(step.feet[foot][portion].lane.unwrap())));
                }
            }
        }
    }
    let expected_notes: Vec<_> = expected_notes
        .iter()
        .map(|note| {
            assert_eq!(note["Kind"], "1", "{name}");
            (
                note["Row"].as_u64().unwrap(),
                note["Lane"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        notes, expected_notes,
        "{name}: generated lanes must match the original C# generator"
    );
    let expected_nodes = expected["Nodes"].as_array().unwrap();
    assert_eq!(generated.steps.len(), expected_nodes.len(), "{name}");
    for (actual, expected) in generated.steps.iter().zip(expected_nodes) {
        assert_eq!(actual.row, expected["Row"].as_u64().unwrap(), "{name}");
        assert!(
            (actual.time - expected["Time"].as_f64().unwrap()).abs() < 1e-8,
            "{name}: timestamp at {}",
            actual.row
        );
        for foot in 0..2 {
            assert_eq!(
                actual.feet[foot][0].lane.map(i64::from).unwrap_or(-1),
                expected["Feet"][foot]["Lane"].as_i64().unwrap(),
                "{name}: foot {foot}, row {}",
                actual.row
            );
            assert_eq!(
                actual.link.feet[foot][0].valid,
                expected["Feet"][foot]["Valid"].as_bool().unwrap(),
                "{name}"
            );
            assert_eq!(
                actual.link.feet[foot][0].step as u64,
                expected["Feet"][foot]["Step"].as_u64().unwrap(),
                "{name}"
            );
        }
    }
}

macro_rules! original_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            compare_original($name);
        }
    };
}
original_case!(single_balanced_matches_grooveauthor, "single-balanced");
original_case!(single_seed_zero_matches_grooveauthor, "single-seed-zero");
original_case!(minimum_seed_matches_grooveauthor, "single-minimum-seed");
original_case!(one_step_matches_grooveauthor, "one-step");
original_case!(
    double_context_and_bpm_change_match_grooveauthor,
    "double-context"
);
original_case!(double_baseline_matches_grooveauthor, "double-balanced");
original_case!(
    double_lane_counts_match_grooveauthor,
    "double-with-lanecounts"
);
original_case!(
    double_previous_times_match_grooveauthor,
    "double-with-previoustimes"
);
original_case!(
    double_same_counts_match_grooveauthor,
    "double-with-previoussamecounts"
);
original_case!(
    double_total_steps_match_grooveauthor,
    "double-with-totalsteps"
);
original_case!(double_bpm_change_matches_grooveauthor, "double-with-bpms");
original_case!(double_transitions_match_grooveauthor, "double-transitions");
original_case!(pump_matches_grooveauthor, "pump-balanced");
original_case!(stepmaniax_matches_grooveauthor, "smx-balanced");
original_case!(
    eighths_random_start_matches_grooveauthor,
    "eighths-random-foot"
);
original_case!(specified_footing_matches_grooveauthor, "specified-footing");
original_case!(invalid_root_is_rejected_as_in_grooveauthor, "invalid-root");

#[test]
fn seeded_random_matches_original_dotnet_for_normal_negative_and_extreme_seeds() {
    for seed in [0, 42, -7, i32::MIN, i32::MAX] {
        let expected = fixture(&format!("random-{seed}"), "expected");
        let mut random = SeededRandom::new(seed);
        let actual: Vec<_> = (0..64).map(|_| random.next_i32()).collect();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            expected,
            "Seed {seed}"
        );
    }
}

#[test]
fn original_double_path_has_matching_costs_at_each_step(){compare_original_costs("double-context");}
fn compare_original_costs(name:&str) {
    use deadsync_editor::generation::{
        graph::{ArrowState, FootState, StepGraph},
        search::{BLANK_LINK, Scoring},
        step_data::StepType,
    };
    let input = fixture(name, "request");
    let expected = fixture(name, "expected");
    let request = request(&input);
    let graph = StepGraph::builtin(&request.layout).unwrap();
    let pad = deadsync_editor::pad::Pad::builtin(&request.layout).unwrap();
    let weights = request.config.normalized_weights(&request.layout).unwrap();
    let scoring = Scoring {
        graph: &graph,
        pad,
        config: &request.config,
        pattern: Some(&request.pattern),
        normalized_weights: &weights,
        average_nps: request.average_nps,
    };
    let rest = |lane| FootState {
        lane: Some(lane),
        state: ArrowState::Resting,
    };
    let root = graph
        .find_normal_stance(
            rest(request.previous_footing[0] as u16),
            rest(request.previous_footing[1] as u16),
        )
        .unwrap();
    let mut current = scoring.root(
        root,
        request.context.previous_times[0],
        0.0,
        &request.context,
    );
    for (node, score) in expected["Nodes"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Scores"].as_array().unwrap())
    {
        let next = graph
            .find_normal_stance(
                rest(node["Feet"][0]["Lane"].as_u64().unwrap() as u16),
                rest(node["Feet"][1]["Lane"].as_u64().unwrap() as u16),
            )
            .unwrap();
        let mut link = BLANK_LINK;
        for foot in 0..2 {
            link.feet[foot][0].valid = node["Feet"][foot]["Valid"].as_bool().unwrap();
            link.feet[foot][0].step =
                StepType::from_byte(node["Feet"][foot]["Step"].as_u64().unwrap() as u8).unwrap();
        }
        current = scoring.advance(
            &current,
            next,
            link,
            timing(&input)
                .times([node["Row"].as_u64().unwrap()])
                .unwrap()[0],
            0.0,
            0.0,
        );
        let actual = serde_json::to_value(&current.cost).unwrap();
        for (field, expected) in score.as_object().unwrap() {
            assert!(
                (actual[field].as_f64().unwrap() - expected.as_f64().unwrap()).abs() < 1e-6,
                "row {}, {field}: Rust {}, original {}",
                node["Row"],
                actual[field],
                expected
            );
        }
    }
}

#[test]
fn preview_apply_undo_redo_save_reopen_preserves_other_notes_and_unknown_tags() {
    let input = fixture("single-balanced", "request");
    let request = request(&input);
    let source = b"#VERSION:0.83;#TITLE:Test;#VENDOR_DATA:keep;#BPMS:0=150;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:\n0000\n0000\n0000\n0000\n,\n0000\n0000\n0000\n0000\n,\n1000\n0000\n0000\n0000\n;";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, source).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let revision = file.session.revision();
    let generated = pattern::generate(
        &request,
        &timing(&input),
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(
        file.session.serialize().unwrap(),
        source,
        "Preview must not mutate the document"
    );
    generated.apply(&mut file.session, 0, 0, revision).unwrap();
    assert!(
        file.session
            .grid(0)
            .unwrap()
            .get(Position {
                player: 0,
                lane: 0,
                row: 384
            })
            .is_some()
    );
    let edited = file.session.serialize().unwrap();
    assert!(
        generated.apply(&mut file.session, 0, 0, revision).is_err(),
        "Old preview must be refused"
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), source);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), edited);
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        reopened.session.grid(0).unwrap().iter().count(),
        generated.steps.len() + 1
    );
    assert_eq!(
        reopened
            .session
            .document()
            .text(None, "VENDOR_DATA")
            .as_deref(),
        Some("keep")
    );
}

#[test]
fn cancellation_during_search_returns_no_partial_pattern() {
    let input = fixture("single-balanced", "request");
    let request = request(&input);
    let cancelled = AtomicBool::new(false);
    let mut completed = 0;
    let result = pattern::generate(&request, &timing(&input), &cancelled, |done, _| {
        completed = done;
        if done == 4 {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    assert!(result.is_err());
    assert_eq!(completed, 4);
}

#[test]
fn named_presets_are_independent_valid_and_round_trip_every_advanced_setting() {
    use deadsync_editor::generation::config::PerformedPreset;
    for preset in [
        PerformedPreset::Balanced,
        PerformedPreset::StaminaSingle,
        PerformedPreset::StaminaDouble,
        PerformedPreset::PatternBalanced,
        PerformedPreset::PatternNoCandles,
        PerformedPreset::PatternNoInward,
        PerformedPreset::PatternNoTransitionLimit,
    ] {
        let config = PerformedConfig::preset(preset);
        config.validate().unwrap();
        let encoded = serde_json::to_vec(&config).unwrap();
        let restored: PerformedConfig = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(config, restored);
        assert_eq!(
            config
                .normalized_weights("dance-single")
                .unwrap()
                .iter()
                .sum::<f64>(),
            1.0
        );
    }
    assert_eq!(
        PatternConfig::sixteenths(),
        serde_json::from_value::<PatternConfig>(
            fixture("single-balanced", "request")["PatternConfig"].clone()
        )
        .unwrap()
    );
    assert_eq!(
        PerformedConfig::balanced(),
        serde_json::from_value::<PerformedConfig>(
            fixture("single-balanced", "request")["PerformedConfig"].clone()
        )
        .unwrap()
    );
}

original_case!(stops_and_delays_match_grooveauthor, "timing-stops-delays");
original_case!(
    overlapping_warps_match_grooveauthor,
    "timing-overlapping-warps"
);
original_case!(
    simultaneous_timing_matches_grooveauthor,
    "timing-simultaneous"
);

#[test]
fn every_original_graph_layout_matches_grooveauthor() {
    for layout in [
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
    ] {
        compare_original(&format!("layout-{layout}"));
    }
}

#[test]
fn timing_preparation_checks_cancellation_between_rows_and_events() {
    use deadsync_editor::{
        Error,
        generation::timing::{Event, EventKind, GenerationTiming},
    };
    let clock = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(150.0),
    }])
    .unwrap();
    let mut checks = 0;
    let result = clock.times_checked(0..1_000_000, || {
        checks += 1;
        if checks == 4 {
            return Err(Error::InvalidEdit("Generation cancelled".into()));
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(checks, 4);
}

#[test]
fn original_search_single_leaf_mutations_match_outputs_and_performed_costs(){
    compare_original("parity-pattern-beatsubdivision");
    compare_original("parity-pattern-startingfootchoice");
    compare_original("parity-pattern-startingfootspecified");
    compare_original("parity-pattern-leftfootstartchoice");
    compare_original("parity-pattern-leftfootstartlanespecified");
    compare_original("parity-pattern-leftfootendchoice");
    compare_original("parity-pattern-leftfootendlanespecified");
    compare_original("parity-pattern-rightfootstartchoice");
    compare_original("parity-pattern-rightfootstartlanespecified");
    compare_original("parity-pattern-rightfootendchoice");
    compare_original("parity-pattern-rightfootendlanespecified");
    compare_original("parity-pattern-samearrowstepweight");
    compare_original("parity-pattern-newarrowstepweight");
    compare_original("parity-pattern-steptypecheckperiod");
    compare_original("parity-pattern-limitsamearrowsinarowperfoot");
    compare_original("parity-pattern-maxsamearrowsinarowperfoot");
    compare_original("parity-performed-facing-maxinwardpercentage");
    compare_original_costs("parity-performed-facing-maxinwardpercentage");
    compare_original("parity-performed-facing-inwardpercentagecutoff");
    compare_original_costs("parity-performed-facing-inwardpercentagecutoff");
    compare_original("parity-performed-facing-maxoutwardpercentage");
    compare_original_costs("parity-performed-facing-maxoutwardpercentage");
    compare_original("parity-performed-facing-outwardpercentagecutoff");
    compare_original_costs("parity-performed-facing-outwardpercentagecutoff");
    compare_original("parity-performed-steptightening-speedtighteningenabled");
    compare_original_costs("parity-performed-steptightening-speedtighteningenabled");
    compare_original("parity-performed-steptightening-speedmintimeseconds");
    compare_original_costs("parity-performed-steptightening-speedmintimeseconds");
    compare_original("parity-performed-steptightening-speedmaxtimeseconds");
    compare_original_costs("parity-performed-steptightening-speedmaxtimeseconds");
    compare_original("parity-performed-steptightening-speedtighteningmindistance");
    compare_original_costs("parity-performed-steptightening-speedtighteningmindistance");
    compare_original("parity-performed-steptightening-distancetighteningenabled");
    compare_original_costs("parity-performed-steptightening-distancetighteningenabled");
    compare_original("parity-performed-steptightening-distancemin");
    compare_original_costs("parity-performed-steptightening-distancemin");
    compare_original("parity-performed-steptightening-distancemax");
    compare_original_costs("parity-performed-steptightening-distancemax");
    compare_original("parity-performed-steptightening-stretchtighteningenabled");
    compare_original_costs("parity-performed-steptightening-stretchtighteningenabled");
    compare_original("parity-performed-steptightening-stretchdistancemin");
    compare_original_costs("parity-performed-steptightening-stretchdistancemin");
    compare_original("parity-performed-steptightening-stretchdistancemax");
    compare_original_costs("parity-performed-steptightening-stretchdistancemax");
    compare_original("parity-performed-steptightening-lateralminpaneldistance");
    compare_original_costs("parity-performed-steptightening-lateralminpaneldistance");
    compare_original("parity-performed-steptightening-longitudinalminpaneldistance");
    compare_original_costs("parity-performed-steptightening-longitudinalminpaneldistance");
    compare_original("parity-performed-lateraltightening-enabled");
    compare_original_costs("parity-performed-lateraltightening-enabled");
    compare_original("parity-performed-lateraltightening-relativenps");
    compare_original_costs("parity-performed-lateraltightening-relativenps");
    compare_original("parity-performed-lateraltightening-absolutenps");
    compare_original_costs("parity-performed-lateraltightening-absolutenps");
    compare_original("parity-performed-lateraltightening-speed");
    compare_original_costs("parity-performed-lateraltightening-speed");
    compare_original("parity-performed-transitions-enabled");
    compare_original_costs("parity-performed-transitions-enabled");
    compare_original("parity-performed-transitions-stepspertransitionmin");
    compare_original_costs("parity-performed-transitions-stepspertransitionmin");
    compare_original("parity-performed-transitions-stepspertransitionmax");
    compare_original_costs("parity-performed-transitions-stepspertransitionmax");
    compare_original("parity-performed-transitions-minimumpadwidth");
    compare_original_costs("parity-performed-transitions-minimumpadwidth");
    compare_original("parity-performed-transitions-transitioncutoffpercentage");
    compare_original_costs("parity-performed-transitions-transitioncutoffpercentage");
    compare_original("parity-performed-arrowweights-dance-single-0");
    compare_original_costs("parity-performed-arrowweights-dance-single-0");
    compare_original("parity-performed-arrowweights-dance-single-1");
    compare_original_costs("parity-performed-arrowweights-dance-single-1");
    compare_original("parity-performed-arrowweights-dance-single-2");
    compare_original_costs("parity-performed-arrowweights-dance-single-2");
    compare_original("parity-performed-arrowweights-dance-single-3");
    compare_original_costs("parity-performed-arrowweights-dance-single-3");
    compare_original("parity-performed-arrowweights-dance-double-0");
    compare_original_costs("parity-performed-arrowweights-dance-double-0");
    compare_original("parity-performed-arrowweights-dance-double-1");
    compare_original_costs("parity-performed-arrowweights-dance-double-1");
    compare_original("parity-performed-arrowweights-dance-double-2");
    compare_original_costs("parity-performed-arrowweights-dance-double-2");
    compare_original("parity-performed-arrowweights-dance-double-3");
    compare_original_costs("parity-performed-arrowweights-dance-double-3");
    compare_original("parity-performed-arrowweights-dance-double-4");
    compare_original_costs("parity-performed-arrowweights-dance-double-4");
    compare_original("parity-performed-arrowweights-dance-double-5");
    compare_original_costs("parity-performed-arrowweights-dance-double-5");
    compare_original("parity-performed-arrowweights-dance-double-6");
    compare_original_costs("parity-performed-arrowweights-dance-double-6");
    compare_original("parity-performed-arrowweights-dance-double-7");
    compare_original_costs("parity-performed-arrowweights-dance-double-7");
    compare_original("parity-performed-arrowweights-dance-solo-0");
    compare_original_costs("parity-performed-arrowweights-dance-solo-0");
    compare_original("parity-performed-arrowweights-dance-solo-1");
    compare_original_costs("parity-performed-arrowweights-dance-solo-1");
    compare_original("parity-performed-arrowweights-dance-solo-2");
    compare_original_costs("parity-performed-arrowweights-dance-solo-2");
    compare_original("parity-performed-arrowweights-dance-solo-3");
    compare_original_costs("parity-performed-arrowweights-dance-solo-3");
    compare_original("parity-performed-arrowweights-dance-solo-4");
    compare_original_costs("parity-performed-arrowweights-dance-solo-4");
    compare_original("parity-performed-arrowweights-dance-solo-5");
    compare_original_costs("parity-performed-arrowweights-dance-solo-5");
    compare_original("parity-performed-arrowweights-dance-threepanel-0");
    compare_original_costs("parity-performed-arrowweights-dance-threepanel-0");
    compare_original("parity-performed-arrowweights-dance-threepanel-1");
    compare_original_costs("parity-performed-arrowweights-dance-threepanel-1");
    compare_original("parity-performed-arrowweights-dance-threepanel-2");
    compare_original_costs("parity-performed-arrowweights-dance-threepanel-2");
    compare_original("parity-performed-arrowweights-pump-single-0");
    compare_original_costs("parity-performed-arrowweights-pump-single-0");
    compare_original("parity-performed-arrowweights-pump-single-1");
    compare_original_costs("parity-performed-arrowweights-pump-single-1");
    compare_original("parity-performed-arrowweights-pump-single-2");
    compare_original_costs("parity-performed-arrowweights-pump-single-2");
    compare_original("parity-performed-arrowweights-pump-single-3");
    compare_original_costs("parity-performed-arrowweights-pump-single-3");
    compare_original("parity-performed-arrowweights-pump-single-4");
    compare_original_costs("parity-performed-arrowweights-pump-single-4");
    compare_original("parity-performed-arrowweights-pump-halfdouble-0");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-0");
    compare_original("parity-performed-arrowweights-pump-halfdouble-1");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-1");
    compare_original("parity-performed-arrowweights-pump-halfdouble-2");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-2");
    compare_original("parity-performed-arrowweights-pump-halfdouble-3");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-3");
    compare_original("parity-performed-arrowweights-pump-halfdouble-4");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-4");
    compare_original("parity-performed-arrowweights-pump-halfdouble-5");
    compare_original_costs("parity-performed-arrowweights-pump-halfdouble-5");
    compare_original("parity-performed-arrowweights-pump-double-0");
    compare_original_costs("parity-performed-arrowweights-pump-double-0");
    compare_original("parity-performed-arrowweights-pump-double-1");
    compare_original_costs("parity-performed-arrowweights-pump-double-1");
    compare_original("parity-performed-arrowweights-pump-double-2");
    compare_original_costs("parity-performed-arrowweights-pump-double-2");
    compare_original("parity-performed-arrowweights-pump-double-3");
    compare_original_costs("parity-performed-arrowweights-pump-double-3");
    compare_original("parity-performed-arrowweights-pump-double-4");
    compare_original_costs("parity-performed-arrowweights-pump-double-4");
    compare_original("parity-performed-arrowweights-pump-double-5");
    compare_original_costs("parity-performed-arrowweights-pump-double-5");
    compare_original("parity-performed-arrowweights-pump-double-6");
    compare_original_costs("parity-performed-arrowweights-pump-double-6");
    compare_original("parity-performed-arrowweights-pump-double-7");
    compare_original_costs("parity-performed-arrowweights-pump-double-7");
    compare_original("parity-performed-arrowweights-pump-double-8");
    compare_original_costs("parity-performed-arrowweights-pump-double-8");
    compare_original("parity-performed-arrowweights-pump-double-9");
    compare_original_costs("parity-performed-arrowweights-pump-double-9");
    compare_original("parity-performed-arrowweights-smx-beginner-0");
    compare_original_costs("parity-performed-arrowweights-smx-beginner-0");
    compare_original("parity-performed-arrowweights-smx-beginner-1");
    compare_original_costs("parity-performed-arrowweights-smx-beginner-1");
    compare_original("parity-performed-arrowweights-smx-beginner-2");
    compare_original_costs("parity-performed-arrowweights-smx-beginner-2");
    compare_original("parity-performed-arrowweights-smx-single-0");
    compare_original_costs("parity-performed-arrowweights-smx-single-0");
    compare_original("parity-performed-arrowweights-smx-single-1");
    compare_original_costs("parity-performed-arrowweights-smx-single-1");
    compare_original("parity-performed-arrowweights-smx-single-2");
    compare_original_costs("parity-performed-arrowweights-smx-single-2");
    compare_original("parity-performed-arrowweights-smx-single-3");
    compare_original_costs("parity-performed-arrowweights-smx-single-3");
    compare_original("parity-performed-arrowweights-smx-single-4");
    compare_original_costs("parity-performed-arrowweights-smx-single-4");
    compare_original("parity-performed-arrowweights-smx-dual-0");
    compare_original_costs("parity-performed-arrowweights-smx-dual-0");
    compare_original("parity-performed-arrowweights-smx-dual-1");
    compare_original_costs("parity-performed-arrowweights-smx-dual-1");
    compare_original("parity-performed-arrowweights-smx-dual-2");
    compare_original_costs("parity-performed-arrowweights-smx-dual-2");
    compare_original("parity-performed-arrowweights-smx-dual-3");
    compare_original_costs("parity-performed-arrowweights-smx-dual-3");
    compare_original("parity-performed-arrowweights-smx-dual-4");
    compare_original_costs("parity-performed-arrowweights-smx-dual-4");
    compare_original("parity-performed-arrowweights-smx-dual-5");
    compare_original_costs("parity-performed-arrowweights-smx-dual-5");
    compare_original("parity-performed-arrowweights-smx-full-0");
    compare_original_costs("parity-performed-arrowweights-smx-full-0");
    compare_original("parity-performed-arrowweights-smx-full-1");
    compare_original_costs("parity-performed-arrowweights-smx-full-1");
    compare_original("parity-performed-arrowweights-smx-full-2");
    compare_original_costs("parity-performed-arrowweights-smx-full-2");
    compare_original("parity-performed-arrowweights-smx-full-3");
    compare_original_costs("parity-performed-arrowweights-smx-full-3");
    compare_original("parity-performed-arrowweights-smx-full-4");
    compare_original_costs("parity-performed-arrowweights-smx-full-4");
    compare_original("parity-performed-arrowweights-smx-full-5");
    compare_original_costs("parity-performed-arrowweights-smx-full-5");
    compare_original("parity-performed-arrowweights-smx-full-6");
    compare_original_costs("parity-performed-arrowweights-smx-full-6");
    compare_original("parity-performed-arrowweights-smx-full-7");
    compare_original_costs("parity-performed-arrowweights-smx-full-7");
    compare_original("parity-performed-arrowweights-smx-full-8");
    compare_original_costs("parity-performed-arrowweights-smx-full-8");
    compare_original("parity-performed-arrowweights-smx-full-9");
    compare_original_costs("parity-performed-arrowweights-smx-full-9");
}

#[test]
fn active_single_leaf_contexts_match_original_outputs_and_performed_costs(){
    compare_original("parity-pattern-startingfootspecified-42-base-active");
    compare_original("parity-pattern-startingfootspecified-42-mutated-active");
    compare_original("parity-pattern-leftfootstartlanespecified-42-base-active");
    compare_original("parity-pattern-leftfootstartlanespecified-42-mutated-active");
    compare_original("parity-pattern-leftfootendchoice-42-base-active");
    compare_original("parity-pattern-leftfootendchoice-42-mutated-active");
    compare_original("parity-pattern-leftfootendlanespecified-42-base-active");
    compare_original("parity-pattern-leftfootendlanespecified-42-mutated-active");
    compare_original("parity-pattern-rightfootstartlanespecified-42-base-active");
    compare_original("parity-pattern-rightfootstartlanespecified-42-mutated-active");
    compare_original("parity-pattern-rightfootendlanespecified-42-base-active");
    compare_original("parity-pattern-rightfootendlanespecified-42-mutated-active");
    compare_original("parity-performed-facing-maxinwardpercentage-42-base-active");
    compare_original_costs("parity-performed-facing-maxinwardpercentage-42-base-active");
    compare_original("parity-performed-facing-maxinwardpercentage-42-mutated-active");
    compare_original_costs("parity-performed-facing-maxinwardpercentage-42-mutated-active");
    compare_original("parity-performed-facing-inwardpercentagecutoff-42-base-active");
    compare_original_costs("parity-performed-facing-inwardpercentagecutoff-42-base-active");
    compare_original("parity-performed-facing-inwardpercentagecutoff-42-mutated-active");
    compare_original_costs("parity-performed-facing-inwardpercentagecutoff-42-mutated-active");
    compare_original("parity-performed-facing-maxoutwardpercentage-42-base-active");
    compare_original_costs("parity-performed-facing-maxoutwardpercentage-42-base-active");
    compare_original("parity-performed-facing-maxoutwardpercentage-42-mutated-active");
    compare_original_costs("parity-performed-facing-maxoutwardpercentage-42-mutated-active");
    compare_original("parity-performed-facing-outwardpercentagecutoff-42-base-active");
    compare_original_costs("parity-performed-facing-outwardpercentagecutoff-42-base-active");
    compare_original("parity-performed-facing-outwardpercentagecutoff-42-mutated-active");
    compare_original_costs("parity-performed-facing-outwardpercentagecutoff-42-mutated-active");
    compare_original("parity-performed-steptightening-distancetighteningenabled-42-base-active");
    compare_original_costs("parity-performed-steptightening-distancetighteningenabled-42-base-active");
    compare_original("parity-performed-steptightening-distancetighteningenabled-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-distancetighteningenabled-42-mutated-active");
    compare_original("parity-performed-steptightening-distancemin-42-base-active");
    compare_original_costs("parity-performed-steptightening-distancemin-42-base-active");
    compare_original("parity-performed-steptightening-distancemin-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-distancemin-42-mutated-active");
    compare_original("parity-performed-steptightening-distancemax-42-base-active");
    compare_original_costs("parity-performed-steptightening-distancemax-42-base-active");
    compare_original("parity-performed-steptightening-distancemax-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-distancemax-42-mutated-active");
    compare_original("parity-performed-steptightening-stretchtighteningenabled-42-base-active");
    compare_original_costs("parity-performed-steptightening-stretchtighteningenabled-42-base-active");
    compare_original("parity-performed-steptightening-stretchtighteningenabled-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-stretchtighteningenabled-42-mutated-active");
    compare_original("parity-performed-steptightening-stretchdistancemin-42-base-active");
    compare_original_costs("parity-performed-steptightening-stretchdistancemin-42-base-active");
    compare_original("parity-performed-steptightening-stretchdistancemin-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-stretchdistancemin-42-mutated-active");
    compare_original("parity-performed-steptightening-stretchdistancemax-42-base-active");
    compare_original_costs("parity-performed-steptightening-stretchdistancemax-42-base-active");
    compare_original("parity-performed-steptightening-stretchdistancemax-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-stretchdistancemax-42-mutated-active");
    compare_original("parity-performed-steptightening-longitudinalminpaneldistance-42-base-active");
    compare_original_costs("parity-performed-steptightening-longitudinalminpaneldistance-42-base-active");
    compare_original("parity-performed-steptightening-longitudinalminpaneldistance-42-mutated-active");
    compare_original_costs("parity-performed-steptightening-longitudinalminpaneldistance-42-mutated-active");
    compare_original("parity-performed-lateraltightening-enabled-42-base-active");
    compare_original_costs("parity-performed-lateraltightening-enabled-42-base-active");
    compare_original("parity-performed-lateraltightening-enabled-42-mutated-active");
    compare_original_costs("parity-performed-lateraltightening-enabled-42-mutated-active");
    compare_original("parity-performed-lateraltightening-relativenps-42-base-active");
    compare_original_costs("parity-performed-lateraltightening-relativenps-42-base-active");
    compare_original("parity-performed-lateraltightening-relativenps-42-mutated-active");
    compare_original_costs("parity-performed-lateraltightening-relativenps-42-mutated-active");
    compare_original("parity-performed-lateraltightening-absolutenps-42-base-active");
    compare_original_costs("parity-performed-lateraltightening-absolutenps-42-base-active");
    compare_original("parity-performed-lateraltightening-absolutenps-42-mutated-active");
    compare_original_costs("parity-performed-lateraltightening-absolutenps-42-mutated-active");
    compare_original("parity-performed-lateraltightening-speed-42-base-active");
    compare_original_costs("parity-performed-lateraltightening-speed-42-base-active");
    compare_original("parity-performed-lateraltightening-speed-42-mutated-active");
    compare_original_costs("parity-performed-lateraltightening-speed-42-mutated-active");
    compare_original("parity-performed-transitions-enabled-42-base-active");
    compare_original_costs("parity-performed-transitions-enabled-42-base-active");
    compare_original("parity-performed-transitions-enabled-42-mutated-active");
    compare_original_costs("parity-performed-transitions-enabled-42-mutated-active");
    compare_original("parity-performed-transitions-stepspertransitionmin-42-base-active");
    compare_original_costs("parity-performed-transitions-stepspertransitionmin-42-base-active");
    compare_original("parity-performed-transitions-stepspertransitionmin-42-mutated-active");
    compare_original_costs("parity-performed-transitions-stepspertransitionmin-42-mutated-active");
    compare_original("parity-performed-transitions-stepspertransitionmax-42-base-active");
    compare_original_costs("parity-performed-transitions-stepspertransitionmax-42-base-active");
    compare_original("parity-performed-transitions-stepspertransitionmax-42-mutated-active");
    compare_original_costs("parity-performed-transitions-stepspertransitionmax-42-mutated-active");
    compare_original("parity-performed-transitions-minimumpadwidth-42-base-active");
    compare_original_costs("parity-performed-transitions-minimumpadwidth-42-base-active");
    compare_original("parity-performed-transitions-minimumpadwidth-42-mutated-active");
    compare_original_costs("parity-performed-transitions-minimumpadwidth-42-mutated-active");
    compare_original("parity-performed-transitions-transitioncutoffpercentage-42-base-active");
    compare_original_costs("parity-performed-transitions-transitioncutoffpercentage-42-base-active");
    compare_original("parity-performed-transitions-transitioncutoffpercentage-42-mutated-active");
    compare_original_costs("parity-performed-transitions-transitioncutoffpercentage-42-mutated-active");
}
