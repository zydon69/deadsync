use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Severity},
    chart_deletion,
    document::{Document, Format},
    history::Session,
    output_policy::OutputPolicy,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn session(extra: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#TITLE:Policy;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;{extra}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").into_bytes(),Format::Ssc).unwrap())
}
fn advanced(s: &Session, format: Format) -> OutputPolicy {
    OutputPolicy::Advanced {
        format,
        options: Options::default(),
        timing_chart: s.chart_ids()[0].clone(),
    }
}
fn prepare(
    policy: &OutputPolicy,
    s: &mut Session,
) -> deadsync_editor::output_policy::PreparedOutput {
    policy
        .prepare_output(s, Limits::default(), &AtomicBool::new(false))
        .unwrap()
}
#[test]
fn native_lossless_retains_unknown_bytes_and_chartless_documents() {
    for source in [b"#VERSION:0.83;#TITLE:Empty;#vendor: x\\;y /*raw*/;".as_slice(), b"#VERSION:0.83;#TITLE:Native;#VENDOR:custom\\;text;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#LOCALVENDOR:Keep;#NOTES:1000;"] {
        let mut s=Session::new(Document::parse(source,Format::Ssc).unwrap());
        let policy=OutputPolicy::NativeLossless;let p=prepare(&policy,&mut s);
        assert!(p.issues().is_empty());let out=p.generate(&s,&BTreeSet::new(),1<<20,&AtomicBool::new(false)).unwrap();
        assert_eq!(out.bytes(),source);assert_eq!(out.format(),Format::Ssc);assert!(out.is_current(&s));assert!(!s.is_dirty());
        assert!(p.generate(&s,&BTreeSet::from([0]),1<<20,&AtomicBool::new(false)).is_err());
    }
}
#[test]
fn advanced_policy_keeps_target_codec_when_undo_restores_unrepresentable_source() {
    let mut s = session("#WARPS:1=1;");
    let policy = advanced(&s, Format::Sm);
    s.set_text(None, "WARPS", "").unwrap();
    let prepared = prepare(&policy, &mut s);
    assert!(
        !prepared
            .issues()
            .iter()
            .any(|i| i.severity == Severity::Error)
    );
    let ack = prepared
        .issues()
        .iter()
        .enumerate()
        .filter(|(_, i)| i.severity == Severity::Warning)
        .map(|(i, _)| i)
        .collect();
    let output = prepared
        .generate(&s, &ack, 1 << 20, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(output.format(), Format::Sm);
    assert_eq!(s.document().format, Format::Ssc);
    let approved = s.serialize().unwrap();
    s.undo().unwrap();
    assert!(!prepared.is_current(&s));
    assert!(!output.is_current(&s));
    let fresh = prepare(&policy, &mut s);
    assert!(fresh.issues().iter().any(|i| i.severity == Severity::Error
        && i.kind == IssueKind::OmittedEvent
        && i.field == "WARPS"));
    assert!(
        fresh
            .generate(&s, &BTreeSet::new(), 1 << 20, &AtomicBool::new(false))
            .is_err()
    );
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), approved);
    // Pure history traversal restores the exact approved content and GUIDs.
    // No new edit/revision lifetime was created, so this review remains safe.
    assert!(prepared.is_current(&s));
    assert!(output.is_current(&s));
    s.set_text(None, "TITLE", "new edit after approval").unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), approved);
    // A new edit followed by Undo has a different monotone lifetime.
    assert!(!prepared.is_current(&s));
    assert!(!output.is_current(&s));
    let fresh = prepare(&policy, &mut s);
    assert_eq!(fresh.policy(), &policy);
}
#[test]
fn timing_identity_survives_source_index_shift_and_requires_reselection_after_deletion() {
    let mut s = session("");
    let draft = deadsync_editor::chart_creation::Draft::capture(&s).unwrap();
    draft.apply(&mut s, "dance-double").unwrap();
    let selected = s.chart_ids()[1].clone();
    let policy = OutputPolicy::Advanced {
        format: Format::Ssc,
        options: Options::default(),
        timing_chart: selected.clone(),
    };
    assert_eq!(policy.timing_chart(&s).unwrap(), Some(1));
    // Presentation order is independent of source indices. The policy stores
    // neither presentation rank nor an index from a sorted view.
    let presentation = [s.chart_ids()[1].clone(), s.chart_ids()[0].clone()];
    assert_eq!(presentation[0], selected);
    assert_eq!(policy.timing_chart(&s).unwrap(), Some(1));
    chart_deletion::Draft::capture(&s, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(policy.timing_chart(&s).unwrap(), Some(0));
    prepare(&policy, &mut s);
    chart_deletion::Draft::capture(&s, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(policy.timing_chart(&s).is_err());
    assert!(
        policy
            .prepare_output(&mut s, Limits::default(), &AtomicBool::new(false))
            .is_err()
    );
    s.undo().unwrap();
    assert_eq!(policy.timing_chart(&s).unwrap(), Some(0));
}
#[test]
fn a_new_loss_requires_fresh_review_and_native_reviews_reject_edit_then_undo() {
    let mut s = session("");
    let native = prepare(&OutputPolicy::NativeLossless, &mut s);
    s.set_text(None, "TITLE", "changed").unwrap();
    s.undo().unwrap();
    assert!(!native.is_current(&s));
    assert!(
        native
            .generate(&s, &BTreeSet::new(), 1 << 20, &AtomicBool::new(false))
            .is_err()
    );
    let policy = advanced(&s, Format::Sm);
    let first = prepare(&policy, &mut s);
    s.set_text(None, "LASTSECONDHINT", "123").unwrap();
    assert!(!first.is_current(&s));
    let second = prepare(&policy, &mut s);
    assert!(
        second
            .issues()
            .iter()
            .any(|i| i.kind == IssueKind::OmittedEvent && i.field == "LASTSECONDHINT")
    );
    assert!(
        second
            .generate(&s, &BTreeSet::new(), 1 << 20, &AtomicBool::new(false))
            .is_err()
    );
}
#[test]
fn limits_cancellation_and_foreign_session_do_not_modify_native_source() {
    let mut s = session("");
    let before = s.serialize().unwrap();
    let policy = OutputPolicy::NativeLossless;
    assert!(
        policy
            .prepare_output(
                &mut s,
                Limits {
                    bytes: 1,
                    ..Limits::default()
                },
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert!(
        policy
            .prepare_output(&mut s, Limits::default(), &AtomicBool::new(true))
            .is_err()
    );
    let p = prepare(&policy, &mut s);
    assert!(
        p.generate(&s, &BTreeSet::new(), 1, &AtomicBool::new(false))
            .is_err()
    );
    let other = session("");
    assert!(!p.is_current(&other));
    assert!(
        p.generate(&other, &BTreeSet::new(), 1 << 20, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(s.serialize().unwrap(), before);
    assert!(!s.is_dirty());
    assert!(s.undo_label().is_none());
}
