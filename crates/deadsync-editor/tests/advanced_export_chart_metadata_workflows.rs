use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
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
fn all_128_original_metadata_cases_classify_omitted_scope_without_claiming_equal_music_changes() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-metadata.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-metadata.expected.json"
    ))
    .unwrap();
    let mut count = 0;
    for (input, oracle) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert!(oracle["Error"].is_null());
        assert_eq!(oracle["Saved"], true);
        let name = input["Name"].as_str().unwrap();
        let field = name.split('-').next().unwrap();
        let mut s = Session::new(
            Document::parse(decode(input["Bytes"].as_str().unwrap()), Format::Ssc).unwrap(),
        );
        let before = s.serialize().unwrap();
        let target = if input["Target"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let p = prepare(&mut s, target, Options::default());
        let issues: Vec<_> = p
            .issues()
            .iter()
            .filter(|i| i.kind == IssueKind::OmittedChartMetadata)
            .collect();
        let wanted = target == Format::Sm && !name.contains("-absent-") && field != "RADARVALUES";
        assert_eq!(issues.len(), usize::from(wanted), "{name}");
        if wanted {
            assert_eq!(issues[0].field, field);
            assert_eq!(issues[0].chart, Some(0));
            assert_eq!(issues[0].identity, Some(s.chart_ids()[0].clone()));
            assert_eq!(issues[0].severity, Severity::Warning);
            assert!(p.validate_review(&s, &BTreeSet::new()).is_err());
            if field == "MUSIC" {
                assert_eq!(
                    issues[0].detail.contains("music played"),
                    name.contains("-different-"),
                    "{name}"
                );
            }
        }
        assert_eq!(s.serialize().unwrap(), before);
        count += 1;
    }
    assert_eq!(count, 128);
}
#[test]
fn acknowledged_multichart_music_scope_loss_is_explicit_and_radar_survives_sm_header() {
    let text = "#VERSION:0.83;#TITLE:Keep;#CREDIT:Song;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:A;#DIFFICULTY:Hard;#METER:9;#MUSIC:song.ogg;#RADARVALUES:1,2,3,4,5;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#DESCRIPTION:B;#DIFFICULTY:Hard;#METER:9;#MUSIC:other.ogg;#CREDIT:Other;#CHARTNAME:Named;#CHARTSTYLE:Versus;#SAMPLESTART:2;#SAMPLELENGTH:3;#SELECTABLE:NO;#NOTES:10000000;";
    let mut s = Session::new(Document::parse(text.as_bytes(), Format::Ssc).unwrap());
    let before = s.serialize().unwrap();
    let p = prepare(&mut s, Format::Sm, Options::default());
    let warnings: BTreeSet<_> = p
        .issues()
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (v.severity == Severity::Warning).then_some(i))
        .collect();
    assert!(
        p.generate(&s, &BTreeSet::new(), 1024 * 1024, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(
        p.issues()
            .iter()
            .filter(|i| i.kind == IssueKind::OmittedChartMetadata)
            .count(),
        8
    );
    let output = p
        .generate(&s, &warnings, 1024 * 1024, &AtomicBool::new(false))
        .unwrap();
    let document = Document::parse(output.bytes(), Format::Sm).unwrap();
    assert_eq!(document.text(None, "MUSIC").as_deref(), Some("song.ogg"));
    assert!(
        String::from_utf8_lossy(output.bytes()).contains("#NOTES:dance-single:A:Hard:9:1,2,3,4,5:")
    );
    assert_eq!(document.charts().len(), 2);
    assert_eq!(s.serialize().unwrap(), before);
    let mut options = Options::default();
    options.anonymize = true;
    let anonymized = prepare(&mut s, Format::Sm, options);
    assert!(
        !anonymized
            .issues()
            .iter()
            .any(|i| i.kind == IssueKind::OmittedChartMetadata
                && matches!(i.field.as_str(), "CREDIT" | "CHARTNAME" | "CHARTSTYLE"))
    );
    assert!(
        anonymized
            .issues()
            .iter()
            .any(|i| i.kind == IssueKind::AnonymizedField && i.field == "CHARTNAME")
    );
}
