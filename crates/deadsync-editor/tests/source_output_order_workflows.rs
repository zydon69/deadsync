use deadsync_editor::{
    advanced_export::{Limits, Options, Severity},
    chart_order::{Orderer, Snapshot},
    document::{Document, Format},
    history::Session,
    output_policy::OutputPolicy,
    source_output_order::{CultureRevision, SourceOutputOrder},
    storage::{FileSession, SaveAsOutcome, SaveAttempt},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
struct Culture(AtomicU64);
impl CultureRevision for Culture {
    fn revision(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}
struct ReferenceOrder {
    culture: &'static str,
}
impl Orderer for ReferenceOrder {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "fixtures/grooveauthor/chart-order.reference.json"
        ))
        .unwrap();
        let strings = reference["strings"].as_array().unwrap();
        let culture = reference["cultures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["culture"] == self.culture)
            .unwrap();
        snapshot.indices_with(|a, b| {
            let a = strings.iter().position(|s| s == a).unwrap();
            let b = strings.iter().position(|s| s == b).unwrap();
            culture["text_comparisons"][a][b].as_i64().unwrap().cmp(&0)
        })
    }
}
fn source() -> String {
    let mut s="#VERSION:0.83;#TITLE:Sort;#MUSIC:audio.ogg;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0.125,\"DefaultChartType\":\"dance_single\",\"DefaultChartIndex\":0};".to_owned();
    for (name, bpm, notes) in [
        ("zebra", 120, "1000"),
        ("Ångström", 180, "0100"),
        ("Éclair", 240, "0010"),
    ] {
        s += &format!(
            "#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:{name};#DESCRIPTION:{name};#DIFFICULTY:Hard;#METER:9;#BPMS:0={bpm};#GAChartDataVersion:1;#GAChartData:{{\"ChartOwner\":\"{name}\"}};#NOTES:{notes};"
        );
    }
    s
}
fn session() -> Session {
    Session::new(Document::parse(source().as_bytes(), Format::Ssc).unwrap())
}
fn policy(format: Format) -> OutputPolicy {
    OutputPolicy::AdvancedSongTiming {
        format,
        options: Options {
            require_identical_timing_in_sm: false,
            ..Default::default()
        },
    }
}
fn capture(
    session: &mut Session,
    culture: &Arc<Culture>,
    locale: &'static str,
) -> SourceOutputOrder {
    SourceOutputOrder::capture(
        session,
        &mut ReferenceOrder { culture: locale },
        culture.clone(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn generate(
    session: &mut Session,
    policy: &OutputPolicy,
    order: Option<&SourceOutputOrder>,
) -> Vec<u8> {
    let output = match order {
        Some(order) => policy.prepare_ordered_output(
            session,
            Limits::default(),
            &AtomicBool::new(false),
            order,
        ),
        None => policy.prepare_output(session, Limits::default(), &AtomicBool::new(false)),
    }
    .unwrap();
    assert!(
        !output
            .issues()
            .iter()
            .any(|issue| issue.severity == Severity::Error),
        "{:?}",
        output.issues()
    );
    let ack = output
        .issues()
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(index, _)| index)
        .collect();
    output
        .generate(session, &ack, 128 * 1024 * 1024, &AtomicBool::new(false))
        .unwrap()
        .bytes()
        .to_vec()
}
fn names(document: &Document) -> Vec<String> {
    document
        .charts()
        .iter()
        .map(|chart| chart.description.clone())
        .collect()
}
fn ack(issues: &[deadsync_editor::advanced_export::Issue]) -> BTreeSet<usize> {
    issues
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(index, _)| index)
        .collect()
}
#[test]
fn recorded_source_cultures_sort_output_and_song_pointer_together_but_native_and_legacy_stay_physical()
 {
    for (locale, expected, index) in [
        ("en-US", vec!["Ångström", "Éclair", "zebra"], 2),
        ("fr-FR", vec!["Ångström", "Éclair", "zebra"], 2),
        ("sv-SE", vec!["Éclair", "zebra", "Ångström"], 1),
    ] {
        let mut s = session();
        let native = s.serialize().unwrap();
        let culture = Arc::new(Culture(AtomicU64::new(1)));
        let order = capture(&mut s, &culture, locale);
        let output = Document::parse(
            generate(&mut s, &policy(Format::Ssc), Some(&order)),
            Format::Ssc,
        )
        .unwrap();
        assert_eq!(names(&output), expected);
        let metadata: serde_json::Value =
            serde_json::from_str(&output.text(None, "GASongData").unwrap()).unwrap();
        assert_eq!(metadata["DefaultChartIndex"], index);
        assert_eq!(s.song_timing_chart().unwrap(), Some(0));
        assert_eq!(s.serialize().unwrap(), native);
        let legacy =
            Document::parse(generate(&mut s, &policy(Format::Ssc), None), Format::Ssc).unwrap();
        assert_eq!(names(&legacy), ["zebra", "Ångström", "Éclair"]);
        let native_output = generate(&mut s, &OutputPolicy::NativeLossless, Some(&order));
        assert_eq!(
            names(&Document::parse(native_output, Format::Ssc).unwrap()),
            ["zebra", "Ångström", "Éclair"]
        );
    }
}
#[test]
fn explicit_timing_guid_does_not_turn_into_a_sorted_index_and_sm_custom_suffixes_follow_written_rank()
 {
    let mut s = session();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    let order = capture(&mut s, &culture, "fr-FR");
    let override_policy = OutputPolicy::Advanced {
        format: Format::Sm,
        options: Options {
            require_identical_timing_in_sm: false,
            ..Default::default()
        },
        timing_chart: s.chart_ids()[2].clone(),
    };
    let output =
        Document::parse(generate(&mut s, &override_policy, Some(&order)), Format::Sm).unwrap();
    assert_eq!(names(&output), ["Ångström", "Éclair", "zebra"]);
    assert_eq!(output.text(None, "BPMS").as_deref(), Some("0=240"));
    for (index, owner) in ["Ångström", "Éclair", "zebra"].into_iter().enumerate() {
        let fields: serde_json::Value = serde_json::from_str(
            &output
                .text(None, &format!("GAChartDataCHART{index:04}"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(fields["ChartOwner"], owner);
    }
    let fields: serde_json::Value =
        serde_json::from_str(&output.text(None, "GASongDataSONG").unwrap()).unwrap();
    assert_eq!(fields["DefaultChartIndex"], 2);
}
#[test]
fn locale_changes_without_a_frame_and_edit_undo_revoke_order_and_prepared_bytes() {
    let mut s = session();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    let order = capture(&mut s, &culture, "fr-FR");
    let prepared = policy(Format::Ssc)
        .prepare_ordered_output(&mut s, Limits::default(), &AtomicBool::new(false), &order)
        .unwrap();
    let output = prepared
        .generate(
            &s,
            &ack(prepared.issues()),
            128 * 1024 * 1024,
            &AtomicBool::new(false),
        )
        .unwrap();
    let native = OutputPolicy::NativeLossless
        .prepare_ordered_output(&mut s, Limits::default(), &AtomicBool::new(false), &order)
        .unwrap();
    culture.0.fetch_add(1, Ordering::Release);
    assert!(native.is_current(&s));
    assert!(!order.is_current(&s));
    assert!(!prepared.is_current(&s));
    assert!(!output.is_current(&s));
    assert!(
        prepared
            .generate(
                &s,
                &ack(prepared.issues()),
                128 * 1024 * 1024,
                &AtomicBool::new(false)
            )
            .is_err()
    );
    let order = capture(&mut s, &culture, "en-US");
    s.set_text(None, "TITLE", "interleave").unwrap();
    s.undo().unwrap();
    assert!(!order.is_current(&s));
    assert!(
        policy(Format::Ssc)
            .prepare_ordered_output(&mut s, Limits::default(), &AtomicBool::new(false), &order)
            .is_err()
    );
    let mut foreign = session();
    assert!(
        policy(Format::Ssc)
            .prepare_ordered_output(
                &mut foreign,
                Limits::default(),
                &AtomicBool::new(false),
                &order
            )
            .is_err()
    );
}
#[test]
fn storage_reviews_recheck_culture_and_save_as_rebinds_only_comparison_invariant_media_candidate() {
    let root = tempfile::tempdir().unwrap();
    let from = root.path().join("song.ssc");
    std::fs::write(&from, source()).unwrap();
    let mut file = FileSession::open(&from).unwrap();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    let order = capture(&mut file.session, &culture, "en-US");
    let review = file
        .prepare_save_with_policy_ordered(policy(Format::Ssc), &order)
        .unwrap();
    culture.0.fetch_add(1, Ordering::Release);
    assert!(!review.is_current(&file));
    assert!(file.save_reviewed(review, &BTreeSet::new()).is_err());
    assert_eq!(std::fs::read(&from).unwrap(), source().as_bytes());
    let order = capture(&mut file.session, &culture, "sv-SE");
    let destination = root.path().join("nested");
    std::fs::create_dir(&destination).unwrap();
    let target = destination.join("copy.ssc");
    let review = file
        .prepare_save_as_with_policy_and_revisions_ordered(
            &target,
            policy(Format::Ssc),
            &[],
            &order,
        )
        .unwrap();
    let acknowledged = ack(review.issues());
    let ids = file.session.chart_ids().to_vec();
    assert!(matches!(
        file.save_as_reviewed(review, &acknowledged).unwrap(),
        SaveAsOutcome::Durable { .. }
    ));
    assert_eq!(file.session.chart_ids(), ids);
    assert_eq!(
        names(file.session.document()),
        ["zebra", "Ångström", "Éclair"]
    );
    let output = Document::parse(std::fs::read(&target).unwrap(), Format::Ssc).unwrap();
    assert_eq!(names(&output), ["Éclair", "zebra", "Ångström"]);
    assert_eq!(output.text(None, "MUSIC").as_deref(), Some("../audio.ogg"));
}
#[test]
fn recovery_has_no_persisted_cultural_authority_and_current_save_reacquires_it() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    std::fs::write(&path, source()).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    let old = capture(&mut file.session, &culture, "en-US");
    let review = file
        .prepare_save_with_policy_ordered(policy(Format::Ssc), &old)
        .unwrap();
    let acknowledged = ack(review.issues());
    assert!(matches!(
        file.save_reviewed(review, &acknowledged).unwrap(),
        SaveAttempt::Saved
    ));
    file.session.set_text(None, "TITLE", "recovered").unwrap();
    file.save_recovery().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    reopened.recover().unwrap();
    assert!(!old.is_current(&reopened.session));
    let order = capture(&mut reopened.session, &culture, "sv-SE");
    let review = reopened
        .prepare_save_with_policy_ordered(reopened.output_policy().clone(), &order)
        .unwrap();
    let acknowledged = ack(review.issues());
    assert!(matches!(
        reopened.save_reviewed(review, &acknowledged).unwrap(),
        SaveAttempt::Saved
    ));
    assert_eq!(
        names(&Document::parse(std::fs::read(path).unwrap(), Format::Ssc).unwrap()),
        ["Éclair", "zebra", "Ångström"]
    );
}
#[test]
fn malformed_host_permutations_and_cancellation_never_grant_an_order() {
    struct Bad;
    impl Orderer for Bad {
        fn order(&mut self, _: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
            Ok(vec![0, 0, 2])
        }
    }
    let mut s = session();
    let bytes = s.serialize().unwrap();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    assert!(
        SourceOutputOrder::capture(&mut s, &mut Bad, culture.clone(), &AtomicBool::new(false))
            .is_err()
    );
    assert!(
        SourceOutputOrder::capture(
            &mut s,
            &mut ReferenceOrder { culture: "en-US" },
            culture,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert_eq!(s.serialize().unwrap(), bytes);
    assert!(!s.is_dirty());
}

#[test]
fn host_locale_change_during_ordering_never_mints_a_certificate() {
    struct Changing(Arc<Culture>);
    impl Orderer for Changing {
        fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
            self.0.0.fetch_add(1, Ordering::Release);
            Ok(snapshot.source_indices().to_vec())
        }
    }
    let mut s = session();
    let original = s.serialize().unwrap();
    let culture = Arc::new(Culture(AtomicU64::new(1)));
    assert!(
        SourceOutputOrder::capture(
            &mut s,
            &mut Changing(culture.clone()),
            culture,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert_eq!(s.serialize().unwrap(), original);
    assert!(!s.is_dirty());
}
