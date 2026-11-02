use deadsync_editor::{
    advanced_export::{self, Limits, Options, Severity},
    chart_creation,
    document::{Document, Format},
    generation::metadata::Guid,
    history::Session,
    output_policy::OutputPolicy,
    storage::{FileSession, SaveAsOutcome, SaveAttempt},
};
use std::{collections::BTreeSet, fs, path::Path, sync::atomic::AtomicBool};
fn format(value: &str) -> Format {
    match value {
        "sm" => Format::Sm,
        "ssc" => Format::Ssc,
        _ => panic!("unknown format"),
    }
}
fn ack(issues: &[advanced_export::Issue]) -> BTreeSet<usize> {
    assert!(
        issues
            .iter()
            .all(|issue| issue.severity == Severity::Warning)
    );
    issues.iter().enumerate().map(|(index, _)| index).collect()
}
fn policy(target: Format, anonymize: bool, omit: bool) -> OutputPolicy {
    OutputPolicy::AdvancedChartless {
        format: target,
        options: Options {
            anonymize,
            omit_custom_data: omit,
            ..Default::default()
        },
    }
}
fn fixture(root: &Path, source: Format) -> FileSession {
    let path = root.join(format!("source.{}", source.extension()));
    fs::write(
        &path,
        format!(
            "{}#TITLE:Native;#CREDIT:Hidden author;#OFFSET:0.25;",
            if source == Format::Ssc {
                "#VERSION:0.83;"
            } else {
                ""
            }
        ),
    )
    .unwrap();
    FileSession::open(&path).unwrap()
}
#[test]
fn pinned_null_fallback_writer_oracle_covers_sixteen_chartless_format_option_cases() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chartless-export");
    let request: serde_json::Value =
        serde_json::from_slice(&fs::read(base.join("request.json")).unwrap()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(base.join("expected.json")).unwrap()).unwrap();
    assert_eq!(request.as_array().unwrap().len(), 16);
    for (case, reference) in request
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
    {
        assert_eq!(case["name"], reference["name"]);
        assert_eq!(reference["initial_charts"], 0);
        assert_eq!(reference["output_charts"], 0);
        let source = format(case["source"].as_str().unwrap());
        let target = format(case["target"].as_str().unwrap());
        let mut input = case["input"].as_str().unwrap().to_owned();
        let suffix = if source == Format::Sm { "SONG" } else { "" };
        input.push_str(&format!("#GASongDataVersion{suffix}:1;#GASongData{suffix}:{{\"SyncOffset\":0.25,\"FutureOwner\":57}};"));
        let mut session = Session::new(Document::parse(input.into_bytes(), source).unwrap());
        let native = session.serialize().unwrap();
        let selected = policy(
            target,
            case["anonymize"].as_bool().unwrap(),
            case["omit"].as_bool().unwrap(),
        );
        let plan = selected
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
            .unwrap();
        for field in [
            "BPMS",
            "STOPS",
            "DELAYS",
            "WARPS",
            "SCROLLS",
            "SPEEDS",
            "TIMESIGNATURES",
            "TICKCOUNTS",
            "COMBOS",
            "FAKES",
            "LABELS",
            "ATTACKS",
            "DISPLAYBPM",
        ] {
            assert!(
                plan.issues().iter().any(|issue| issue.chart.is_none()
                    && issue.field == field
                    && issue.severity == Severity::Warning),
                "{} lost field {field} must appear before publication",
                case["name"]
            );
        }
        if target == Format::Sm {
            assert!(plan.issues().iter().any(
                |issue| issue.field == "LASTSECONDHINT" && issue.severity == Severity::Warning
            ));
        }
        let accepted = ack(plan.issues());
        let output = plan
            .generate(
                &session,
                &accepted,
                Limits::default().bytes,
                &AtomicBool::new(false),
            )
            .unwrap();
        let document = Document::parse(output.bytes().to_vec(), target).unwrap();
        assert!(document.charts().is_empty());
        assert!(session.chart_ids().is_empty());
        assert_eq!(session.serialize().unwrap(), native);
        assert_eq!(
            document.text(None, "TITLE").as_deref(),
            reference["title"].as_str()
        );
        for field in [
            "BPMS",
            "STOPS",
            "DELAYS",
            "WARPS",
            "SCROLLS",
            "SPEEDS",
            "TICKCOUNTS",
            "COMBOS",
            "FAKES",
            "LABELS",
            "ATTACKS",
            "DISPLAYBPM",
        ] {
            assert!(
                document
                    .text(None, field)
                    .unwrap_or_default()
                    .trim()
                    .is_empty(),
                "{} {field}",
                case["name"]
            );
        }
        assert_eq!(
            document.text(None, "TIMESIGNATURES").as_deref(),
            Some("0=4=4")
        );
        assert_eq!(
            document
                .text(None, "OFFSET")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            reference["tags"]["OFFSET"]
                .as_str()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        );
        assert_eq!(document.text(None, "MUSIC").as_deref(), Some("music.ogg"));
        assert_eq!(
            document.text(None, "CREDIT").unwrap_or_default(),
            reference["tags"]["CREDIT"].as_str().unwrap()
        );
        assert_eq!(
            document.text(None, "GENRE").unwrap_or_default(),
            reference["tags"]["GENRE"].as_str().unwrap()
        );
        let custom = document
            .tags()
            .iter()
            .filter(|tag| tag.name.starts_with("GASONGDATA"))
            .collect::<Vec<_>>();
        if case["omit"].as_bool().unwrap() {
            assert!(custom.is_empty());
        } else {
            assert_eq!(custom.len(), 2);
            let name = if target == Format::Sm {
                "GASONGDATASONG"
            } else {
                "GASONGDATA"
            };
            let value = document.text(None, name).unwrap();
            assert!(value.contains("FutureOwner") && value.contains("57"));
        }
    }
}
#[test]
fn old_index_api_and_strict_guid_policy_never_invent_a_chartless_fallback() {
    let mut session = Session::new(
        Document::parse(b"#VERSION:0.83;#TITLE:Empty;".to_vec(), Format::Ssc).unwrap(),
    );
    assert!(
        advanced_export::Plan::prepare(
            &mut session,
            Format::Sm,
            0,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        OutputPolicy::Advanced {
            format: Format::Sm,
            options: Default::default(),
            timing_chart: Guid::random()
        }
        .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
        .is_err()
    );
    assert_eq!(
        advanced_export::Plan::prepare_chartless(
            &mut session,
            Format::Sm,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(false)
        )
        .unwrap()
        .timing_chart(),
        None
    );
    let draft = chart_creation::Draft::capture(&session).unwrap();
    draft.apply(&mut session, "dance-single").unwrap();
    assert!(
        advanced_export::Plan::prepare_chartless(
            &mut session,
            Format::Sm,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        policy(Format::Sm, false, false)
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
            .is_err()
    );
    let selected = OutputPolicy::Advanced {
        format: Format::Sm,
        options: Default::default(),
        timing_chart: session.chart_ids()[0].clone(),
    };
    assert!(
        selected
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
            .is_ok()
    );
    assert_eq!(
        advanced_export::Plan::prepare(
            &mut session,
            Format::Sm,
            0,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(false)
        )
        .unwrap()
        .timing_chart(),
        Some(0)
    );
}
#[test]
fn chart_creation_and_undo_revoke_old_review_without_weakening_chartless_policy() {
    let root = tempfile::tempdir().unwrap();
    let mut file = fixture(root.path(), Format::Ssc);
    let selected = policy(Format::Ssc, false, false);
    let review = file.prepare_save_with_policy(selected.clone()).unwrap();
    let accepted = ack(review.issues());
    file.save_reviewed(review, &accepted).unwrap();
    let old = file.prepare_save().unwrap();
    let draft = chart_creation::Draft::capture(&file.session).unwrap();
    draft.apply(&mut file.session, "dance-single").unwrap();
    assert!(file.prepare_save().is_err());
    assert!(!old.is_current(&file));
    file.session.undo().unwrap();
    assert!(file.prepare_save().is_ok());
    assert!(!old.is_current(&file));
    assert!(file.save_reviewed(old, &BTreeSet::new()).is_err());
    assert_eq!(file.output_policy(), &selected);
}
#[test]
fn cross_format_chartless_save_and_v2_recovery_keep_native_codec_metadata_and_policy() {
    for (source, target) in [(Format::Sm, Format::Ssc), (Format::Ssc, Format::Sm)] {
        let root = tempfile::tempdir().unwrap();
        let mut file = fixture(root.path(), source);
        let destination = root.path().join(format!("output.{}", target.extension()));
        let selected = policy(target, true, false);
        let review = file
            .prepare_save_as_with_policy(&destination, selected.clone())
            .unwrap();
        let accepted = ack(review.issues());
        assert!(matches!(
            file.save_as_reviewed(review, &accepted).unwrap(),
            SaveAsOutcome::Durable { .. }
        ));
        assert_eq!(file.session.document().format, source);
        assert!(file.session.chart_ids().is_empty());
        assert!(
            Document::parse(fs::read(&destination).unwrap(), target)
                .unwrap()
                .charts()
                .is_empty()
        );
        file.session
            .set_text(None, "CREDIT", "Unsaved private author")
            .unwrap();
        file.save_recovery().unwrap();
        let mut reopened = FileSession::open(&destination).unwrap();
        reopened.recover().unwrap();
        assert_eq!(reopened.session.document().format, source);
        assert_eq!(reopened.output_policy(), &selected);
        assert!(reopened.session.chart_ids().is_empty());
        assert_eq!(
            reopened.session.document().text(None, "CREDIT").as_deref(),
            Some("Unsaved private author")
        );
        assert!(reopened.session.is_dirty());
        let fresh = reopened.prepare_save().unwrap();
        let accepted = ack(fresh.issues());
        assert!(matches!(
            reopened.save_reviewed(fresh, &accepted).unwrap(),
            SaveAttempt::Saved
        ));
    }
}
#[test]
fn source_null_fallback_timing_loss_requires_ack_before_any_destination_is_created() {
    let root = tempfile::tempdir().unwrap();
    let mut file = fixture(root.path(), Format::Ssc);
    file.session.set_text(None, "BPMS", "0=123").unwrap();
    let destination = root.path().join("empty.sm");
    let selected = policy(Format::Sm, false, false);
    let review = file
        .prepare_save_as_with_policy(&destination, selected)
        .unwrap();
    assert!(
        review
            .issues()
            .iter()
            .any(|issue| issue.field == "BPMS" && issue.severity == Severity::Warning)
    );
    assert!(file.save_as(review).is_err());
    assert!(!destination.exists());
    let review = file
        .prepare_save_as_with_policy(&destination, policy(Format::Sm, false, false))
        .unwrap();
    let accepted = ack(review.issues());
    file.save_as_reviewed(review, &accepted).unwrap();
    assert_eq!(
        file.session.document().text(None, "BPMS").as_deref(),
        Some("0=123")
    );
    assert!(
        Document::parse(fs::read(destination).unwrap(), Format::Sm)
            .unwrap()
            .text(None, "BPMS")
            .unwrap_or_default()
            .is_empty()
    );
}
#[test]
fn chartless_limits_cancel_and_invalid_music_offset_refuse_without_mutation() {
    let mut session = Session::new(
        Document::parse(b"#VERSION:0.83;#OFFSET:not-a-number;".to_vec(), Format::Ssc).unwrap(),
    );
    let before = session.serialize().unwrap();
    let selected = policy(Format::Sm, false, false);
    assert!(
        selected
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    // Limits/cancellation need valid input, or the malformed OFFSET would make
    // those assertions pass before their intended boundary is ever reached.
    let mut session = Session::new(
        Document::parse(b"#VERSION:0.83;#OFFSET:0.25;".to_vec(), Format::Ssc).unwrap(),
    );
    let before = session.serialize().unwrap();
    assert!(
        selected
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(false))
            .is_ok()
    );
    assert!(
        selected
            .prepare_output(
                &mut session,
                Limits {
                    bytes: 4,
                    ..Limits::default()
                },
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert!(
        selected
            .prepare_output(&mut session, Limits::default(), &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
}

#[test]
fn crash_recovery_keeps_native_chart_added_under_chartless_output_until_explicit_timing_choice() {
    let root = tempfile::tempdir().unwrap();
    let mut file = fixture(root.path(), Format::Ssc);
    let selected = policy(Format::Ssc, true, false);
    let review = file.prepare_save_with_policy(selected.clone()).unwrap();
    let accepted = ack(review.issues());
    file.save_reviewed(review, &accepted).unwrap();
    let draft = chart_creation::Draft::capture(&file.session).unwrap();
    draft.apply(&mut file.session, "dance-single").unwrap();
    recover_requires_explicit_chart(&mut file, selected);
}
#[test]
fn crash_recovery_keeps_native_draft_after_selected_timing_guid_is_deleted() {
    let root = tempfile::tempdir().unwrap();
    let mut file = fixture(root.path(), Format::Ssc);
    for _ in 0..2 {
        let draft = chart_creation::Draft::capture(&file.session).unwrap();
        draft.apply(&mut file.session, "dance-single").unwrap();
    }
    let selected = OutputPolicy::Advanced {
        format: Format::Ssc,
        options: Options {
            anonymize: true,
            ..Default::default()
        },
        timing_chart: file.session.chart_ids()[0].clone(),
    };
    let review = file.prepare_save_with_policy(selected.clone()).unwrap();
    let accepted = ack(review.issues());
    file.save_reviewed(review, &accepted).unwrap();
    let draft = deadsync_editor::chart_deletion::Draft::capture(&file.session, 0).unwrap();
    draft.apply(&mut file.session).unwrap();
    recover_requires_explicit_chart(&mut file, selected);
}
fn recover_requires_explicit_chart(file: &mut FileSession, selected: OutputPolicy) {
    file.session
        .set_text(None, "CREDIT", "Native author survives recovery")
        .unwrap();
    let native = file.session.serialize().unwrap();
    let ids = file.session.chart_ids().to_vec();
    assert!(file.prepare_save().is_err());
    file.save_recovery().unwrap();
    let path = file.require_existing_path().unwrap().to_owned();
    let mut recovered = FileSession::open(&path).unwrap();
    recovered.recover().unwrap();
    assert_eq!(recovered.output_policy(), &selected);
    assert_eq!(recovered.session.chart_ids(), ids);
    assert_eq!(recovered.session.serialize().unwrap(), native);
    assert!(recovered.session.is_dirty());
    assert!(recovered.prepare_save().is_err());
    let chosen = OutputPolicy::Advanced {
        format: Format::Ssc,
        options: Options {
            anonymize: true,
            ..Default::default()
        },
        timing_chart: ids[0].clone(),
    };
    let review = recovered.prepare_save_with_policy(chosen.clone()).unwrap();
    let accepted = ack(review.issues());
    recovered.save_reviewed(review, &accepted).unwrap();
    assert_eq!(recovered.output_policy(), &chosen);
    assert_eq!(
        recovered.session.document().text(None, "CREDIT").as_deref(),
        Some("Native author survives recovery")
    );
}

#[test]
fn ignored_sm_timing_tags_require_loss_ack_without_becoming_effective_events() {
    use deadsync_editor::{
        advanced_export::IssueKind, event_clipboard::Selection,
        generation::timing::GenerationTiming,
    };
    let raw =
        "#WARPS:2=1;#FAKES:2=1;#COMBOS:2=2=3;#LABELS:2=Opaque;#SCROLLS:2=-1;#SPEEDS:2=1.5=0=0;";
    let fields = ["WARPS", "FAKES", "COMBOS", "LABELS", "SCROLLS", "SPEEDS"];
    for with_chart in [false, true] {
        let chart = if with_chart {
            "#NOTES:dance-single::Hard:9::1000;"
        } else {
            ""
        };
        let clean = format!("#TITLE:Keep;#BPMS:0=120;{chart}");
        let input = format!("#TITLE:Keep;#BPMS:0=120;{raw}{chart}");
        let mut session = Session::new(Document::parse(input.as_bytes(), Format::Sm).unwrap());
        if with_chart {
            let mut control = Session::new(Document::parse(clean.as_bytes(), Format::Sm).unwrap());
            let actual = Selection::capture(&mut session, 0, 0, 192).unwrap();
            let expected = Selection::capture(&mut control, 0, 0, 192).unwrap();
            assert_eq!(
                actual.events(),
                expected.events(),
                "raw tags must not create editor timing events"
            );
            assert_eq!(
                GenerationTiming::from_simfile(input.as_bytes(), Format::Sm, 0)
                    .unwrap()
                    .times([0, 48, 96, 192])
                    .unwrap(),
                GenerationTiming::from_simfile(clean.as_bytes(), Format::Sm, 0)
                    .unwrap()
                    .times([0, 48, 96, 192])
                    .unwrap()
            );
        }
        let before = session.serialize().unwrap();
        for target in [Format::Sm, Format::Ssc] {
            let plan = if with_chart {
                advanced_export::Plan::prepare(
                    &mut session,
                    target,
                    0,
                    Options::default(),
                    Limits::default(),
                    &AtomicBool::new(false),
                )
            } else {
                advanced_export::Plan::prepare_chartless(
                    &mut session,
                    target,
                    Options::default(),
                    Limits::default(),
                    &AtomicBool::new(false),
                )
            }
            .unwrap();
            for field in fields {
                assert!(
                    plan.issues().iter().any(|issue| issue.chart.is_none()
                        && issue.field == field
                        && issue.kind == IssueKind::UnknownProperty
                        && issue.severity == Severity::Warning),
                    "opaque {field} loss must be visible"
                );
            }
            assert!(
                plan.generate(
                    &session,
                    &BTreeSet::new(),
                    Limits::default().bytes,
                    &AtomicBool::new(false)
                )
                .is_err()
            );
            let output = plan
                .generate(
                    &session,
                    &ack(plan.issues()),
                    Limits::default().bytes,
                    &AtomicBool::new(false),
                )
                .unwrap();
            let document = Document::parse(output.bytes().to_vec(), target).unwrap();
            for field in fields {
                for scope in [None, with_chart.then_some(0)] {
                    assert!(
                        !document
                            .text(scope, field)
                            .unwrap_or_default()
                            .split(',')
                            .any(|part| part.split('=').next().is_some_and(|beat| beat
                                .trim()
                                .parse::<f64>()
                                .ok()
                                == Some(2.))),
                        "ignored raw {field} at beat2 must not activate in exported timing"
                    );
                }
            }
            assert_eq!(
                session.serialize().unwrap(),
                before,
                "review/export must preserve native raw data"
            );
        }
    }
}
