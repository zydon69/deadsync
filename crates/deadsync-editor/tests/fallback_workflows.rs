use deadsync_editor::generation::{
    expressed::{InstanceType, LinkInstance},
    fallbacks::{self, Fallbacks},
    graph::{FootAction, PortionAction},
    reference_sort,
    search::BLANK_LINK,
    step_data::StepType,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn fixture(name: &str, suffix: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grooveauthor")
        .join(format!("{name}.{suffix}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn decode(value: &Value) -> LinkInstance {
    let mut result = LinkInstance {
        link: BLANK_LINK,
        types: [[InstanceType::Default; 2]; 2],
    };
    for foot in 0..2 {
        for portion in 0..2 {
            let index = foot * 8 + portion * 4;
            result.link.feet[foot][portion] = PortionAction {
                valid: value[index] == 1,
                step: StepType::from_byte(value[index + 1].as_u64().unwrap() as u8).unwrap(),
                action: match value[index + 2].as_u64().unwrap() {
                    0 => FootAction::Tap,
                    1 => FootAction::Hold,
                    2 => FootAction::Release,
                    _ => panic!(),
                },
            };
            result.types[foot][portion] = match value[index + 3].as_u64().unwrap() {
                0 => InstanceType::Default,
                1 => InstanceType::Roll,
                2 => InstanceType::Fake,
                3 => InstanceType::Lift,
                _ => panic!(),
            };
        }
    }
    result
}
fn encode(link: LinkInstance) -> Value {
    json!(
        (0..4)
            .flat_map(|i| {
                let a = link.link.feet[i / 2][i % 2];
                [
                    u64::from(a.valid),
                    a.step as u64,
                    a.action as u64,
                    link.types[i / 2][i % 2] as u64,
                ]
            })
            .collect::<Vec<_>>()
    )
}
#[test]
fn all_step_fallback_lists_and_exact_replacement_order_match_original() {
    let input = fixture("fallback-replacements", "request");
    let expected = fixture("fallback-replacements", "expected");
    let fallbacks = Fallbacks::builtin();
    for step in StepType::ALL {
        let list: Vec<_> = fallbacks.list(step).iter().map(|s| *s as u8).collect();
        assert_eq!(json!(list), expected["Lists"][step as usize], "{step:?}");
    }
    let cases = expected["Cases"].as_array().unwrap();
    assert_eq!(cases.len(), input["Cases"].as_array().unwrap().len());
    assert!(cases.len() > 450);
    let cancelled = AtomicBool::new(false);
    for (case, definition) in cases.iter().zip(input["Cases"].as_array().unwrap()) {
        assert_eq!(case["Label"], definition["Label"]);
        let source = decode(&case["Source"]);
        let enabled = !definition["Disabled"].as_bool().unwrap_or(false);
        let actual =
            fallbacks::replacements(source, enabled.then_some(fallbacks), &cancelled).unwrap();
        let expected = case["Links"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{}", case["Label"]);
        for (&link, expected) in actual.iter().zip(expected) {
            assert_eq!(encode(link), expected["Link"], "{}", case["Label"]);
            assert_eq!(
                fallbacks::contains_blank_foot(source, link),
                expected["BlankFoot"].as_bool().unwrap()
            );
            assert_eq!(
                fallbacks::removed_steps(source, link) as u64,
                expected["Removed"].as_u64().unwrap()
            );
        }
        if enabled {
            let cached = fallbacks.replacements(source, &cancelled).unwrap();
            assert!(
                Arc::ptr_eq(&actual, &cached),
                "Repeated requests share immutable cached results"
            );
        }
    }
}
#[test]
fn reference_sort_preserves_dotnet_permutation_including_equal_keys() {
    let input = fixture("reference-sort", "request");
    let expected = fixture("reference-sort", "expected");
    for (values, expected) in input["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
    {
        let mut pairs: Vec<_> = values
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, key)| (key.as_i64().unwrap(), i))
            .collect();
        reference_sort::sort(&mut pairs, |a, b| a.0.cmp(&b.0));
        assert_eq!(
            json!(pairs.iter().map(|p| p.1).collect::<Vec<_>>()),
            *expected
        );
    }
}
fn definition() -> Value {
    let lists: serde_json::Map<String, Value> = StepType::ALL
        .into_iter()
        .map(|step| (format!("{step:?}"), json!([format!("{step:?}")])))
        .collect();
    json!({"StepTypeFallbacks": lists})
}
#[test]
fn custom_configs_handle_json_comments_includes_and_validation_without_partial_state() {
    let mut base = definition();
    base["StepTypeFallbacks"]["NewArrow"] = json!(["NewArrow", "*SameArrow"]);
    let json = serde_json::to_string(&base).unwrap();
    let commented = format!("/* custom */ {json} // end\n");
    let custom = Fallbacks::parse(commented.as_bytes()).unwrap();
    assert_eq!(
        custom.list(StepType::NewArrow),
        [StepType::NewArrow, StepType::SameArrow]
    );
    let mut lower_keys = base.clone();
    let object = lower_keys["StepTypeFallbacks"].as_object_mut().unwrap();
    let same = object.remove("SameArrow").unwrap();
    object.insert("sameArrow".into(), same);
    assert!(Fallbacks::parse(&serde_json::to_vec(&lower_keys).unwrap()).is_ok());
    for invalid in [
        json!(["*NewArrow"]),
        json!(["NewArrow", "SameArrow", "SameArrow"]),
        json!([]),
        json!(["NotAStep"]),
    ] {
        let mut config = base.clone();
        config["StepTypeFallbacks"]["NewArrow"] = invalid;
        assert!(Fallbacks::parse(&serde_json::to_vec(&config).unwrap()).is_err());
    }
    let mut missing = base.clone();
    missing["StepTypeFallbacks"]
        .as_object_mut()
        .unwrap()
        .remove("Swing");
    assert!(Fallbacks::parse(&serde_json::to_vec(&missing).unwrap()).is_err());
    assert!(Fallbacks::parse(format!("{json} /* never closed").as_bytes()).is_err());
    for malformed in ["[, ]", "{,}", "{\"value\":,}"] {
        let mut text = json.clone();
        text.pop();
        text.push_str(&format!(",\"Ignored\":{malformed}}}"));
        assert!(Fallbacks::parse(text.as_bytes()).is_err());
    }
    let mut text = json;
    text.pop();
    text.push_str(",\"Ignored\":\"https://host/path/*literal*/\\\"\",}");
    assert!(Fallbacks::parse(text.as_bytes()).is_ok());
}
#[test]
fn cancelled_generation_cannot_return_partial_or_cached_replacements() {
    let source = decode(&fixture("fallback-replacements", "expected")["Cases"][0]["Source"]);
    let cancelled = AtomicBool::new(true);
    assert!(
        Fallbacks::builtin()
            .replacements(source, &cancelled)
            .is_err()
    );
    assert!(fallbacks::replacements(source, None, &cancelled).is_err());
    let expected = fixture("fallback-replacements", "expected");
    let case = expected["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["Label"] == "mixed-17-Left")
        .unwrap();
    let mut corrupt = decode(&case["Source"]);
    corrupt.link.feet[0][1].step = StepType::SameArrow;
    assert!(
        Fallbacks::builtin()
            .replacements(corrupt, &AtomicBool::new(false))
            .is_err()
    );
}

#[test]
fn parallel_generation_shares_one_immutable_cached_result() {
    let expected = fixture("fallback-replacements", "expected");
    let source = decode(
        &expected["Cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["Label"] == "jump-17-17")
            .unwrap()["Source"],
    );
    let config = Fallbacks::parse(include_bytes!(
        "../reference/fallbacks/default-steptype-fallbacks.json"
    ))
    .unwrap();
    let barrier = std::sync::Barrier::new(8);
    let cancelled = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    config.replacements(source, &cancelled).unwrap()
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        for result in &results[1..] {
            assert!(Arc::ptr_eq(&results[0], result));
        }
    });
}
