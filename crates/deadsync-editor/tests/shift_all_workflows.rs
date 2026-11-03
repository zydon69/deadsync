use deadsync_editor::{
    document::{Document, Format},
    event_shift::Report,
    history::Session,
    shift_all::Plan,
};
fn source() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120,1=180;#TIMESIGNATURES:0=4=4;#LABELS:2=target;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;".as_slice(),Format::Ssc).unwrap())
}
#[test]
fn global_shift_is_detached_reviews_counts_and_restores_in_one_undo() {
    let mut s = source();
    let before = s.serialize().unwrap();
    let plan = Plan::prepare(&mut s, 0, 48).unwrap();
    let report = plan.report().clone();
    assert_eq!(report.moved, 4);
    assert_eq!(report.immutable, 2);
    assert_eq!(report.deleted, 0);
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, 0, &report).unwrap();
    let after = s.serialize().unwrap();
    assert_ne!(after, before);
    let grid = s.grid(0).unwrap();
    let heads: Vec<_> = grid
        .iter()
        .map(|(position, note)| (position.row, position.lane, position.player, note.kind))
        .collect();
    assert_eq!(
        heads,
        vec![
            (48, 0, 0, deadsync_editor::notes::Kind::Tap),
            (96, 1, 0, deadsync_editor::notes::Kind::Tap),
        ]
    );
    for (tag, value) in [
        ("BPMS", "0=120,2=180"),
        ("TIMESIGNATURES", "0=4=4"),
        ("LABELS", "3=target"),
    ] {
        assert_eq!(
            s.document().text(Some(0), tag).as_deref(),
            Some(value),
            "{tag}"
        );
    }
    let mut reopened = Session::new(Document::parse(after.clone(), Format::Ssc).unwrap());
    assert_eq!(reopened.grid(0).unwrap(), s.grid(0).unwrap());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn negative_shift_reports_deleted_heads_and_refuses_inexact_review() {
    let mut s = source();
    let before = s.serialize().unwrap();
    let plan = Plan::prepare(&mut s, 0, -48).unwrap();
    assert!(plan.report().deleted >= 1);
    assert_eq!(
        plan.apply(&mut s, 0, &Report::default())
            .unwrap_err()
            .reason,
        deadsync_editor::shift_all::Refusal::MissingReview
    );
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn global_shift_cannot_follow_an_intervening_edit_and_undo() {
    let mut s = source();
    let plan = Plan::prepare(&mut s, 0, 48).unwrap();
    let report = plan.report().clone();
    s.set_text(None, "TITLE", "other").unwrap();
    s.undo().unwrap();
    assert_eq!(
        plan.apply(&mut s, 0, &report).unwrap_err().reason,
        deadsync_editor::shift_all::Refusal::Stale
    );
}
