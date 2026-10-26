use deadsync_editor::{
    chart_focus::Position,
    chart_views::ActiveChartViews,
    document::{Document, Format},
    history::Session,
    playback::Clock,
};
fn session() -> Session {
    let source=b"#VERSION:0.83;#BPMS:0=120;#OFFSET:0.25;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0.1};#NOTEDATA:;#STEPSTYPE:dance-single;#STOPS:1=1;#DELAYS:2=0.25;#WARPS:3=1;#NOTES:1000;";
    Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap())
}
#[test]
fn fractional_row_focus_preserves_warp_rows_pauses_negative_lead_in_and_offsets() {
    let session = session();
    let clock = Clock::capture(&session, 0).unwrap();
    let source = session.serialize().unwrap();
    let revision = session.revision();
    for row in [
        -48.5, -0.125, 0.0, 48.0, 48.5, 96.0, 96.5, 144.0, 160.5, 192.5,
    ] {
        let position = Position::at_fractional_row(&session, 0, row).unwrap();
        assert_eq!(position.row(), row);
        assert_eq!(
            position.chart_time(),
            clock.fractional_chart_time(row).unwrap()
        );
        assert_eq!(
            position.music_time(),
            clock.fractional_music_time(row).unwrap()
        );
        assert_eq!(position.edit_row(), row.max(0.0).floor() as u64);
    }
    let inside = Position::at_fractional_row(&session, 0, 160.5).unwrap();
    assert_ne!(
        clock.position(inside.music_time()).unwrap(),
        inside.row(),
        "warp time inversion would lose the chosen row"
    );
    assert_eq!(Position::at_row(&session, 0, 160).unwrap().row(), 160.0);
    assert_eq!(session.serialize().unwrap(), source);
    assert_eq!(session.revision(), revision);
    assert!(!session.is_dirty());
}
#[test]
fn row_focus_rejects_invalid_chart_or_nonfinite_input_without_mutation() {
    let session = session();
    let before = session.serialize().unwrap();
    for row in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Position::at_fractional_row(&session, 0, row).is_err());
    }
    assert!(Position::at_fractional_row(&session, 99, 0.0).is_err());
    assert_eq!(session.serialize().unwrap(), before);
}
#[test]
fn resident_view_row_focus_reuses_arc_clock_and_rejects_stale_content() {
    let mut session = session();
    let mut views = ActiveChartViews::empty(&session, 0.0).unwrap();
    views.focus(&session, 0).unwrap();
    let view = views.focused().unwrap();
    let clock = view.clock(&session).unwrap();
    let position = view.position_at_row(&session, 160.5).unwrap();
    assert_eq!(position.row(), 160.5);
    assert!(std::sync::Arc::ptr_eq(
        &clock,
        &view.clock(&session).unwrap()
    ));
    session.set_text(Some(0), "BPMS", "0=60").unwrap();
    assert!(view.position_at_row(&session, 160.5).is_err());
}
