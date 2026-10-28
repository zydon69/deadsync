use deadsync_editor::key_commands;
use key_commands::{Binding, Effect, Key, Registry};
fn keys(names: &[&str]) -> Vec<Key> {
    names.iter().map(|n| Key::named(n)).collect()
}
fn binding(id: &str, names: &[&str], repeat: bool) -> Binding {
    Binding {
        id: id.into(),
        alternatives: vec![keys(names)],
        repeat,
        blocks: true,
    }
}
#[test]
fn longer_chord_wins_and_ctrl_release_cannot_trigger_remaining_plain_key() {
    let mut r = Registry::new([
        binding("plain", &["S"], false),
        binding("save", &["LeftControl", "S"], false),
    ]);
    assert!(r.update(0., keys(&["RightControl"])).is_empty());
    assert_eq!(
        r.update(0.1, keys(&["RightControl", "S"])),
        vec![Effect::Press("save".into())]
    );
    assert_eq!(
        r.update(0.2, keys(&["S"])),
        vec![Effect::Release("save".into())]
    );
    assert!(r.update(0.3, keys(&["S"])).is_empty());
    r.update(0.4, keys(&[]));
    assert_eq!(
        r.update(0.5, keys(&["S"])),
        vec![Effect::Press("plain".into())]
    );
}
#[test]
fn repeat_uses_source_strict_boundary_and_one_callback_per_update() {
    let mut r = Registry::new([binding("move", &["Down"], true)]);
    assert_eq!(
        r.update(0., keys(&["Down"])),
        vec![Effect::Press("move".into())]
    );
    assert!(r.update(0.5, keys(&["Down"])).is_empty());
    assert_eq!(
        r.update(0.501, keys(&["Down"])),
        vec![Effect::Repeat("move".into())]
    );
    assert!(r.update(0.52, keys(&["Down"])).is_empty());
    assert_eq!(
        r.update(0.521, keys(&["Down"])),
        vec![Effect::Repeat("move".into())]
    );
    assert_eq!(
        r.update(5., keys(&["Down"])),
        vec![Effect::Repeat("move".into())]
    );
}
#[test]
fn both_modifier_sides_remain_down_until_last_physical_side_released() {
    let mut r = Registry::new([binding("roll", &["LeftShift", "D1"], false)]);
    r.update(0., keys(&["RightShift"]));
    assert_eq!(
        r.update(0.1, keys(&["RightShift", "D1"])),
        vec![Effect::Press("roll".into())]
    );
    assert!(
        r.update(0.2, keys(&["RightShift", "LeftShift", "D1"]))
            .is_empty()
    );
    assert!(r.update(0.3, keys(&["LeftShift", "D1"])).is_empty());
    assert_eq!(
        r.update(0.4, keys(&["D1"])),
        vec![Effect::Release("roll".into())]
    );
    assert_eq!(Key::named("RightAlt").name(), "LeftAlt");
    assert_eq!(Key::named("RightWindows").name(), "LeftWindows");
}
#[test]
fn conflicts_ignore_same_action_empty_alternatives_and_nonblocking_modifiers() {
    let mut modifier = binding("modifier", &["LeftShift"], false);
    modifier.blocks = false;
    let r = Registry::new([
        binding("save", &["LeftControl", "S"], false),
        modifier,
        binding("disabled", &[], false),
        binding("other", &["Z"], false),
    ]);
    assert_eq!(r.conflicts("other", &keys(&["S"])), Vec::<String>::new());
    assert_eq!(r.conflicts("other", &keys(&["LeftControl"])), vec!["save"]);
    assert!(r.conflicts("save", &keys(&["LeftControl", "S"])).is_empty());
}
#[test]
fn cancellation_releases_actions_and_catalog_does_not_invent_mouse_callbacks() {
    let mut r = Registry::new([
        binding("lane", &["D1"], false),
        binding("unbound", &[], false),
    ]);
    assert_eq!(
        r.update(0., keys(&["D1"])),
        vec![Effect::Press("lane".into())]
    );
    assert_eq!(r.cancel(), vec![Effect::Release("lane".into())]);
    assert!(r.cancel().is_empty());
    assert_eq!(key_commands::source::COMMANDS.len(), 164);
    assert!(
        key_commands::source::COMMANDS
            .iter()
            .any(|c| c.id == "Open" && !c.category.is_empty() && !c.label.is_empty())
    );
    assert!(
        !key_commands::source::COMMANDS
            .iter()
            .any(|c| c.id == "MouseSelectionAltBehavior")
    );
}

#[test]
fn final_key_held_before_modifier_matches_executed_source_not_its_stricter_comment() {
    let mut registry = Registry::new([binding("save", &["LeftControl", "S"], false)]);
    assert!(registry.update(0., keys(&["S"])).is_empty());
    assert_eq!(
        registry.update(0.1, keys(&["S", "LeftControl"])),
        vec![Effect::Press("save".into())]
    );
}

#[test]
fn equal_chords_execute_both_callbacks_as_observed_in_complete_source_class() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/key_commands/source-clock.json")).unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == "equal-chords")
        .unwrap();
    let mut registry = Registry::new([
        binding("save", &["Enter"], true),
        binding("play", &["Enter"], true),
    ]);
    let expected: Vec<String> = serde_json::from_value(case["conflicts"].clone()).unwrap();
    assert_eq!(registry.conflicts("save", &keys(&["Enter"])), expected);
    for frame in case["frames"].as_array().unwrap() {
        let down = frame["down"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| Key::named(key.as_str().unwrap()))
            .collect::<Vec<_>>();
        let actual = registry
            .update(frame["time"].as_f64().unwrap(), down)
            .into_iter()
            .map(|effect| match effect {
                Effect::Press(id) | Effect::Repeat(id) => id,
                Effect::Release(id) => format!("{id}:release"),
            })
            .collect::<Vec<_>>();
        let expected: Vec<String> = serde_json::from_value(frame["effects"].clone()).unwrap();
        assert_eq!(actual, expected, "frame {frame}");
    }
}
