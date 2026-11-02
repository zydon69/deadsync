use deadsync_editor::key_binding_preferences::scroll_modifiers::{Kind, Query};
#[test]
fn source_family_and_empty_chord_contract() {
    let q =
        Query::from_post_load(&serde_json::json!([["RightControl", "A"], ["RightShift"]])).unwrap();
    assert_eq!(
        q.alternatives(),
        &[
            vec!["LeftControl".to_owned(), "A".to_owned()],
            vec!["LeftShift".to_owned()]
        ]
    );
    let typed = Query::from_alternatives(vec![
        vec!["RightControl".into(), "A".into()],
        vec!["RightShift".into()],
    ]);
    assert_eq!(typed, q);
    assert!(Query::from_alternatives(vec![vec![]]).always_active());
    assert!(!Query::from_alternatives(vec![vec!["Execute".into()]]).is_down(|_| false));
    assert!(q.is_down(|key| key == "LeftShift"));
    assert!(!q.is_down(|key| key == "LeftControl"));
    assert!(q.is_down(|key| key == "LeftControl" || key == "A"));
    let always = Query::from_post_load(&serde_json::json!([[]])).unwrap();
    assert!(always.always_active());
    assert!(always.is_down(|_| false));
    let unsupported = Query::from_post_load(&serde_json::json!([["Execute"]])).unwrap();
    assert!(!unsupported.always_active());
    assert!(!unsupported.is_down(|_| false));
}
#[test]
fn source_defaults_and_round_trip_do_not_use_native_ctrl_alias() {
    for mac in [false, true] {
        let zoom = Query::defaults(Kind::Zoom, mac);
        assert_eq!(
            zoom.alternatives()[0][0],
            if mac { "LeftWindows" } else { "LeftControl" }
        );
        assert_eq!(Query::from_post_load(&zoom.write()).unwrap(), zoom);
        assert!(!zoom.is_down(|key| key == "Ctrl"));
        assert_eq!(
            Query::defaults(Kind::Spacing, mac).alternatives()[0][0],
            "LeftShift"
        );
    }
}
#[test]
fn modifier_history_owns_each_leaf_separately() {
    use deadsync_editor::key_binding_preferences::scroll_modifiers::Preferences;
    let before = Preferences::defaults(false);
    let mut after = before.clone();
    after.set(
        Kind::Zoom,
        Query::from_post_load(&serde_json::json!([["A"]])).unwrap(),
    );
    let mut external = before.clone();
    external.set(
        Kind::Spacing,
        Query::from_post_load(&serde_json::json!([["B"]])).unwrap(),
    );
    let merged = Preferences::merge(&external, &before, &after).unwrap();
    assert_eq!(merged.get(Kind::Zoom), after.get(Kind::Zoom));
    assert_eq!(merged.get(Kind::Spacing), external.get(Kind::Spacing));
    let reverted = Preferences::merge(&merged, &after, &before).unwrap();
    assert_eq!(reverted.get(Kind::Zoom), before.get(Kind::Zoom));
    assert_eq!(reverted.get(Kind::Spacing), external.get(Kind::Spacing));
    let mut conflict = before.clone();
    conflict.set(
        Kind::Zoom,
        Query::from_post_load(&serde_json::json!([["C"]])).unwrap(),
    );
    assert!(matches!(
        Preferences::merge(&conflict, &before, &after),
        Err(deadsync_editor::Error::ExternalChange)
    ));
}
