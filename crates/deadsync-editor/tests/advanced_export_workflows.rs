use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
};
fn source(charts: &[(&str, &str, &str)]) -> String {
    let mut out = String::from(
        "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};",
    );
    for (layout, notes, timing) in charts {
        out += &format!(
            "#NOTEDATA:;#STEPSTYPE:{layout};#DESCRIPTION:Credit;#DIFFICULTY:Hard;#METER:9;{timing}#NOTES:{notes};"
        );
    }
    out
}
fn session(text: String) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(text.into_bytes()), Format::Ssc).unwrap())
}
fn plan(session: &mut Session, target: Format, options: Options) -> Plan {
    Plan::prepare(
        session,
        target,
        0,
        options,
        Limits::default(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn kind(plan: &Plan, kind: IssueKind) -> bool {
    plan.issues().iter().any(|i| i.kind == kind)
}
#[test]
fn six_typed_options_keep_defaults_unknown_fields_and_reject_wrong_types() {
    let mut root = json!({"Version":1,"Vendor":{"keep":true}})
        .as_object()
        .unwrap()
        .clone();
    let defaults = Options::read(&root).unwrap();
    assert_eq!(defaults, Options::default());
    assert!(defaults.outfox_smx && defaults.require_identical_timing_in_sm);
    let changed = Options {
        require_identical_timing_in_sm: false,
        omit_chart_timing: true,
        omit_custom_data: true,
        anonymize: true,
        stepf2_pump_multiplayer: true,
        outfox_smx: false,
    };
    changed.write(&mut root);
    assert_eq!(Options::read(&root).unwrap(), changed);
    assert_eq!(root["Vendor"], json!({"keep":true}));
    for name in Options::NAMES {
        for value in [Value::Null, json!(1), json!("true"), json!([])] {
            let mut wrong = root.clone();
            wrong.insert(name.into(), value);
            assert!(Options::read(&wrong).is_err());
        }
    }
}
#[test]
fn review_is_immutable_atomic_and_bound_to_exact_session_content() {
    let mut s = session(source(&[("dance-single", "1000", "")]));
    let before = s.serialize().unwrap();
    let state = (
        s.revision(),
        s.content_revision(),
        s.chart_ids().to_vec(),
        s.undo_label().map(str::to_owned),
    );
    let p = plan(&mut s, Format::Ssc, Options::default());
    assert!(p.issues().is_empty());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(
        p.source_bytes(),
        s.serialize_for_source_compatible().unwrap()
    );
    let prepared_document = Document::parse(p.source_bytes(), Format::Ssc).unwrap();
    let metadata: Value =
        serde_json::from_str(&prepared_document.text(None, "GASongData").unwrap()).unwrap();
    assert_eq!(metadata["SyncOffset"], json!(0));
    assert_eq!(metadata["DefaultChartType"], json!("dance_single"));
    assert_eq!(metadata["DefaultChartIndex"], json!(0));
    p.validate_review(&s, &BTreeSet::new()).unwrap();
    assert_eq!(
        state,
        (
            s.revision(),
            s.content_revision(),
            s.chart_ids().to_vec(),
            s.undo_label().map(str::to_owned)
        )
    );
    assert!(!s.is_dirty());
    let other = session(source(&[("dance-single", "1000", "")]));
    assert!(!p.is_current(&other));
    s.edit_notes(
        0,
        "pending",
        [(
            Position {
                player: 0,
                lane: 1,
                row: 48,
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    assert!(!p.is_current(&s));
    assert!(p.validate_review(&s, &BTreeSet::new()).is_err());
    assert!(s.undo().unwrap());
    assert!(p.is_current(&s));
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn shared_timing_compares_typed_inheritance_and_attacks_stay_independent_in_ssc() {
    let mut s = session(source(&[
        ("dance-single", "1000", ""),
        ("dance-double", "10000000", "#BPMS:0.000=120.0;"),
    ]));
    assert!(!kind(
        &plan(&mut s, Format::Sm, Options::default()),
        IssueKind::UnequalTiming
    ));
    let mut changed = session(source(&[
        ("dance-single", "1000", ""),
        ("dance-double", "10000000", "#BPMS:0=150;#OFFSET:0.1;"),
    ]));
    let p = plan(&mut changed, Format::Sm, Options::default());
    assert!(
        p.issues()
            .iter()
            .any(|i| i.kind == IssueKind::UnequalTiming && i.severity == Severity::Error)
    );
    let p = plan(
        &mut changed,
        Format::Sm,
        Options {
            require_identical_timing_in_sm: false,
            ..Options::default()
        },
    );
    assert!(
        p.issues()
            .iter()
            .any(|i| i.kind == IssueKind::UnequalTiming && i.severity == Severity::Warning)
    );
    assert!(p.validate_review(&changed, &BTreeSet::new()).is_err());
    let all = (0..p.issues().len()).collect();
    p.validate_review(&changed, &all).unwrap();
    let mut attacks = session(source(&[
        ("dance-single", "1000", ""),
        (
            "dance-double",
            "10000000",
            "#ATTACKS:TIME=0:LEN=1:MODS=drunk;",
        ),
    ]));
    let p = plan(
        &mut attacks,
        Format::Ssc,
        Options {
            omit_chart_timing: true,
            ..Options::default()
        },
    );
    assert!(
        !p.issues()
            .iter()
            .any(|i| i.field == "ATTACKS" && i.kind == IssueKind::UnequalSharedEvent)
    );
    assert!(kind(
        &plan(&mut attacks, Format::Sm, Options::default()),
        IssueKind::UnequalSharedEvent
    ));
}
#[test]
fn stepf2_preflight_protects_couple_too_and_detects_shared_lane_loss() {
    for layout in ["pump-routine", "pump-couple"] {
        let mut s = session(source(&[(layout, "4000000000\n3000000000&0000000001", "")]));
        let p = plan(
            &mut s,
            Format::Ssc,
            Options {
                stepf2_pump_multiplayer: true,
                ..Options::default()
            },
        );
        assert!(kind(&p, IssueKind::StepF2NoteKind));
        if layout == "pump-couple" {
            assert!(kind(&p, IssueKind::StepF2Layout));
        }
    }
    let mut owners = session(source(&[(
        "pump-routine",
        "1000000000&1000000000&0000000000&0000000000&0000000001",
        "",
    )]));
    let p = plan(
        &mut owners,
        Format::Ssc,
        Options {
            stepf2_pump_multiplayer: true,
            ..Options::default()
        },
    );
    assert!(kind(&p, IssueKind::StepF2Players));
    assert!(kind(&p, IssueKind::StepF2Collision));
    assert!(
        p.validate_review(&owners, &(0..p.issues().len()).collect())
            .is_err()
    );
}
fn decode(s: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
    for b in s.bytes().take_while(|v| *v != b'=') {
        bits = (bits << 6) | alphabet.iter().position(|v| *v == b).unwrap() as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}
#[test]
fn sixty_real_writer_cases_are_reviewed_before_any_output_and_losses_not_silently_approved() {
    let request: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/save-format.request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/save-format.expected.json"
    ))
    .unwrap();
    let mut count = 0;
    let mut blocked_couple = 0;
    for (input, result) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(input["Name"], result["Name"]);
        assert_eq!(result["Saved"], true);
        assert!(result["Error"].is_null());
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let format = if input["Source"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let target = if input["Target"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let mut s =
            Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), format).unwrap());
        let options = Options {
            omit_chart_timing: input["ForceTiming"].as_bool().unwrap(),
            stepf2_pump_multiplayer: input["StepF2"].as_bool().unwrap(),
            outfox_smx: input["Outfox"].as_bool().unwrap(),
            omit_custom_data: !input["Custom"].as_bool().unwrap(),
            ..Options::default()
        };
        let p = plan(&mut s, target, options);
        assert_eq!(s.serialize().unwrap(), bytes);
        assert!(!s.is_dirty());
        if input["Name"].as_str().unwrap().starts_with("pump-couple") {
            assert!(kind(&p, IssueKind::StepF2Layout));
            assert!(
                p.validate_review(&s, &(0..p.issues().len()).collect())
                    .is_err()
            );
            blocked_couple += 1;
        }
        // The original writer's true result is retained alongside full emitted bytes
        // and typed read-back; it is deliberately not treated as proof of safety.
        assert!(!decode(result["Bytes"].as_str().unwrap()).is_empty());
        count += 1;
    }
    assert_eq!(count, 60);
    assert_eq!(blocked_couple, 10);
}
#[test]
fn cancellation_limits_unknowns_and_failed_preflight_preserve_source_and_redo() {
    let mut s = session(source(&[("dance-single", "1000", "")]));
    s.set_text(None, "TITLE", "temporary").unwrap();
    s.undo().unwrap();
    let state = (
        s.serialize().unwrap(),
        s.revision(),
        s.content_revision(),
        s.redo_label().map(str::to_owned),
    );
    for limits in [
        Limits {
            bytes: 1,
            ..Limits::default()
        },
        Limits {
            charts: 0,
            ..Limits::default()
        },
        Limits {
            notes: 0,
            ..Limits::default()
        },
    ] {
        assert!(
            Plan::prepare(
                &mut s,
                Format::Sm,
                0,
                Options::default(),
                limits,
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
    assert!(
        Plan::prepare(
            &mut s,
            Format::Sm,
            0,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert_eq!(
        state,
        (
            s.serialize().unwrap(),
            s.revision(),
            s.content_revision(),
            s.redo_label().map(str::to_owned)
        )
    );
    let mut unknown = session(source(&[("dance-single", "1000", "")]) + "#GAVENDOR:do not delete;");
    let p = plan(
        &mut unknown,
        Format::Ssc,
        Options {
            omit_custom_data: true,
            ..Options::default()
        },
    );
    assert!(kind(&p, IssueKind::UnknownProperty));
}

#[test]
fn shared_preference_file_saves_six_options_conflict_safely_without_touching_simfiles() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("editor.json");
    std::fs::write(&path, br#"{"Version":1,"Vendor":{"keep":true}}"#).unwrap();
    let mut file = deadsync_editor::audio_preferences::File::open(&path).unwrap();
    assert_eq!(file.advanced_save(), Options::default());
    let options = Options {
        omit_custom_data: true,
        outfox_smx: false,
        ..Options::default()
    };
    file.set_advanced_save(options);
    assert!(file.has_changes());
    assert!(file.save().unwrap());
    let reopened = deadsync_editor::audio_preferences::File::open(&path).unwrap();
    assert_eq!(reopened.advanced_save(), options);
    let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["Vendor"], json!({"keep":true}));
    file.set_advanced_save(Options::default());
    let external = br#"{"Version":1,"Vendor":"external"}"#;
    std::fs::write(&path, external).unwrap();
    assert!(file.save().is_err());
    assert!(file.has_changes());
    assert_eq!(std::fs::read(&path).unwrap(), external);
}

#[test]
fn valid_stepf2_decorations_use_existing_codec_and_are_not_blanket_rejected() {
    let mut s = session(source(&[("pump-routine", "{X|h|1|%}[7]000000000", "")]));
    let p = plan(
        &mut s,
        Format::Ssc,
        Options {
            stepf2_pump_multiplayer: true,
            ..Options::default()
        },
    );
    assert!(!kind(&p, IssueKind::StepF2Attributes));
    assert!(!kind(&p, IssueKind::StepF2NoteKind));
    let standard = plan(&mut s, Format::Ssc, Options::default());
    assert!(kind(&standard, IssueKind::StepF2Attributes));
    let mut unknown = session(source(&[("dance-single", "1000", "")]) + "#VENDOR:raw;");
    assert!(
        Plan::prepare(
            &mut unknown,
            Format::Ssc,
            0,
            Options::default(),
            Limits {
                issues: 0,
                ..Limits::default()
            },
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
