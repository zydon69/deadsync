use deadsync_editor::{
    attacks::{Modifier, inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    timing_edit::Scope,
};
use serde_json::Value;
fn source() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120,4=240;#DELAYS:4=1;#STOPS:8=-0.5;#WARPS:12=2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0000,0000,0000,0100;".as_slice(),Format::Ssc).unwrap())
}
#[test]
fn creation_delete_last_modifier_undo_redo_and_explicit_event_delete_match_original_actions() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-lifecycle-clock.expected.json"
    ))
    .unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 8);
    for case in cases.as_array().unwrap() {
        let mut session = source();
        let original = session.serialize().unwrap();
        let row = case["Created"]["Row"].as_u64().unwrap();
        let modifier = &case["Created"]["Modifiers"][0];
        let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
        let length = draft.default_length(row).unwrap();
        assert_eq!(length, modifier["Length"].as_f64().unwrap());
        assert_eq!(
            deadsync_editor::attacks::default_length(&session, 0, row).unwrap(),
            length
        );
        let value = Modifier {
            length,
            ..Modifier::default()
        };
        assert_eq!(value.name, modifier["Name"].as_str().unwrap());
        assert_eq!(value.level, modifier["Level"].as_f64().unwrap());
        assert_eq!(value.speed, modifier["Speed"].as_f64().unwrap());
        draft.add(row, value).unwrap();
        draft.apply(&mut session, 0).unwrap();
        let created = session.serialize().unwrap();
        let timeline = Timeline::capture(&session, 0).unwrap();
        assert_eq!(
            timeline.markers()[0].chart_time,
            case["Created"]["Time"].as_f64().unwrap()
        );
        let mut empty = Draft::capture(&session, 0, Scope::Chart).unwrap();
        empty.remove_modifier(0, 0).unwrap();
        assert_eq!(
            empty.attacks().events().len(),
            case["Deleted"]["EventCount"].as_u64().unwrap() as usize
        );
        assert!(empty.attacks().events()[0].modifiers.is_empty());
        assert_eq!(case["Deleted"]["Caption"], "No Mods");
        empty.apply(&mut session, 0).unwrap();
        let deleted = session.serialize().unwrap();
        let reopened = Session::new(Document::parse(deleted.clone(), Format::Ssc).unwrap());
        let empty = Draft::capture(&reopened, 0, Scope::Chart).unwrap();
        assert_eq!(empty.start_row(0).unwrap(), row);
        assert!(empty.attacks().events()[0].modifiers.is_empty());
        assert_eq!(
            empty.attacks().events()[0].chart_time,
            case["Deleted"]["Time"].as_f64().unwrap()
        );
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), created);
        assert_eq!(
            Timeline::capture(&session, 0).unwrap().markers()[0]
                .modifiers
                .len(),
            case["Restored"]["Modifiers"].as_array().unwrap().len()
        );
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), deleted);
        assert_eq!(case["Redone"]["Modifiers"].as_array().unwrap().len(), 0);
        let mut remove = Draft::capture(&session, 0, Scope::Chart).unwrap();
        remove.remove(0).unwrap();
        remove.apply(&mut session, 0).unwrap();
        assert!(Timeline::capture(&session, 0).unwrap().markers().is_empty());
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), deleted);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), created);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), original);
    }
}
#[test]
fn empty_event_time_and_row_edits_preserve_identity_and_reject_invalid_moves_atomically() {
    let mut session = source();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    draft.add(48, Modifier::default()).unwrap();
    draft.add(96, Modifier::default()).unwrap();
    draft.remove_modifier(0, 0).unwrap();
    let before = draft.attacks().events().to_vec();
    assert!(draft.move_to_row(0, 96).is_err());
    assert!(draft.move_to_row(0, u64::MAX).is_err());
    assert!(draft.set_time(0, f64::NAN).is_err());
    assert!(draft.set_time(0, f64::INFINITY).is_err());
    assert_eq!(draft.attacks().events(), before);
    let index = draft.move_to_row(0, 600).unwrap();
    assert_eq!(index, 1);
    assert!(draft.attacks().events()[index].modifiers.is_empty());
    let same = draft.attacks().events()[index].time;
    assert_eq!(draft.set_time(index, same).unwrap(), index);
    assert_eq!(draft.start_row(index).unwrap(), 600);
    assert!(draft.end_row(index, 0).is_err());
    draft.apply(&mut session, 0).unwrap();
    assert_eq!(
        Timeline::capture(&session, 0).unwrap().markers()[1].row,
        600
    );
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    let index = draft.set_time(1, 0.25).unwrap();
    assert_eq!(index, 0);
    assert_eq!(draft.start_row(index).unwrap(), 24);
    draft.apply(&mut session, 0).unwrap();
    assert_eq!(Timeline::capture(&session, 0).unwrap().markers()[0].row, 24);
}
