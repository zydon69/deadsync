use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    navigation::chart_end_position,
};
fn session(tags: &str) -> Session {
    let bytes=format!("#VERSION:0.83;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#OFFSET:0.25;#BPMS:0=120;{tags}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").into_bytes();
    Session::new(Document::parse(bytes, Format::Ssc).unwrap())
}
#[test]
fn source_chart_end_uses_final_event_and_ignores_preview() {
    for (tags, expected) in [
        ("#LABELS:8=late;", 384.0),
        ("#WARPS:8=4;", 576.0),
        ("#FAKES:8=4;", 576.0),
        ("#FAKES:4=100;#LABELS:8=late;", 384.0),
        ("#SAMPLESTART:100;#SAMPLELENGTH:10;#LABELS:8=late;", 384.0),
    ] {
        let mut source = session(tags);
        let before = source.serialize().unwrap();
        assert_eq!(
            chart_end_position(&mut source, 0).unwrap(),
            expected,
            "{tags}"
        );
        assert_eq!(source.serialize().unwrap(), before);
        assert!(!source.is_dirty());
    }
}
#[test]
fn source_chart_end_includes_pattern_length_and_song_hint_row() {
    use deadsync_editor::generation::metadata::{ChartData, Definition};
    let mut source = session("");
    ChartData {
        patterns: Some(
            [(
                384,
                Definition {
                    length: 96,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        ),
        ..Default::default()
    }
    .save(&mut source, 0)
    .unwrap();
    assert_eq!(chart_end_position(&mut source, 0).unwrap(), 480.0);
    let mut hint = session("#LASTSECONDHINT:10.003;");
    assert_eq!(
        chart_end_position(&mut hint, 0).unwrap(),
        984.0,
        "source GetEndRow truncates widget position"
    );
}

#[test]
fn chart_region_ends_match_real_fumen_and_original_editor_constructors() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/editor-chart-end-region.expected.json"
    ))
    .unwrap();
    for case in oracle["cases"].as_array().unwrap() {
        let tag = if case["kind"] == "warp" {
            "WARPS"
        } else {
            "FAKES"
        };
        let mut source = session(&format!("#{tag}:8={};", case["beats"]));
        let before = source.serialize().unwrap();
        let expected = case["end"].as_u64().unwrap_or(0) as f64;
        let actual = chart_end_position(&mut source, 0).unwrap();
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{tag} beats={}",
            case["beats"]
        );
        assert_eq!(source.serialize().unwrap(), before);
        assert!(!source.is_dirty());
    }
}

#[test]
fn selected_rows_are_last_by_start_and_custom_order_not_by_maximum_end() {
    use deadsync_editor::navigation::chart_end_position_with_selection as end;
    let mut source = session("#FAKES:1=20;");
    let before = source.serialize().unwrap();
    assert_eq!(
        end(&mut source, 0, Some((49, 50))).unwrap(),
        50.,
        "later widget start beats an earlier long region end"
    );
    assert_eq!(
        end(&mut source, 0, Some((48, 2000))).unwrap(),
        1008.,
        "same-row ordinary fake event sorts after SelectedRows"
    );
    let mut later = session("#LABELS:8=last;");
    assert_eq!(
        end(&mut later, 0, Some((192, 999))).unwrap(),
        384.,
        "later label wins even when selected end is much later"
    );
    assert_eq!(end(&mut later, 0, Some((385, 999))).unwrap(), 999.);
    assert_eq!(end(&mut later, 0, Some((-96, -1))).unwrap(), 384.);
    assert_eq!(source.serialize().unwrap(), before);
}
