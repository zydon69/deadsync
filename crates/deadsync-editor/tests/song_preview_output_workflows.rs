use deadsync_editor::{
    advanced_export::{IssueKind, Limits, Options, Plan, Severity},
    document::{Document, Format},
    history::Session,
    song_properties::Draft,
    song_text::{Field, resident},
    storage::FileSession,
};
use std::{collections::BTreeSet, fs, sync::atomic::AtomicBool};

fn source(raw: &str) -> Vec<u8> {
    format!(
        "#TITLE:Preview;#BPMS:0=120;#PREVIEW:{};#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;",
        String::from_utf8(deadsync_editor::document::escape_text(raw)).unwrap()
    )
    .into_bytes()
}
fn session(raw: &str, authored: Option<&str>) -> Session {
    let document = Document::parse(source(raw), Format::Sm).unwrap();
    let document = match authored {
        Some(path) => resident::apply(&document, path).unwrap(),
        None => document,
    };
    Session::new(document)
}
fn edit(file: &mut FileSession, path: &str) {
    let mut draft = Draft::capture(&file.session).unwrap();
    draft.set_text(Field::PreviewMusic, path).unwrap();
    assert!(draft.apply(&mut file.session).unwrap());
}
fn preview(file: &FileSession) -> String {
    resident::effective(file.session.document()).unwrap()
}

#[test]
fn source_exports_review_opaque_and_authored_losses_without_activating_sm_raw_preview() {
    for raw in ["", "ignored/preview.ogg"] {
        for authored in [None, Some("authored/a:b;clip.ogg"), Some("")] {
            for target in [Format::Sm, Format::Ssc] {
                for omit_custom_data in [false, true] {
                    let mut session = session(raw, authored);
                    let before = session.serialize().unwrap();
                    let revision = session.revision();
                    let plan = Plan::prepare_song_timing(
                        &mut session,
                        target,
                        Options {
                            omit_custom_data,
                            ..Options::default()
                        },
                        Limits::default(),
                        &AtomicBool::new(false),
                    )
                    .unwrap();
                    assert!(
                        !plan
                            .issues()
                            .iter()
                            .any(|issue| issue.severity == Severity::Error),
                        "{:?}",
                        plan.issues()
                    );
                    let raw_loss = plan
                        .issues()
                        .iter()
                        .filter(|issue| {
                            issue.field == "PREVIEW" && issue.kind == IssueKind::UnknownProperty
                        })
                        .count();
                    let authored_loss = plan
                        .issues()
                        .iter()
                        .filter(|issue| {
                            issue.field == "PREVIEW" && issue.kind == IssueKind::OmittedEvent
                        })
                        .count();
                    assert_eq!(raw_loss, usize::from(!raw.is_empty()));
                    assert_eq!(
                        authored_loss,
                        usize::from(target == Format::Sm && authored.is_some())
                    );
                    let acknowledgements: BTreeSet<_> = (0..plan.issues().len()).collect();
                    if !acknowledgements.is_empty() {
                        assert!(
                            plan.generate(
                                &session,
                                &BTreeSet::new(),
                                1024 * 1024,
                                &AtomicBool::new(false)
                            )
                            .is_err()
                        );
                    }
                    let output = plan
                        .generate(
                            &session,
                            &acknowledgements,
                            1024 * 1024,
                            &AtomicBool::new(false),
                        )
                        .unwrap();
                    let reopened = Document::parse(output.bytes(), target).unwrap();
                    assert_eq!(resident::read(&reopened).unwrap(), None);
                    assert_eq!(
                        resident::effective(&reopened).unwrap(),
                        if target == Format::Ssc {
                            authored.unwrap_or("")
                        } else {
                            ""
                        }
                    );
                    if target == Format::Sm {
                        assert_eq!(reopened.text(None, "PREVIEW"), None);
                    }
                    assert_eq!(session.serialize().unwrap(), before);
                    assert_eq!(session.revision(), revision);
                    assert!(session.undo_label().is_none());
                }
            }
        }
    }
}

#[test]
fn save_as_rebases_existing_and_added_owner_and_history_but_preserves_opaque_raw() {
    for existing_owner in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let old = base.join("old");
        let new = base.join("new");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&new).unwrap();
        let path = old.join("song.sm");
        let initial = session(
            "opaque/never-rebased.ogg",
            existing_owner.then_some("initial.ogg"),
        );
        fs::write(&path, initial.serialize().unwrap()).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        // Exercise both AddedProperty and physical tag Value histories directly,
        // in addition to the typed SongProperties action used by the product.
        file.session
            .set_text(
                None,
                resident::TAG,
                &serde_json::json!({"version":1,"path":"first.ogg"}).to_string(),
            )
            .unwrap();
        edit(&mut file, "second.ogg");
        edit(&mut file, "");
        file.session.undo().unwrap(); // Keep a Clear in the redo stack during relocation.
        let ids = file.session.chart_ids().to_vec();
        let destination = new.join("song.sm");
        let review = file.prepare_save_as(&destination).unwrap();
        assert!(review.issues().is_empty(), "{:?}", review.issues());
        file.save_as(review).unwrap();
        assert_eq!(preview(&file), "../old/second.ogg");
        assert_eq!(
            file.session.document().text(None, "PREVIEW").as_deref(),
            Some("opaque/never-rebased.ogg")
        );
        assert_eq!(file.session.chart_ids(), ids);
        file.session.redo().unwrap();
        assert_eq!(
            resident::read(file.session.document()).unwrap().as_deref(),
            Some("")
        );
        file.session.undo().unwrap();
        assert_eq!(preview(&file), "../old/second.ogg");
        file.session.undo().unwrap();
        assert_eq!(preview(&file), "../old/first.ogg");
        file.session.undo().unwrap();
        assert_eq!(
            preview(&file),
            if existing_owner {
                "../old/initial.ogg"
            } else {
                ""
            }
        );
        for _ in 0..2 {
            file.session.redo().unwrap();
        }
        assert_eq!(preview(&file), "../old/second.ogg");
        let reopened = FileSession::open(&destination).unwrap();
        assert_eq!(preview(&reopened), "../old/second.ogg");
        assert_eq!(
            reopened.session.document().text(None, "PREVIEW").as_deref(),
            Some("opaque/never-rebased.ogg")
        );
    }
}

#[test]
fn recovery_retains_authored_override_and_clear_without_touching_original() {
    for value in ["preview.ogg", ""] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.sm");
        let original = source("ignored.ogg");
        fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        edit(&mut file, value);
        let draft = file.session.serialize().unwrap();
        file.save_recovery().unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        drop(file);
        let mut file = FileSession::open(&path).unwrap();
        file.recover().unwrap();
        assert_eq!(file.session.serialize().unwrap(), draft);
        assert_eq!(
            resident::read(file.session.document()).unwrap().as_deref(),
            Some(value)
        );
        assert_eq!(
            file.session.document().text(None, "PREVIEW").as_deref(),
            Some("ignored.ogg")
        );
        assert!(file.session.is_dirty());
    }
}

#[test]
fn malformed_preview_owner_refuses_export_and_relocation_without_publication() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.sm");
    fs::write(&path, source("ignored.ogg")).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session
        .set_text(
            None,
            resident::TAG,
            "{\"version\":99,\"path\":\"broken.ogg\"}",
        )
        .unwrap();
    let before = file.session.serialize().unwrap();
    assert!(
        Plan::prepare_song_timing(
            &mut file.session,
            Format::Ssc,
            Options::default(),
            Limits::default(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let destination = root.path().join("copy.sm");
    assert!(file.prepare_save_as(&destination).is_err());
    assert!(!destination.exists());
    assert_eq!(file.session.serialize().unwrap(), before);
}
