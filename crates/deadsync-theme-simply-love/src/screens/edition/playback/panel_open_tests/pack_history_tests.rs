//! Real menu, Pack row editing, physical global history and picker boundaries.
use super::*;
use deadsync_editor::{
    music_file::{Browse, Kind, Picker},
    pack::Data,
    workspace::Command,
};
fn audio(s: &State) -> String {
    if s.is_playing() {
        witness(s).audio
    } else if let Some(p) = s.paused_position() {
        format!(
            "{:?}",
            (
                s.editor_asset_owner(),
                p.row.to_bits(),
                p.music_time.to_bits(),
                p.chart_time.to_bits(),
                s.paused_playback.as_ref().unwrap().intent_origin
            )
        )
    } else {
        "Idle".into()
    }
}
fn position(s: &State) -> (u64, u64, u64) {
    let presentation = s.capture_song_property_presentation().unwrap();
    let p = presentation.focus();
    assert!(p.is_current(&s.workspace));
    (
        p.row().to_bits(),
        p.music_time().to_bits(),
        p.chart_time().to_bits(),
    )
}
fn retained(s: &mut State, sound: &str, focus: (u64, u64, u64), step: &str) {
    assert_eq!(audio(s), sound, "{step}");
    assert_eq!(position(s), focus, "{step}");
    assert!(s.take_audio_commands().is_empty(), "{step}");
}
fn open_pack(s: &mut State) {
    bind(s, Action::OpenMenu);
    press(s, KeyCode::F12);
    assert!(matches!(s.panel, Some(Panel::Menu(0))));
    for _ in 0..18 {
        press(s, KeyCode::ArrowDown);
    }
    assert!(matches!(s.panel, Some(Panel::Menu(18))));
    press(s, KeyCode::Enter);
    assert!(
        matches!(s.panel, Some(Panel::PackProperties)),
        "{}",
        s.status
    );
}
fn row(s: &mut State, index: usize) {
    for _ in 0..10 {
        if s.pack_history_index_for_test() == Some(index) {
            return;
        }
        press(s, KeyCode::ArrowDown);
    }
    panic!("Pack row {index}: {}", s.status)
}
fn setup(mode: usize) -> (tempfile::TempDir, PathBuf, PathBuf, State) {
    let (root, song, mut s) = fixture(false, false);
    let pack = song.parent().unwrap().parent().unwrap().join("Pack.ini");
    fs::write(
        &pack,
        Data::new("Pack", "", 2026, None, 0.009)
            .canonical()
            .unwrap(),
    )
    .unwrap();
    s.workspace.execute(Command::Place(1)).unwrap();
    s.refresh();
    s.navigate_exact_position(-0.25, true);
    if mode != 0 {
        start(&mut s, mode == 3);
        if mode == 2 {
            s.pause_audio();
        }
    }
    s.take_audio_commands();
    (root, song, pack, s)
}
#[test]
fn physical_note_pack_sync_close_undo_redo_and_save_preserve_transport_all_modes_en_fr() {
    for language in ["en", "fr"] {
        for mode in 0..4 {
            let (_root, song, pack, mut s) = setup(mode);
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let notes = s.workspace.file().session.serialize().unwrap();
            let source = fs::read(&song).unwrap();
            let disk = fs::read(&pack).unwrap();
            let content = s.workspace.file().session.content_revision();
            let sound = audio(&s);
            let focus = position(&s);
            open_pack(&mut s);
            row(&mut s, 6);
            press(&mut s, KeyCode::ArrowRight);
            assert_eq!(s.pack_history_data_for_test().unwrap().sync_offset, 0);
            assert_eq!(s.workspace.file().session.content_revision(), content);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), notes);
            assert_eq!(fs::read(&pack).unwrap(), disk);
            retained(&mut s, &sound, focus, "Pack edit");
            // Explicit Save changes only the pack's baseline, not the property action.
            s.control = true;
            press(&mut s, KeyCode::KeyS);
            s.control = false;
            assert!(!s.pack_has_unsaved_changes(), "{}", s.status);
            let saved = fs::read(&pack).unwrap();
            assert_ne!(saved, disk);
            retained(&mut s, &sound, focus, "Save pack");
            press(&mut s, KeyCode::Escape);
            for (action, value) in [(Action::Undo, 1), (Action::Redo, 0)] {
                bind(&mut s, action);
                press(&mut s, KeyCode::F12);
                assert_eq!(
                    s.pack_history_data_for_test().unwrap().sync_offset,
                    value,
                    "{}",
                    s.status
                );
                assert_eq!(s.workspace.file().session.serialize().unwrap(), notes);
                assert_eq!(fs::read(&pack).unwrap(), saved);
                retained(&mut s, &sound, focus, "global pack history");
            }
            open_pack(&mut s);
            assert_eq!(s.pack_history_data_for_test().unwrap().sync_offset, 0);
            press(&mut s, KeyCode::Escape);
            assert_eq!(fs::read(&song).unwrap(), source);
        }
    }
}
struct Choose {
    path: Option<PathBuf>,
    directory: PathBuf,
}
impl Picker for Choose {
    fn choose(&mut self, request: &Browse) -> Result<Option<PathBuf>, deadsync_editor::Error> {
        assert_eq!(request.kind, Kind::Image);
        assert_eq!(request.directory, self.directory);
        Ok(self.path.clone())
    }
}
#[test]
fn physical_banner_browse_cancel_select_clear_after_save_and_global_undo_redo_en_fr() {
    for language in ["en", "fr"] {
        let (_root, _song, pack, mut s) = setup(1);
        i18n::init(deadsync_assets::language::load_for_tests(language));
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        // State::open uses the canonical resource root; macOS /var aliases /private/var.
        let root = pack.parent().unwrap().canonicalize().unwrap();
        let image = root.join("chosen.png");
        image::RgbaImage::new(1, 1).save(&image).unwrap();
        let sound = audio(&s);
        let focus = position(&s);
        open_pack(&mut s);
        row(&mut s, 5);
        let revision = s.workspace.file().session.revision();
        let old = s.pack_history_data_for_test().unwrap();
        press(&mut s, KeyCode::F2);
        assert!(s.process_music_picker(&mut Choose {
            path: None,
            directory: root.clone()
        }));
        assert_eq!(s.workspace.file().session.revision(), revision);
        assert_eq!(s.pack_history_data_for_test().unwrap(), old);
        retained(&mut s, &sound, focus, "Browse Cancel");
        press(&mut s, KeyCode::F2);
        assert!(s.process_music_picker(&mut Choose {
            path: Some(image),
            directory: root.clone()
        }));
        assert_eq!(
            s.pack_history_data_for_test().unwrap().banner.as_deref(),
            Some("chosen.png")
        );
        retained(&mut s, &sound, focus, "Browse selection");
        s.control = true;
        press(&mut s, KeyCode::KeyS);
        s.control = false;
        let saved = fs::read(&pack).unwrap();
        press(&mut s, KeyCode::Escape);
        bind(&mut s, Action::Undo);
        press(&mut s, KeyCode::F12);
        assert_eq!(s.pack_history_data_for_test().unwrap().banner, old.banner);
        assert_eq!(fs::read(&pack).unwrap(), saved);
        retained(&mut s, &sound, focus, "Browse Undo");
        bind(&mut s, Action::Redo);
        press(&mut s, KeyCode::F12);
        assert_eq!(
            s.pack_history_data_for_test().unwrap().banner.as_deref(),
            Some("chosen.png")
        );
        retained(&mut s, &sound, focus, "Browse Redo");
        open_pack(&mut s);
        row(&mut s, 5);
        press(&mut s, KeyCode::F3);
        assert_eq!(
            s.pack_history_data_for_test().unwrap().banner.as_deref(),
            Some("")
        );
        press(&mut s, KeyCode::Escape);
        bind(&mut s, Action::Undo);
        press(&mut s, KeyCode::F12);
        assert_eq!(
            s.pack_history_data_for_test().unwrap().banner.as_deref(),
            Some("chosen.png")
        );
        bind(&mut s, Action::Redo);
        press(&mut s, KeyCode::F12);
        assert_eq!(
            s.pack_history_data_for_test().unwrap().banner.as_deref(),
            Some("")
        );
        assert_eq!(fs::read(&pack).unwrap(), saved);
        retained(&mut s, &sound, focus, "Clear history");
    }
}
#[test]
fn physical_pack_history_external_conflict_is_localized_and_atomic() {
    for language in ["en", "fr"] {
        let (_root, _song, pack, mut s) = setup(3);
        i18n::init(deadsync_assets::language::load_for_tests(language));
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        open_pack(&mut s);
        row(&mut s, 6);
        press(&mut s, KeyCode::ArrowRight);
        press(&mut s, KeyCode::Escape);
        let sound = audio(&s);
        let focus = position(&s);
        let revision = s.workspace.file().session.revision();
        let notes = s.workspace.file().session.serialize().unwrap();
        fs::write(&pack, b"external owner").unwrap();
        bind(&mut s, Action::Undo);
        press(&mut s, KeyCode::F12);
        assert_eq!(s.status, i18n::tr("Edition", "PackConflict"));
        assert_eq!(s.workspace.file().session.revision(), revision);
        assert_eq!(s.pack_history_data_for_test().unwrap().sync_offset, 0);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), notes);
        assert_eq!(fs::read(pack).unwrap(), b"external owner");
        retained(&mut s, &sound, focus, "conflict refusal");
    }
}

#[test]
fn physical_reload_retires_old_property_history_with_localized_refusal() {
    for language in ["en", "fr"] {
        let (_root, _song, pack, mut s) = setup(1);
        i18n::init(deadsync_assets::language::load_for_tests(language));
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        open_pack(&mut s);
        row(&mut s, 6);
        press(&mut s, KeyCode::ArrowRight);
        let external = Data::new("External pack", "", 2027, None, 0.009);
        fs::write(&pack, external.canonical().unwrap()).unwrap();
        press(&mut s, KeyCode::F5);
        assert_eq!(s.status, i18n::tr("Edition", "PackConflict"));
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::ArrowUp);
        press(&mut s, KeyCode::Enter);
        assert_eq!(s.pack_history_data_for_test().unwrap(), external);
        press(&mut s, KeyCode::Escape);
        let sound = audio(&s);
        let focus = position(&s);
        let revision = s.workspace.file().session.revision();
        bind(&mut s, Action::Undo);
        press(&mut s, KeyCode::F12);
        assert_eq!(s.status, i18n::tr("Edition", "PackHistoryUnavailable"));
        assert_eq!(s.workspace.file().session.revision(), revision);
        assert_eq!(s.pack_history_data_for_test().unwrap(), external);
        assert_eq!(fs::read(&pack).unwrap(), external.canonical().unwrap());
        retained(&mut s, &sound, focus, "old owner refusal");
    }
}
#[test]
fn physical_text_field_edits_form_one_global_action_per_focus_session() {
    for field in 0..6 {
        let (_root, _song, pack, mut s) = setup(1);
        open_pack(&mut s);
        row(&mut s, field);
        let before = s.pack_history_data_for_test().unwrap();
        let sound = audio(&s);
        let focus = position(&s);
        s.control = true;
        press(&mut s, KeyCode::KeyA);
        s.control = false;
        assert!(s.text_input(if field == 4 { "20" } else { "New" }));
        assert!(s.text_input(if field == 4 { "30" } else { " value" }));
        assert_eq!(
            s.pack_history_data_for_test().unwrap(),
            before,
            "typing is only a cached field draft"
        );
        press(&mut s, KeyCode::Escape);
        let after = s.pack_history_data_for_test().unwrap();
        assert_ne!(after, before);
        bind(&mut s, Action::Undo);
        press(&mut s, KeyCode::F12);
        assert_eq!(s.pack_history_data_for_test().unwrap(), before);
        assert!(
            !s.workspace
                .file()
                .session
                .history_changes_pack_properties(false),
            "one action for a continuously edited field"
        );
        retained(&mut s, &sound, focus, "field Undo");
        bind(&mut s, Action::Redo);
        press(&mut s, KeyCode::F12);
        assert_eq!(s.pack_history_data_for_test().unwrap(), after);
        assert_eq!(fs::read(&pack).unwrap(), before.canonical().unwrap());
        retained(&mut s, &sound, focus, "field Redo");
    }
}

#[test]
fn physical_text_return_to_original_before_deactivation_preserves_undo_redo_and_transport() {
    let (_root, _song, pack, mut s) = setup(0);
    s.workspace.execute(Command::Undo).unwrap();
    s.refresh();
    s.navigate_exact_position(-0.25, true);
    start(&mut s, false);
    s.take_audio_commands();
    let sound = audio(&s);
    let focus = position(&s);
    let disk = fs::read(&pack).unwrap();
    let source = s.workspace.file().session.serialize().unwrap();
    let revision = s.workspace.file().session.revision();
    let history = (
        s.workspace.file().session.undo_label().map(str::to_owned),
        s.workspace.file().session.redo_label().map(str::to_owned),
    );
    assert!(history.1.is_some());
    open_pack(&mut s);
    row(&mut s, 0);
    let original = s.pack_history_data_for_test().unwrap();
    for text in ["Temporary", "Pack"] {
        s.control = true;
        press(&mut s, KeyCode::KeyA);
        s.control = false;
        assert!(s.text_input(text));
        assert_eq!(s.pack_history_data_for_test().unwrap(), original);
        assert_eq!(s.workspace.file().session.revision(), revision);
        assert_eq!(
            (
                s.workspace.file().session.undo_label().map(str::to_owned),
                s.workspace.file().session.redo_label().map(str::to_owned)
            ),
            history
        );
    }
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.pack_history_data_for_test().unwrap(), original);
    assert_eq!(s.workspace.file().session.revision(), revision);
    assert_eq!(
        (
            s.workspace.file().session.undo_label().map(str::to_owned),
            s.workspace.file().session.redo_label().map(str::to_owned)
        ),
        history
    );
    assert_eq!(fs::read(pack).unwrap(), disk);
    assert_eq!(s.workspace.file().session.serialize().unwrap(), source);
    assert!(!s.pack_has_unsaved_changes());
    retained(&mut s, &sound, focus, "net-zero cached field");
}
