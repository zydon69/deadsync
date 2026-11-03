use deadsync_editor::{
    chart_deletion::Draft,
    chart_views::ActiveChartViews,
    document::{Document, Format},
    generation::metadata::Guid,
    history::Session,
};
fn make_session() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:identical;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:identical;#OFFSET:0.25;#BPMS:0=60;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:identical;#NOTES:0010;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;".to_vec(),Format::Ssc).unwrap())
}
fn ids(views: &ActiveChartViews) -> Vec<Guid> {
    views
        .entries()
        .iter()
        .map(|view| view.identity().clone())
        .collect()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
#[test]
fn temporary_focus_is_unique_and_dedicated_views_keep_resident_identity() {
    let session = make_session();
    let source = session.serialize().unwrap();
    let mut views = ActiveChartViews::empty(&session, 1.01).unwrap();
    views.focus(&session, 0).unwrap();
    views.show(&session, 0, true).unwrap();
    assert!(!views.focused().unwrap().dedicated()); // Show alone does not pin an existing temporary view.
    views.focus(&session, 1).unwrap();
    assert_eq!(views.entries().len(), 1);
    assert_eq!(views.focused().unwrap().source_index(), 1);
    views.set_dedicated(&session, 1, true).unwrap();
    views.focus(&session, 2).unwrap();
    views.set_dedicated(&session, 0, true).unwrap();
    assert_eq!(
        ids(&views),
        [
            session.chart_ids()[2].clone(),
            session.chart_ids()[1].clone(),
            session.chart_ids()[0].clone()
        ]
    );
    assert_eq!(
        views
            .entries()
            .iter()
            .filter(|view| !view.dedicated())
            .count(),
        1
    );
    views.focus(&session, 1).unwrap();
    views.set_selection_anchor(&session, Some(4)).unwrap();
    assert_eq!(views.focused().unwrap().selection(), Some((4, 60)));
    views.focus(&session, 2).unwrap();
    assert!(
        views
            .entries()
            .iter()
            .all(|view| view.selection_anchor().is_none())
    );
    assert_eq!(session.serialize().unwrap(), source);
    assert!(!session.is_dirty());
    assert!(session.undo_label().is_none());
}
#[test]
fn show_close_and_unpin_follow_reference_adjacent_focus_policy() {
    let session = make_session();
    let mut views = ActiveChartViews::empty(&session, 0.75).unwrap();
    views.focus(&session, 0).unwrap();
    views.set_dedicated(&session, 1, true).unwrap();
    views.show(&session, 2, false).unwrap(); // Replaces temporary0; its right neighbor1 becomes focused.
    assert_eq!(views.focused().unwrap().source_index(), 1);
    assert_eq!(views.entries().len(), 2);
    views.set_dedicated(&session, 2, false).unwrap();
    assert_eq!(views.entries().len(), 1);
    views.set_dedicated(&session, 1, false).unwrap();
    assert!(!views.focused().unwrap().dedicated());
    views.close(&session, &session.chart_ids()[1]).unwrap();
    assert!(views.entries().is_empty());
    assert!(views.focused().is_none());
    close(views.music_time(), 0.75);
}
#[test]
fn active_view_order_and_focus_traversal_are_separate_from_song_and_source_order() {
    let session = make_session();
    let source = session.serialize().unwrap();
    let mut views = ActiveChartViews::empty(&session, 0.0).unwrap();
    for index in 0..3 {
        views.set_dedicated(&session, index, true).unwrap();
    }
    views
        .move_view(&session, &session.chart_ids()[0], true)
        .unwrap();
    assert_eq!(
        ids(&views),
        [
            session.chart_ids()[1].clone(),
            session.chart_ids()[0].clone(),
            session.chart_ids()[2].clone()
        ]
    );
    views.cycle_focus(&session, false).unwrap();
    assert_eq!(views.focused().unwrap().source_index(), 2);
    views.cycle_focus(&session, false).unwrap();
    assert_eq!(views.focused().unwrap().source_index(), 1);
    views.cycle_focus(&session, true).unwrap();
    assert_eq!(views.focused().unwrap().source_index(), 2);
    let order = ids(&views);
    views
        .move_view(&session, &session.chart_ids()[2], true)
        .unwrap();
    assert_eq!(ids(&views), order);
    views.close(&session, &session.chart_ids()[2]).unwrap();
    assert_eq!(views.focused().unwrap().source_index(), 0); // Last view falls back left.
    assert_eq!(session.serialize().unwrap(), source);
    assert_eq!(session.revision(), 0);
}
#[test]
fn shared_time_uses_per_chart_clocks_and_song_edits_refresh_inheriting_views() {
    let mut session = make_session();
    let mut views = ActiveChartViews::empty(&session, 1.01).unwrap();
    for index in 0..3 {
        views.set_dedicated(&session, index, true).unwrap();
    }
    close(views.entries()[0].position().row(), 96.96);
    close(views.entries()[1].position().row(), 60.48);
    close(views.entries()[2].position().row(), 96.96);
    session.set_text(None, "BPMS", "0=60").unwrap();
    views.reconcile(&session, None).unwrap();
    close(views.entries()[0].position().row(), 48.48);
    close(views.entries()[1].position().row(), 60.48);
    close(views.entries()[2].position().row(), 48.48);
    session.set_text(None, "OFFSET", "0.125").unwrap();
    views.reconcile(&session, None).unwrap();
    close(views.entries()[0].position().row(), 54.48);
    close(views.entries()[1].position().row(), 60.48);
    views.set_music_time(&session, -0.25).unwrap();
    for entry in views.entries() {
        close(entry.position().music_time(), -0.25);
        assert_eq!(entry.position().edit_row(), 0);
    }
}
#[test]
fn failed_clock_refresh_and_invalid_indices_leave_registry_atomically_unchanged() {
    let mut session = make_session();
    let mut views = ActiveChartViews::empty(&session, 1.01).unwrap();
    views.set_dedicated(&session, 0, true).unwrap();
    views.set_dedicated(&session, 1, true).unwrap();
    let order = ids(&views);
    let oldrows: Vec<_> = views
        .entries()
        .iter()
        .map(|view| view.position().row())
        .collect();
    for chart in [3, usize::MAX] {
        assert!(views.focus(&session, chart).is_err());
        assert_eq!(ids(&views), order);
    }
    for time in [f64::NAN, f64::INFINITY] {
        assert!(views.set_music_time(&session, time).is_err());
        close(views.music_time(), 1.01);
    }
    session.set_text(Some(1), "OFFSET", "NaN").unwrap();
    let source = session.serialize().unwrap();
    assert!(views.set_music_time(&session, 2.0).is_err());
    close(views.music_time(), 1.01);
    assert_eq!(ids(&views), order);
    assert_eq!(
        views
            .entries()
            .iter()
            .map(|view| view.position().row())
            .collect::<Vec<_>>(),
        oldrows
    );
    assert_eq!(session.serialize().unwrap(), source);
}
#[test]
fn structural_reconciliation_rebinds_ids_through_delete_undo_redo_and_duplicate() {
    let mut session = make_session();
    let original = session.chart_ids().to_vec();
    let mut views = ActiveChartViews::empty(&session, 0.75).unwrap();
    for index in 0..3 {
        views.set_dedicated(&session, index, true).unwrap();
    }
    views.focus(&session, 1).unwrap();
    Draft::capture(&session, 1)
        .unwrap()
        .apply(&mut session)
        .unwrap();
    views.reconcile(&session, None).unwrap();
    assert_eq!(views.focused().unwrap().identity(), &original[2]);
    assert_eq!(views.focused().unwrap().source_index(), 1);
    assert_eq!(ids(&views), [original[0].clone(), original[2].clone()]);
    assert!(session.undo().unwrap());
    views.reconcile(&session, Some(1)).unwrap();
    assert_eq!(views.focused().unwrap().identity(), &original[1]);
    assert!(!views.focused().unwrap().dedicated());
    assert!(session.redo().unwrap());
    views.reconcile(&session, None).unwrap();
    assert!(
        views
            .entries()
            .iter()
            .all(|view| view.identity() != &original[1])
    );
    let revision = session.revision();
    let duplicate = session.duplicate_chart(0, revision).unwrap();
    let duplicate_id = session.chart_ids()[duplicate].clone();
    views.focus(&session, duplicate).unwrap();
    assert_eq!(views.focused().unwrap().identity(), &duplicate_id);
    session.undo().unwrap();
    views.reconcile(&session, Some(0)).unwrap();
    assert!(
        views
            .entries()
            .iter()
            .all(|view| view.identity() != &duplicate_id)
    );
    session.redo().unwrap();
    views.reconcile(&session, None).unwrap();
    assert!(
        views
            .entries()
            .iter()
            .all(|view| view.identity() != &duplicate_id)
    ); // Undo does not promise to restore every closed tab.
}
#[test]
fn empty_registry_still_belongs_to_exact_session_and_unknown_layouts_prune_by_identity() {
    let mut session = make_session();
    let reopened = make_session();
    let mut views = ActiveChartViews::empty(&session, 0.0).unwrap();
    assert!(!views.belongs_to(&reopened));
    assert!(views.focus(&reopened, 0).is_err());
    views.focus(&session, 0).unwrap();
    views.set_dedicated(&session, 1, true).unwrap();
    assert!(views.focus(&session, 3).is_err());
    prepared_deletion(&session, 0).apply(&mut session).unwrap();
    views.reconcile(&session, None).unwrap();
    assert_eq!(views.entries().len(), 1);
    assert_eq!(views.focused().unwrap().source_index(), 0);
    views.close(&session, &session.chart_ids()[0]).unwrap();
    assert!(views.belongs_to(&session));
    assert!(views.reconcile(&reopened, None).is_err());
}
#[test]
fn dedicated_views_are_bounded_by_unique_eligible_charts_without_an_invented_tab_cap() {
    let mut session = make_session();
    for _ in 0..20 {
        let revision = session.revision();
        session.duplicate_chart(0, revision).unwrap();
    }
    let mut views = ActiveChartViews::empty(&session, 0.0).unwrap();
    for index in 0..session.chart_ids().len() {
        if index != 3 {
            views.set_dedicated(&session, index, true).unwrap();
        }
    }
    let count = views.entries().len();
    assert_eq!(count, session.chart_ids().len() - 1);
    for index in 0..session.chart_ids().len() {
        if index != 3 {
            views.set_dedicated(&session, index, true).unwrap();
        }
    }
    assert_eq!(views.entries().len(), count);
    assert_eq!(
        views
            .entries()
            .iter()
            .filter(|view| !view.dedicated())
            .count(),
        0
    );
}

#[test]
fn immutable_clocks_survive_time_focus_and_order_changes_without_recapture() {
    let mut session = make_session();
    let mut views = ActiveChartViews::empty(&session, 0.0).unwrap();
    views.set_dedicated(&session, 0, true).unwrap();
    views.set_dedicated(&session, 1, true).unwrap();
    let clocks: Vec<_> = views
        .entries()
        .iter()
        .map(|view| (view.identity().clone(), view.clock(&session).unwrap()))
        .collect();
    let before = deadsync_editor::audio_preferences::Audio::default();
    let mut after = before;
    after.set_music_rate(1.2).unwrap();
    assert!(session.set_audio_preferences(before, after, None).unwrap());
    assert!(session.undo().unwrap());
    for time in [0.25, 1.01, -0.5, 3.125] {
        views.synchronize(&session, Some(1), time).unwrap();
        views.cycle_focus(&session, false).unwrap();
        views
            .move_view(&session, &session.chart_ids()[0], true)
            .unwrap();
        for (identity, clock) in &clocks {
            let view = views
                .entries()
                .iter()
                .find(|view| view.identity() == identity)
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(
                clock,
                &view.clock(&session).unwrap()
            ));
            close(view.position().music_time(), time);
            let mapped = view.position_at_time(&session, time + 0.125).unwrap();
            close(mapped.row(), clock.position(time + 0.125).unwrap());
            close(mapped.music_time(), time + 0.125);
        }
    }
}

#[test]
fn timing_and_undo_invalidate_clock_handles_and_another_session_is_rejected() {
    let mut session = make_session();
    let mut views = ActiveChartViews::empty(&session, 1.0).unwrap();
    views.set_dedicated(&session, 0, true).unwrap();
    views.set_dedicated(&session, 1, true).unwrap();
    let old_view = views.entries()[0].clone();
    let original = old_view.clock(&session).unwrap();
    session.set_text(None, "BPMS", "0=60").unwrap();
    assert!(old_view.clock(&session).is_err());
    assert!(old_view.position_at_time(&session, 2.0).is_err());
    views.synchronize(&session, Some(0), 1.0).unwrap();
    let edited = views.entries()[0].clock(&session).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&original, &edited));
    close(views.entries()[0].position().row(), 48.0);
    close(views.entries()[1].position().row(), 60.0);
    assert!(session.undo().unwrap());
    views.synchronize(&session, Some(0), 1.0).unwrap();
    assert!(!std::sync::Arc::ptr_eq(
        &edited,
        &views.entries()[0].clock(&session).unwrap()
    ));
    close(views.entries()[0].position().row(), 96.0);
    let other = make_session();
    assert!(views.entries()[0].clock(&other).is_err());
    assert!(views.synchronize(&other, Some(0), 2.0).is_err());
    close(views.music_time(), 1.0);
}

fn prepared_deletion(
    session: &deadsync_editor::history::Session,
    chart: usize,
) -> deadsync_editor::chart_deletion::Draft {
    let mut draft = deadsync_editor::chart_deletion::Draft::capture(session, chart).unwrap();
    let snapshot = session.song_timing_snapshot().unwrap();
    let order = snapshot.indices_with(str::cmp).unwrap();
    draft
        .prepare_song_timing(
            session,
            &deadsync_editor::open_preferences::Preferences::default(),
            &order,
        )
        .unwrap();
    draft
}
