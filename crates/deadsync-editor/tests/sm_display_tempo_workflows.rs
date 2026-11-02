use deadsync_editor::{
    advanced_export::{Limits, Options, Severity},
    analysis,
    chart_properties::Draft,
    display_tempo::{Mode, Value},
    document::{Document, Format},
    history::Session,
    output_policy::OutputPolicy,
    playback::Clock,
    storage::{FileSession, SaveAsOutcome},
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
const SOURCE: &[u8] = b"#TITLE:Display;#BPMS:0=120,4=180;#DISPLAYBPM:*;#ATTACKS:TIME=1:LEN=1:MODS=100% Drunk;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000,0100;#NOTES:dance-single::Easy:2:0,0,0,0,0:0010,0001;";
fn session() -> Session {
    Session::new(Document::parse(SOURCE, Format::Sm).unwrap())
}
fn set(s: &mut Session, chart: usize, value: &str) {
    let mut d = Draft::capture(s, chart).unwrap();
    d.set_display_tempo(Value::parse(value));
    assert!(d.apply(s, chart).unwrap());
}
fn value(s: &Session, chart: usize) -> Value {
    Draft::capture(s, chart).unwrap().display_tempo().clone()
}
fn output(s: &mut Session, format: Format, fallback: usize) -> Vec<u8> {
    output_options(
        s,
        format,
        fallback,
        Options {
            require_identical_timing_in_sm: false,
            ..Default::default()
        },
    )
}
fn output_options(s: &mut Session, format: Format, fallback: usize, options: Options) -> Vec<u8> {
    let policy = OutputPolicy::Advanced {
        format,
        options,
        timing_chart: s.chart_ids()[fallback].clone(),
    };
    let prepared = policy
        .prepare_output(s, Limits::default(), &AtomicBool::new(false))
        .unwrap();
    assert!(
        !prepared
            .issues()
            .iter()
            .any(|issue| issue.severity == Severity::Error),
        "{:?}",
        prepared.issues()
    );
    let ack = prepared
        .issues()
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (v.severity == Severity::Warning).then_some(i))
        .collect::<BTreeSet<_>>();
    prepared
        .generate(s, &ack, 16 * 1024 * 1024, &AtomicBool::new(false))
        .unwrap()
        .bytes()
        .to_vec()
}
#[test]
fn sm_chart_display_modes_are_independent_and_actual_is_explicit_empty_with_exact_undo() {
    let mut s = session();
    let original = s.serialize().unwrap();
    let clocks: Vec<_> = (0..2)
        .map(|c| {
            Clock::capture(&s, c)
                .unwrap()
                .music_time(240)
                .unwrap()
                .to_bits()
        })
        .collect();
    set(&mut s, 0, "90:180");
    let specified = s.serialize().unwrap();
    assert_eq!(value(&s, 0).serialize(), "90.000000:180.000000");
    assert_eq!(value(&s, 1).mode(), Mode::Random);
    assert_eq!(
        s.document().raw_property(None, "DISPLAYBPM"),
        Some(b"*".as_slice())
    );
    assert!(s.document().text(None, "DSTimingCHART0000").is_some());
    assert!(s.document().text(None, "DSTimingCHART0001").is_none());
    for c in 0..2 {
        assert_eq!(
            Clock::capture(&s, c)
                .unwrap()
                .music_time(240)
                .unwrap()
                .to_bits(),
            clocks[c]
        );
        assert_eq!(
            deadsync_editor::attacks::placement::Timeline::capture(&s, c)
                .unwrap()
                .markers()[0]
                .row,
            96
        );
    }
    set(&mut s, 0, "");
    let actual = s.serialize().unwrap();
    assert_eq!(value(&s, 0).mode(), Mode::Actual);
    assert_eq!(value(&s, 1).mode(), Mode::Random);
    let owner: serde_json::Value =
        serde_json::from_str(&s.document().text(None, "DSTimingCHART0000").unwrap()).unwrap();
    assert_eq!(owner["fields"]["DISPLAYBPM"], "");
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), specified);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    s.redo().unwrap();
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), actual);
    let new = deadsync_editor::chart_creation::Draft::capture(&s)
        .unwrap()
        .apply(&mut s, "dance-single")
        .unwrap();
    assert_eq!(value(&s, new).mode(), Mode::Actual);
    assert_eq!(value(&s, 1).mode(), Mode::Random);
}
#[test]
fn save_recovery_reopen_and_reviewed_native_save_as_keep_owner_and_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.sm");
    std::fs::write(&source, SOURCE).unwrap();
    let mut file = FileSession::open(&source).unwrap();
    set(&mut file.session, 0, "90:180");
    file.save_recovery().unwrap();
    let mut recovered = FileSession::open(&source).unwrap();
    recovered.recover().unwrap();
    assert_eq!(
        value(&recovered.session, 0).serialize(),
        "90.000000:180.000000"
    );
    assert_eq!(value(&recovered.session, 1).mode(), Mode::Random);
    file.save().unwrap();
    let mut reopened = FileSession::open(&source).unwrap();
    assert_eq!(
        value(&reopened.session, 0).serialize(),
        "90.000000:180.000000"
    );
    assert_eq!(value(&reopened.session, 1).mode(), Mode::Random);
    let source_bytes = std::fs::read(&source).unwrap();
    let destination = root.path().join("copy.sm");
    let review = reopened.prepare_save_as(&destination).unwrap();
    assert!(review.issues().is_empty());
    assert!(matches!(
        reopened.save_as_reviewed(review, &BTreeSet::new()).unwrap(),
        SaveAsOutcome::Durable { .. }
    ));
    let copy = FileSession::open(&destination).unwrap();
    assert_eq!(copy.session.document().format, Format::Sm);
    assert_eq!(value(&copy.session, 0).serialize(), "90.000000:180.000000");
    assert_eq!(value(&copy.session, 1).mode(), Mode::Random);
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    let conflict = root.path().join("conflict.sm");
    let review = reopened.prepare_save_as(&conflict).unwrap();
    std::fs::write(&conflict, b"external").unwrap();
    assert!(reopened.save_as_reviewed(review, &BTreeSet::new()).is_err());
    assert_eq!(std::fs::read(&conflict).unwrap(), b"external");
    assert_eq!(
        value(&reopened.session, 0).serialize(),
        "90.000000:180.000000"
    );
}
#[test]
fn source_compatible_ssc_and_sm_fallback_export_consume_local_display_owner_without_mutating_session()
 {
    let mut s = session();
    set(&mut s, 0, "90:180");
    let before = s.serialize().unwrap();
    let ssc = Session::new(Document::parse(output(&mut s, Format::Ssc, 0), Format::Ssc).unwrap());
    assert_eq!(value(&ssc, 0).serialize(), "90.000000:180.000000");
    assert_eq!(value(&ssc, 1).mode(), Mode::Random);
    assert!(ssc.document().raw_property(None, "DISPLAYBPM").is_none());
    let sm = Session::new(Document::parse(output(&mut s, Format::Sm, 0), Format::Sm).unwrap());
    assert_eq!(value(&sm, 0).serialize(), "90.000000:180.000000");
    assert_eq!(value(&sm, 1).serialize(), "90.000000:180.000000");
    let sm = Session::new(Document::parse(output(&mut s, Format::Sm, 1), Format::Sm).unwrap());
    assert_eq!(value(&sm, 0).mode(), Mode::Random);
    assert_eq!(value(&sm, 1).mode(), Mode::Random);
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.document().format, Format::Sm);
    set(&mut s, 0, "");
    let ssc = Session::new(Document::parse(output(&mut s, Format::Ssc, 0), Format::Ssc).unwrap());
    assert_eq!(value(&ssc, 0).mode(), Mode::Actual);
    assert_eq!(value(&ssc, 1).mode(), Mode::Random);
}
#[test]
fn worker_analysis_local_owner_duplicate_and_reindex_keep_display_identity() {
    let mut s = session();
    set(&mut s, 0, "90:180");
    let bytes = s.serialize().unwrap();
    let options = rssp::AnalysisOptions::default();
    let local = analysis::chart(&s, 0, &options).unwrap();
    assert_eq!(
        local.chart_display_bpm.as_deref(),
        Some("90.000000:180.000000")
    );
    let worker = analysis::task::Task::capture(&s, 0)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(worker.summary().chart_display_bpm, local.chart_display_bpm);
    assert_eq!(s.serialize().unwrap(), bytes);
    let duplicate = s.duplicate_chart(0, s.revision()).unwrap();
    assert_eq!(value(&s, duplicate).serialize(), "90.000000:180.000000");
    let mut deletion = deadsync_editor::chart_deletion::Draft::capture(&s, 0).unwrap();
    // This workflow checks display-owner reindexing. The fallback chart order
    // must still be reviewed; cultural sorting has its own workflow coverage.
    let order = s.song_timing_snapshot().unwrap();
    deletion
        .prepare_song_timing(&s, &Default::default(), order.source_indices())
        .unwrap();
    deletion.apply(&mut s).unwrap();
    assert_eq!(value(&s, 0).mode(), Mode::Random);
    assert_eq!(value(&s, duplicate - 1).serialize(), "90.000000:180.000000");
}

#[test]
fn ssc_omit_timing_preserves_equal_local_displays_and_still_refuses_unequal_ones() {
    let mut s = session();
    set(&mut s, 0, "90:180");
    let options = Options {
        omit_chart_timing: true,
        ..Default::default()
    };
    let policy = OutputPolicy::Advanced {
        format: Format::Ssc,
        options,
        timing_chart: s.chart_ids()[0].clone(),
    };
    let prepared = policy
        .prepare_output(&mut s, Limits::default(), &AtomicBool::new(false))
        .unwrap();
    assert!(
        prepared
            .issues()
            .iter()
            .any(|i| i.field == "DISPLAYBPM" && i.severity == Severity::Error)
    );
    set(&mut s, 1, "90:180");
    let original = s.serialize().unwrap();
    let exported = output_options(&mut s, Format::Ssc, 0, options);
    let result = Session::new(Document::parse(exported, Format::Ssc).unwrap());
    assert!(result.document().raw_property(None, "DISPLAYBPM").is_none());
    for chart in 0..2 {
        assert_eq!(
            result.document().text(Some(chart), "DISPLAYBPM").as_deref(),
            Some("90.000000:180.000000")
        );
        assert!(
            result
                .document()
                .raw_property(Some(chart), "BPMS")
                .is_none()
        );
        assert_eq!(value(&result, chart).serialize(), "90.000000:180.000000");
    }
    assert_eq!(s.serialize().unwrap(), original);
}

#[test]
fn pinned_displaytempo_model_and_actual_fumen_writer_match_local_and_fallback_outputs() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/sm-display-tempo.expected.json"
    ))
    .unwrap();
    for case in reference["cases"].as_array().unwrap() {
        assert_eq!(case["independent"], true);
        let mut s = session();
        for (chart, key) in [(0, "first"), (1, "second")] {
            let target = Value::parse(case[key].as_str().unwrap());
            if !value(&s, chart).same_serialized_value(&target) {
                set(&mut s, chart, case[key].as_str().unwrap());
            }
        }
        let format = if case["sm"].as_bool().unwrap() {
            Format::Sm
        } else {
            Format::Ssc
        };
        let options = Options {
            require_identical_timing_in_sm: false,
            omit_chart_timing: case["omit"].as_bool().unwrap(),
            ..Default::default()
        };
        let bytes = output_options(&mut s, format, 0, options);
        let actual = Session::new(Document::parse(bytes, format).unwrap());
        let source = Document::parse(case["output"].as_str().unwrap().as_bytes(), format).unwrap();
        assert_eq!(
            actual.document().text(None, "DISPLAYBPM"),
            source.text(None, "DISPLAYBPM"),
            "{} global scope",
            case["name"]
        );
        for chart in 0..2 {
            assert_eq!(
                value(&actual, chart).serialize(),
                case["charts"][chart]["display"].as_str().unwrap(),
                "{} chart{chart}",
                case["name"]
            );
            if format == Format::Ssc {
                assert_eq!(
                    actual.document().text(Some(chart), "DISPLAYBPM"),
                    source.text(Some(chart), "DISPLAYBPM")
                );
            }
        }
    }
}
