use deadsync_editor::{
    Error,
    document::{Document, Format},
    generation::{
        config::PatternConfig,
        metadata::Registry,
        profile_catalog::Task,
        profiles::{Kind, Profile},
    },
    history::Session,
};
use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
};
fn session() -> Session {
    Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    )
}
#[test]
fn every_preset_can_be_captured_and_foreign_or_stale_sessions_are_refused() {
    let mut s = session();
    let catalog = Task::capture(&s, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let registry = Registry::builtin();
    for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
        for entry in catalog.entries(kind) {
            let profile = catalog.profile(&s, 0, kind, &entry.guid).unwrap();
            assert_eq!(profile.guid(), &entry.guid);
            match profile.config() {
                deadsync_editor::generation::profiles::Config::Pattern(c) => {
                    assert_eq!(registry.patterns.get(&entry.guid), Some(c))
                }
                deadsync_editor::generation::profiles::Config::Performed(c) => {
                    assert_eq!(registry.performed.get(&entry.guid), Some(c))
                }
                deadsync_editor::generation::profiles::Config::Expressed(c) => {
                    assert_eq!(registry.expressed.get(&entry.guid), Some(c))
                }
            }
        }
    }
    let id = catalog.entries(Kind::Expressed)[0].guid.clone();
    assert!(
        catalog
            .profile(&session(), 0, Kind::Expressed, &id)
            .is_err()
    );
    s.set_text(None, "TITLE", "Changed").unwrap();
    assert!(catalog.profile(&s, 0, Kind::Expressed, &id).is_err());
}
#[test]
fn changed_custom_file_cannot_be_captured_from_old_catalog() {
    let s = session();
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile::new_pattern("Custom", PatternConfig::sixteenths()).unwrap();
    let file = profile.export_new(dir.path()).unwrap();
    let catalog = Task::capture(&s, 0, Some(dir.path()))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        catalog
            .profile(&s, 0, Kind::Pattern, profile.guid())
            .unwrap()
            .config(),
        profile.config()
    );
    fs::write(file.path(), b"external change").unwrap();
    assert!(matches!(
        catalog.profile(&s, 0, Kind::Pattern, profile.guid()),
        Err(Error::ExternalChange)
    ));
}

#[test]
fn writable_capture_protects_presets_and_refuses_changes_before_capture_or_save() {
    let s = session();
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile::new_pattern("Original", PatternConfig::sixteenths()).unwrap();
    let original = profile.export_new(dir.path()).unwrap();
    let catalog = Task::capture(&s, 0, Some(dir.path()))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let builtin = &catalog.entries(Kind::Expressed)[0].guid;
    assert!(
        catalog
            .profile_file(&s, 0, Kind::Expressed, builtin)
            .is_err()
    );
    assert!(
        catalog
            .profile_file(&session(), 0, Kind::Pattern, profile.guid())
            .is_err()
    );
    let mut captured = catalog
        .profile_file(&s, 0, Kind::Pattern, profile.guid())
        .unwrap();
    let mut edited = captured.profile().clone();
    edited.rename("Changed").unwrap();
    fs::write(original.path(), b"external change after capture").unwrap();
    assert!(matches!(captured.save(&edited), Err(Error::ExternalChange)));
    assert!(matches!(
        catalog.profile_file(&s, 0, Kind::Pattern, profile.guid()),
        Err(Error::ExternalChange)
    ));
    assert_eq!(
        fs::read(original.path()).unwrap(),
        b"external change after capture"
    );
}

#[test]
fn anonymous_import_keeps_catalog_identity_when_captured_and_saved_in_place() {
    let s = session();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ecc-anonymous.json");
    fs::write(&path, br#"{"Name":"Anonymous","Config":{}}"#).unwrap();
    let catalog = Task::capture(&s, 0, Some(dir.path()))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let id = catalog
        .entries(Kind::Expressed)
        .iter()
        .find(|e| e.builtin.is_none())
        .unwrap()
        .guid
        .clone();
    assert_eq!(
        catalog.profile(&s, 0, Kind::Expressed, &id).unwrap().guid(),
        &id
    );
    let mut file = catalog.profile_file(&s, 0, Kind::Expressed, &id).unwrap();
    assert_eq!(file.profile().guid(), &id);
    let mut edited = file.profile().clone();
    edited.rename("Stable identity").unwrap();
    file.save(&edited).unwrap();
    let reopened = deadsync_editor::generation::profiles::ProfileFile::open(&path).unwrap();
    assert_eq!(reopened.profile().guid(), &id);
}
