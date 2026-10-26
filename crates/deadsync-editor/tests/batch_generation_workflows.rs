use deadsync_editor::{
    document::{Document, Format},
    generation::{
        batch_job::{Preview, Task},
        job::{ChartOptions, ChartTask, ProfileOverrides},
    },
    history::{ChartAddition, Session},
    notes::{Grid, Note, Position},
    workspace::{Command, Workspace},
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn bytes(format: Format) -> Vec<u8> {
    let mut text = String::from("#TITLE:Batch;#BPMS:0=120;#VENDOR:keep;");
    for (layout, difficulty, meter, notes) in [
        ("dance-single", "Easy", "3", "1000\n0100\n0010\n0001"),
        ("pump-single", "Medium", "5", "10000\n01000\n00100\n00010"),
        ("dance-single", "Hard", "9", "0010\n0001\n1000\n0100"),
    ] {
        match format {
            Format::Ssc => text.push_str(&format!("#NOTEDATA:;#STEPSTYPE:{layout};#DESCRIPTION:Source {meter};#DIFFICULTY:{difficulty};#METER:{meter};#VENDOR_CHART:keep {meter};#NOTES:{notes};")),
            Format::Sm => text.push_str(&format!("#NOTES:{layout}:Source {meter}:{difficulty}:{meter}:0,0,0,0,0:{notes};")),
        }
    }
    text.into_bytes()
}
fn session(format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(bytes(format)), format).unwrap())
}
fn task(session: &Session) -> Task {
    Task::capture(
        session,
        "dance-single",
        ChartOptions::balanced("dance-double"),
        None,
        ProfileOverrides::default(),
    )
    .unwrap()
}
fn preview(session: &Session) -> Preview {
    task(session).run(&AtomicBool::new(false), |_| {}).unwrap()
}

#[test]
fn scheduled_attacks_survive_atomic_batch_conversion_in_song_scoped_formats() {
    use deadsync_editor::attacks::placement::Timeline;
    for format in [Format::Sm, Format::Ssc] {
        let input = String::from_utf8(bytes(format)).unwrap().replace(
            "#VENDOR:keep;",
            "#VENDOR:keep;#ATTACKS:TIME=1:LEN=2:MODS=drunk;",
        );
        let mut source = Session::new(Document::parse(input.as_bytes(), format).unwrap());
        let before = source.serialize().unwrap();
        let result = preview(&source);
        assert_eq!(result.report().generated, 2);
        result.apply(&mut source).unwrap();
        let saved = source.serialize().unwrap();
        let reopened = Session::new(Document::parse(saved.clone(), format).unwrap());
        for chart in 0..5 {
            let attacks = Timeline::capture(&reopened, chart).unwrap();
            assert_eq!(attacks.markers().len(), 1);
            assert_eq!(attacks.markers()[0].row, 96);
        }
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), saved);
    }
}
#[test]
fn batch_matches_individual_conversions_and_round_trips_as_one_action() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = session(format);
        source
            .edit_notes(
                0,
                "Pending edit",
                [(
                    Position {
                        player: 0,
                        lane: 2,
                        row: 24,
                    },
                    Some(Note::tap()),
                )],
            )
            .unwrap();
        let before = source.serialize().unwrap();
        let result = preview(&source);
        assert_eq!(result.report().total, 2);
        assert_eq!(result.report().generated, 2);
        assert_eq!(result.report().failed, 0);
        for (entry, index) in result.entries().iter().zip([0, 2]) {
            assert_eq!(entry.source_chart, index);
            let individual =
                ChartTask::capture(&source, index, ChartOptions::balanced("dance-double"))
                    .unwrap()
                    .run(&AtomicBool::new(false), |_| {})
                    .unwrap();
            assert_eq!(entry.preview().unwrap().grid(), individual.grid());
            assert_eq!(entry.preview().unwrap().report(), individual.report());
        }
        assert_eq!(source.serialize().unwrap(), before);
        assert_eq!(result.apply(&mut source).unwrap(), vec![3, 4]);
        let after = source.serialize().unwrap();
        for (new, old) in [(3, 0), (4, 2)] {
            assert_eq!(
                source.document().charts()[new].difficulty,
                source.document().charts()[old].difficulty
            );
            assert_eq!(
                source.document().charts()[new].meter,
                source.document().charts()[old].meter
            );
            if format == Format::Ssc {
                assert_eq!(
                    source.document().text(Some(new), "VENDOR_CHART"),
                    source.document().text(Some(old), "VENDOR_CHART")
                );
            }
        }
        let mut reopened =
            Session::new(Document::parse(Arc::<[u8]>::from(after.clone()), format).unwrap());
        for index in 0..5 {
            assert_eq!(source.grid(index).unwrap(), reopened.grid(index).unwrap());
        }
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
    }
}
#[test]
fn cancellation_at_start_between_charts_and_ready_never_changes_session() {
    let source = session(Format::Ssc);
    let before = source.serialize().unwrap();
    assert!(task(&source).run(&AtomicBool::new(true), |_| {}).is_err());
    let cancelled = AtomicBool::new(false);
    assert!(
        task(&source)
            .run(&cancelled, |progress| {
                if progress.completed == 1 {
                    cancelled.store(true, Ordering::Relaxed);
                }
            })
            .is_err()
    );
    let cancelled = AtomicBool::new(false);
    assert!(
        task(&source)
            .run(&cancelled, |progress| {
                if progress.phase == deadsync_editor::generation::job::Phase::Ready {
                    cancelled.store(true, Ordering::Relaxed);
                }
            })
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.undo_label(), None);
}
#[test]
fn foreign_and_stale_previews_cannot_append_charts() {
    let mut source = session(Format::Ssc);
    let result = preview(&source);
    let mut foreign = session(Format::Ssc);
    assert!(result.apply(&mut foreign).is_err());
    source.set_text(None, "TITLE", "Changed").unwrap();
    let changed = source.serialize().unwrap();
    assert!(result.apply(&mut source).is_err());
    assert_eq!(source.serialize().unwrap(), changed);
    assert_eq!(foreign.document().charts().len(), 3);
}
#[test]
fn invalid_last_addition_preserves_document_and_redo() {
    let mut source = session(Format::Ssc);
    source.set_text(None, "TITLE", "Redo me").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let redo_label = source.redo_label().unwrap().to_owned();
    let valid = Grid::new(8, 1).unwrap();
    let invalid = Grid::new(4, 1).unwrap();
    let revision = source.revision();
    assert!(
        source
            .append_charts(
                &[
                    ChartAddition {
                        source_chart: 0,
                        layout: "dance-double",
                        grid: &valid
                    },
                    ChartAddition {
                        source_chart: 2,
                        layout: "dance-double",
                        grid: &invalid
                    },
                ],
                revision,
                "Batch"
            )
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some(redo_label.as_str()));
    assert!(source.redo().unwrap());
    assert_eq!(
        source.document().text(None, "TITLE").as_deref(),
        Some("Redo me")
    );
}
#[test]
fn workspace_restores_previously_focused_chart_on_undo_and_first_result_on_redo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch.ssc");
    fs::write(&path, bytes(Format::Ssc)).unwrap();
    let mut workspace = Workspace::open(&path, 1).unwrap();
    let result = workspace
        .batch_generation_task(
            "dance-single",
            ChartOptions::balanced("dance-double"),
            None,
            ProfileOverrides::default(),
        )
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    workspace.apply_batch_generated(&result).unwrap();
    assert_eq!(workspace.chart(), 3);
    workspace.execute(Command::SelectChart(4)).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.chart(), 1);
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.chart(), 3);
    workspace.execute(Command::Save).unwrap();
    assert_eq!(Workspace::open(&path, 4).unwrap().chart(), 4);
}

#[test]
fn failed_sources_remain_reviewable_and_successes_apply_together() {
    use deadsync_editor::generation::metadata::{ChartData, Guid};
    let mut source = session(Format::Ssc);
    let mut data = ChartData::existing_or_initial(source.document(), 0).unwrap();
    data.expressed_chart_config = Guid::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let result = preview(&source);
    assert_eq!(result.report().generated, 1);
    assert_eq!(result.report().failed, 1);
    assert!(result.entries()[0].error().is_some());
    assert!(result.entries()[0].preview().is_none());
    assert!(result.entries()[1].error().is_none());
    assert_eq!(result.apply(&mut source).unwrap(), vec![3]);
    assert_eq!(source.document().charts()[3].difficulty, "Hard");
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn all_failed_preview_does_not_erase_redo_or_append_anything() {
    use deadsync_editor::generation::metadata::{ChartData, Guid};
    let mut source = session(Format::Ssc);
    for chart in [0, 2] {
        let mut data = ChartData::existing_or_initial(source.document(), chart).unwrap();
        data.expressed_chart_config = Guid::default();
        data.save(&mut source, chart).unwrap();
    }
    source.set_text(None, "TITLE", "Redo title").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let redo_label = source.redo_label().unwrap().to_owned();
    let result = preview(&source);
    assert_eq!(result.report().generated, 0);
    assert_eq!(result.report().failed, 2);
    assert!(result.apply(&mut source).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some(redo_label.as_str()));
    assert!(source.redo().unwrap());
}

#[test]
fn profile_snapshot_is_shared_even_if_files_disappear_between_sources() {
    use deadsync_editor::generation::{
        config::PerformedConfig,
        metadata::DYNAMIC,
        profiles::{Kind, Profile},
    };
    let source = session(Format::Ssc);
    let folder = tempfile::tempdir().unwrap();
    let profile = Profile::parse(
        Kind::Performed,
        &serde_json::to_vec(&serde_json::json!({
            "Guid":"11111111-2222-3333-4444-555555555555",
            "Name":"Batch snapshot",
            "Config":PerformedConfig::balanced()
        }))
        .unwrap(),
    )
    .unwrap();
    let file = profile.export_new(folder.path()).unwrap();
    let overrides = ProfileOverrides {
        performed: Some(profile.guid().clone()),
        expressed: Some(DYNAMIC.parse().unwrap()),
    };
    let mut options = ChartOptions::balanced("dance-double");
    options.seed = -177;
    let mut removed = false;
    let result = Task::capture(
        &source,
        "dance-single",
        options.clone(),
        Some(folder.path()),
        overrides.clone(),
    )
    .unwrap()
    .run(&AtomicBool::new(false), |progress| {
        if progress.completed == 1 && !removed {
            fs::remove_file(file.path()).unwrap();
            removed = true;
        }
    })
    .unwrap();
    assert!(removed);
    assert_eq!(result.report().generated, 2);
    for entry in result.entries() {
        assert_eq!(entry.preview().unwrap().seed(), -177);
        assert_eq!(
            entry.preview().unwrap().performed_profile(),
            Some(profile.guid())
        );
    }
    assert!(
        ChartTask::capture(&source, 2, options)
            .unwrap()
            .with_profile_overrides(Some(folder.path()), overrides)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn each_source_uses_its_own_saved_interpretation_with_one_shared_seed() {
    use deadsync_editor::generation::metadata::{ChartData, Registry};
    for format in [Format::Sm, Format::Ssc] {
        let mut source = session(format);
        let ids: Vec<_> = Registry::builtin()
            .expressed
            .keys()
            .cloned()
            .take(2)
            .collect();
        assert_eq!(ids.len(), 2);
        for (chart, id) in [0, 2].into_iter().zip(&ids) {
            let mut data = ChartData::existing_or_initial(source.document(), chart).unwrap();
            data.expressed_chart_config = id.clone();
            data.save(&mut source, chart).unwrap();
        }
        let mut options = ChartOptions::balanced("dance-double");
        options.seed = 777;
        let result = Task::capture(
            &source,
            "dance-single",
            options.clone(),
            None,
            ProfileOverrides::default(),
        )
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
        assert_eq!(result.report().generated, 2);
        for (entry, id) in result.entries().iter().zip(ids) {
            let individual = ChartTask::capture(&source, entry.source_chart, options.clone())
                .unwrap()
                .with_chart_expression(None)
                .unwrap()
                .run(&AtomicBool::new(false), |_| {})
                .unwrap();
            let generated = entry.preview().unwrap();
            assert_eq!(generated.expression_profile(), Some(&id));
            assert_eq!(generated.seed(), 777);
            assert_eq!(generated.grid(), individual.grid());
        }
    }
}
