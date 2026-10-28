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
    assert!(report.moved >= 3);
    assert!(report.immutable >= 2);
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, 0, &report).unwrap();
    let after = s.serialize().unwrap();
    assert_ne!(after, before);
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
