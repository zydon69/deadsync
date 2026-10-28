use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    note_swap::{self, Pair},
    notes::Position,
    timing_edit::conversion::{Direction, Plan, Refusal},
};
use std::collections::BTreeSet;
fn session(timing: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=60;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").into_bytes(),Format::Ssc).unwrap())
}
#[test]
fn typed_zero_stop_collision_and_stale_reasons_have_no_partial_effects() {
    let s = session("#WARPS:1=3,2=1;");
    assert_eq!(
        Plan::prepare_detailed(&s, 0, Direction::WarpsToNegativeStops, None)
            .err()
            .unwrap()
            .kind(),
        Refusal::ZeroDurationStop
    );
    let mut s = session("#WARPS:1=1;#STOPS:1=2;");
    let before = s.serialize().unwrap();
    let plan = Plan::prepare_detailed(&s, 0, Direction::WarpsToNegativeStops, None).unwrap();
    assert_eq!(
        plan.apply_detailed(&mut s, &BTreeSet::new())
            .err()
            .unwrap()
            .kind(),
        Refusal::CollisionReview
    );
    assert_eq!(s.serialize().unwrap(), before);
    s.set_text(None, "TITLE", "edit").unwrap();
    assert_eq!(
        plan.apply_detailed(&mut s, &BTreeSet::from([48]))
            .err()
            .unwrap()
            .kind(),
        Refusal::Stale
    );
}
#[test]
fn typed_zero_sustain_swap_preserves_original_and_history() {
    let mut s = session("");
    let before = s.serialize().unwrap();
    let selected = BTreeSet::from([Position {
        row: 0,
        lane: 0,
        player: 0,
    }]);
    assert_eq!(
        note_swap::apply_detailed(&mut s, 0, &selected, Pair::HoldsTaps)
            .err()
            .unwrap()
            .kind(),
        note_swap::Refusal::ZeroDurationSustain
    );
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.undo_label().is_none());
}
