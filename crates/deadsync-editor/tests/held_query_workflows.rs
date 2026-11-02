use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    key_binding_preferences::{
        Preferences, inspector,
        scroll_modifiers::{Kind, Query},
        source_profile::{self, Platform},
    },
    receptor_preferences::Placement,
};
use std::sync::Arc;
fn query(keys: &[&str]) -> Query {
    Query::from_alternatives(vec![keys.iter().map(|name| (*name).to_owned()).collect()])
}
const NEW: [Kind; 4] = [
    Kind::SelectionMisc,
    Kind::SelectionToggle,
    Kind::SelectionExtend,
    Kind::ReceptorAxis,
];
#[test]
fn source_six_held_queries_use_platform_defaults_postload_and_never_register_callbacks() {
    for (platform, control) in [
        (Platform::Mac, "LeftWindows"),
        (Platform::Other, "LeftControl"),
    ] {
        let imported=source_profile::import(&Preferences::default(),r#"{"PreferencesKeyBinds":{"MouseSelectionAltBehavior":[["RightAlt"]],"MouseSelectionShiftBehavior":[[]],"LockReceptorMoveAxis":[["None"]]}}"#,platform).unwrap();
        assert!(imported.report.retained_modifiers.is_empty());
        assert_eq!(imported.report.applied_modifiers.len(), 6);
        assert_eq!(
            imported
                .preferences
                .scroll_modifiers
                .get(Kind::SelectionToggle),
            &query(&[control])
        );
        assert_eq!(
            imported
                .preferences
                .scroll_modifiers
                .get(Kind::SelectionMisc),
            &query(&["LeftAlt"])
        );
        assert!(
            imported
                .preferences
                .scroll_modifiers
                .get(Kind::SelectionExtend)
                .is_down(|_| false)
        );
        assert!(
            imported
                .preferences
                .scroll_modifiers
                .get(Kind::ReceptorAxis)
                .is_down(|_| false)
        );
        for kind in NEW {
            assert!(
                deadsync_editor::key_binding_preferences::Action::from_id(kind.source_name())
                    .is_none()
            );
            assert!(
                imported
                    .report
                    .applied_modifiers
                    .iter()
                    .any(|id| id == kind.source_name())
            );
        }
    }
    let unknown = query(&["FutureHostKey"]);
    assert!(!unknown.is_down(|_| false));
    assert!(!Query::from_alternatives(vec![]).is_down(|_| true));
}
#[test]
fn four_query_leaves_persist_memory_disk_and_undo_redo_without_editing_song() {
    for disk in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let mut file = if disk {
            std::fs::write(&path, br#"{"Version":1,"KeyBinds":{"Vendor":17}}"#).unwrap();
            File::open(&path).unwrap()
        } else {
            File::memory().unwrap()
        };
        let mut session = Session::new(
            Document::parse(b"#VERSION:0.83;#TITLE:unchanged;".as_slice(), Format::Ssc).unwrap(),
        );
        let bytes = session.serialize().unwrap();
        let before = file.key_bindings();
        let mut target = (*before).clone();
        for (i, kind) in NEW.into_iter().enumerate() {
            target
                .scroll_modifiers
                .set(kind, query(&[if i % 2 == 0 { "F13" } else { "RightAlt" }]));
        }
        let target = Arc::new(target);
        assert!(
            session
                .set_key_binding_preferences(before.clone(), target.clone(), Some(&file))
                .unwrap()
        );
        file = session
            .take_key_binding_preferences_update()
            .unwrap()
            .file
            .unwrap();
        assert_eq!(file.key_bindings(), target);
        assert!(!file.has_changes());
        for redo in [false, true] {
            session.capture_source_preferences_context(&file);
            if redo {
                session.redo().unwrap();
            } else {
                session.undo().unwrap();
            }
            let update = session.take_key_binding_preferences_update().unwrap();
            assert_eq!(
                update.preferences,
                if redo { target.clone() } else { before.clone() }
            );
            file = update.file.unwrap();
            assert_eq!(session.serialize().unwrap(), bytes);
            assert!(!file.has_changes());
        }
        if disk {
            let reopened = File::open(&path).unwrap();
            assert_eq!(reopened.key_bindings(), target);
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(raw["KeyBinds"]["Vendor"], 17);
        } else {
            assert!(file.path().as_os_str().is_empty());
        }
    }
}
#[test]
fn held_queries_merge_independently_and_same_leaf_conflict_is_atomic() {
    let before = Preferences::default();
    let mut edited = before.clone();
    edited
        .scroll_modifiers
        .set(Kind::SelectionToggle, query(&["F13"]));
    let mut external = before.clone();
    external
        .scroll_modifiers
        .set(Kind::ReceptorAxis, query(&["F14"]));
    let merged = inspector::merge(&external, &before, &edited).unwrap();
    assert_eq!(
        merged.scroll_modifiers.get(Kind::SelectionToggle),
        edited.scroll_modifiers.get(Kind::SelectionToggle)
    );
    assert_eq!(
        merged.scroll_modifiers.get(Kind::ReceptorAxis),
        external.scroll_modifiers.get(Kind::ReceptorAxis)
    );
    let undone = inspector::merge(&merged, &edited, &before).unwrap();
    assert_eq!(
        undone.scroll_modifiers.get(Kind::SelectionToggle),
        before.scroll_modifiers.get(Kind::SelectionToggle)
    );
    assert_eq!(
        undone.scroll_modifiers.get(Kind::ReceptorAxis),
        external.scroll_modifiers.get(Kind::ReceptorAxis)
    );
    external
        .scroll_modifiers
        .set(Kind::SelectionToggle, query(&["F15"]));
    let stable = external.clone();
    assert!(inspector::merge(&external, &before, &edited).is_err());
    assert_eq!(external, stable);
}
#[test]
fn original_dominant_axis_projection_obeys_permissions_ties_and_safe_extremes() {
    let base = Placement {
        x: 100,
        y: 100,
        ..Default::default()
    };
    for (cursor, locked, expected) in [
        ((110, 105), false, (110, 105)),
        ((110, 105), true, (110, 100)),
        ((105, 110), true, (100, 110)),
        ((110, 110), true, (100, 110)),
        ((i32::MAX, i32::MAX), true, (100, i32::MAX)),
    ] {
        let moved = base.moved_to(cursor, locked, false);
        assert_eq!((moved.x, moved.y), expected);
    }
    let x_locked = Placement {
        lock_x: true,
        ..base
    };
    assert_eq!(
        x_locked.moved_to((110, 105), true, false),
        Placement { y: 105, ..x_locked }
    );
    assert_eq!(
        x_locked.moved_to((110, 105), true, true),
        Placement { x: 110, ..x_locked }
    );
    let y_locked = Placement {
        lock_y: true,
        ..base
    };
    assert_eq!(
        y_locked.moved_to((105, 110), true, false),
        Placement { x: 105, ..y_locked }
    );
    let centered = Placement {
        center: true,
        ..base
    };
    assert_eq!(
        centered.moved_to((110, 105), true, false),
        Placement { y: 105, ..centered }
    );
}
#[test]
fn actual_disk_query_cas_preserves_disjoint_changes_and_refuses_owned_conflict_without_consuming_undo()
 {
    for conflict in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"Version":1,"KeyBinds":{"Vendor":17}}"#).unwrap();
        let file = File::open(&path).unwrap();
        let before = file.key_bindings();
        let mut after = (*before).clone();
        after
            .scroll_modifiers
            .set(Kind::SelectionToggle, query(&["F13"]));
        let after = Arc::new(after);
        let mut s = Session::new(
            Document::parse(b"#VERSION:0.83;#TITLE:unchanged;".as_slice(), Format::Ssc).unwrap(),
        );
        let bytes = s.serialize().unwrap();
        s.set_key_binding_preferences(before.clone(), after, Some(&file))
            .unwrap();
        s.take_key_binding_preferences_update().unwrap();
        let mut external = File::open(&path).unwrap();
        let mut p = (*external.key_bindings()).clone();
        p.scroll_modifiers.set(
            if conflict {
                Kind::SelectionToggle
            } else {
                Kind::ReceptorAxis
            },
            query(&["F15"]),
        );
        external.set_key_bindings(Arc::new(p)).unwrap();
        external.save().unwrap();
        let live = File::open(&path).unwrap();
        let revision = s.revision();
        let disk = std::fs::read(&path).unwrap();
        s.capture_source_preferences_context(&live);
        if conflict {
            assert!(s.undo().is_err());
            assert_eq!(s.revision(), revision);
            assert!(s.undo_label().is_some());
            assert!(s.take_key_binding_preferences_update().is_none());
            assert_eq!(std::fs::read(&path).unwrap(), disk);
        } else {
            assert!(s.undo().unwrap());
            let update = s.take_key_binding_preferences_update().unwrap();
            assert_eq!(
                update
                    .preferences
                    .scroll_modifiers
                    .get(Kind::SelectionToggle),
                before.scroll_modifiers.get(Kind::SelectionToggle)
            );
            assert_eq!(
                update.preferences.scroll_modifiers.get(Kind::ReceptorAxis),
                &query(&["F15"])
            );
            assert_eq!(
                File::open(&path).unwrap().key_bindings(),
                update.preferences
            );
        }
        assert_eq!(s.serialize().unwrap(), bytes);
    }
}
