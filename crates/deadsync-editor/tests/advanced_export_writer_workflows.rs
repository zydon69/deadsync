use deadsync_editor::{
    advanced_export::{Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
    notes::{Kind, Position},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
};
fn session(format: Format) -> Session {
    let charts = match format {
        Format::Sm => {
            "#NOTES:dance-single:Author:Hard:9:0,0,0,0,0:1000\n0001;#NOTES:dance-double:Peer:Easy:2::00000001;"
        }
        Format::Ssc => {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Author;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0001;#NOTEDATA:;#STEPSTYPE:dance-double;#DESCRIPTION:Peer;#DIFFICULTY:Easy;#METER:2;#NOTES:00000001;"
        }
    };
    Session::new(
        Document::parse(
            format!(
                "#VERSION:0.83;#TITLE:Keep;#ARTIST:Artist;#MUSIC:music.ogg;#BPMS:0=120;{charts}"
            )
            .into_bytes(),
            format,
        )
        .unwrap(),
    )
}
fn prepare(s: &mut Session, target: Format, options: Options) -> Plan {
    Plan::prepare(
        s,
        target,
        0,
        options,
        Limits::default(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn acknowledged(p: &Plan) -> BTreeSet<usize> {
    (0..p.issues().len()).collect()
}
#[test]
fn both_directions_generate_detached_output_and_preserve_source_history() {
    for format in [Format::Sm, Format::Ssc] {
        for target in [Format::Sm, Format::Ssc] {
            let mut s = session(format);
            let before = s.serialize().unwrap();
            let ids = s.chart_ids().to_vec();
            let p = prepare(&mut s, target, Options::default());
            let output = p
                .generate(&s, &acknowledged(&p), 1024 * 1024, &AtomicBool::new(false))
                .unwrap();
            assert_eq!(output.format(), target);
            assert!(output.is_current(&s));
            let document = Document::parse(Arc::<[u8]>::from(output.bytes()), target).unwrap();
            assert_eq!(document.charts().len(), 2);
            assert_eq!(document.text(None, "TITLE").as_deref(), Some("Keep"));
            assert_eq!(document.text(None, "ARTIST").as_deref(), Some("Artist"));
            assert_eq!(s.serialize().unwrap(), before);
            assert_eq!(s.chart_ids(), ids);
            assert!(!s.is_dirty());
            assert!(s.undo_label().is_none());
            assert_eq!(s.document().format, format);
        }
    }
}
#[test]
fn stepf2_export_uses_native_codec_preserves_fake_flags_and_flattens_only_reviewed_mines() {
    for notes in [
        "F000000000&0000000001",
        "1000000000&000000000M",
        "{X|h|1|%}[7]000000000",
    ] {
        let mut s=Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:{notes};").into_bytes(),Format::Ssc).unwrap());
        let options = Options {
            stepf2_pump_multiplayer: true,
            ..Options::default()
        };
        for target in [Format::Sm, Format::Ssc] {
            let p = prepare(&mut s, target, options);
            if notes.contains('M') {
                assert!(
                    p.generate(&s, &BTreeSet::new(), 1024 * 1024, &AtomicBool::new(false))
                        .is_err()
                );
            }
            let output = p
                .generate(&s, &acknowledged(&p), 1024 * 1024, &AtomicBool::new(false))
                .unwrap();
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(output.bytes()), target).unwrap());
            let grid = reopened.grid(0).unwrap();
            assert!(grid.uses_stepf2());
            if notes.contains('M') {
                assert_eq!(
                    grid.get(Position {
                        player: 0,
                        lane: 9,
                        row: 0
                    })
                    .unwrap()
                    .kind,
                    Kind::Mine
                );
                assert!(
                    grid.get(Position {
                        player: 1,
                        lane: 9,
                        row: 0
                    })
                    .is_none()
                );
            } else {
                assert_eq!(
                    grid.get(Position {
                        player: 0,
                        lane: 0,
                        row: 0
                    })
                    .unwrap()
                    .kind,
                    Kind::Fake
                );
            }
        }
    }
}
#[test]
fn output_caps_cancellation_stale_reviews_and_unknown_properties_never_return_bytes() {
    let mut s = session(Format::Ssc);
    let p = prepare(&mut s, Format::Sm, Options::default());
    assert!(
        p.generate(&s, &acknowledged(&p), 1, &AtomicBool::new(false))
            .is_err()
    );
    assert!(
        p.generate(&s, &acknowledged(&p), 1024 * 1024, &AtomicBool::new(true))
            .is_err()
    );
    s.set_text(None, "TITLE", "changed").unwrap();
    assert!(
        p.generate(&s, &acknowledged(&p), 1024 * 1024, &AtomicBool::new(false))
            .is_err()
    );
    let mut source = session(Format::Ssc);
    source.set_text(None, "VENDOR", "raw").unwrap();
    let p = prepare(&mut source, Format::Sm, Options::default());
    assert!(p.issues().iter().any(|i| i.severity == Severity::Error));
    assert!(
        p.generate(
            &source,
            &acknowledged(&p),
            1024 * 1024,
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
#[test]
fn anonymize_and_remove_custom_data_require_loss_review_without_mutating_original() {
    let mut s = session(Format::Ssc);
    s.set_text(None, "CREDIT", "Creator").unwrap();
    s.set_text(None, "BACKGROUND", "private.png").unwrap();
    s.set_text(None, "GASongDataVersion", "1").unwrap();
    s.set_text(None, "GASongData", r#"{"SyncOffset":0,"Future":true}"#)
        .unwrap();
    let original = s.serialize().unwrap();
    let p = prepare(
        &mut s,
        Format::Ssc,
        Options {
            anonymize: true,
            omit_custom_data: true,
            ..Options::default()
        },
    );
    assert!(!p.issues().is_empty());
    assert!(
        p.generate(&s, &BTreeSet::new(), 1024 * 1024, &AtomicBool::new(false))
            .is_err()
    );
    let output = p
        .generate(&s, &acknowledged(&p), 1024 * 1024, &AtomicBool::new(false))
        .unwrap();
    let document = Document::parse(Arc::<[u8]>::from(output.bytes()), Format::Ssc).unwrap();
    assert!(document.text(None, "CREDIT").is_none());
    assert!(document.text(None, "BACKGROUND").is_none());
    assert!(document.text(None, "GASongData").is_none());
    assert!(document.charts().iter().all(|c| c.description.is_empty()));
    assert_eq!(document.text(None, "TITLE").as_deref(), Some("Keep"));
    assert_eq!(s.serialize().unwrap(), original);
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
fn sixty_original_writer_cases_have_explicit_rejection_or_matching_note_readback() {
    use serde_json::Value;
    let request: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/save-format.request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/save-format.expected.json"
    ))
    .unwrap();
    let admission: Value = serde_json::from_str(include_str!(
        "fixtures/save-format-native-admission.json"
    ))
    .unwrap();
    let admissions = admission["cases"].as_array().unwrap();
    assert_eq!(request["Cases"].as_array().unwrap().len(), 60);
    assert_eq!(expected["Cases"].as_array().unwrap().len(), 60);
    assert_eq!(admissions.len(), 60);
    let (mut generated, mut blocked) = (0, 0);
    let mut classifications = std::collections::BTreeMap::new();
    for (input, result) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(input["Name"], result["Name"]);
        let name = input["Name"].as_str().unwrap();
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
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let mut s = Session::new(Document::parse(bytes.clone(), format).unwrap());
        let options = Options {
            omit_chart_timing: input["ForceTiming"].as_bool().unwrap(),
            stepf2_pump_multiplayer: input["StepF2"].as_bool().unwrap(),
            outfox_smx: input["Outfox"].as_bool().unwrap(),
            omit_custom_data: !input["Custom"].as_bool().unwrap(),
            ..Options::default()
        };
        let p = prepare(&mut s, target, options);
        let reviewed = &admissions[generated + blocked];
        assert_eq!(reviewed["name"], input["Name"]);
        let classification = reviewed["review"]["kind"].as_str().unwrap();
        *classifications.entry(classification.to_owned()).or_insert(0usize) += 1;
        match classification {
            "accepted" => assert!(reviewed["errors"].as_array().unwrap().is_empty()),
            "source_editor_guard" => assert!(!reviewed["review"]["guards"].as_array().unwrap().is_empty()),
            "source_writer_loss" => {
                let symbols = |charts: &Value| charts.as_array().unwrap().iter().map(|chart| {
                    chart["Events"].as_array().unwrap().iter().filter(|event| event["Lane"].as_i64().unwrap() >= 0).count()
                }).sum::<usize>();
                assert!(symbols(&result["Before"]) > symbols(&result["After"]), "{name}: loss witness changed");
            }
            "source_reader_exclusion" => assert!(result["Before"].as_array().unwrap().is_empty(), "{name}"),
            _ => panic!("{name}: unreviewed admission classification"),
        }
        assert_eq!(classification == "accepted", reviewed["errors"].as_array().unwrap().is_empty(), "{name}");
        assert_eq!(reviewed["source_writer_saved"], result["Saved"], "{name}");
        let mut errors: Vec<_> = p
            .issues()
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .map(|issue| serde_json::json!({
                "kind": format!("{:?}", issue.kind),
                "field": issue.field,
                "chart": issue.chart,
            }))
            .collect();
        errors.sort_by_key(Value::to_string);
        let mut expected_errors = reviewed["errors"].as_array().unwrap().clone();
        expected_errors.sort_by_key(Value::to_string);
        assert_eq!(errors, expected_errors, "{name}: admission policy changed");
        if p.issues().iter().any(|i| i.severity == Severity::Error) {
            assert!(
                p.generate(
                    &s,
                    &acknowledged(&p),
                    2 * 1024 * 1024,
                    &AtomicBool::new(false)
                )
                .is_err(),
                "{name}"
            );
            blocked += 1;
            continue;
        }
        let output = p
            .generate(
                &s,
                &acknowledged(&p),
                2 * 1024 * 1024,
                &AtomicBool::new(false),
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let mut actual =
            Session::new(Document::parse(Arc::<[u8]>::from(output.bytes()), target).unwrap());
        let charts = result["After"].as_array().unwrap();
        assert_eq!(actual.document().charts().len(), charts.len(), "{name}");
        for (index, chart) in charts.iter().enumerate() {
            assert_eq!(
                actual.document().charts()[index].steps_type,
                chart["Layout"].as_str().unwrap(),
                "{name}"
            );
            let grid = actual.grid(index).unwrap();
            assert_eq!(
                grid.players() as u64,
                chart["Players"].as_u64().unwrap(),
                "{name}"
            );
            let mut notes = Vec::new();
            for (position, note) in grid.iter() {
                let symbol = match note.kind {
                    Kind::Tap => "1",
                    Kind::Hold => "2",
                    Kind::Roll => "4",
                    Kind::Mine => "M",
                    Kind::Lift => "L",
                    Kind::Fake => "F",
                    Kind::Keysound => "K",
                    Kind::Attack => "A",
                };
                notes.push((
                    position.row,
                    position.lane as u64,
                    position.player as u64,
                    symbol.to_owned(),
                ));
                if let Some(end) = note.end {
                    notes.push((
                        end,
                        position.lane as u64,
                        position.player as u64,
                        "3".to_owned(),
                    ));
                }
            }
            let mut reference: Vec<_> = chart["Events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["Lane"].as_i64().unwrap() >= 0)
                .map(|event| {
                    (
                        event["Row"].as_u64().unwrap(),
                        event["Lane"].as_u64().unwrap(),
                        event["Player"].as_u64().unwrap(),
                        event["Source"].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
            notes.sort();
            reference.sort();
            assert_eq!(notes, reference, "{name}");
        }
        assert_eq!(s.serialize().unwrap(), bytes, "{name}");
        assert!(!s.is_dirty());
        generated += 1;
    }
    assert_eq!((generated, blocked), (28, 32));
    assert_eq!(classifications, std::collections::BTreeMap::from([
        ("accepted".to_owned(), 28),
        ("source_editor_guard".to_owned(), 20),
        ("source_reader_exclusion".to_owned(), 6),
        ("source_writer_loss".to_owned(), 6),
    ]));
}
