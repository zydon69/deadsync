use deadsync_editor::{
    document::{Document, Format, LoadContext, Snapshot},
    history::Session,
    playback::Clock,
    song_properties,
    storage::{FileSession, SaveAsOutcome},
};
use std::sync::atomic::AtomicBool;
const SOURCE: &str = "#VERSION:0.83;\n// untouched source spelling\n#TITLE:Open Sync;#BPMS:0=120;#OFFSET:0.125;#UnknownFuture : raw\\;value;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\r\n0100\r\n0010\r\n0001;";
fn context(value: f64) -> LoadContext {
    LoadContext::new(value).unwrap()
}
fn session(value: f64) -> Session {
    Session::new(
        Document::parse_with_context(SOURCE.as_bytes(), Format::Ssc, context(value)).unwrap(),
    )
}
#[test]
fn fallback_is_captured_immutable_and_v1_missing_sync_retains_csharp_zero() {
    let s = session(0.375);
    assert_eq!(
        song_properties::sync_offset(s.document())
            .unwrap()
            .to_bits(),
        0.375f64.to_bits()
    );
    assert_eq!(s.serialize().unwrap(), SOURCE.as_bytes());
    let snapshot = Snapshot::capture(&s).unwrap();
    assert_eq!(snapshot.context(), context(0.375));
    let s2 = Session::new(snapshot.parse().unwrap());
    assert_eq!(
        Clock::capture(&s2, 0).unwrap().music_time(48).unwrap(),
        0.75
    );
    let explicit = format!("#GASongDataVersion:1;#GASongData:{{\"Future\":true}};{SOURCE}");
    let explicit =
        Document::parse_with_context(explicit.into_bytes(), Format::Ssc, context(0.375)).unwrap();
    assert_eq!(song_properties::sync_offset(&explicit).unwrap(), 0.0);
    assert!(LoadContext::new(f64::NAN).is_err());
    assert!(LoadContext::new(f64::INFINITY).is_err());
}
#[test]
fn structural_undo_and_snapshots_preserve_exact_fallback_and_clock() {
    let mut s = session(-0.25);
    let before = Clock::capture(&s, 0).unwrap().music_time(96).unwrap();
    let revision = s.revision();
    let chart = s.duplicate_chart(0, revision).unwrap();
    assert_eq!(chart, 1);
    assert_eq!(song_properties::sync_offset(s.document()).unwrap(), -0.25);
    assert_eq!(
        Clock::capture(&s, chart).unwrap().music_time(96).unwrap(),
        before
    );
    assert!(s.undo().unwrap());
    assert_eq!(
        Clock::capture(&s, 0).unwrap().music_time(96).unwrap(),
        before
    );
    assert!(s.redo().unwrap());
    assert_eq!(
        Clock::capture(&s, 1).unwrap().music_time(96).unwrap(),
        before
    );
    assert_eq!(
        s.snapshot_document().unwrap().load_context(),
        context(-0.25)
    );
}
#[test]
fn native_save_materializes_output_only_and_reopen_ignores_new_fallback() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.ssc");
    std::fs::write(&path, SOURCE).unwrap();
    let mut file = FileSession::open_with_context(&path, context(0.25)).unwrap();
    let live = file.session.serialize().unwrap();
    let clock = Clock::capture(&file.session, 0)
        .unwrap()
        .music_time(96)
        .unwrap();
    file.save().unwrap();
    assert!(!file.session.is_dirty());
    assert_eq!(file.session.serialize().unwrap(), live);
    assert_eq!(
        Clock::capture(&file.session, 0)
            .unwrap()
            .music_time(96)
            .unwrap(),
        clock
    );
    let disk = std::fs::read(&path).unwrap();
    assert_ne!(disk, live);
    let (song, chart) = SOURCE.split_once("#NOTEDATA:").unwrap();
    assert!(disk.starts_with(song.as_bytes()));
    assert!(disk.ends_with(format!("#NOTEDATA:{chart}").as_bytes()));
    let reopened = FileSession::open_with_context(&path, context(0.875)).unwrap();
    assert_eq!(
        song_properties::sync_offset(reopened.session.document()).unwrap(),
        0.25
    );
    assert_eq!(
        Clock::capture(&reopened.session, 0)
            .unwrap()
            .music_time(96)
            .unwrap(),
        clock
    );
    let once = disk;
    file.save().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), once);
}
#[test]
fn recovery_and_save_as_retain_chosen_context_without_global_preferences() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.ssc");
    std::fs::write(&path, SOURCE).unwrap();
    let mut file = FileSession::open_with_context(&path, context(0.625)).unwrap();
    file.session
        .set_text(None, "TITLE", "Recovered title")
        .unwrap();
    file.save_recovery().unwrap();
    let mut recovered = FileSession::open_with_context(&path, context(-0.25)).unwrap();
    recovered.recover().unwrap();
    assert_eq!(
        song_properties::sync_offset(recovered.session.document()).unwrap(),
        0.625
    );
    assert_eq!(recovered.session.document().load_context(), context(0.625));
    assert_eq!(
        recovered.session.document().text(None, "TITLE").unwrap(),
        "Recovered title"
    );
    let target = root.path().join("copy.ssc");
    let review = recovered.prepare_save_as(&target).unwrap();
    assert!(matches!(
        recovered.save_as(review).unwrap(),
        SaveAsOutcome::Durable { .. }
    ));
    let copy = FileSession::open_with_context(&target, context(0.125)).unwrap();
    assert_eq!(
        song_properties::sync_offset(copy.session.document()).unwrap(),
        0.625
    );
    assert_eq!(recovered.session.document().load_context(), context(0.625));
}
#[test]
fn density_worker_uses_snapshot_context_when_mapping_music_time_regions() {
    let source = SOURCE.replace("#OFFSET:0.125;", "#OFFSET:0;#LASTSECONDHINT:4.25;");
    let s = Session::new(
        Document::parse_with_context(source.into_bytes(), Format::Ssc, context(0.5)).unwrap(),
    );
    let task = deadsync_editor::density::task::Task::capture(&s, 0, Default::default()).unwrap();
    let report = task.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        report.series.measures.len(),
        2,
        "music hint4.25 minus chosen sync0.5 falls in measure1; lost context would create measure2"
    );
    assert!(report.is_current(&s, 0));
}
#[test]
fn native_and_advanced_reviews_capture_one_context_without_mutating_source_tokens() {
    use deadsync_editor::{
        advanced_export::{Limits, Options},
        output_policy::OutputPolicy,
    };
    use std::collections::BTreeSet;
    // The separate native Save test above retains the unknown tag exactly.
    // Source-compatible output correctly blocks that tag, so this clock test
    // uses the otherwise identical exportable input.
    let source = SOURCE.replace(r"#UnknownFuture : raw\;value;", "");
    assert_ne!(source, SOURCE);
    let mut s = Session::new(
        Document::parse_with_context(source.into_bytes(), Format::Ssc, context(0.375)).unwrap(),
    );
    let original = s.serialize().unwrap();
    let identity = s.chart_ids()[0].clone();
    let policies = [
        OutputPolicy::NativeLossless,
        OutputPolicy::Advanced {
            format: Format::Ssc,
            options: Options::default(),
            timing_chart: identity,
        },
    ];
    for policy in policies {
        let plan = policy
            .prepare_output(&mut s, Limits::default(), &AtomicBool::new(false))
            .unwrap();
        assert!(plan.is_current(&s));
        let acknowledged: BTreeSet<_> = (0..plan.issues().len()).collect();
        let output = plan
            .generate(&s, &acknowledged, usize::MAX, &AtomicBool::new(false))
            .unwrap();
        let reopened =
            Document::parse_with_context(output.bytes().to_vec(), output.format(), context(-0.625))
                .unwrap();
        assert_eq!(song_properties::sync_offset(&reopened).unwrap(), 0.375);
        assert!(plan.is_current(&s));
        assert_eq!(s.serialize().unwrap(), original);
        assert!(!s.is_dirty());
    }
}
