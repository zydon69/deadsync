use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
    song_properties::{Draft, Marker},
    song_text::{Field, Text},
    storage::FileSession as File,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn bytes(format: Format, chartless: bool) -> Vec<u8> {
    let mut text = String::from(
        "#TITLE:Keep;#BPMS:0=120;#SAMPLESTART:1.25;#SAMPLELENGTH:5;#LASTSECONDHINT:2;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};",
    );
    if !chartless {
        text.push_str(if format == Format::Sm {
            "#NOTES:dance-single::Easy:1::0000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:1;#NOTES:0000;"
        });
    }
    text.into_bytes()
}
#[test]
fn both_marker_models_match_72_original_callback_history_cases_with_explicit_noop_adaptation() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/song-marker-source.json"
    ))
    .unwrap();
    let mut count = 0;
    for case in oracle["Callbacks"].as_array().unwrap() {
        let format = if case["format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let mut session = Session::new(Document::parse(bytes(format, false), format).unwrap());
        let before = session.serialize().unwrap();
        let revision = session.revision();
        let mut changed = false;
        if case["gate"] == "ready" {
            let mut draft = Draft::capture(&session).unwrap();
            draft
                .move_marker_to_time(
                    if case["end"].as_bool().unwrap() {
                        Marker::EndHint
                    } else {
                        Marker::Preview
                    },
                    case["time"].as_f64().unwrap(),
                )
                .unwrap();
            changed = draft.apply(&mut session).unwrap();
        }
        let draft = Draft::capture(&session).unwrap();
        assert_eq!(
            draft.sample_start(),
            case["after"]["Sample"].as_f64().unwrap()
        );
        assert_eq!(
            draft.last_second_hint(),
            case["after"]["Hint"].as_f64().unwrap()
        );
        if changed {
            assert!(case["HadUndo"].as_bool().unwrap());
            let after = session.serialize().unwrap();
            assert!(session.undo().unwrap());
            assert_eq!(session.serialize().unwrap(), before);
            assert!(session.redo().unwrap());
            assert_eq!(session.serialize().unwrap(), after);
        } else {
            assert_eq!(session.revision(), revision);
            assert_eq!(session.serialize().unwrap(), before);
            assert!(session.undo_label().is_none());
        }
        // Original queue enqueues an identical property value; native's documented no-op invariant intentionally does not.
        count += 1;
    }
    assert_eq!(count, 72);
}
#[test]
fn all_song_text_and_end_hint_are_editable_in_sm_and_lossless_save_reload_keeps_every_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let mut initial = bytes(Format::Sm, false);
    initial.extend_from_slice(b"#VENDOR:opaque\\;value;#JACKET:legacy.png;");
    std::fs::write(&path, &initial).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut draft = Draft::capture(&file.session).unwrap();
    for field in Field::ALL {
        assert!(draft.supports_text(field));
        draft
            .set_text(field, &format!("{}; \\ été", field.tag()))
            .unwrap();
    }
    draft.set_last_second_hint(17.123456789).unwrap();
    assert!(draft.apply(&mut file.session).unwrap());
    let changed = file.session.serialize().unwrap();
    file.save().unwrap();
    let reopened = File::open(&path).unwrap();
    let text = Text::read(reopened.session.document());
    for field in Field::ALL {
        assert_eq!(text.get(field), format!("{}; \\ été", field.tag()));
    }
    assert_eq!(
        Draft::capture(&reopened.session)
            .unwrap()
            .last_second_hint(),
        17.123456789
    );
    assert_eq!(
        reopened.session.document().text(None, "VENDOR"),
        file.session.document().text(None, "VENDOR")
    );
    assert!(file.session.undo().unwrap());
    assert_eq!(file.session.serialize().unwrap(), initial);
    assert!(file.session.redo().unwrap());
    assert_eq!(file.session.serialize().unwrap(), changed);
    let mut stale = Draft::capture(&file.session).unwrap();
    stale.set_text(Field::Jacket, "other.png").unwrap();
    file.session.set_text(None, "ARTIST", "concurrent").unwrap();
    let before = file.session.serialize().unwrap();
    assert!(stale.apply(&mut file.session).is_err());
    assert_eq!(file.session.serialize().unwrap(), before);
}
#[test]
fn dedicated_sm_preview_is_effective_and_invalid_marker_values_never_partially_change_a_draft() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("preview.wav"), b"preview").unwrap();
    let mut session = Session::new(Document::parse(bytes(Format::Sm, false), Format::Sm).unwrap());
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_text(Field::PreviewMusic, "preview.wav").unwrap();
    draft.apply(&mut session).unwrap();
    let draft = Draft::capture(&session).unwrap();
    let audio = draft
        .sample_audio_from_directory(&session, 0, dir.path())
        .unwrap();
    assert_eq!(audio.path, dir.path().join("preview.wav"));
    assert_eq!(audio.start, 0.0);
    assert_eq!(audio.length, None);
    let mut draft = Draft::capture(&session).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(draft.move_marker_to_time(Marker::EndHint, value).is_err());
        assert!(draft.move_marker_to_time(Marker::Preview, value).is_err());
    }
    assert!(!draft.has_changes());
    assert!(!draft.apply(&mut session).unwrap());
}
#[test]
fn source_compatible_writer_matches_original_omission_matrix_and_requires_all_losses_reviewed() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/song-marker-source.json"
    ))
    .unwrap();
    let cancel = AtomicBool::new(false);
    let fields = [
        Field::Origin,
        Field::Jacket,
        Field::CdImage,
        Field::DiscImage,
        Field::PreviewVideo,
        Field::PreviewMusic,
    ];
    for field in fields {
        assert!(field.omitted_by_source_sm());
    }
    for case in oracle["Writers"].as_array().unwrap() {
        let target = if case["target"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let chartless = case["chartless"].as_bool().unwrap();
        for source in [Format::Sm, Format::Ssc] {
            let mut session =
                Session::new(Document::parse(bytes(source, chartless), source).unwrap());
            let mut draft = Draft::capture(&session).unwrap();
            for field in fields {
                draft
                    .set_text(field, &format!("{}.asset", field.tag()))
                    .unwrap();
            }
            draft.apply(&mut session).unwrap();
            let before = session.serialize().unwrap();
            let plan = if chartless {
                Plan::prepare_chartless(
                    &mut session,
                    target,
                    Options::default(),
                    Limits::default(),
                    &cancel,
                )
            } else {
                Plan::prepare(
                    &mut session,
                    target,
                    0,
                    Options::default(),
                    Limits::default(),
                    &cancel,
                )
            }
            .unwrap();
            let omitted = [
                "ORIGIN",
                "JACKET",
                "CDIMAGE",
                "DISCIMAGE",
                "PREVIEWVID",
                "PREVIEW",
                "LASTSECONDHINT",
            ];
            for field in omitted {
                assert_eq!(case["tags"].get(field).is_none(), target == Format::Sm);
                let issues: Vec<_> = plan
                    .issues()
                    .iter()
                    .filter(|issue| issue.field == field && issue.kind == IssueKind::OmittedEvent)
                    .collect();
                assert_eq!(issues.len(), usize::from(target == Format::Sm), "{field}");
            }
            if target == Format::Sm {
                assert!(
                    plan.generate(&session, &BTreeSet::new(), Limits::default().bytes, &cancel)
                        .is_err()
                );
            }
            let acknowledged: BTreeSet<_> = plan
                .issues()
                .iter()
                .enumerate()
                .filter(|(_, issue)| issue.severity == Severity::Warning)
                .map(|(i, _)| i)
                .collect();
            let output = plan
                .generate(&session, &acknowledged, Limits::default().bytes, &cancel)
                .unwrap();
            let exported = Document::parse(output.bytes().to_vec(), target).unwrap();
            for field in omitted {
                assert_eq!(
                    exported.text(None, field).is_some(),
                    target == Format::Ssc,
                    "{field}"
                );
            }
            assert_eq!(session.serialize().unwrap(), before);
        }
    }
}
