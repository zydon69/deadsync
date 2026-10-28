use deadsync_editor::{
    Error,
    advanced_export::{Options, Severity},
    document::{Document, Format},
    output_policy::OutputPolicy,
    storage::{FileSession, SaveAsOutcome, SaveAttempt},
};
use std::{collections::BTreeSet, fs, path::Path};
fn source() -> String {
    "#VERSION:0.83;#TITLE:Private title;#CREDIT:Private author;#MUSIC:audio.ogg;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;".into()
}
fn file(root: &Path) -> FileSession {
    let path = root.join("source.ssc");
    fs::write(&path, source()).unwrap();
    FileSession::open(&path).unwrap()
}
fn policy(file: &FileSession, format: Format, options: Options) -> OutputPolicy {
    OutputPolicy::Advanced {
        format,
        options,
        timing_chart: file.session.chart_ids()[0].clone(),
    }
}
fn acknowledgements(issues: &[deadsync_editor::advanced_export::Issue]) -> BTreeSet<usize> {
    assert!(
        !issues.iter().any(|issue| issue.severity == Severity::Error),
        "{issues:?}"
    );
    issues
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(index, _)| index)
        .collect()
}
fn convert(file: &mut FileSession, destination: &Path, format: Format, options: Options) {
    let review = file
        .prepare_save_as_with_policy(destination, policy(file, format, options))
        .unwrap();
    let ack = acknowledgements(review.issues());
    assert!(matches!(
        file.save_as_reviewed(review, &ack).unwrap(),
        SaveAsOutcome::Durable { .. }
    ));
}
#[test]
fn cross_format_uses_relocated_media_keeps_native_codec_and_structural_undo() {
    let root = tempfile::tempdir().unwrap();
    let mut file = file(root.path());
    file.session.set_text(None, "WARPS", "1=1").unwrap();
    let revision = file.session.revision();
    file.session.duplicate_chart(0, revision).unwrap();
    file.session.set_text(None, "WARPS", "").unwrap();
    let ids = file.session.chart_ids().to_vec();
    let destination_dir = root.path().join("Published");
    fs::create_dir(&destination_dir).unwrap();
    let destination = destination_dir.join("song.sm");
    convert(&mut file, &destination, Format::Sm, Options::default());
    assert_eq!(file.session.document().format, Format::Ssc);
    assert_eq!(file.session.chart_ids(), ids);
    assert_eq!(
        file.session.document().text(None, "MUSIC").as_deref(),
        Some("../audio.ogg")
    );
    let target = Document::parse(fs::read(&destination).unwrap(), Format::Sm).unwrap();
    assert_eq!(target.text(None, "MUSIC").as_deref(), Some("../audio.ogg"));
    assert_eq!(
        fs::read(root.path().join("source.ssc")).unwrap(),
        source().as_bytes()
    );
    let before = fs::read(&destination).unwrap();
    file.session.undo().unwrap();
    file.session.undo().unwrap();
    assert_eq!(file.session.document().format, Format::Ssc);
    assert_eq!(file.session.document().charts().len(), 1);
    assert!(file.session.is_dirty());
    match file.save_or_review().unwrap() {
        SaveAttempt::NeedsReview(review) => assert!(
            review
                .issues()
                .iter()
                .any(|issue| issue.severity == Severity::Error && issue.field == "WARPS")
        ),
        _ => panic!("unrepresentable Undo must require a new review"),
    };
    assert_eq!(fs::read(&destination).unwrap(), before);
}
#[test]
fn target_codec_survives_reverse_conversion_and_native_policy_cannot_write_wrong_extension() {
    let root = tempfile::tempdir().unwrap();
    let source_path = root.path().join("source.sm");
    fs::write(&source_path,"#TITLE:SM;#BPMS:0=120;#GASongDataVersionSONG:1;#GASongDataSONG:{\"SyncOffset\":0};#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;").unwrap();
    let mut file = FileSession::open(&source_path).unwrap();
    let target = root.path().join("target.ssc");
    convert(&mut file, &target, Format::Ssc, Options::default());
    assert_eq!(file.session.document().format, Format::Sm);
    let bytes = fs::read(&target).unwrap();
    assert!(
        file.prepare_save_with_policy(OutputPolicy::NativeLossless)
            .is_err()
    );
    assert_eq!(fs::read(&target).unwrap(), bytes);
    file.session.set_text(None, "TITLE", "Edited").unwrap();
    match file.save_or_review().unwrap() {
        SaveAttempt::Saved => {}
        SaveAttempt::NeedsReview(review) => {
            let ack = acknowledgements(review.issues());
            assert!(matches!(
                file.save_reviewed(review, &ack).unwrap(),
                SaveAttempt::Saved
            ));
        }
        _ => panic!("unexpected durability failure"),
    };
    assert!(
        std::str::from_utf8(&fs::read(&target).unwrap())
            .unwrap()
            .contains("#NOTEDATA:")
    );
}
#[test]
fn each_lossy_save_requires_new_typed_review_and_recovery_retains_native_metadata() {
    let root = tempfile::tempdir().unwrap();
    let mut file = file(root.path());
    let target = root.path().join("anonymous.ssc");
    let options = Options {
        anonymize: true,
        ..Options::default()
    };
    convert(&mut file, &target, Format::Ssc, options);
    let policy = file.output_policy().clone();
    let ids = file.session.chart_ids().to_vec();
    let bytes = fs::read(&target).unwrap();
    assert_eq!(
        Document::parse(bytes.clone(), Format::Ssc)
            .unwrap()
            .text(None, "CREDIT")
            .unwrap_or_default(),
        ""
    );
    assert_eq!(
        file.session.document().text(None, "CREDIT").as_deref(),
        Some("Private author")
    );
    file.session
        .set_text(None, "ARTIST", "Private artist")
        .unwrap();
    assert!(matches!(file.save(), Err(Error::OutputReviewRequired)));
    assert_eq!(fs::read(&target).unwrap(), bytes);
    file.save_recovery().unwrap();
    let recovery = fs::read(file.recovery_path().unwrap()).unwrap();
    assert!(recovery.starts_with(b"DEADSYNC-EDITOR-RECOVERY-2\n"));
    let mut reopened = FileSession::open(&target).unwrap();
    reopened.recover().unwrap();
    assert_eq!(reopened.output_policy(), &policy);
    assert_eq!(reopened.session.chart_ids(), ids);
    assert_eq!(reopened.session.document().format, Format::Ssc);
    assert_eq!(
        reopened.session.document().text(None, "CREDIT").as_deref(),
        Some("Private author")
    );
    assert_eq!(
        reopened.session.document().text(None, "ARTIST").as_deref(),
        Some("Private artist")
    );
    assert!(reopened.session.is_dirty());
}
#[test]
fn save_review_is_revoked_by_edits_and_by_policy_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let mut file = file(root.path());
    let stale = file.prepare_save().unwrap();
    file.session.set_text(None, "TITLE", "Intervening").unwrap();
    file.session.undo().unwrap();
    assert!(file.save_reviewed(stale, &BTreeSet::new()).is_err());
    let stale = file.prepare_save().unwrap();
    let first = file
        .prepare_save_with_policy(policy(&file, Format::Ssc, Options::default()))
        .unwrap();
    let ack = acknowledgements(first.issues());
    assert!(matches!(
        file.save_reviewed(first, &ack).unwrap(),
        SaveAttempt::Saved
    ));
    let second = file
        .prepare_save_with_policy(OutputPolicy::NativeLossless)
        .unwrap();
    assert!(matches!(
        file.save_reviewed(second, &BTreeSet::new()).unwrap(),
        SaveAttempt::Saved
    ));
    assert!(file.save_reviewed(stale, &BTreeSet::new()).is_err());
}
#[test]
fn unacknowledged_save_as_and_stale_review_do_not_publish_or_install_policy() {
    let root = tempfile::tempdir().unwrap();
    let mut file = file(root.path());
    let target = root.path().join("target.sm");
    let selected = policy(
        &file,
        Format::Sm,
        Options {
            anonymize: true,
            ..Options::default()
        },
    );
    let review = file
        .prepare_save_as_with_policy(&target, selected.clone())
        .unwrap();
    assert!(!review.issues().is_empty());
    assert!(matches!(
        file.save_as(review),
        Err(Error::OutputReviewRequired)
    ));
    assert!(!target.exists());
    assert!(matches!(file.output_policy(), OutputPolicy::NativeLossless));
    let review = file.prepare_save_as_with_policy(&target, selected).unwrap();
    let ack = acknowledgements(review.issues());
    file.session.set_text(None, "TITLE", "Intervening").unwrap();
    file.session.undo().unwrap();
    assert!(file.save_as_reviewed(review, &ack).is_err());
    assert!(!target.exists());
    assert!(matches!(file.output_policy(), OutputPolicy::NativeLossless));
}
#[test]
fn legacy_v1_recovery_still_loads_with_native_policy() {
    let root = tempfile::tempdir().unwrap();
    let mut file = file(root.path());
    let baseline = fs::read(file.require_existing_path().unwrap()).unwrap();
    let recovered = source().replace("Private title", "Legacy draft");
    let mut bytes = b"DEADSYNC-EDITOR-RECOVERY-1\n".to_vec();
    bytes.extend_from_slice(&(baseline.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&baseline);
    bytes.extend_from_slice(recovered.as_bytes());
    fs::write(file.recovery_path().unwrap(), bytes).unwrap();
    file.recover().unwrap();
    assert_eq!(
        file.session.document().text(None, "TITLE").as_deref(),
        Some("Legacy draft")
    );
    assert!(matches!(file.output_policy(), OutputPolicy::NativeLossless));
    assert!(file.session.is_dirty());
}
