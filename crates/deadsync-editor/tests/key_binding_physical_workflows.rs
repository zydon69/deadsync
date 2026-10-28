use deadsync_editor::{
    key_binding_preferences::{
        Preferences,
        physical::{self, Effects},
    },
    key_commands::{Effect, Key, Registry},
};
fn keys(names: &[&str]) -> Vec<Key> {
    names.iter().map(|key| Key::named(key)).collect()
}
fn update(registry: &mut Registry, effects: &mut Effects, t: f64, names: &[&str]) -> Vec<Effect> {
    let raw = registry.update(t, keys(names));
    effects.coalesce(registry, raw)
}
#[test]
fn native_alias_and_source_physical_modifiers_remain_distinct_and_warn_on_real_overlap() {
    let mut p = Preferences::default();
    p.commands
        .insert("Save".into(), vec![vec!["LeftWindows".into(), "S".into()]]);
    p.commands
        .insert("Undo".into(), vec![vec!["LeftControl".into(), "S".into()]]);
    p.commands
        .insert("Copy".into(), vec![vec!["Ctrl".into(), "S".into()]]);
    p.validate().unwrap();
    let conflicts = physical::conflicts(&p, "Save", &["LeftWindows".into(), "S".into()]);
    assert!(conflicts.contains(&"Copy".to_owned()));
    assert!(!conflicts.contains(&"Undo".to_owned()));
    for (modifier, expected) in [
        ("LeftWindows", "Save"),
        ("RightWindows", "Save"),
        ("LeftControl", "Undo"),
        ("RightControl", "Undo"),
    ] {
        let mut r = Registry::new(p.bindings());
        let mut e = Effects::default();
        update(&mut r, &mut e, 0., &[modifier]);
        let actual = update(&mut r, &mut e, 0.1, &[modifier, "S"]);
        assert!(actual.contains(&Effect::Press(expected.into())));
        assert!(actual.contains(&Effect::Press("Copy".into())));
        let other = if expected == "Save" { "Undo" } else { "Save" };
        assert!(!actual.contains(&Effect::Press(other.into())));
    }
}
#[test]
fn native_alias_is_one_press_repeat_and_final_release_with_both_physical_families() {
    let mut p = Preferences::default();
    p.commands
        .insert("MoveDown".into(), vec![vec!["Ctrl".into(), "F13".into()]]);
    p.validate().unwrap();
    let mut r = Registry::new(p.bindings());
    let mut e = Effects::default();
    update(
        &mut r,
        &mut e,
        0.,
        &["LeftControl", "RightControl", "LeftWindows", "RightWindows"],
    );
    assert_eq!(
        update(
            &mut r,
            &mut e,
            0.1,
            &[
                "LeftControl",
                "RightControl",
                "LeftWindows",
                "RightWindows",
                "F13"
            ]
        ),
        vec![Effect::Press("MoveDown".into())]
    );
    assert_eq!(
        update(
            &mut r,
            &mut e,
            0.601,
            &[
                "LeftControl",
                "RightControl",
                "LeftWindows",
                "RightWindows",
                "F13"
            ]
        ),
        vec![Effect::Repeat("MoveDown".into())]
    );
    for (i, held) in [
        vec!["RightControl", "LeftWindows", "RightWindows", "F13"],
        vec!["LeftWindows", "RightWindows", "F13"],
        vec!["RightWindows", "F13"],
    ]
    .into_iter()
    .enumerate()
    {
        // Stay below the repeat interval during each individual modifier release.
        assert!(update(&mut r, &mut e, 0.602 + i as f64 * 0.001, &held).is_empty());
        assert!(e.is_active("MoveDown"));
    }
    assert_eq!(
        update(&mut r, &mut e, 0.605, &["F13"]),
        vec![Effect::Release("MoveDown".into())]
    );
    assert!(!e.is_active("MoveDown"));
}
#[test]
fn cancellation_releases_logical_alias_once_and_clears_all_ownership() {
    let mut p = Preferences::default();
    p.commands
        .insert("Save".into(), vec![vec!["Ctrl".into(), "F13".into()]]);
    let mut r = Registry::new(p.bindings());
    let mut e = Effects::default();
    update(&mut r, &mut e, 0., &["LeftControl", "LeftWindows"]);
    update(&mut r, &mut e, 0.1, &["LeftControl", "LeftWindows", "F13"]);
    let raw = r.cancel();
    assert_eq!(e.coalesce(&r, raw), vec![Effect::Release("Save".into())]);
    assert!(!e.is_active("Save"));
    let raw = r.cancel();
    assert!(e.coalesce(&r, raw).is_empty());
}
#[test]
fn physical_side_aliases_normalize_but_control_and_super_never_collapse() {
    assert_eq!(
        physical::expand(&["RightControl".into(), "S".into()]),
        vec![keys(&["LeftControl", "S"])]
    );
    assert_eq!(
        physical::expand(&["RightWindows".into(), "S".into()]),
        vec![keys(&["LeftWindows", "S"])]
    );
    assert_ne!(
        physical::expand(&["LeftControl".into(), "S".into()]),
        physical::expand(&["LeftWindows".into(), "S".into()])
    );
    let mut p = Preferences::default();
    p.commands.insert(
        "Save".into(),
        vec![vec!["Ctrl".into(), "LeftControl".into(), "S".into()]],
    );
    assert!(p.validate().is_err());
}
