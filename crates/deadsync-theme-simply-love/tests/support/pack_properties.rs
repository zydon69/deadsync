use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{document::Format, pack::Document as PackDocument};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, path::PathBuf, time::Instant};
fn key(state: &mut State, code: KeyCode, pressed: bool) -> bool {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    })
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn control(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
}
fn recover(state: &mut State) {
    key(state, KeyCode::ControlLeft, true);
    press(state, KeyCode::KeyR);
    key(state, KeyCode::ControlLeft, false);
}
fn replace_text(state: &mut State, value: &str) {
    control(state, KeyCode::KeyA);
    assert!(state.text_input(value));
}
fn text(state: &State) -> String {
    fn visit(actor: &Actor, result: &mut String) {
        match actor {
            Actor::Text { content, .. } => {
                result.push_str(content);
                result.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    visit(child, result)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut result = String::new();
    for actor in &actors {
        visit(actor, &mut result)
    }
    result
}
fn tr(key: &str) -> String {
    i18n::tr("Edition", key).to_string()
}
fn setup(format: Format, chartless: bool) -> (tempfile::TempDir, PathBuf, PathBuf, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("Pack");
    let song = pack.join("Song");
    fs::create_dir_all(&song).unwrap();
    let source = match (format, chartless) {
        (_, true) => "#VERSION:0.83;#TITLE:Fixture;#OFFSET:0.009;#BPMS:0=120;",
        (Format::Sm, false) => {
            "#TITLE:Fixture;#OFFSET:0.009;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;"
        }
        _ => {
            "#VERSION:0.83;#TITLE:Fixture;#OFFSET:0.009;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
        }
    };
    let path = song.join(if format == Format::Sm {
        "song.sm"
    } else {
        "song.ssc"
    });
    fs::write(&path, source).unwrap();
    let state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    (dir, pack, path, state, source.as_bytes().to_vec())
}
fn open(state: &mut State, chartless: bool) {
    press(state, KeyCode::F1);
    for _ in 0..if chartless { 7 } else { 18 } {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
    assert!(
        text(state).contains(&tr("PackPropertiesTitle")),
        "{}",
        text(state)
    );
}
fn pack_bytes() -> Vec<u8> {
    b";preserve\r\n[Group]\r\nVersion=7\r\nDisplayTitle=Original\r\nYear=2020\r\nSyncOffset=7\r\nUnknown=opaque\r\n".to_vec()
}
#[test]
fn explicit_create_edit_seven_fields_save_and_reopen_preserves_song_sm_ssc() {
    for format in [Format::Sm, Format::Ssc] {
        let (_dir, pack, path, mut state, song) = setup(format, false);
        open(&mut state, false);
        assert!(text(&state).contains(&tr("PackMissing")));
        assert!(!pack.join("Pack.ini").exists());
        press(&mut state, KeyCode::Enter);
        for value in [
            "Été 日本",
            "Ete Nihon",
            "Sort",
            "Series",
            "2025",
            "../Shared/banner.png",
        ] {
            replace_text(&mut state, value);
            press(&mut state, KeyCode::ArrowDown);
        }
        assert!(!state.wants_text_input());
        assert_eq!(
            deadsync_editor::song_properties::sync_offset(
                state.workspace().file().session.document()
            )
            .unwrap(),
            0.009
        );
        assert!(
            text(&state)
                .contains(i18n::tr_fmt("Edition", "PackSync", &[("value", "ITG")]).as_ref())
        );
        press(&mut state, KeyCode::ArrowRight);
        assert!(
            text(&state)
                .contains(i18n::tr_fmt("Edition", "PackSync", &[("value", "NULL")]).as_ref())
        );
        control(&mut state, KeyCode::KeyS);
        let data = PackDocument::parse(fs::read(pack.join("Pack.ini")).unwrap()).unwrap();
        assert_eq!(data.data().title.as_deref(), Some("Été 日本"));
        assert_eq!(
            data.data().title_transliteration.as_deref(),
            Some("Ete Nihon")
        );
        assert_eq!(data.data().title_sort.as_deref(), Some("Sort"));
        assert_eq!(data.data().series.as_deref(), Some("Series"));
        assert_eq!(data.data().year, 2025);
        assert_eq!(data.data().banner.as_deref(), Some("../Shared/banner.png"));
        assert_eq!(data.data().sync_offset, 0);
        assert!(!state.workspace().file().session.is_dirty());
        assert_eq!(fs::read(&path).unwrap(), song);
        press(&mut state, KeyCode::Escape);
        open(&mut state, false);
        assert!(text(&state).contains("Été 日本"));
        assert!(!pack.join("Song/Pack.ini").exists());
    }
}
#[test]
fn close_retains_new_draft_and_editor_exit_requires_explicit_discard() {
    let (_dir, pack, path, mut state, song) = setup(Format::Ssc, false);
    open(&mut state, false);
    press(&mut state, KeyCode::Enter);
    replace_text(&mut state, "Retained");
    press(&mut state, KeyCode::Escape);
    assert!(!pack.join("Pack.ini").exists());
    assert!(text(&state).contains(&tr("PackEditsKept")));
    state.workspace().file().save_recovery().unwrap();
    let source_binding =
        deadsync_editor::chart_order::SessionBinding::capture(&state.workspace().file().session);
    recover(&mut state);
    assert!(text(&state).contains(&tr("PackRecoveryBlocked")));
    assert!(source_binding.belongs_to(&state.workspace().file().session));
    assert!(!state.workspace().file().session.is_dirty());
    assert!(text(&state).contains("Retained"));
    press(&mut state, KeyCode::Escape);
    assert!(!key(&mut state, KeyCode::Escape, true));
    key(&mut state, KeyCode::Escape, false);
    assert!(text(&state).contains(&tr("PackPropertiesTitle")));
    assert!(text(&state).contains("Retained"));
    press(&mut state, KeyCode::PageDown);
    press(&mut state, KeyCode::Enter);
    assert!(!pack.join("Pack.ini").exists());
    assert!(key(&mut state, KeyCode::Escape, true));
    key(&mut state, KeyCode::Escape, false);
    assert_eq!(fs::read(path).unwrap(), song);
}
#[test]
fn focus_nullable_fields_and_unknown_sync_is_clean_then_explicit_discard_restores_disk() {
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    let bytes = pack_bytes();
    fs::write(pack.join("Pack.ini"), &bytes).unwrap();
    open(&mut state, false);
    for _ in 0..6 {
        press(&mut state, KeyCode::ArrowDown);
    }
    assert!(text(&state).contains(&tr("PackClean")));
    assert!(text(&state).contains("7"));
    press(&mut state, KeyCode::Escape);
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), bytes);
    open(&mut state, false);
    press(&mut state, KeyCode::PageUp);
    replace_text(&mut state, "Unsaved");
    press(&mut state, KeyCode::Escape);
    open(&mut state, false);
    assert!(text(&state).contains("Unsaved"));
    press(&mut state, KeyCode::PageDown);
    press(&mut state, KeyCode::Enter);
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn conflict_requires_confirmed_reload_and_failed_reload_retains_draft() {
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    fs::write(pack.join("Pack.ini"), pack_bytes()).unwrap();
    open(&mut state, false);
    replace_text(&mut state, "My edits");
    let external = String::from_utf8(pack_bytes())
        .unwrap()
        .replace("Original", "External");
    fs::write(pack.join("Pack.ini"), external.as_bytes()).unwrap();
    control(&mut state, KeyCode::KeyS);
    assert!(text(&state).contains(&tr("PackConflict")));
    assert_eq!(
        fs::read(pack.join("Pack.ini")).unwrap(),
        external.as_bytes()
    );
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains(&tr("PackReloadWarning")));
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains("External"));
    assert!(text(&state).contains(&tr("PackClean")));
    replace_text(&mut state, "Retain on invalid reload");
    fs::write(pack.join("Pack.ini"), b"malformed external").unwrap();
    control(&mut state, KeyCode::KeyS);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains(&tr("PackReadFailed")));
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    press(&mut state, KeyCode::Enter);
    assert!(text(&state).contains("Retain on invalid reload"));
    assert_eq!(
        fs::read(pack.join("Pack.ini")).unwrap(),
        b"malformed external"
    );
}
#[test]
fn invalid_year_is_kept_across_close_and_blocks_save_until_corrected_or_discarded() {
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    fs::write(pack.join("Pack.ini"), pack_bytes()).unwrap();
    open(&mut state, false);
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    replace_text(&mut state, "-1");
    press(&mut state, KeyCode::Escape);
    open(&mut state, false);
    assert!(text(&state).contains("-1"));
    control(&mut state, KeyCode::KeyS);
    assert!(text(&state).contains(&tr("PackInvalidYear")));
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), pack_bytes());
    replace_text(&mut state, "2027");
    control(&mut state, KeyCode::KeyS);
    assert_eq!(
        PackDocument::parse(fs::read(pack.join("Pack.ini")).unwrap())
            .unwrap()
            .data()
            .year,
        2027
    );
}
#[test]
fn newly_appeared_file_and_external_deletion_never_trigger_implicit_recreation() {
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    open(&mut state, false);
    press(&mut state, KeyCode::Enter);
    fs::write(pack.join("Pack.ini"), pack_bytes()).unwrap();
    control(&mut state, KeyCode::KeyS);
    assert!(text(&state).contains(&tr("PackConflict")));
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), pack_bytes());
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    fs::write(pack.join("Pack.ini"), pack_bytes()).unwrap();
    open(&mut state, false);
    replace_text(&mut state, "Edit");
    fs::remove_file(pack.join("Pack.ini")).unwrap();
    control(&mut state, KeyCode::KeyS);
    assert!(text(&state).contains(&tr("PackConflict")));
    assert!(!pack.join("Pack.ini").exists());
}
#[test]
fn chartless_pack_access_and_unicode_clipboard_use_the_same_text_pipeline() {
    struct Clipboard(String);
    impl deadsync_editor::clipboard::TextClipboard for Clipboard {
        fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
            Ok(self.0.clone())
        }
        fn write_text(&mut self, value: &str) -> Result<(), deadsync_editor::clipboard::Error> {
            self.0 = value.into();
            Ok(())
        }
    }
    let (_dir, pack, path, mut state, song) = setup(Format::Ssc, true);
    assert!(!state.workspace().has_chart());
    open(&mut state, true);
    press(&mut state, KeyCode::Enter);
    control(&mut state, KeyCode::KeyA);
    state.text_preedit("Été");
    assert!(text(&state).contains("Été"));
    state.text_preedit("");
    assert!(state.text_input("Été 日本"));
    control(&mut state, KeyCode::KeyA);
    control(&mut state, KeyCode::KeyC);
    let mut clipboard = Clipboard(String::new());
    assert!(state.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.0, "Été 日本");
    control(&mut state, KeyCode::KeyX);
    assert!(state.process_text_clipboard(&mut clipboard));
    control(&mut state, KeyCode::KeyV);
    assert!(state.process_text_clipboard(&mut clipboard));
    control(&mut state, KeyCode::KeyS);
    assert_eq!(
        PackDocument::parse(fs::read(pack.join("Pack.ini")).unwrap())
            .unwrap()
            .data()
            .title
            .as_deref(),
        Some("Été 日本")
    );
    assert_eq!(fs::read(path).unwrap(), song);
}

#[test]
fn clean_pack_allows_keyboard_song_recovery_without_changing_pack_file() {
    let (_dir, pack, _path, mut state, _song) = setup(Format::Ssc, false);
    fs::write(pack.join("Pack.ini"), pack_bytes()).unwrap();
    open(&mut state, false);
    press(&mut state, KeyCode::Escape);
    state.workspace().file().save_recovery().unwrap();
    let source_binding =
        deadsync_editor::chart_order::SessionBinding::capture(&state.workspace().file().session);
    recover(&mut state);
    assert!(text(&state).contains(&tr("DraftRecovered")));
    assert!(!source_binding.belongs_to(&state.workspace().file().session));
    assert!(state.workspace().file().session.is_dirty());
    assert_eq!(fs::read(pack.join("Pack.ini")).unwrap(), pack_bytes());
}
