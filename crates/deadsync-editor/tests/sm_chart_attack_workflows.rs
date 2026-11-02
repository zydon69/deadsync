use deadsync_editor::{
    attacks::{self, inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    storage::FileSession,
    timing_edit::{Scope, TempoDraft},
};
fn reference() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "fixtures/grooveauthor/sm-chart-attacks.expected.json"
    ))
    .unwrap()
}
fn session() -> Session {
    Session::new(
        Document::parse(
            reference()["input"].as_str().unwrap().as_bytes(),
            Format::Sm,
        )
        .unwrap(),
    )
}
fn rows(s: &Session, c: usize) -> Vec<u64> {
    Timeline::capture(s, c)
        .unwrap()
        .markers()
        .iter()
        .map(|m| m.row)
        .collect()
}
fn moved(s: &mut Session) {
    let mut d = Draft::capture(s, 0, Scope::Chart).unwrap();
    let mut m = d.attacks().events()[0].modifiers[0].clone();
    m.name = "Reverse".into();
    d.set_at_row(0, 0, 240, m).unwrap();
    d.apply(s, 0).unwrap();
}
fn clear(s: &mut Session) {
    let mut d = Draft::capture(s, 0, Scope::Chart).unwrap();
    while !d.attacks().events().is_empty() {
        d.remove(0).unwrap();
    }
    assert!(d.apply(s, 0).unwrap());
}
#[test]
fn actual_fumen_independent_chart_move_clear_and_exact_undo_redo() {
    let reference = reference();
    assert_eq!(reference["independent"], true);
    let mut s = session();
    let original = s.serialize().unwrap();
    assert!(Draft::capture(&s, 0, Scope::Chart).unwrap().inherited());
    assert_eq!(rows(&s, 0), [96]);
    assert_eq!(rows(&s, 1), [96]);
    moved(&mut s);
    let after = s.serialize().unwrap();
    let d = Draft::capture(&s, 0, Scope::Chart).unwrap();
    assert!(!d.inherited());
    let e = &d.attacks().events()[0];
    assert_eq!(
        e.row,
        reference["moved"][0]["attacks"][0]["row"].as_u64().unwrap()
    );
    assert_eq!(e.time, 2.5);
    assert_eq!(e.modifiers[0].name, "Reverse");
    assert_eq!(rows(&s, 1), [96]);
    assert_eq!(
        s.document().raw_property(None, "ATTACKS"),
        Some(b"TIME=1:LEN=1:MODS=100% Drunk".as_slice())
    );
    clear(&mut s);
    let cleared = s.serialize().unwrap();
    assert!(rows(&s, 0).is_empty());
    assert_eq!(rows(&s, 1), [96]);
    assert!(!Draft::capture(&s, 0, Scope::Chart).unwrap().inherited());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    s.redo().unwrap();
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), cleared);
}
#[test]
fn native_save_recovery_and_reopen_preserve_explicit_empty_and_source_song() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("attacks.sm");
    std::fs::write(&path, reference()["input"].as_str().unwrap()).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    moved(&mut file.session);
    clear(&mut file.session);
    file.save_recovery().unwrap();
    let mut recovered = FileSession::open(&path).unwrap();
    recovered.recover().unwrap();
    assert!(rows(&recovered.session, 0).is_empty());
    assert_eq!(rows(&recovered.session, 1), [96]);
    file.save().unwrap();
    let reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.document().format, Format::Sm);
    assert!(rows(&reopened.session, 0).is_empty());
    assert_eq!(rows(&reopened.session, 1), [96]);
    assert!(
        !Draft::capture(&reopened.session, 0, Scope::Chart)
            .unwrap()
            .inherited()
    );
}
#[test]
fn timing_changes_duplicate_delete_and_reindex_keep_local_attack_rows() {
    let mut s = session();
    moved(&mut s);
    let mut tempo = TempoDraft::capture(&s, 0, Scope::Chart).unwrap();
    tempo.set(0, 240.).unwrap();
    tempo.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), [240]);
    assert_eq!(rows(&s, 1), [96]);
    assert_eq!(
        Draft::capture(&s, 0, Scope::Chart)
            .unwrap()
            .attacks()
            .events()[0]
            .time,
        1.25
    );
    let before = s.serialize().unwrap();
    let duplicate = s.duplicate_chart(0, s.revision()).unwrap();
    assert_eq!(rows(&s, duplicate), [240]);
    let duplicated = s.serialize().unwrap();
    let mut deletion = deadsync_editor::chart_deletion::Draft::capture(&s, 0).unwrap();
    // This workflow isolates attack-owner reindexing. Cultural fallback sorting
    // is exercised independently by song_timing_chart_workflows.
    let order = s.song_timing_snapshot().unwrap();
    deletion.prepare_song_timing(&s, &Default::default(), order.source_indices()).unwrap();
    deletion.apply(&mut s).unwrap();
    assert_eq!(rows(&s, 0), [96]);
    assert_eq!(rows(&s, duplicate - 1), [240]);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), duplicated);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn malformed_future_owner_and_stale_draft_refuse_atomically_without_erasing_redo() {
    for value in [
        r#"{"version":2,"source":[]}"#,
        r#"{"version":1,"source":[88]}"#,
    ] {
        let mut s = session();
        s.set_text(None, "DSChartAttacksCHART0000", value).unwrap();
        s.set_text(None, "TITLE", "Redo").unwrap();
        s.undo().unwrap();
        let before = s.serialize().unwrap();
        let label = s.redo_label().unwrap().to_owned();
        assert!(Draft::capture(&s, 0, Scope::Chart).is_err());
        assert_eq!(s.serialize().unwrap(), before);
        assert_eq!(s.redo_label(), Some(label.as_str()));
    }
    let mut s = session();
    let mut stale = Draft::capture(&s, 0, Scope::Chart).unwrap();
    stale.remove(0).unwrap();
    s.set_text(None, "TITLE", "External draft").unwrap();
    let before = s.serialize().unwrap();
    assert!(stale.apply(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert!(attacks::supports_resident_chart_attacks(s.document()));
}

#[test]
fn analysis_consumes_attack_owner_even_without_resident_timing_and_explicit_empty() {
    let mut s = session();
    moved(&mut s);
    let bytes = s.serialize().unwrap();
    let options = rssp::AnalysisOptions::default();
    let local = deadsync_editor::analysis::chart(&s, 0, &options).unwrap();
    assert!(local.chart_has_own_attacks);
    assert!(local.chart_attacks.as_deref().unwrap().contains("Reverse"));
    let peer = deadsync_editor::analysis::chart(&s, 1, &options).unwrap();
    assert!(peer.chart_attacks.as_deref().unwrap().contains("Drunk"));
    assert!(!peer.chart_has_own_attacks);
    assert_eq!(s.serialize().unwrap(), bytes);
    clear(&mut s);
    let empty = deadsync_editor::analysis::chart(&s, 0, &options).unwrap();
    assert!(empty.chart_has_own_attacks);
    assert!(
        empty
            .chart_attacks
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    );
    assert!(
        deadsync_editor::analysis::chart(&s, 1, &options)
            .unwrap()
            .chart_attacks
            .as_deref()
            .unwrap()
            .contains("Drunk")
    );
}
fn output(s: &mut Session, format: Format) -> Vec<u8> {
    use deadsync_editor::{
        advanced_export::{Limits, Options, Severity},
        output_policy::OutputPolicy,
    };
    use std::{collections::BTreeSet, sync::atomic::AtomicBool};
    let policy = OutputPolicy::Advanced {
        format,
        options: Options {
            require_identical_timing_in_sm: false,
            ..Default::default()
        },
        timing_chart: s.chart_ids()[0].clone(),
    };
    let prepared = policy
        .prepare_output(s, Limits::default(), &AtomicBool::new(false))
        .unwrap();
    assert!(
        !prepared
            .issues()
            .iter()
            .any(|i| i.severity == Severity::Error),
        "{:?}",
        prepared.issues()
    );
    let acknowledged = prepared
        .issues()
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (v.severity == Severity::Warning).then_some(i))
        .collect::<BTreeSet<_>>();
    prepared
        .generate(s, &acknowledged, 16 * 1024 * 1024, &AtomicBool::new(false))
        .unwrap()
        .bytes()
        .to_vec()
}
#[test]
fn ssc_export_materializes_independent_lists_sm_fallback_is_explicit_loss_boundary() {
    let mut s = session();
    moved(&mut s);
    let before = s.serialize().unwrap();
    let bytes = output(&mut s, Format::Ssc);
    let reopened = Session::new(Document::parse(bytes, Format::Ssc).unwrap());
    assert_eq!(rows(&reopened, 0), [240]);
    assert_eq!(rows(&reopened, 1), [96]);
    assert_eq!(s.serialize().unwrap(), before);
    clear(&mut s);
    let before = s.serialize().unwrap();
    let ssc = Session::new(Document::parse(output(&mut s, Format::Ssc), Format::Ssc).unwrap());
    assert!(rows(&ssc, 0).is_empty());
    assert_eq!(rows(&ssc, 1), [96]);
    let sm = Session::new(Document::parse(output(&mut s, Format::Sm), Format::Sm).unwrap());
    assert!(rows(&sm, 0).is_empty());
    assert!(rows(&sm, 1).is_empty());
    assert_eq!(
        reference()["cleared_sm_reopened"][1]["attacks"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.document().format, Format::Sm);
}
#[test]
fn generated_append_copies_local_attacks_new_empty_chart_does_not_inherit_and_clipboard_stays_local()
 {
    let mut s = session();
    moved(&mut s);
    let grid = s.grid(0).unwrap().clone();
    let appended = s
        .append_chart(0, "dance-single", &grid, s.revision(), "Generate clone")
        .unwrap();
    assert_eq!(rows(&s, appended), [240]);
    let empty = deadsync_editor::chart_creation::Draft::capture(&s)
        .unwrap()
        .apply(&mut s, "dance-single")
        .unwrap();
    assert!(rows(&s, empty).is_empty());
    assert!(!Draft::capture(&s, empty, Scope::Chart).unwrap().inherited());
    let source = Draft::capture(&s, 0, Scope::Chart).unwrap();
    let clipboard = source.copy_events(&[0]).unwrap();
    let mut target = Draft::capture(&s, empty, Scope::Chart).unwrap();
    target.paste_events(&clipboard, 48).unwrap();
    target.apply(&mut s, empty).unwrap();
    assert_eq!(rows(&s, empty), [48]);
    assert_eq!(rows(&s, 0), [240]);
    assert_eq!(rows(&s, 1), [96]);
}
