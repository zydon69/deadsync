use deadsync_editor::{
    advanced_export::{IssueKind, Options},
    document::Format,
    output_policy::OutputPolicy,
    storage::{FileSession, SaveAsOutcome},
    timing_edit::{
        Scope,
        region::{self, Kind},
    },
    workspace::{Command, Workspace},
};
use std::collections::BTreeSet;
const SM: &str = "#TITLE:Original;#OFFSET:0.25;#BPMS:0=120;#NOTES:dance-single:chart:Challenge:9:0,0,0,0,0:1000;#NOTES:dance-single:second:Hard:7:0,0,0,0,0:0100;";
fn file(extra: &str) -> (tempfile::TempDir, FileSession) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("song.sm");
    std::fs::write(&path, format!("{SM}{extra}")).unwrap();
    let file = FileSession::open(&path).unwrap();
    (root, file)
}
fn promote(file: &mut FileSession) {
    let plan = file.prepare_sm_authoring().unwrap();
    file.apply_sm_authoring(plan, Options::default()).unwrap();
}
#[test]
fn dirty_sm_promotion_is_atomic_undoable_and_does_not_change_origin_witness_or_ids() {
    let (root, mut file) = file("");
    let path = file.require_existing_path().unwrap().to_owned();
    let disk = std::fs::read(&path).unwrap();
    let watch = file.external_snapshot().unwrap();
    let ids = file.session.chart_ids().to_vec();
    file.session.set_text(None, "TITLE", "dirty").unwrap();
    let dirty = file.session.serialize().unwrap();
    let old_policy = file.output_policy().clone();
    promote(&mut file);
    assert_eq!(file.session.document().format, Format::Ssc);
    assert_eq!(file.require_existing_path().unwrap(), path);
    assert_eq!(file.resource_base(), root.path().canonicalize().unwrap());
    assert_eq!(file.session.chart_ids(), ids);
    assert!(watch.is_current(&file));
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    let policy = file.output_policy().clone();
    assert!(matches!(
        policy,
        OutputPolicy::AdvancedSongTiming {
            format: Format::Sm,
            ..
        }
    ));
    let mut warp = region::Draft::capture(&file.session, 0, Scope::Chart).unwrap();
    warp.set(Kind::Warp, 192, 48).unwrap();
    warp.apply(&mut file.session, 0).unwrap();
    let mut workspace = Workspace::from_file(file, 0).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert!(
        workspace
            .file()
            .session
            .document()
            .text(Some(0), "WARPS")
            .unwrap_or_default()
            .is_empty()
    );
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.document().format, Format::Sm);
    assert_eq!(workspace.file().output_policy(), &old_policy);
    assert_eq!(workspace.file().session.serialize().unwrap(), dirty);
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.file().session.document().format, Format::Ssc);
    assert_eq!(workspace.file().output_policy(), &policy);
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(
        workspace
            .file()
            .session
            .document()
            .text(Some(0), "WARPS")
            .as_deref(),
        Some("4=1")
    );
    assert_eq!(std::fs::read(path).unwrap(), disk);
}
#[test]
fn save_sm_requires_actual_loss_review_while_save_as_ssc_retains_events_and_policy_through_undo() {
    let (root, mut file) = file("");
    promote(&mut file);
    let old = file.require_existing_path().unwrap().to_owned();
    let disk = std::fs::read(&old).unwrap();
    let mut warp = region::Draft::capture(&file.session, 0, Scope::Chart).unwrap();
    warp.set(Kind::Warp, 192, 48).unwrap();
    warp.apply(&mut file.session, 0).unwrap();
    let save = file.prepare_save().unwrap();
    assert!(save.issues().iter().any(|issue| matches!(
        issue.kind,
        IssueKind::OmittedEvent | IssueKind::UnequalTiming
    )));
    assert!(file.save_reviewed(save, &BTreeSet::new()).is_err());
    assert_eq!(std::fs::read(&old).unwrap(), disk);
    let destination = root.path().canonicalize().unwrap().join("copy.ssc");
    let policy = OutputPolicy::AdvancedSongTiming {
        format: Format::Ssc,
        options: Options::default(),
    };
    let review = file
        .prepare_save_as_with_policy(&destination, policy.clone())
        .unwrap();
    assert!(review.issues().is_empty(), "{:?}", review.issues());
    let outcome = file.save_as(review).unwrap();
    match outcome {
        SaveAsOutcome::Durable { .. } => {}
        SaveAsOutcome::WrittenPendingDurability { .. } => panic!("fixture must sync its directory"),
    }
    let bytes = std::fs::read(&destination).unwrap();
    let document = deadsync_editor::document::Document::parse(bytes, Format::Ssc).unwrap();
    assert_eq!(document.text(Some(0), "WARPS").as_deref(), Some("4=1"));
    assert_eq!(std::fs::read(old).unwrap(), disk);
    let mut workspace = Workspace::from_file(file, 0).unwrap();
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.document().format, Format::Sm);
    assert_eq!(workspace.file().output_policy(), &policy);
    assert_eq!(
        workspace.file().require_existing_path().unwrap(),
        destination
    );
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.file().output_policy(), &policy);
}
#[test]
fn unknown_or_stale_promotion_refuses_without_erasing_dirty_history_or_source() {
    let (_root, mut unknown) = file("#UNMAPPEDPROPERTY:keep exactly;");
    unknown.session.set_text(None, "TITLE", "dirty").unwrap();
    let before = unknown.session.serialize().unwrap();
    let policy = unknown.output_policy().clone();
    let undo = unknown.session.undo_label().map(str::to_owned);
    assert!(unknown.prepare_sm_authoring().is_err());
    assert_eq!(unknown.session.serialize().unwrap(), before);
    assert_eq!(unknown.output_policy(), &policy);
    assert_eq!(unknown.session.undo_label(), undo.as_deref());
    let (_root, mut stale) = file("");
    let plan = stale.prepare_sm_authoring().unwrap();
    stale.session.set_text(None, "TITLE", "later").unwrap();
    let before = stale.session.serialize().unwrap();
    assert!(stale.apply_sm_authoring(plan, Options::default()).is_err());
    assert_eq!(stale.session.serialize().unwrap(), before);
    assert_eq!(stale.session.document().format, Format::Sm);
    let plan = stale.prepare_sm_authoring().unwrap();
    std::fs::write(stale.require_existing_path().unwrap(), b"external").unwrap();
    assert!(stale.apply_sm_authoring(plan, Options::default()).is_err());
    assert_eq!(stale.session.serialize().unwrap(), before);
}

#[test]
fn old_namespace_open_witness_and_save_review_do_not_revive_after_promotion_undo() {
    let (_root, mut file) = file("");
    file.save().unwrap();
    let witness = file.published_open_witness().unwrap();
    let review = file.prepare_save().unwrap();
    assert!(review.is_current(&file));
    promote(&mut file);
    assert!(!review.is_current(&file));
    assert!(witness.validate_source(&file).is_err());
    let mut workspace = Workspace::from_file(file, 0).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert!(witness.validate_source(workspace.file()).is_err());
}

#[test]
fn native_publication_pins_destination_format_across_promotion_history() {
    for publish_ssc in [false, true] {
        let (root, mut file) = file("");
        promote(&mut file);
        if !publish_ssc {
            file.session.undo().unwrap();
        }
        let format = if publish_ssc { Format::Ssc } else { Format::Sm };
        let destination = root.path().canonicalize().unwrap().join(if publish_ssc {
            "native.ssc"
        } else {
            "native.sm"
        });
        let review = file
            .prepare_save_as_with_policy(&destination, OutputPolicy::NativeLossless)
            .unwrap();
        file.save_as(review).unwrap();
        if publish_ssc {
            file.session.undo().unwrap();
        } else {
            file.session.redo().unwrap();
        }
        assert!(
            matches!(file.output_policy(),OutputPolicy::AdvancedSongTiming{format:actual,..} if *actual==format)
        );
        assert!(file.prepare_save().is_ok());
        assert_eq!(file.require_existing_path().unwrap(), destination);
    }
}

#[test]
fn promotion_preserves_unsorted_metadata_chart_identity_with_identical_notes_and_clock() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("unsorted.sm");
    std::fs::write(&path,"#TITLE:Order;#BPMS:0=120;#NOTES:dance-single:z-last:Challenge:9:0,0,0,0,0:1000;#NOTES:dance-single:a-first:Easy:2:0,0,0,0,0:1000;").unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let ids = file.session.chart_ids().to_vec();
    promote(&mut file);
    assert_eq!(file.session.chart_ids(), ids);
    for (chart, description, difficulty) in [(0, "z-last", "Challenge"), (1, "a-first", "Easy")] {
        assert_eq!(
            file.session
                .document()
                .text(Some(chart), "DESCRIPTION")
                .as_deref(),
            Some(description)
        );
        assert_eq!(
            file.session
                .document()
                .text(Some(chart), "DIFFICULTY")
                .as_deref(),
            Some(difficulty)
        );
    }
}

#[test]
fn opaque_notes_are_preserved_exactly_or_promotion_refuses_atomically() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("opaque.sm");
    std::fs::write(&path, SM.replace("1000", "0[7]X00")).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let before = file.session.serialize().unwrap();
    let grid = file.session.grid(0).unwrap().clone();
    match file.prepare_sm_authoring() {
        Ok(plan) => match file.apply_sm_authoring(plan, Options::default()) {
            Ok(_) => assert_eq!(file.session.grid(0).unwrap(), &grid),
            Err(_) => assert_eq!(file.session.serialize().unwrap(), before),
        },
        Err(_) => assert_eq!(file.session.serialize().unwrap(), before),
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}
