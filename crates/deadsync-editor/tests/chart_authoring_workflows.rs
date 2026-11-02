use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    chart_properties::Draft,
    chart_text::{EXTENSION, Field, Text},
    document::{Document, Format},
    generation::metadata::ChartData,
    history::Session,
    storage::FileSession,
};
use serde_json::Value;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn bytes(format: Format) -> Vec<u8> {
    format!("#TITLE:source;#CREDIT:global;#MUSIC:global.ogg;#VENDOR:keep;#BPMS:0=120;{}",if format==Format::Sm {"#NOTES:dance-single:before:Easy:1::1000;"}else{"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:before;#DIFFICULTY:Easy;#METER:1;#NOTES:1000;"}).into_bytes()
}
fn session(format: Format) -> Session {
    Session::new(Document::parse(bytes(format), format).unwrap())
}
fn source_field(name: &str) -> Field {
    match name {
        "Name" => Field::Name,
        "Description" => Field::Description,
        "Style" => Field::Style,
        "Credit" => Field::Credit,
        "MusicPath" => Field::Music,
        _ => panic!("unknown oracle field"),
    }
}
#[test]
fn forty_original_setters_have_format_independent_authoring_and_one_exact_undo() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/key_commands/chart-authoring-source.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 40);
    for case in cases {
        let format = if case["format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let field = source_field(case["field"].as_str().unwrap());
        let mut s = session(format);
        let mut before = Draft::capture(&s, 0).unwrap();
        before.set_text(field, "before").unwrap();
        before.apply(&mut s, 0).unwrap();
        let original = s.serialize().unwrap();
        let notes = s.grid(0).unwrap().clone();
        let mut draft = Draft::capture(&s, 0).unwrap();
        assert!(draft.supports_text(field));
        draft
            .set_text(field, case["value"].as_str().unwrap())
            .unwrap();
        assert!(draft.apply(&mut s, 0).unwrap());
        assert_eq!(
            Text::read(s.document(), 0).unwrap().get(field),
            case["after"].as_str().unwrap()
        );
        let saved = s.serialize().unwrap();
        let reload = Document::parse(saved.clone(), format).unwrap();
        assert_eq!(
            Text::read(&reload, 0).unwrap().get(field),
            case["redone"].as_str().unwrap()
        );
        assert_eq!(s.grid(0).unwrap(), &notes);
        assert!(s.undo().unwrap());
        assert_eq!(s.serialize().unwrap(), original);
        assert!(s.redo().unwrap());
        assert_eq!(s.serialize().unwrap(), saved);
        let clean = Draft::capture(&s, 0).unwrap();
        assert!(!clean.apply(&mut s, 0).unwrap());
        assert_eq!(s.serialize().unwrap(), saved);
    }
}
fn authored() -> Session {
    let mut s = session(Format::Sm);
    let mut draft = Draft::capture(&s, 0).unwrap();
    for (field, value) in [
        (Field::Name, "name"),
        (Field::Description, "  exact  "),
        (Field::Credit, "author"),
        (Field::Style, "Keyboard"),
        (Field::Music, "chart.ogg"),
    ] {
        draft.set_text(field, value).unwrap();
    }
    draft.apply(&mut s, 0).unwrap();
    s
}
// Unknown vendor tags intentionally block SourceCompatible export. Other tests
// retain that tag to verify lossless authoring; these writer tests use a fully
// mapped input so they can exercise chart metadata emission after review.
fn exportable_authored() -> Session {
    let input=String::from_utf8(authored().serialize().unwrap()).unwrap();
    assert_eq!(input.matches("#VENDOR:keep;").count(),1);
    Session::new(Document::parse(input.replace("#VENDOR:keep;","").into_bytes(),Format::Sm).unwrap())
}
fn plan(s: &mut Session, target: Format) -> Plan {
    Plan::prepare(
        s,
        target,
        0,
        Options::default(),
        Limits::default(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn source_sm_requires_review_for_all_omitted_chart_scopes_and_description_normalization() {
    let mut s = exportable_authored();
    let before = s.serialize().unwrap();
    let plan = plan(&mut s, Format::Sm);
    for field in ["CHARTNAME", "CHARTSTYLE", "CREDIT", "MUSIC", "DESCRIPTION"] {
        assert!(
            plan.issues()
                .iter()
                .any(|issue| issue.kind == IssueKind::OmittedChartMetadata && issue.field == field),
            "{field}: {:?}",
            plan.issues()
        );
    }
    assert!(
        plan.generate(&s, &BTreeSet::new(), 1_000_000, &AtomicBool::new(false))
            .is_err()
    );
    let ack = plan
        .issues()
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(i, _)| i)
        .collect();
    let out = plan
        .generate(&s, &ack, 1_000_000, &AtomicBool::new(false))
        .unwrap();
    let doc = Document::parse(out.bytes(), Format::Sm).unwrap();
    let text = Text::read(&doc, 0).unwrap();
    assert_eq!(text.get(Field::Name), "");
    assert_eq!(text.get(Field::Description), "exact");
    assert_eq!(text.get(Field::Music), "");
    assert!(
        !ChartData::read(&doc, 0)
            .unwrap()
            .unwrap()
            .extra
            .contains_key(EXTENSION)
    );
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn source_ssc_materializes_supported_text_instead_of_leaking_native_envelope() {
    let mut s = exportable_authored();
    let expected = Text::read(s.document(), 0).unwrap();
    let plan = plan(&mut s, Format::Ssc);
    let ack = plan
        .issues()
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.severity == Severity::Warning)
        .map(|(i, _)| i)
        .collect();
    let out = plan
        .generate(&s, &ack, 1_000_000, &AtomicBool::new(false))
        .unwrap();
    let doc = Document::parse(out.bytes(), Format::Ssc).unwrap();
    let actual = Text::read(&doc, 0).unwrap();
    for field in Field::ALL {
        assert_eq!(actual.get(field), expected.get(field));
    }
    assert!(
        !ChartData::read(&doc, 0)
            .unwrap()
            .unwrap()
            .extra
            .contains_key(EXTENSION)
    );
}
#[test]
fn sm_save_as_relocates_current_undo_redo_and_duplicate_chart_override_media() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    let new = root.path().join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&new).unwrap();
    let path = old.join("song.sm");
    std::fs::write(&path, bytes(Format::Sm)).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    for value in ["first.ogg", "second.ogg"] {
        let mut draft = Draft::capture(&file.session, 0).unwrap();
        draft.set_text(Field::Music, value).unwrap();
        draft.apply(&mut file.session, 0).unwrap();
    }
    file.session.undo().unwrap();
    let ids = file.session.chart_ids().to_vec();
    let target = new.join("copy.sm");
    let review = file.prepare_save_as(&target).unwrap();
    file.save_as(review).unwrap();
    assert_eq!(
        Text::read(file.session.document(), 0)
            .unwrap()
            .get(Field::Music),
        "../old/first.ogg"
    );
    assert_eq!(file.session.chart_ids(), ids);
    file.session.redo().unwrap();
    assert_eq!(
        Text::read(file.session.document(), 0)
            .unwrap()
            .get(Field::Music),
        "../old/second.ogg"
    );
    file.session.undo().unwrap();
    file.session.undo().unwrap();
    assert_eq!(
        Text::read(file.session.document(), 0)
            .unwrap()
            .get(Field::Music),
        ""
    );
    file.session.redo().unwrap();
    let revision = file.session.revision();
    file.session.duplicate_chart(0, revision).unwrap();
    assert_eq!(
        Text::read(file.session.document(), 1)
            .unwrap()
            .get(Field::Music),
        "../old/first.ogg"
    );
}
#[test]
fn invalid_owned_schema_is_rejected_without_editing_or_shadowing_unknown_metadata() {
    let mut s = session(Format::Sm);
    s.set_text(None, "GAChartDataVersionCHART0000", "1")
        .unwrap();
    s.set_text(
        None,
        "GAChartDataCHART0000",
        r#"{"DeadSyncChartText":{"Version":1,"Fields":{"MUSIC":4}},"Vendor":{"keep":true}}"#,
    )
    .unwrap();
    let before = s.serialize().unwrap();
    assert!(Draft::capture(&s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn chart_receipt_keeps_warp_and_stop_positions_exact_but_refuses_clock_changes() {
    use deadsync_editor::chart_focus::Position;
    for timing in ["#WARPS:1=2;", "#STOPS:1=1;"] {
        let raw = format!(
            "#VERSION:0.83;#BPMS:0=120;#OFFSET:0;{timing}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
        );
        let mut s = Session::new(Document::parse(raw.into_bytes(), Format::Ssc).unwrap());
        let before = if timing.contains("WARPS") {
            Position::at_fractional_row(&s, 0, 60.5).unwrap()
        } else {
            Position::at_time(&s, 0, 0.75).unwrap()
        };
        let mut draft = Draft::capture(&s, 0).unwrap();
        draft.set_text(Field::Name, "metadata").unwrap();
        let result = draft.apply_with_presentation_receipt(&mut s, 0).unwrap();
        let receipt = result.presentation_receipt().unwrap();
        assert!(receipt.preserves_audio());
        let after = before.rebind_song_properties(&s, receipt).unwrap();
        assert_eq!(after.row().to_bits(), before.row().to_bits());
        assert_eq!(after.music_time().to_bits(), before.music_time().to_bits());
        assert_eq!(after.chart_time().to_bits(), before.chart_time().to_bits());
        let mut draft = Draft::capture(&s, 0).unwrap();
        draft.set_text(Field::Music, "different.ogg").unwrap();
        let result = draft.apply_with_presentation_receipt(&mut s, 0).unwrap();
        assert!(!result.presentation_receipt().unwrap().preserves_audio());
        let mut draft = Draft::capture(&s, 0).unwrap();
        draft.set_use_chart_offset(true);
        draft.set_music_offset(0.25).unwrap();
        let result = draft.apply_with_presentation_receipt(&mut s, 0).unwrap();
        assert!(result.presentation_receipt().is_none());
    }
}
#[test]
fn explicitly_cleared_credit_remains_empty_natively_but_ssc_source_reload_loss_is_reviewed() {
 let mut s=session(Format::Ssc);let mut draft=Draft::capture(&s,0).unwrap();assert_eq!(draft.text(Field::Credit),"global");assert_eq!(draft.text(Field::Music),"");draft.set_text(Field::Credit,"").unwrap();draft.apply(&mut s,0).unwrap();assert_eq!(Text::read(s.document(),0).unwrap().get(Field::Credit),"");let plan=plan(&mut s,Format::Ssc);assert!(plan.issues().iter().any(|issue|issue.kind==IssueKind::OmittedChartMetadata&&issue.field=="CREDIT"));assert!(plan.generate(&s,&BTreeSet::new(),1_000_000,&AtomicBool::new(false)).is_err());
}
#[test]
fn sm_local_music_is_distinct_from_effective_playback_and_sample_fallback() {
 let root=tempfile::tempdir().unwrap();let path=root.path().join("song.sm");for name in ["global.ogg","chart.ogg"]{std::fs::write(root.path().join(name),b"command-only fixture").unwrap();}let mut s=session(Format::Sm);
 assert_eq!(Text::read(s.document(),0).unwrap().get(Field::Music),"");
 for value in ["chart.ogg",""] {let mut draft=Draft::capture(&s,0).unwrap();draft.set_text(Field::Music,value).unwrap();draft.apply(&mut s,0).unwrap();let expected=root.path().join(if value.is_empty(){"global.ogg"}else{value});assert_eq!(Text::read(s.document(),0).unwrap().get(Field::Music),value);assert_eq!(deadsync_editor::playback::Preview::capture(&s,0,&path).unwrap().music(),expected);let mut song=deadsync_editor::song_properties::Draft::capture(&s).unwrap();song.set_sample_length(2.0).unwrap();assert_eq!(song.sample_audio(&s,0,&path).unwrap().path,expected);}
}
#[test]
fn owned_extension_updates_keep_unknown_members_and_invalid_revisions_are_atomic() {
 let mut s=session(Format::Sm);s.set_text(None,"GAChartDataVersionCHART0000","1").unwrap();s.set_text(None,"GAChartDataCHART0000",r#"{"Vendor":{"keep":true},"DeadSyncChartText":{"Version":1,"Fields":{"VendorField":{"x":2}},"VendorNative":{"keep":3}}}"#).unwrap();let mut draft=Draft::capture(&s,0).unwrap();draft.set_text(Field::Name,"new").unwrap();draft.apply(&mut s,0).unwrap();let data=ChartData::read(s.document(),0).unwrap().unwrap();assert_eq!(data.extra["Vendor"]["keep"],true);assert_eq!(data.extra[EXTENSION]["Fields"]["VendorField"]["x"],2);assert_eq!(data.extra[EXTENSION]["VendorNative"]["keep"],3);
 let mut stale=Draft::capture(&s,0).unwrap();stale.set_text(Field::Music,"other.ogg").unwrap();s.set_text(None,"TITLE","intervening").unwrap();let before=s.serialize().unwrap();assert!(stale.apply(&mut s,0).is_err());assert_eq!(s.serialize().unwrap(),before);
}
