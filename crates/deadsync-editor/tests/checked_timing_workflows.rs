use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, Selection, TimingKind},
    history::Session,
    timing_edit::conversion::{Direction, Refusal},
};
use std::collections::BTreeSet;
fn session(timing: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").into_bytes(),Format::Ssc).unwrap())
}
fn choose(s: &mut Selection, predicate: impl Fn(Event) -> bool) {
    s.select_all(false);
    for i in 0..s.events().len() {
        if predicate(s.events()[i]) {
            s.set_selected(i, true).unwrap();
        }
    }
}
#[test]
fn checked_warp_heads_do_not_expand_to_other_events_in_same_row_interval() {
    let mut s = session("#WARPS:1=1,3=1;");
    let before = s.serialize().unwrap();
    let mut chosen = Selection::capture(&mut s, 0, 0, 192).unwrap();
    choose(&mut chosen, |e| {
        matches!(
            e,
            Event::Timing {
                row: 48,
                kind: TimingKind::Warp
            }
        )
    });
    let plan = chosen
        .prepare_timing_conversion(&s, Direction::WarpsToNegativeStops)
        .unwrap();
    assert_eq!(plan.report().converted, 1);
    assert_eq!(s.serialize().unwrap(), before);
    plan.apply(&mut s, &BTreeSet::new()).unwrap();
    assert_eq!(s.document().text(None, "WARPS").as_deref(), Some("3=1"));
    assert_eq!(s.document().text(None, "STOPS").as_deref(), Some("1=-0.5"));
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn checked_stop_preset_converts_negative_only_and_preserves_positive_stop() {
    let mut s = session("#STOPS:1=1,3=-0.5;");
    let mut selected = Selection::capture(&mut s, 0, 0, 192).unwrap();
    choose(&mut selected, |e| {
        matches!(
            e,
            Event::Timing {
                kind: TimingKind::Stop,
                ..
            }
        )
    });
    let plan = selected
        .prepare_timing_conversion(&s, Direction::NegativeStopsToWarps)
        .unwrap();
    assert_eq!(plan.report().converted, 1);
    plan.apply(&mut s, &BTreeSet::new()).unwrap();
    assert_eq!(s.document().text(None, "STOPS").as_deref(), Some("1=1"));
    assert_eq!(s.document().text(None, "WARPS").as_deref(), Some("3=1"));
}
#[test]
fn stale_foreign_or_empty_checked_choices_never_change_timing() {
    let mut s = session("#WARPS:1=1;");
    let mut selected = Selection::capture(&mut s, 0, 0, 192).unwrap();
    selected.select_all(false);
    let before = s.serialize().unwrap();
    let plan = selected
        .prepare_timing_conversion(&s, Direction::WarpsToNegativeStops)
        .unwrap();
    assert_eq!(plan.report().converted, 0);
    assert!(!plan.apply(&mut s, &BTreeSet::new()).unwrap());
    assert_eq!(s.serialize().unwrap(), before);
    let foreign = session("#WARPS:1=1;");
    assert_eq!(
        selected
            .prepare_timing_conversion(&foreign, Direction::WarpsToNegativeStops)
            .err()
            .unwrap()
            .kind(),
        Refusal::Stale
    );
    s.set_text(None, "TITLE", "changed").unwrap();
    let before = s.serialize().unwrap();
    assert_eq!(
        selected
            .prepare_timing_conversion(&s, Direction::WarpsToNegativeStops)
            .err()
            .unwrap()
            .kind(),
        Refusal::Stale
    );
    assert_eq!(s.serialize().unwrap(), before);
}
