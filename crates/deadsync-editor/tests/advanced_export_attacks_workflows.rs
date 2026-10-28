use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn source(shared: bool) -> Session {
    let attack = "#ATTACKS:TIME=0:LEN=1:MODS=drunk;";
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;{}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;{}#NOTES:10000000;",if shared{attack}else{""},if shared{""}else{attack}).into_bytes(),Format::Ssc).unwrap())
}
#[test]
fn chart_attacks_without_split_timing_have_explicit_game_loss_warning() {
    for shared in [false, true] {
        let mut s = source(shared);
        let before = s.serialize().unwrap();
        let p = Plan::prepare(
            &mut s,
            Format::Ssc,
            0,
            Options {
                omit_chart_timing: true,
                ..Options::default()
            },
            Limits::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let warnings: Vec<_> = p
            .issues()
            .iter()
            .filter(|i| i.kind == IssueKind::ChartAttacksWithoutSplitTiming)
            .collect();
        assert_eq!(warnings.len(), if shared { 2 } else { 1 });
        assert!(warnings.iter().all(|i| i.severity == Severity::Warning
            && i.field == "ATTACKS"
            && i.detail.contains("ignored by StepMania")));
        assert!(p.validate_review(&s, &BTreeSet::new()).is_err());
        p.validate_review(&s, &(0..p.issues().len()).collect())
            .unwrap();
        assert_eq!(s.serialize().unwrap(), before);
        assert!(!s.is_dirty());
    }
}
#[test]
fn retained_split_timing_does_not_report_chart_attack_game_loss() {
    let mut s = source(false);
    let p = Plan::prepare(
        &mut s,
        Format::Ssc,
        0,
        Options::default(),
        Limits::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        !p.issues()
            .iter()
            .any(|i| i.kind == IssueKind::ChartAttacksWithoutSplitTiming)
    );
}
