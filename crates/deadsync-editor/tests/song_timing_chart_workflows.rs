use deadsync_editor::{
    advanced_export::{Limits, Options, Severity},
    chart_deletion,
    chart_focus::Position,
    chart_identity::Difficulty,
    document::{Document, Format},
    history::Session,
    open_preferences::Preferences,
    output_policy::OutputPolicy,
    song_properties::Draft,
    storage::{FileSession, SaveAsOutcome},
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
const SOURCE: &str = "#VERSION:0.83;#TITLE:Timing;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Zebra;#DIFFICULTY:Hard;#METER:9;#BPMS:0=120;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Éclair;#DIFFICULTY:Hard;#METER:9;#BPMS:0=240;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-double;#DESCRIPTION:Peer;#DIFFICULTY:Hard;#METER:9;#BPMS:0=180;#NOTES:10000000;";
fn session(source: &str) -> Session {
    Session::new(Document::parse(source.as_bytes(), Format::Ssc).unwrap())
}
fn choose(session: &mut Session, index: Option<usize>) {
    let mut draft = Draft::capture(session).unwrap();
    draft.set_timing_chart(session, index).unwrap();
    draft.apply(session).unwrap();
}
fn output(session: &mut Session, policy: &OutputPolicy) -> Vec<u8> {
    let prepared = policy
        .prepare_output(session, Limits::default(), &AtomicBool::new(false))
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
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(i, _)| i)
        .collect::<BTreeSet<_>>();
    prepared
        .generate(session, &ack, 256 * 1024 * 1024, &AtomicBool::new(false))
        .unwrap()
        .bytes()
        .to_vec()
}
#[test]
fn initial_custom_index_counts_only_valid_loaded_charts_before_sort() {
    let source = format!(
        "#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0,\"DefaultChartType\":\"dance_single\",\"DefaultChartIndex\":1}};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1;{SOURCE}"
    );
    let mut s = session(&source);
    assert_eq!(s.song_timing_chart().unwrap(), Some(2));
    let id = s.song_timing_choice().unwrap().identity().cloned();
    s.set_text(Some(2), "DESCRIPTION", "A now sorts first")
        .unwrap();
    assert_eq!(s.song_timing_choice().unwrap().identity(), id.as_ref());
    assert_eq!(s.song_timing_chart().unwrap(), Some(2));
    assert!(s.undo().unwrap());
    assert_eq!(s.song_timing_chart().unwrap(), Some(2));
}
#[test]
fn initial_preferences_are_captured_and_view_bytes_stay_lossless() {
    let mut preferences = Preferences::default();
    preferences.set_steps_type("dance-double").unwrap();
    preferences.set_difficulty(Difficulty::Hard);
    let document =
        Document::parse_with_context(SOURCE.as_bytes(), Format::Ssc, preferences.load_context())
            .unwrap();
    let s = Session::new(document);
    preferences.set_steps_type("dance-single").unwrap();
    assert_eq!(s.song_timing_chart().unwrap(), Some(2));
    assert_eq!(s.serialize().unwrap(), SOURCE.as_bytes());
}
#[test]
fn chosen_deletion_uses_injected_current_preferences_and_order_undo_restores_identity() {
    let mut s = session(SOURCE);
    choose(&mut s, Some(2));
    let id = s.song_timing_choice().unwrap().identity().cloned().unwrap();
    let mut draft = chart_deletion::Draft::capture(&s, 2).unwrap();
    draft
        .prepare_song_timing(&s, &Preferences::default(), &[1, 0, 2])
        .unwrap();
    draft.apply(&mut s).unwrap();
    assert_eq!(s.song_timing_chart().unwrap(), Some(1));
    assert!(s.undo().unwrap());
    assert_eq!(s.song_timing_choice().unwrap().identity(), Some(&id));
    assert!(s.redo().unwrap());
    assert_eq!(s.song_timing_chart().unwrap(), Some(1));
}
#[test]
fn null_choice_survives_peer_deletion_and_clone_but_type_creation_adopts_new_chart() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    std::fs::write(&path, SOURCE).unwrap();
    let mut w = deadsync_editor::workspace::Workspace::open(&path, 0).unwrap();
    let mut draft = w.song_properties().unwrap();
    draft.set_timing_chart(&w.file().session, None).unwrap();
    w.apply_song_properties(&draft).unwrap();
    let deletion = w.deletion_draft().unwrap();
    w.delete_chart(&deletion).unwrap();
    assert_eq!(w.file().session.song_timing_chart().unwrap(), None);
    w.execute(deadsync_editor::workspace::Command::DuplicateChart)
        .unwrap();
    assert_eq!(w.file().session.song_timing_chart().unwrap(), None);
    let draft = w.new_chart_draft().unwrap();
    let index = w.create_chart(&draft, "dance-single").unwrap();
    assert_eq!(w.file().session.song_timing_chart().unwrap(), Some(index));
    w.execute(deadsync_editor::workspace::Command::Undo)
        .unwrap();
    assert_eq!(w.file().session.song_timing_chart().unwrap(), None);
}
#[test]
fn pure_choice_receipts_preserve_fractional_warp_position_through_undo_redo() {
    let mut s = session(&SOURCE.replace(
        "#BPMS:0=120;#NOTEDATA:",
        "#BPMS:0=120;#WARPS:1=2;#NOTEDATA:",
    ));
    let focus = Position::at_fractional_row(&s, 0, 60.5).unwrap();
    let mut draft = Draft::capture(&s).unwrap();
    draft.set_timing_chart(&s, Some(1)).unwrap();
    let application = draft.apply_with_presentation_receipt(&mut s).unwrap();
    let receipt = application.presentation_receipt().unwrap();
    assert!(receipt.preserves_audio());
    let after = focus.rebind_song_properties(&s, receipt).unwrap();
    assert_eq!(after.row().to_bits(), focus.row().to_bits());
    let application = s.apply_song_timing_history(false).unwrap();
    let restored = after
        .rebind_song_properties(&s, application.presentation_receipt().unwrap())
        .unwrap();
    assert_eq!(restored.row().to_bits(), focus.row().to_bits());
    let application = s.apply_song_timing_history(true).unwrap();
    assert_eq!(
        restored
            .rebind_song_properties(&s, application.presentation_receipt().unwrap())
            .unwrap()
            .row()
            .to_bits(),
        focus.row().to_bits()
    );
}
#[test]
fn source_compatible_follows_song_while_legacy_override_stays_explicit_and_old_review_stales() {
    let mut s = session(SOURCE);
    let follow = OutputPolicy::AdvancedSongTiming {
        format: Format::Ssc,
        options: Options::default(),
    };
    let legacy = OutputPolicy::Advanced {
        format: Format::Ssc,
        options: Options::default(),
        timing_chart: s.chart_ids()[0].clone(),
    };
    let review = follow
        .prepare_output(&mut s, Limits::default(), &AtomicBool::new(false))
        .unwrap();
    choose(&mut s, Some(1));
    assert!(!review.is_current(&s));
    let document = Document::parse(output(&mut s, &follow), Format::Ssc).unwrap();
    assert_eq!(
        document
            .text(None, "BPMS")
            .unwrap()
            .split('=')
            .nth(1)
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        240.0
    );
    let fields: serde_json::Value =
        serde_json::from_str(&document.text(None, "GASongData").unwrap()).unwrap();
    assert_eq!(fields["DefaultChartIndex"], 1);
    let document = Document::parse(output(&mut s, &legacy), Format::Ssc).unwrap();
    assert_eq!(
        document
            .text(None, "BPMS")
            .unwrap()
            .split('=')
            .nth(1)
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        120.0
    );
    choose(&mut s, None);
    assert_eq!(follow.timing_chart(&s).unwrap(), None);
    let document = Document::parse(output(&mut s, &follow), Format::Ssc).unwrap();
    let fields: serde_json::Value =
        serde_json::from_str(&document.text(None, "GASongData").unwrap()).unwrap();
    assert!(fields["DefaultChartType"].is_null());
}
#[test]
fn recovery_preserves_explicit_null_and_source_choice_survives_native_save_as() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    std::fs::write(&path, SOURCE).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    choose(&mut file.session, None);
    file.save_recovery().unwrap();
    let mut recovered = FileSession::open(&path).unwrap();
    assert!(recovered.session.song_timing_chart().unwrap().is_some());
    recovered.recover().unwrap();
    assert_eq!(recovered.session.song_timing_chart().unwrap(), None);
    choose(&mut recovered.session, Some(1));
    let id = recovered
        .session
        .song_timing_choice()
        .unwrap()
        .identity()
        .cloned();
    let destination = root.path().join("copy.ssc");
    let review = recovered.prepare_save_as(&destination).unwrap();
    assert!(matches!(
        recovered.save_as(review).unwrap(),
        SaveAsOutcome::Durable { .. }
    ));
    assert_eq!(
        recovered.session.song_timing_choice().unwrap().identity(),
        id.as_ref()
    );
    assert_eq!(
        FileSession::open(&destination)
            .unwrap()
            .session
            .song_timing_chart()
            .unwrap(),
        Some(1)
    );
}
#[test]
fn null_song_fallback_reviews_local_timing_loss_in_sm_and_omitted_ssc() {
    let mut s = session(SOURCE);
    choose(&mut s, None);
    for (format, options, severity) in [
        (
            Format::Sm,
            Options {
                require_identical_timing_in_sm: false,
                ..Default::default()
            },
            Severity::Warning,
        ),
        (
            Format::Sm,
            Options {
                require_identical_timing_in_sm: true,
                ..Default::default()
            },
            Severity::Error,
        ),
        (
            Format::Ssc,
            Options {
                omit_chart_timing: true,
                ..Default::default()
            },
            Severity::Error,
        ),
    ] {
        let policy = OutputPolicy::AdvancedSongTiming { format, options };
        let prepared = policy
            .prepare_output(&mut s, Limits::default(), &AtomicBool::new(false))
            .unwrap();
        assert!(prepared.issues().iter().any(|issue| issue.kind
            == deadsync_editor::advanced_export::IssueKind::UnequalTiming
            && issue.chart == Some(1)
            && issue.severity == severity));
        assert!(
            prepared
                .generate(
                    &s,
                    &BTreeSet::new(),
                    256 * 1024 * 1024,
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
}
#[test]
fn actual_source_writer_24_cases_header_and_metadata_match_without_mutating_native_draft() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-output.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/song-timing-output.expected.json"
    ))
    .unwrap();
    assert_eq!(request.as_array().unwrap().len(), 24);
    for (input, reference) in request
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
    {
        assert_eq!(input["name"], reference["name"]);
        let mut s = session(&format!(
            "#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0.25,\"FutureOwner\":57}};{}",
            input["input"].as_str().unwrap()
        ));
        choose(
            &mut s,
            input["fallback"].as_u64().map(|index| index as usize),
        );
        let native = s.serialize().unwrap();
        let options = Options {
            require_identical_timing_in_sm: false,
            anonymize: input["anonymize"].as_bool().unwrap(),
            omit_custom_data: input["omit"].as_bool().unwrap(),
            ..Default::default()
        };
        let format = if input["target"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let bytes = output(
            &mut s,
            &OutputPolicy::AdvancedSongTiming { format, options },
        );
        let document = Document::parse(bytes, format).unwrap();
        let number = |value: &str| {
            value
                .trim()
                .split('=')
                .nth(1)
                .and_then(|v| v.parse::<f64>().ok())
        };
        let bpm = document.text(None, "BPMS").unwrap_or_default();
        assert_eq!(
            number(&bpm),
            number(reference["header"]["BPMS"].as_str().unwrap()),
            "{}",
            input["name"]
        );
        let metadata = document
            .text(None, "GASongData")
            .or_else(|| document.text(None, "GASongDataSONG"));
        if options.omit_custom_data {
            assert!(metadata.is_none());
        } else {
            let fields: serde_json::Value = serde_json::from_str(&metadata.unwrap()).unwrap();
            assert_eq!(fields["FutureOwner"], 57);
            assert_eq!(
                fields["DefaultChartIndex"],
                input["fallback"].as_u64().unwrap_or(0)
            );
            assert_eq!(
                fields["DefaultChartType"].is_null(),
                input["fallback"].is_null()
            );
        }
        assert_eq!(s.serialize().unwrap(), native);
        assert_eq!(document.charts().len(), 2);
    }
}
#[test]
fn prepared_deletion_fallback_cannot_survive_edit_undo_or_take_over_another_song() {
    let mut s = session(SOURCE);
    choose(&mut s, Some(2));
    let mut draft = chart_deletion::Draft::capture(&s, 2).unwrap();
    draft
        .prepare_song_timing(&s, &Preferences::default(), &[1, 0, 2])
        .unwrap();
    s.set_text(None, "TITLE", "interleave").unwrap();
    s.undo().unwrap();
    let bytes = s.serialize().unwrap();
    assert!(draft.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), bytes);
    let mut other = session(SOURCE);
    assert!(draft.apply(&mut other).is_err());
}
#[test]
fn native_default_save_persists_fallback_choice_across_changed_open_preferences_but_not_view_bytes()
{
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    let source = SOURCE.replace(
        "#DESCRIPTION:Éclair;#DIFFICULTY:Hard",
        "#DESCRIPTION:Éclair;#DIFFICULTY:Easy",
    );
    std::fs::write(&path, &source).unwrap();
    let mut hard = Preferences::default();
    hard.set_difficulty(Difficulty::Hard);
    let mut file = FileSession::open_with_context(&path, hard.load_context()).unwrap();
    assert_eq!(file.session.song_timing_chart().unwrap(), Some(0));
    assert!(!file.session.is_dirty());
    file.save().unwrap();
    assert_eq!(file.session.serialize().unwrap(), source.as_bytes());
    let mut easy = Preferences::default();
    easy.set_difficulty(Difficulty::Easy);
    let untagged = Session::new(
        Document::parse_with_context(source.as_bytes(), Format::Ssc, easy.load_context()).unwrap(),
    );
    assert_eq!(untagged.song_timing_chart().unwrap(), Some(1));
    let reopened = FileSession::open_with_context(&path, easy.load_context()).unwrap();
    assert_eq!(reopened.session.song_timing_chart().unwrap(), Some(0));
}
#[test]
fn future_song_metadata_stays_opaque_on_native_save_and_explicit_omit_can_export_it() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("opaque.ssc");
    let source = format!("#GASongDataVersion:7;#GASongData:{{\"FutureOpaque\":\"keep\"}};{SOURCE}");
    std::fs::write(&path, &source).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session.set_text(None, "TITLE", "Changed").unwrap();
    file.save().unwrap();
    let document = Document::parse(std::fs::read(&path).unwrap(), Format::Ssc).unwrap();
    assert_eq!(document.text(None, "GASongDataVersion").unwrap(), "7");
    assert_eq!(
        document.text(None, "GASongData").unwrap(),
        "{\"FutureOpaque\":\"keep\"}"
    );
    assert!(
        !document
            .text(None, "GASongData")
            .unwrap()
            .contains("DefaultChart")
    );
    assert!(file.session.serialize_for_source_compatible().is_err());
    let bytes = output(
        &mut file.session,
        &OutputPolicy::AdvancedSongTiming {
            format: Format::Ssc,
            options: Options {
                omit_custom_data: true,
                ..Default::default()
            },
        },
    );
    let document = Document::parse(bytes, Format::Ssc).unwrap();
    assert!(document.text(None, "GASongData").is_none());
    assert!(document.text(None, "GASongDataVersion").is_none());
    assert_eq!(document.charts().len(), 3);
}

#[test]
fn sm_choice_metadata_keeps_fractional_presentation_receipts_on_undo_and_redo() {
    let source = "#TITLE:SM;#BPMS:0=120;#NOTES:dance-single:First:Hard:9::1000;#NOTES:dance-single:Second:Hard:9::0100;";
    let mut session = Session::new(Document::parse(source.as_bytes(), Format::Sm).unwrap());
    let focus = Position::at_fractional_row(&session, 0, 60.5).unwrap();
    let mut draft = Draft::capture(&session).unwrap();
    draft.set_timing_chart(&session, Some(1)).unwrap();
    let applied = draft.apply_with_presentation_receipt(&mut session).unwrap();
    let receipt = applied.presentation_receipt().unwrap();
    assert!(receipt.preserves_audio());
    let mut position = focus.rebind_song_properties(&session, receipt).unwrap();
    assert!(session.document().text(None, "GASongDataSONG").is_some());
    for forward in [false, true] {
        let applied = session.apply_song_timing_history(forward).unwrap();
        let receipt = applied.presentation_receipt().unwrap();
        assert!(receipt.preserves_audio());
        position = position.rebind_song_properties(&session, receipt).unwrap();
        assert_eq!(position.row().to_bits(), focus.row().to_bits());
        assert_eq!(
            session.song_timing_chart().unwrap(),
            Some(usize::from(forward))
        );
    }
}
