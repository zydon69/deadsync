use deadsync_editor::{
    chart_properties::Draft,
    display_tempo::{Mode, Value},
    document::{Document, Format},
    history::Session,
};
use std::sync::Arc;

#[test]
fn original_display_modes_ranges_and_ui_transitions() {
    for (source, mode, output) in [
        ("", Mode::Actual, ""),
        ("*", Mode::Random, "*"),
        (" * ", Mode::Actual, ""),
        ("bad", Mode::Actual, ""),
        ("120", Mode::Specified, "120.000000"),
        ("180:90", Mode::Specified, "180.000000:90.000000"),
        ("-2:3", Mode::Specified, "-2.000000:3.000000"),
    ] {
        let value = Value::parse(source);
        assert_eq!(value.mode(), mode);
        assert_eq!(value.serialize(), output);
    }
    let mut value = Value::parse("120");
    value.set_mode(Mode::Random, (0.0, 0.0)).unwrap();
    value.set_mode(Mode::Specified, (150.0, 150.0)).unwrap();
    assert!(!value.separate_max());
    value.set_separate_max(true);
    let original = value.clone();
    value.set_separate_max(false);
    assert!(original.same_serialized_value(&value));
    value.set_mode(Mode::Actual, (0.0, 0.0)).unwrap();
    value.set_mode(Mode::Specified, (100.0, 200.0)).unwrap();
    assert!(value.separate_max());
    value.set_separate_max(false);
    assert_eq!(value.max(), 100.0);
    assert!(value.set_min(f64::NAN).is_err());
    assert_eq!(value.min(), 100.0);
    let adjacent = f64::from_bits(120_f64.to_bits() + 1);
    assert_eq!(
        Value::parse(&format!("120:{adjacent:?}")).serialize(),
        "120.000000"
    );
}
fn session(format: Format, local: &str) -> Session {
    let chart = if format == Format::Sm {
        "#NOTES:dance-single:test:Hard:9:0,0,0,0,0:0000;".to_owned()
    } else {
        format!("#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;{local}#NOTES:0000;")
    };
    Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                format!("#VERSION:0.83;#BPMS:0=120;#DISPLAYBPM:*;{chart}").into_bytes(),
            ),
            format,
        )
        .unwrap(),
    )
}
#[test]
fn inherited_ssc_empty_override_and_sm_global_writes_are_atomic() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = session(format, "");
        let before = session.serialize().unwrap();
        let mut draft = Draft::capture(&session, 0).unwrap();
        assert_eq!(draft.display_tempo().mode(), Mode::Random);
        draft.set_display_tempo(Value::parse("90:180"));
        draft.set_rating(12).unwrap();
        assert!(draft.apply(&mut session, 0).unwrap());
        let scope = (format == Format::Ssc).then_some(0);
        assert_eq!(
            session.document().text(scope, "DISPLAYBPM").as_deref(),
            Some("90.000000:180.000000")
        );
        let after = session.serialize().unwrap();
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
        assert!(draft.apply(&mut session, 0).is_err());
    }
    let mut session = session(Format::Ssc, "#DISPLAYBPM:;");
    let draft = Draft::capture(&session, 0).unwrap();
    assert_eq!(draft.display_tempo().mode(), Mode::Actual);
    assert!(!draft.apply(&mut session, 0).unwrap());
}
#[test]
fn unrelated_changes_preserve_malformed_display_source() {
    let mut session = session(Format::Ssc, "#DISPLAYBPM:garbage;");
    let mut draft = Draft::capture(&session, 0).unwrap();
    draft.set_rating(10).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert_eq!(
        session.document().text(Some(0), "DISPLAYBPM").as_deref(),
        Some("garbage")
    );
}

#[test]
fn actual_range_uses_chart_timing_and_actual_mode_writes_empty_local_override() {
    let mut source = session(Format::Ssc, "#BPMS:0=90,8=180;");
    let mut draft = Draft::capture(&source, 0).unwrap();
    assert_eq!(
        draft.actual_display_tempo_range(&source, 0).unwrap(),
        (90.0, 180.0)
    );
    let mut value = draft.display_tempo().clone();
    value.set_mode(Mode::Actual, (0.0, 0.0)).unwrap();
    draft.set_display_tempo(value);
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "DISPLAYBPM").as_deref(),
        Some("")
    );
    assert_eq!(
        source.document().text(None, "DISPLAYBPM").as_deref(),
        Some("*")
    );
    assert_eq!(
        Draft::capture(&source, 0).unwrap().display_tempo().mode(),
        Mode::Actual
    );
}
