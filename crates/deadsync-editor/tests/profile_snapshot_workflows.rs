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
