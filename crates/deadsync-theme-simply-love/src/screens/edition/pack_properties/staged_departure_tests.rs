use super::*;
use std::fs;
fn fixture() -> (tempfile::TempDir, State, PathBuf) {
    i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("Song");
    fs::create_dir(&song).unwrap();
    let path = song.join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#TITLE:Song;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let pack = root.path().join("Pack.ini");
    fs::write(
        &pack,
        Data::new("Original", "", 2026, None, 0.009)
            .canonical()
            .unwrap(),
    )
    .unwrap();
    let mut state = State::open(&path, 0).unwrap();
    state.open_pack_properties();
    (root, state, pack)
}
#[test]
fn coupled_save_flushes_retained_field_even_after_panel_and_row_changed_and_undo_stays_memory_only()
{
    for index in [0, 3, 6, 8] {
        let (_root, mut state, pack) = fixture();
        let song = state.workspace.file().session.serialize().unwrap();
        let original = state
            .pack_properties
            .as_ref()
            .unwrap()
            .file
            .as_ref()
            .unwrap()
            .data()
            .clone();
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(Field::Title, "Final cached title")
            .unwrap();
        state.pack_properties.as_mut().unwrap().index = index;
        state.panel = Some(Panel::FileUnsaved(1));
        state.text_buffer = None;
        assert!(state.save_coupled_pack(), "index {index}: {}", state.status);
        let published = fs::read(&pack).unwrap();
        assert!(!state.pack_has_unsaved_changes());
        assert_eq!(
            state
                .pack_properties
                .as_ref()
                .unwrap()
                .file
                .as_ref()
                .unwrap()
                .data()
                .title
                .as_deref(),
            Some("Final cached title")
        );
        assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
        state
            .workspace
            .execute(deadsync_editor::workspace::Command::Undo)
            .unwrap();
        assert_eq!(
            state
                .pack_properties
                .as_ref()
                .unwrap()
                .file
                .as_ref()
                .unwrap()
                .history_owner()
                .unwrap()
                .data()
                .unwrap(),
            original
        );
        assert_eq!(
            fs::read(&pack).unwrap(),
            published,
            "Undo after explicit Save is memory-only"
        );
    }
}
#[test]
fn departure_validates_all_retained_fields_before_any_property_or_disk_publication() {
    let (_root, mut state, pack) = fixture();
    let before = fs::read(&pack).unwrap();
    let data = state
        .pack_properties
        .as_ref()
        .unwrap()
        .file
        .as_ref()
        .unwrap()
        .data()
        .clone();
    state
        .pack_properties
        .as_mut()
        .unwrap()
        .set_text(Field::Title, "Valid cached title")
        .unwrap();
    assert!(
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(Field::Year, "invalid")
            .is_err()
    );
    state.pack_properties.as_mut().unwrap().index = 8;
    state.panel = None;
    assert!(!state.save_coupled_pack());
    assert_eq!(
        state
            .pack_properties
            .as_ref()
            .unwrap()
            .file
            .as_ref()
            .unwrap()
            .data(),
        &data
    );
    assert!(state.workspace.file().session.undo_label().is_none());
    assert_eq!(fs::read(&pack).unwrap(), before);
    assert!(state.pack_has_unsaved_changes());
}
#[test]
fn ignored_external_pack_accepts_cached_input_but_coupled_save_preserves_external_bytes_and_history()
 {
    let (_root, mut state, pack) = fixture();
    let before = state
        .pack_properties
        .as_ref()
        .unwrap()
        .file
        .as_ref()
        .unwrap()
        .data()
        .clone();
    let external = b"[Group]\nDisplayTitle=External replacement\n";
    fs::write(&pack, external).unwrap();
    state
        .pack_properties
        .as_mut()
        .unwrap()
        .set_text(Field::Title, "Local after Ignore")
        .unwrap();
    state.pack_properties.as_mut().unwrap().index = 8;
    state.panel = None;
    assert!(!state.save_coupled_pack());
    assert!(state.pack_has_unsaved_changes());
    assert_eq!(
        state
            .pack_properties
            .as_ref()
            .unwrap()
            .file
            .as_ref()
            .unwrap()
            .data(),
        &before
    );
    assert!(state.workspace.file().session.undo_label().is_none());
    assert_eq!(fs::read(&pack).unwrap(), external);
}

#[test]
fn invalid_retained_fields_keep_localized_diagnostics_and_publish_nothing_en_fr() {
    for language in ["en", "fr"] {
        for (field, value, key) in [
            (Field::Title, "Invalid\nmultiline", "PackInvalidText"),
            (Field::Year, "invalid", "PackInvalidYear"),
            (Field::Year, "-1", "PackInvalidYear"),
        ] {
            let (_root, mut state, pack) = fixture();
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let bytes = fs::read(&pack).unwrap();
            assert!(
                state
                    .pack_properties
                    .as_mut()
                    .unwrap()
                    .set_text(field, value)
                    .is_err()
            );
            state.pack_properties.as_mut().unwrap().index = 8;
            state.panel = None;
            assert!(!state.save_coupled_pack());
            assert_eq!(state.status, i18n::tr("Edition", key), "{language} {key}");
            assert_eq!(fs::read(&pack).unwrap(), bytes);
            assert!(state.workspace.file().session.undo_label().is_none());
            assert!(state.pack_has_unsaved_changes());
        }
    }
}
