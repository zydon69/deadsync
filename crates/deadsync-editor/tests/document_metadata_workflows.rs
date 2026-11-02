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
        "#TITLE",
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

#[test]
fn hashes_inside_values_and_missing_semicolon_recovery_match_msd() {
    for format in [Format::Sm, Format::Ssc] {
        let source = b"#TITLE:Song #1 in C#;\n#ARTIST:Artist\n#SUBTITLE:Recovered;";
        let document = Document::parse(source.as_slice(), format).unwrap();
        assert_eq!(
            document.text(None, "TITLE").as_deref(),
            Some("Song #1 in C#")
        );
        assert_eq!(document.text(None, "ARTIST").as_deref(), Some("Artist"));
        assert_eq!(
            document.text(None, "SUBTITLE").as_deref(),
            Some("Recovered")
        );
        assert_eq!(document.serialize(), source);
        let metadata = Metadata::parse(source.as_slice(), format).unwrap();
        assert_eq!(metadata.text("TITLE"), document.text(None, "TITLE"));
    }
}

#[test]
fn carriage_return_and_indented_tag_recovery_preserve_source() {
    let source = b"#TITLE:Song #1;\r#ARTIST:Artist\r  \t#SUBTITLE:Recovered;";
    let document = Document::parse(source.as_slice(), Format::Ssc).unwrap();
    assert_eq!(document.text(None, "ARTIST").as_deref(), Some("Artist"));
    assert_eq!(
        document.text(None, "SUBTITLE").as_deref(),
        Some("Recovered")
    );
    assert_eq!(document.serialize(), source);
}

#[test]
fn hashes_inside_sm_chart_descriptions_are_plain_characters() {
    let source = b"#TITLE:Song #1;#NOTES:dance-single:C# exercise:Hard:9::1000;";
    let document = Document::parse(source.as_slice(), Format::Sm).unwrap();
    assert_eq!(document.charts()[0].description, "C# exercise");
    assert_eq!(document.serialize(), source);
}

#[test]
fn final_unterminated_values_and_notes_match_msd_without_rewriting_source() {
    for (format, source) in [
        (Format::Sm, "#TITLE:Final # value"),
        (Format::Ssc, "#TITLE:Final # value"),
        (
            Format::Sm,
            "#TITLE:Final;#NOTES:dance-single::Hard:9::1000\n0000",
        ),
        (
            Format::Ssc,
            "#TITLE:Final;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000",
        ),
    ] {
        let document = Document::parse(source.as_bytes(), format).unwrap();
        let metadata = Metadata::parse(source.as_bytes(), format).unwrap();
        assert_eq!(document.serialize(), source.as_bytes());
        assert_eq!(metadata.text("TITLE"), document.text(None, "TITLE"));
        for chart in document.charts() {
            assert_eq!(document.value(chart.notes.clone()), b"1000\n0000");
        }
    }
}

#[test]
fn sm_chart_without_final_semicolon_can_be_duplicated_and_undone() {
    use deadsync_editor::{
        file_origin::{NewSong, Origin},
        history::Session,
        storage::FileSession,
        workspace::{Command, Workspace},
    };
    let source = b"#TITLE:Final;#NOTES:dance-single::Hard:9::1000\n0000";
    let document = Document::parse(source.as_slice(), Format::Sm).unwrap();
    let file = FileSession::from_new(NewSong {
        origin: Origin::Untitled {
            resource_base: std::env::temp_dir(),
        },
        session: Session::new(document),
    })
    .unwrap();
    let mut workspace = Workspace::from_file(file, 0).unwrap();
    workspace.execute(Command::DuplicateChart).unwrap();
    assert_eq!(workspace.file().session.document().charts().len(), 2);
    let serialized = workspace.file().session.serialize().unwrap();
    assert_eq!(
        Document::parse(serialized, Format::Sm)
            .unwrap()
            .charts()
            .len(),
        2
    );
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
}
