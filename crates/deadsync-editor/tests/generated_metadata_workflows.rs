//! Native correction of source stale-Extras loss, plus SM global owner cloning.
use deadsync_editor::{
    chart_properties::Draft,
    chart_text::Field,
    display_tempo::Value as Display,
    document::{Format, escape_text},
    generation::{
        job::{ChartOptions, ChartTask},
        metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
    },
    history::Session,
    storage::FileSession,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, sync::atomic::AtomicBool};
fn raw(format: Format, old: bool, orphan: bool) -> Vec<u8> {
    let mut bytes =
        b"#VERSION:0.83;#TITLE:Metadata;#OFFSET:1.25;#BPMS:0=120;#DISPLAYBPM:90;#VENDOR:keep;"
            .to_vec();
    let original=json!({"MusicOffset":-0.25,"ShouldUseChartMusicOffset":true,"ExpressedChartConfig":"a19d532e-b0ce-4759-ad1c-02ecbbdf2efd","Patterns":{"48":{"PatternConfigGuid":SIXTEENTHS,"PerformedChartConfigGuid":PATTERN_BALANCED,"Length":24,"RandomSeed":7,"StartPositionInclusive":true,"EndPositionInclusive":true}},"UnknownOriginal":{"retain":7}}).to_string();
    if format == Format::Sm && old {
        bytes.extend(b"#GAChartDataVersionCHART0000:1;#GAChartDataCHART0000:");
        bytes.extend(escape_text(&original));
        bytes.push(b';');
    }
    if format == Format::Sm && orphan {
        bytes.extend(b"#GAChartDataCHART0002:{\"Orphan\":true};");
    }
    for chart in 0..2 {
        bytes.extend_from_slice(match format{Format::Sm=>b"#NOTES:dance-single:Original:Hard:9:0,0,0,0,0:",Format::Ssc=>b"#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Original;#DIFFICULTY:Hard;#METER:9;#DISPLAYBPM:90;"});
        if format == Format::Ssc {
            if old && chart == 0 {
                bytes.extend(b"#GAChartDataVersion:1;#GAChartData:");
                bytes.extend(escape_text(&original));
                bytes.push(b';');
            }
            bytes.extend(b"#NOTES:");
        }
        bytes.extend_from_slice(if chart == 0 {
            b"1000\n0100\n0010\n0001"
        } else {
            b"0001"
        });
        bytes.push(b';');
    }
    bytes
}
fn edit(session: &mut Session) {
    let mut data = ChartData::existing_or_initial(session.document(), 0).unwrap();
    data.expressed_chart_config = "0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
    data.patterns = Some(BTreeMap::from([(
        96,
        Definition {
            pattern_config_guid: SIXTEENTHS.parse().unwrap(),
            performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
            length: 48,
            random_seed: 42,
            start_position_inclusive: true,
            end_position_inclusive: true,
            extra: BTreeMap::from([("UnknownMarker".into(), json!({"retain":9}))]),
            ..Definition::default()
        },
    )]));
    data.extra
        .insert("UnknownLive".into(), json!({"retain":[1,2,3]}));
    data.save(session, 0).unwrap();
    let mut properties = Draft::capture(session, 0).unwrap();
    properties.set_music_offset(0.375).unwrap();
    properties.set_use_chart_offset(true);
    properties.set_display_tempo(Display::parse("140:180"));
    for (field, value) in [
        (Field::Name, "Live name"),
        (Field::Description, "Live description"),
        (Field::Credit, "Live credit"),
        (Field::Style, "Live style"),
        (Field::Music, "live.ogg"),
    ] {
        properties.set_text(field, value).unwrap();
    }
    properties.apply(session, 0).unwrap();
}
fn metadata(session: &Session, chart: usize) -> Value {
    let data = ChartData::existing_or_initial(session.document(), chart).unwrap();
    let properties = Draft::capture(session, chart).unwrap();
    json!({"Data":data,"Texts":Field::ALL.map(|field|properties.text(field).to_owned()),"EffectiveOffset":properties.effective_music_offset(),"Display":[properties.display_tempo().min(),properties.display_tempo().max()]})
}
#[test]
fn live_chart_metadata_survives_generation_save_reopen_undo_redo_in_both_formats() {
    let original: Value = serde_json::from_str(include_str!(
        "fixtures/generated-metadata/source.expected.json"
    ))
    .unwrap();
    assert_eq!(original[0]["Generated"]["PatternRows"], json!([]));
    assert_eq!(original[1]["Generated"]["PatternRows"], json!([48]));
    assert_eq!(
        original[2]["Generated"], original[2]["Live"],
        "matching source control"
    );
    for format in [Format::Sm, Format::Ssc] {
        for old in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join(format!("song.{}", format.extension()));
            fs::write(&path, raw(format, old, false)).unwrap();
            let mut file = FileSession::open(&path).unwrap();
            edit(&mut file.session);
            let live = metadata(&file.session, 0);
            assert_eq!(live["Data"]["Patterns"]["96"]["UnknownMarker"]["retain"], 9);
            assert_eq!(live["EffectiveOffset"], 0.375);
            assert_eq!(live["Display"], json!([140., 180.]));
            let peer = file.session.grid(1).unwrap().clone();
            let source = file.session.grid(0).unwrap().clone();
            let before = file.session.serialize_for_output().unwrap();
            let mut options = ChartOptions::balanced("dance-double");
            options.seed = 42;
            let preview = ChartTask::capture(&file.session, 0, options)
                .unwrap()
                .with_chart_expression(None)
                .unwrap()
                .run(&AtomicBool::new(false), |_| {})
                .unwrap();
            let output = preview.grid().clone();
            let generated = preview.apply(&mut file.session).unwrap();
            assert_eq!(generated, 2);
            assert_eq!(
                metadata(&file.session, generated),
                live,
                "{format:?} old={old}: retain current owners, not stale source extras"
            );
            assert_eq!(metadata(&file.session, 0), live);
            assert_eq!(file.session.grid(1).unwrap(), &peer);
            assert_eq!(file.session.grid(generated).unwrap().columns(), 8);
            file.save().unwrap();
            let saved = fs::read(&path).unwrap();
            let mut reopened = FileSession::open(&path).unwrap();
            assert_eq!(metadata(&reopened.session, generated), live);
            assert_eq!(metadata(&reopened.session, 0), live);
            assert_eq!(reopened.session.grid(generated).unwrap(), &output);
            assert_eq!(reopened.session.grid(1).unwrap(), &peer);
            file.session.undo().unwrap();
            assert_eq!(file.session.document().charts().len(), 2);
            assert_eq!(metadata(&file.session, 0), live);
            assert_eq!(file.session.grid(0).unwrap(), &source);
            assert_eq!(file.session.serialize_for_output().unwrap(), before);
            assert_eq!(
                fs::read(&path).unwrap(),
                saved,
                "Undo is resident, not another publication"
            );
            file.session.redo().unwrap();
            assert_eq!(file.session.document().charts().len(), 3);
            assert_eq!(metadata(&file.session, generated), live);
            assert_eq!(file.session.serialize_for_output().unwrap(), saved);
        }
    }
}
#[test]
fn sm_generated_metadata_orphan_collision_refuses_the_entire_append() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("orphan.sm");
    fs::write(&path, raw(Format::Sm, false, true)).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    edit(&mut file.session);
    let before = file.session.serialize().unwrap();
    let revision = file.session.revision();
    let label = file.session.undo_label().map(str::to_owned);
    let ids = file.session.chart_ids().to_vec();
    let preview = ChartTask::capture(&file.session, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    assert!(preview.apply(&mut file.session).is_err());
    assert_eq!(file.session.serialize().unwrap(), before);
    assert_eq!(file.session.revision(), revision);
    assert_eq!(file.session.chart_ids(), ids.as_slice());
    assert_eq!(file.session.undo_label().map(str::to_owned), label);
    assert_eq!(file.session.document().charts().len(), 2);
}
