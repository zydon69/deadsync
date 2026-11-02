use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, Selection, TimingKind},
    event_shift::reviewed::{Plan, Refusal},
    history::Session,
};
const SOURCE: &[u8] = b"#VERSION:0.83;#BPMS:0=120,1=180;#TIMESIGNATURES:0=4=4;#LABELS:2=keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;";
fn session() -> Session {
    Session::new(Document::parse(SOURCE, Format::Ssc).unwrap())
}
fn pick(s: &mut Session, include_initial: bool) -> Selection {
    let mut selected = Selection::capture(s, 0, 0, i32::MAX as u64).unwrap();
    for i in 0..selected.events().len() {
        let event = selected.events()[i];
        let on = matches!(event, Event::Note(_))
            || matches!(
                event,
                Event::Timing {
                    row: 48,
                    kind: TimingKind::Tempo
                }
            )
            || include_initial && matches!(event, Event::Timing { row: 0, .. });
        selected.set_selected(i, on).unwrap();
    }
    selected
}
#[test]
fn checked_shift_preserves_unchecked_events_and_records_source_undo_redo_selection() {
    let mut s = session();
    let before = s.serialize().unwrap();
    let selected = pick(&mut s, true);
    let originals: Vec<_> = selected
        .events()
        .iter()
        .enumerate()
        .filter(|(i, e)| selected.is_selected(*i) && !matches!(e, Event::Timing { row: 0, .. }))
        .map(|(_, e)| *e)
        .collect();
    let plan = Plan::checked(&mut s, 0, &selected, 48).unwrap();
    let report = plan.report().clone();
    assert_eq!(report.moved, 3);
    assert_eq!(report.immutable, 2);
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, 0, &report).unwrap();
    assert_eq!(s.document().text(None, "LABELS").as_deref(), Some("2=keep"));
    let after = s.serialize().unwrap();
    let update = s.take_event_selection_update().unwrap();
    assert!(update.is_current(&s));
    assert_eq!(update.events(), report.selection_after);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.take_event_selection_update().unwrap().events(), originals);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
    assert_eq!(
        s.take_event_selection_update().unwrap().events(),
        report.selection_after
    );
}
#[test]
fn negative_shift_deletion_still_restores_deleted_originals_in_undo_selection() {
    let mut s = session();
    let before = s.serialize().unwrap();
    let selected = pick(&mut s, false);
    let originals: Vec<_> = selected
        .events()
        .iter()
        .enumerate()
        .filter(|(i, _)| selected.is_selected(*i))
        .map(|(_, e)| *e)
        .collect();
    let plan = Plan::checked(&mut s, 0, &selected, -48).unwrap();
    let report = plan.report().clone();
    assert_eq!(report.deleted, 1);
    plan.apply(&mut s, 0, &report).unwrap();
    assert_eq!(
        s.take_event_selection_update().unwrap().events(),
        report.selection_after
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.take_event_selection_update().unwrap().events(), originals);
}
#[test]
fn checked_shift_requires_current_exact_selection_and_exact_review() {
    let mut s = session();
    let mut selected = Selection::capture(&mut s, 0, 0, 96).unwrap();
    selected.select_all(false);
    assert!(Plan::checked(&mut s, 0, &selected, 48).is_err());
    selected.select_all(true);
    assert!(Plan::checked(&mut s, 1, &selected, 48).is_err());
    let plan = Plan::checked(&mut s, 0, &selected, 48).unwrap();
    assert_eq!(
        plan.apply(&mut s, 0, &Default::default())
            .unwrap_err()
            .reason,
        Refusal::MissingReview
    );
    let plan = Plan::checked(&mut s, 0, &selected, 48).unwrap();
    let review = plan.report().clone();
    s.set_text(None, "TITLE", "changed").unwrap();
    assert!(Plan::checked(&mut s, 0, &selected, 48).is_err());
    s.undo().unwrap();
    assert_eq!(
        plan.apply(&mut s, 0, &review).unwrap_err().reason,
        Refusal::Stale
    );
    // Undo restores the events/revision, so a newly reviewed action is valid.
    // The old deferred plan remains revoked by its monotone IntentGuard above.
    let fresh = Plan::checked(&mut s, 0, &selected, 48).unwrap();
    let review = fresh.report().clone();
    fresh.apply(&mut s, 0, &review).unwrap();
}

#[test]
fn original_callback_routing_oracle_covers_all_snap_rows_and_focus_edit_selection_guards() {
    use deadsync_editor::{
        key_binding_preferences::{
            Action, Preferences,
            source_profile::{self, Platform},
        },
        workspace::{SNAP_ROWS, snap_shift_rows},
    };
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/selected-shift-routing.expected.json"
    ))
    .unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 80);
    for case in cases.as_array().unwrap() {
        let calls = case["Calls"].as_array().unwrap();
        if case["Mode"] != "selected" {
            assert!(calls.is_empty());
            continue;
        }
        assert_eq!(calls.len(), 1);
        let snap = case["Snap"].as_u64().unwrap();
        let index = SNAP_ROWS.iter().position(|&v| v == snap).unwrap();
        let expected = snap_shift_rows(index) as i64
            * if case["Later"].as_bool().unwrap() {
                1
            } else {
                -1
            };
        assert_eq!(calls[0]["Rows"].as_i64().unwrap(), expected);
        assert_eq!(calls[0]["Selected"], serde_json::json!([96, 24]));
    }
    let native = Preferences::default();
    for platform in [Platform::Mac, Platform::Other] {
        let source = source_profile::preset(&native, platform).unwrap();
        for action in [Action::ShiftEarlier, Action::ShiftLater] {
            assert!(action.is_selected_review());
            assert_eq!(native.commands[action.id()], vec![Vec::<String>::new()]);
            assert!(source.report.applied.iter().any(|id| id == action.id()));
            assert!(!source.preferences.source_blockers.contains_key(action.id()));
        }
    }
}
