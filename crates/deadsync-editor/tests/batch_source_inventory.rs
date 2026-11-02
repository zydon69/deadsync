use deadsync_editor::{
    document::{Document, Format},
    generation::batch_job,
    history::Session,
};
#[test]
fn inventory_uses_the_same_effective_layout_as_batch_capture_without_rewriting_stepf2() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-double;#NOTES:X000000000;#NOTEDATA:;#STEPSTYPE:pump-double;#NOTES:1000000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let session = Session::new(Document::parse(&bytes[..], Format::Ssc).unwrap());
    assert_eq!(
        batch_job::source_charts(&session),
        vec![(1, "pump-double"), (2, "dance-single")]
    );
    assert_eq!(session.serialize().unwrap(), bytes);
    assert_eq!(session.document().charts()[0].steps_type, "pump-double");
}
