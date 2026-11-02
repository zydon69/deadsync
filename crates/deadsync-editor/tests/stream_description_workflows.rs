use deadsync_editor::{
    chart_text::{Field, Text},
    density::{StreamSettings, task::Task},
    document::{Document, Format},
    history::Session,
};
use std::sync::atomic::AtomicBool;

fn session(format: Format, description: &str) -> Session {
    let source = if format == Format::Sm {
        format!(
            "#TITLE:source;#Opaque:user;#BPMS:0=120;#NOTES:dance-single:{description}:Hard:9::1000\n0100\n0010\n0001;"
        )
    } else {
        format!(
            "#TITLE:source;#Opaque:user;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:{description};#NOTES:1000\n0100\n0010\n0001;"
        )
    };
    Session::new(Document::parse(source.into_bytes(), format).unwrap())
}

#[test]
fn copy_notation_matches_six_original_callback_history_cases_in_both_formats() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/stream-description.expected.json"
    ))
    .unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 6);
    for format in [Format::Sm, Format::Ssc] {
        for case in cases.as_array().unwrap() {
            let mut session = session(format, case["before"].as_str().unwrap());
            // An SM header is normalized by both real readers. Install the
            // authored fixture value through the same typed chart model first.
            let mut initial =
                deadsync_editor::chart_properties::Draft::capture(&session, 0).unwrap();
            initial
                .set_text(Field::Description, case["before"].as_str().unwrap())
                .unwrap();
            initial.apply(&mut session, 0).unwrap();
            let before = session.serialize().unwrap();
            let prior_undo = session.undo_label().map(str::to_owned);
            let mut report = Task::capture(&session, 0, StreamSettings::default())
                .unwrap()
                .run(&AtomicBool::new(false))
                .unwrap();
            // Only the callback is under comparison here; the original source
            // stream computation has its own density oracle and next test.
            report.series.breakdown = Some(case["stream"].as_str().unwrap().into());
            let draft = report.description_draft(&session, 0).unwrap();
            let applied = draft
                .apply_with_presentation_receipt(&mut session, 0)
                .unwrap();
            assert_eq!(applied.changed(), case["hadUndo"].as_bool().unwrap());
            assert_eq!(
                Text::read(session.document(), 0)
                    .unwrap()
                    .get(Field::Description),
                case["after"].as_str().unwrap()
            );
            if applied.changed() {
                assert!(applied.presentation_receipt().unwrap().preserves_audio());
                let after = session.serialize().unwrap();
                let reloaded = Document::parse(after.clone(), format).unwrap();
                assert_eq!(
                    Text::read(&reloaded, 0).unwrap().get(Field::Description),
                    case["redone"].as_str().unwrap()
                );
                assert!(session.undo().unwrap());
                assert_eq!(session.serialize().unwrap(), before);
                assert_eq!(session.undo_label().map(str::to_owned), prior_undo);
                assert!(session.redo().unwrap());
                assert_eq!(session.serialize().unwrap(), after);
            } else {
                assert_eq!(session.serialize().unwrap(), before);
                assert_eq!(session.undo_label().map(str::to_owned), prior_undo);
            }
        }
    }
}

#[test]
fn actual_source_density_not_rssp_supplies_the_description_and_empty_clears_it() {
    let mut session = session(Format::Ssc, "manual");
    let mut settings = StreamSettings::default();
    settings.threshold = 4;
    let report = Task::capture(&session, 0, settings)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let expected = report
        .series
        .breakdown
        .clone()
        .expect("four real steps form source quarter-note stream");
    let draft = report.description_draft(&session, 0).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert_eq!(
        Text::read(session.document(), 0)
            .unwrap()
            .get(Field::Description),
        expected
    );
    settings.threshold = 192;
    let empty = Task::capture(&session, 0, settings)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(empty.series.breakdown.is_none());
    empty
        .description_draft(&session, 0)
        .unwrap()
        .apply(&mut session, 0)
        .unwrap();
    assert_eq!(
        Text::read(session.document(), 0)
            .unwrap()
            .get(Field::Description),
        ""
    );
}

#[test]
fn foreign_wrong_chart_and_stale_reports_cannot_prepare_an_edit() {
    let mut session = session(Format::Ssc, "manual");
    let report = Task::capture(&session, 0, StreamSettings::default())
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let foreign = Session::new(Document::parse(session.serialize().unwrap(), Format::Ssc).unwrap());
    assert!(report.description_draft(&foreign, 0).is_err());
    assert!(report.description_draft(&session, 1).is_err());
    session.set_text(None, "TITLE", "edited").unwrap();
    let bytes = session.serialize().unwrap();
    assert!(report.description_draft(&session, 0).is_err());
    assert_eq!(session.serialize().unwrap(), bytes);
}
