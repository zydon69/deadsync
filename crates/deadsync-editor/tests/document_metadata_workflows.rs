use deadsync_editor::{
    Error,
    document::{
        Document, Format,
        metadata::{Limits, Metadata},
    },
};
#[test]
fn header_only_and_empty_notes_are_readonly_and_never_relax_editable_parse() {
    for notes in ["", "#NOTES:;"] {
        let source = format!(
            "#VERSION:0.83;#TITLE:Header;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;{notes}"
        );
        let headers = Metadata::parse(source.as_bytes(), Format::Ssc).unwrap();
        assert_eq!(headers.charts().len(), 1);
        assert_eq!(headers.charts()[0].steps_type, "dance-single");
        assert_eq!(headers.charts()[0].meter, "9");
        assert_eq!(headers.text("TITLE").as_deref(), Some("Header"));
        assert!(
            matches!(Document::parse(source.as_bytes(),Format::Ssc),Err(Error::InvalidFormat(message)) if message=="Chart is missing its note data")
        );
    }
}
#[test]
fn multiple_chart_order_and_strict_source_ranges_remain_unchanged() {
    let source=b"// keep\n#VERSION:0.83;#TITLE:A\\:B;#X:opaque;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Easy;#METER:13;#NOTES:0000000000;";
    let strict = Document::parse(source.as_slice(), Format::Ssc).unwrap();
    let headers = Metadata::parse(source.as_slice(), Format::Ssc).unwrap();
    assert_eq!(headers.text("TITLE"), strict.text(None, "TITLE"));
    assert_eq!(headers.charts().len(), strict.charts().len());
    for (header, chart) in headers.charts().iter().zip(strict.charts()) {
        assert_eq!(header.steps_type, chart.steps_type);
        assert_eq!(header.meter, chart.meter);
        assert!(!strict.value(chart.notes.clone()).is_empty());
    }
    assert_eq!(strict.serialize().as_slice(), source.as_slice());
}
#[test]
fn later_syntax_error_keeps_priority_over_missing_note_data_in_both_modes() {
    for missing in ["", "#NOTES:;"] {
        let source = format!(
            "#TITLE:Keep;#NOTEDATA:;#STEPSTYPE:dance-single;{missing}#NOTEDATA:;#STEPSTYPE:pump-single;#NOTES:00000;#BROKEN"
        );
        let strict = Document::parse(source.as_bytes(), Format::Ssc)
            .unwrap_err()
            .to_string();
        let readonly = Metadata::parse(source.as_bytes(), Format::Ssc)
            .unwrap_err()
            .to_string();
        assert_eq!(strict, readonly);
        assert!(strict.starts_with("Unterminated tag at byte"), "{strict}");
    }
}
#[test]
fn metadata_bounds_do_not_change_strict_parse_or_writer_contract() {
    let source = b"#TITLE:Bounds;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    for limits in [
        Limits {
            source_bytes: source.len() - 1,
            ..Limits::default()
        },
        Limits {
            charts: 0,
            ..Limits::default()
        },
        Limits {
            tags: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Metadata::parse_with_limits(source.as_slice(), Format::Ssc, limits),
            Err(Error::Unsupported(_))
        ));
    }
    let strict = Document::parse(source.as_slice(), Format::Ssc).unwrap();
    assert_eq!(strict.serialize().as_slice(), source.as_slice());
}
#[test]
fn malformed_sm_header_and_syntax_are_not_accepted_by_readonly_mode() {
    for source in [
        "#TITLE:X;#NOTES:dance-single::Hard;",
        "#BAD-NAME:x;",
        "#TITLE:unterminated",
    ] {
        assert_eq!(
            Document::parse(source.as_bytes(), Format::Sm)
                .unwrap_err()
                .to_string(),
            Metadata::parse(source.as_bytes(), Format::Sm)
                .unwrap_err()
                .to_string()
        );
    }
}
