use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, Selection, TimingKind},
    history::Session,
    timing_edit::conversion::{Direction, Refusal},
};
use std::collections::BTreeSet;
const WARPS:&[u8]=b"#VERSION:0.83;#TITLE:Selected;#BPMS:0=60;#WARPS:1=2,2=2,5=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n1000\n1000\n1000;";
const STOPS:&[u8]=b"#VERSION:0.83;#TITLE:Selected;#BPMS:0=60;#STOPS:1=-2,1.5=1,2=-1,4=-1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n1000\n1000\n1000;";
fn session(b: &[u8]) -> Session {
    Session::new(Document::parse(b, Format::Ssc).unwrap())
}
fn event(row: u64, kind: TimingKind) -> Event {
    Event::Timing { row, kind }
}
fn pick(s: &mut Selection, e: Event, on: bool) {
    let i = s.events().iter().position(|v| *v == e).unwrap();
    s.set_selected(i, on).unwrap();
}
#[test]
fn mixed_selected_timing_preserves_user_enumeration_and_undo_redo_source_selections() {
    for (bytes, direction, from, to) in [
        (
            WARPS,
            Direction::WarpsToNegativeStops,
            TimingKind::Warp,
            TimingKind::Stop,
        ),
        (
            STOPS,
            Direction::NegativeStopsToWarps,
            TimingKind::Stop,
            TimingKind::Warp,
        ),
    ] {
        let mut s = session(bytes);
        let before = s.serialize().unwrap();
        let mut selected = Selection::capture(&mut s, 0, 0, i32::MAX as u64).unwrap();
        selected.select_all(false);
        pick(&mut selected, event(96, from), true);
        let note = selected
            .events()
            .iter()
            .find(|e| matches!(e, Event::Note(_)))
            .copied()
            .unwrap();
        pick(&mut selected, note, true);
        pick(&mut selected, event(48, from), true);
        if from == TimingKind::Stop {
            pick(&mut selected, event(72, TimingKind::Stop), true);
        }
        let plan = selected.prepare_timing_conversion(&s, direction).unwrap();
        assert_eq!(plan.report().selection_after, vec![96, 48]);
        assert!(plan.apply(&mut s, &BTreeSet::new()).unwrap());
        let after = s.serialize().unwrap();
        let update = s.take_event_selection_update().unwrap();
        assert!(update.is_current(&s));
        assert_eq!(update.events(), &[event(96, to), event(48, to)]);
        s.undo().unwrap();
        let update = s.take_event_selection_update().unwrap();
        assert!(update.is_current(&s));
        assert_eq!(update.events(), &[event(96, from), event(48, from)]);
        assert_eq!(s.serialize().unwrap(), before);
        s.redo().unwrap();
        assert_eq!(
            s.take_event_selection_update().unwrap().events(),
            &[event(96, to), event(48, to)]
        );
        assert_eq!(s.serialize().unwrap(), after);
    }
}
#[test]
fn source_hashset_reuse_order_survives_expansion_and_additive_presets() {
    use deadsync_editor::event_clipboard::presets::{Mode, Preset};
    let bytes=b"#VERSION:0.83;#BPMS:0=60;#WARPS:1=1,2=1,3=1,4=1,5=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut s = session(bytes);
    let mut selected = Selection::capture(&mut s, 0, 0, 240).unwrap();
    selected.select_all(false);
    for row in [48, 96, 144] {
        pick(&mut selected, event(row, TimingKind::Warp), true);
    }
    for row in [96, 48] {
        pick(&mut selected, event(row, TimingKind::Warp), false);
    }
    for row in [192, 240] {
        pick(&mut selected, event(row, TimingKind::Warp), true);
    }
    let expected = vec![192, 240, 144];
    assert_eq!(
        selected
            .selected_events()
            .map(Event::row)
            .collect::<Vec<_>>(),
        expected
    );
    let mut expanded = selected.expand_to_chart(&mut s).unwrap();
    assert_eq!(
        expanded
            .selected_events()
            .map(Event::row)
            .collect::<Vec<_>>(),
        expected
    );
    expanded
        .apply_preset(Preset::Miscellaneous, None, Mode::Add)
        .unwrap();
    assert_eq!(
        expanded
            .selected_events()
            .take(3)
            .map(Event::row)
            .collect::<Vec<_>>(),
        expected
    );
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/selected-timing.expected.json")).unwrap();
    let case = oracle
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Mode"] == "reused")
        .unwrap();
    assert_eq!(case["Calls"][0]["Rows"], serde_json::json!(expected));
}
#[test]
fn collisions_require_explicit_review_and_restore_every_deleted_original_selection() {
    let mut s=session(b"#VERSION:0.83;#BPMS:0=120;#WARPS:1=1,3=1;#STOPS:1=2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;");
    let mut selected = Selection::capture(&mut s, 0, 0, 192).unwrap();
    selected.select_all(false);
    for row in [48, 144] {
        pick(&mut selected, event(row, TimingKind::Warp), true);
    }
    let plan = selected
        .prepare_timing_conversion(&s, Direction::WarpsToNegativeStops)
        .unwrap();
    let before = s.serialize().unwrap();
    let revision = s.revision();
    assert_eq!(
        plan.apply_detailed(&mut s, &BTreeSet::new())
            .unwrap_err()
            .kind(),
        Refusal::CollisionReview
    );
    assert_eq!(s.revision(), revision);
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.take_event_selection_update().is_none());
    plan.apply(&mut s, &BTreeSet::from([48])).unwrap();
    assert_eq!(
        s.take_event_selection_update().unwrap().events(),
        &[event(144, TimingKind::Stop)]
    );
    s.undo().unwrap();
    assert_eq!(
        s.take_event_selection_update().unwrap().events(),
        &[event(48, TimingKind::Warp), event(144, TimingKind::Warp)]
    );
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn selection_receipts_are_session_bound_and_unrelated_history_does_not_replay_them() {
    let mut s = session(WARPS);
    let selected = Selection::capture(&mut s, 0, 48, 48).unwrap();
    let plan = selected
        .prepare_timing_conversion(&s, Direction::WarpsToNegativeStops)
        .unwrap();
    plan.apply(&mut s, &BTreeSet::new()).unwrap();
    let effect = s.take_event_selection_update().unwrap();
    let foreign = session(&s.serialize().unwrap());
    assert!(!effect.is_current(&foreign));
    s.set_text(None, "TITLE", "other edit").unwrap();
    assert!(s.take_event_selection_update().is_none());
    s.undo().unwrap();
    assert!(s.take_event_selection_update().is_none());
    s.undo().unwrap();
    assert!(s.take_event_selection_update().is_some());
    s.redo().unwrap();
    assert!(s.take_event_selection_update().is_some());
}
