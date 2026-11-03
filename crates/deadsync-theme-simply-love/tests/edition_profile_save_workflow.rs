use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|text| text.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn finish(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while s.is_generating() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::KeyE);
}
fn custom(
    path: &std::path::Path,
    profiles: &std::path::Path,
) -> (State, std::path::PathBuf, Vec<u8>) {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            metadata::ChartData,
            profiles::{Kind, Profile},
        },
        history::Session,
    };
    let profile=Profile::parse(Kind::Expressed,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Future":"keep","Config":{"MinLevelForBrackets":4}}"#).unwrap();
    let file = profile.export_new(profiles).unwrap();
    let mut session = Session::new(
        Document::parse(
            std::sync::Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    ChartData {
        expressed_chart_config: profile.guid().clone(),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let source = session.serialize().unwrap();
    fs::write(path, &source).unwrap();
    (
        State::open_with_profiles(path, 0, profiles).unwrap(),
        file.path().to_owned(),
        source,
    )
}
fn original_save(s: &mut State) {
    press(s, KeyCode::End);
    for _ in 0..3 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
}
#[test]
fn unicode_rename_and_configuration_update_existing_profile_without_changing_identity_or_song() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::Digit7);
    press(&mut s, KeyCode::F2);
    assert!(s.wants_text_input());
    s.text_preedit("日本");
    press(&mut s, KeyCode::Enter);
    assert!(s.wants_text_input());
    assert!(s.text_input("Été 日本語 🎵"));
    assert!(has(&s, "Été 日本語 🎵"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!has(&s, "Été 日本語 🎵"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(has(&s, "Été 日本語 🎵"));
    press(&mut s, KeyCode::Enter);
    assert!(!s.wants_text_input());
    original_save(&mut s);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let saved = deadsync_editor::generation::profiles::ProfileFile::open(&file).unwrap();
    assert_eq!(saved.profile().name(), Some("Été 日本語 🎵"));
    let deadsync_editor::generation::profiles::Config::Expressed(config) = saved.profile().config()
    else {
        panic!()
    };
    assert_eq!(config.min_level_for_brackets, 17);
    assert!(fs::read_to_string(file).unwrap().contains("keep"));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
    assert!(has(&s, "Été 日本語 🎵"));
}
#[test]
fn external_change_blocks_in_place_save_and_leaves_profile_draft_reviewable() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Pending"));
    press(&mut s, KeyCode::Enter);
    fs::write(&file, b"External content").unwrap();
    original_save(&mut s);
    assert_eq!(fs::read(file).unwrap(), b"External content");
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert!(has(&s, "Save original custom profile"));
    assert!(has(&s, "changed on disk"));
    press(&mut s, KeyCode::Escape);
    assert!(!s.wants_text_input());
}
#[test]
fn builtins_cannot_be_overwritten_and_cancelled_names_keep_other_staged_controls() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    fs::write(
        &path,
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit9);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Discard me"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Home);
    assert!(has(&s, "Minimum bracket level: 9"));
    original_save(&mut s);
    assert!(has(&s, "Built-in preset: cannot overwrite"));
    assert!(!profiles.exists());
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn profile_name_clipboard_uses_local_undo_and_cancels_stale_requests_without_io() {
    use deadsync_editor::clipboard::{Error, TextClipboard};
    struct Board {
        text: String,
        reads: usize,
        writes: usize,
    }
    impl TextClipboard for Board {
        fn read_text(&mut self) -> Result<String, Error> {
            self.reads += 1;
            Ok(self.text.clone())
        }
        fn write_text(&mut self, text: &str) -> Result<(), Error> {
            self.writes += 1;
            self.text = text.into();
            Ok(())
        }
    }
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F2);
    let mut board = Board {
        text: "Été:日本語".into(),
        reads: 0,
        writes: 0,
    };
    ctrl(&mut s, KeyCode::KeyV);
    assert!(s.process_text_clipboard(&mut board));
    assert!(has(&s, "Été:日本語"));
    ctrl(&mut s, KeyCode::KeyA);
    ctrl(&mut s, KeyCode::KeyX);
    assert!(s.process_text_clipboard(&mut board));
    assert_eq!(board.text, "Été:日本語");
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Été:日本語"));
    ctrl(&mut s, KeyCode::KeyV);
    let reads = board.reads;
    press(&mut s, KeyCode::Escape);
    assert!(!s.process_text_clipboard(&mut board));
    assert_eq!(board.reads, reads);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(
        fs::read(profiles.join("ecc-12345678-9abc-4def-8123-456789abcdef.json")).unwrap(),
        original
    );
}

#[test]
fn ime_composition_keys_do_not_delete_the_previously_committed_name() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, _, source) = custom(&path, &profiles);
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(has(&s, "Custom - Original"));
    s.text_preedit("日");
    press(&mut s, KeyCode::Backspace);
    s.text_preedit("");
    assert!(has(&s, "Custom - Original"), "{:?}", texts(&s));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn saved_history_survives_catalog_reload_and_refuses_pending_edits() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Saved name"));
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    let saved = fs::read(&file).unwrap();
    assert_ne!(saved, original);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Pending name"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F7);
    assert!(has(&s, "Save or discard pending"));
    assert_eq!(fs::read(&file).unwrap(), saved);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::End);
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowDown);
    }
    assert!(has(&s, i18n::tr("Edition", "ProfileUndoSave").as_ref()));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert_eq!(fs::read(&file).unwrap(), original);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F8);
    finish(&mut s);
    assert_eq!(fs::read(&file).unwrap(), saved, "{:?}", texts(&s));
    assert!(has(&s, "Saved name"));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn profile_deletion_confirmation_uses_global_history_and_preserves_song_file() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F9);
    assert!(has(&s, "Delete this custom profile?"));
    press(&mut s, KeyCode::Enter);
    assert_eq!(fs::read(&file).unwrap(), original);
    press(&mut s, KeyCode::F9);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(!file.exists());
    assert!(has(&s, "Profile deleted."), "{:?}", texts(&s));
    let repaired = s.workspace().file().session.serialize().unwrap();
    assert_ne!(repaired, source);
    press(&mut s, KeyCode::Digit2);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!file.exists());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    fs::write(&file, b"external replacement").unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(fs::read(&file).unwrap(), b"external replacement");
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    fs::remove_file(&file).unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(fs::read(&file).unwrap(), original);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    assert_eq!(fs::read(path).unwrap(), source);
}

fn conversion_selection(s: &mut State, custom: bool) {
    press(s, KeyCode::F3);
    for _ in 0..5 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::ArrowDown);
    if custom {
        for _ in 0..20 {
            if has(s, "Original") {
                break;
            }
            press(s, KeyCode::ArrowRight);
        }
        assert!(has(s, "Original"));
    } else {
        press(s, KeyCode::ArrowRight);
    }
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
    press(s, KeyCode::Escape);
}
fn expression_summary(s: &mut State) -> String {
    press(s, KeyCode::F3);
    for _ in 0..5 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
    let template = i18n::tr("Edition", "GenerationExpressionProfile");
    let prefix = template.split('{').next().unwrap();
    let summary = texts(s)
        .into_iter()
        .find(|text| text.starts_with(prefix))
        .expect("conversion profile inspector displays its selected interpretation");
    press(s, KeyCode::Escape);
    press(s, KeyCode::Escape);
    summary
}
fn delete_original(s: &mut State) {
    open(s);
    press(s, KeyCode::F9);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
#[test]
fn deleting_selected_conversion_profile_restores_its_selection_on_global_undo_and_redo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    conversion_selection(&mut s, true);
    assert!(expression_summary(&mut s).contains("Original"));
    delete_original(&mut s);
    assert!(!file.exists());
    let fallback = expression_summary(&mut s);
    assert!(!fallback.contains("Original"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(file.exists());
    assert!(expression_summary(&mut s).contains("Original"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(expression_summary(&mut s), fallback);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(expression_summary(&mut s).contains("Original"));
    assert_eq!(fs::read(path).unwrap(), source);
}
#[test]
fn a_later_explicit_conversion_choice_is_not_overwritten_by_deletion_undo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, _) = custom(&path, &profiles);
    conversion_selection(&mut s, true);
    delete_original(&mut s);
    conversion_selection(&mut s, false);
    let chosen = expression_summary(&mut s);
    assert!(!chosen.contains("Original"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(file.exists());
    assert_eq!(expression_summary(&mut s), chosen);
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(expression_summary(&mut s), chosen);
}

#[test]
fn equal_guids_in_different_profile_families_restore_only_the_deleted_family() {
    use deadsync_editor::generation::profiles::{Kind, Profile};
    for delete_movement in [false, true] {
        init();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.ssc");
        let profiles = dir.path().join("profiles");
        let (mut s, expression_file, source) = custom(&path, &profiles);
        let movement_json = serde_json::json!({
            "Guid": "12345678-9abc-4def-8123-456789abcdef", "Name": "Movement",
            "Config": deadsync_editor::generation::profiles::Config::editor_defaults(Kind::Performed).json().unwrap()
        });
        let movement = Profile::parse(
            Kind::Performed,
            &serde_json::to_vec(&movement_json).unwrap(),
        )
        .unwrap();
        let movement_file = movement.export_new(&profiles).unwrap().path().to_owned();
        press(&mut s, KeyCode::F3);
        for _ in 0..5 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        finish(&mut s);
        for _ in 0..20 {
            if has(&s, "Movement") {
                break;
            }
            press(&mut s, KeyCode::ArrowRight);
        }
        assert!(has(&s, "Movement"));
        press(&mut s, KeyCode::ArrowDown);
        for _ in 0..20 {
            if has(&s, "Original") {
                break;
            }
            press(&mut s, KeyCode::ArrowRight);
        }
        assert!(has(&s, "Original"));
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::Escape);
        if delete_movement {
            press(&mut s, KeyCode::F3);
            for _ in 0..5 {
                press(&mut s, KeyCode::ArrowDown);
            }
            press(&mut s, KeyCode::Enter);
            finish(&mut s);
            press(&mut s, KeyCode::KeyE);
            press(&mut s, KeyCode::F9);
            press(&mut s, KeyCode::ArrowDown);
            press(&mut s, KeyCode::Enter);
        } else {
            delete_original(&mut s);
        }
        assert_eq!(movement_file.exists(), !delete_movement);
        assert_eq!(expression_file.exists(), delete_movement);
        press(&mut s, KeyCode::F3);
        assert_eq!(has(&s, "Movement"), !delete_movement);
        assert_eq!(has(&s, "Original"), delete_movement);
        press(&mut s, KeyCode::Escape);
        ctrl(&mut s, KeyCode::KeyZ);
        press(&mut s, KeyCode::F3);
        assert!(has(&s, "Movement"));
        assert!(has(&s, "Original"));
        press(&mut s, KeyCode::Escape);
        ctrl(&mut s, KeyCode::KeyY);
        press(&mut s, KeyCode::F3);
        assert_eq!(has(&s, "Movement"), !delete_movement);
        assert_eq!(has(&s, "Original"), delete_movement);
        press(&mut s, KeyCode::Escape);
        if delete_movement {
            delete_original(&mut s);
            assert!(!movement_file.exists());
            assert!(!expression_file.exists());
            ctrl(&mut s, KeyCode::KeyZ);
            assert!(expression_summary(&mut s).contains("Original"));
            assert!(!movement_file.exists());
            ctrl(&mut s, KeyCode::KeyZ);
            press(&mut s, KeyCode::F3);
            assert!(has(&s, "Movement"));
            assert!(has(&s, "Original"));
            press(&mut s, KeyCode::Escape);
            ctrl(&mut s, KeyCode::KeyY);
            assert!(!movement_file.exists());
            assert!(expression_file.exists());
            ctrl(&mut s, KeyCode::KeyY);
            assert!(!movement_file.exists());
            assert!(!expression_file.exists());
            ctrl(&mut s, KeyCode::KeyZ);
            press(&mut s, KeyCode::F3);
            press(&mut s, KeyCode::ArrowDown);
            press(&mut s, KeyCode::ArrowRight);
            press(&mut s, KeyCode::Escape);
            ctrl(&mut s, KeyCode::KeyZ);
            assert!(movement_file.exists());
            press(&mut s, KeyCode::F3);
            assert!(!has(&s, "Movement"));
            assert!(has(&s, "Original"));
            press(&mut s, KeyCode::Escape);
        }
        assert_eq!(fs::read(path).unwrap(), source);
    }
}

#[test]
fn keyboard_exports_the_current_profile_draft_without_overwriting_or_changing_the_library() {
    use deadsync_editor::generation::profiles::{Config, Loaded, ProfileFile};
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, original, source) = custom(&path, &profiles);
    let original_bytes = fs::read(&original).unwrap();
    let exported = dir.path().join("ecc-été.json");
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::Digit7);
    press(&mut s, KeyCode::F11);
    assert!(s.wants_text_input());
    assert!(s.text_input(exported.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    assert!(!s.wants_text_input());
    let saved = ProfileFile::open(&exported).unwrap();
    let Config::Expressed(config) = saved.profile().config() else {
        panic!()
    };
    assert_eq!(config.min_level_for_brackets, 17);
    assert_ne!(
        saved.profile().guid(),
        ProfileFile::open(&original).unwrap().profile().guid()
    );
    let bytes = fs::read(&exported).unwrap();
    press(&mut s, KeyCode::F11);
    assert!(s.text_input(exported.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    assert!(s.wants_text_input());
    assert_eq!(fs::read(&exported).unwrap(), bytes);
    press(&mut s, KeyCode::Escape);
    assert_eq!(Loaded::directory(&profiles).unwrap().profiles.len(), 1);
    assert_eq!(fs::read(original).unwrap(), original_bytes);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
}
#[test]
fn keyboard_import_stages_a_new_identity_and_only_save_copy_adds_it_to_the_library() {
    use deadsync_editor::generation::profiles::{Config, Loaded, ProfileFile};
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, original, source) = custom(&path, &profiles);
    let original_bytes = fs::read(&original).unwrap();
    let shared = dir.path().join("ecc-shared.json");
    let shared_bytes = br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Shared","Future":1e999,"Config":{"MinLevelForBrackets":9}}"#;
    fs::write(&shared, shared_bytes).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F10);
    assert!(s.text_input(shared.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    assert!(!s.wants_text_input());
    assert_eq!(Loaded::directory(&profiles).unwrap().profiles.len(), 1);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F10);
    assert!(s.text_input(shared.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    let copy = loaded
        .profiles
        .values()
        .find(|f| f.profile().name() == Some("Shared"))
        .unwrap();
    assert_ne!(
        copy.profile().guid(),
        ProfileFile::open(&original).unwrap().profile().guid()
    );
    let Config::Expressed(config) = copy.profile().config() else {
        panic!()
    };
    assert_eq!(config.min_level_for_brackets, 9);
    assert!(fs::read_to_string(copy.path()).unwrap().contains("1e999"));
    assert_eq!(fs::read(&shared).unwrap(), shared_bytes);
    assert_eq!(fs::read(&original).unwrap(), original_bytes);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn failed_import_keeps_path_editable_and_leaves_the_existing_draft_intact() {
    use deadsync_editor::generation::profiles::{Config, Loaded};
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, original, source) = custom(&path, &profiles);
    let original_bytes = fs::read(&original).unwrap();
    let wrong = dir.path().join("pc-wrong.json");
    let broken = dir.path().join("ecc-broken.json");
    fs::write(&wrong, b"wrong kind").unwrap();
    fs::write(&broken, b"{broken").unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F10);
    assert!(s.text_input(wrong.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    assert!(s.wants_text_input());
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input(broken.to_str().unwrap()));
    press(&mut s, KeyCode::Enter);
    assert!(s.wants_text_input());
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    for file in loaded.profiles.values() {
        let Config::Expressed(config) = file.profile().config() else {
            panic!()
        };
        assert_eq!(config.min_level_for_brackets, 4);
    }
    assert_eq!(fs::read(original).unwrap(), original_bytes);
    assert_eq!(fs::read(wrong).unwrap(), b"wrong kind");
    assert_eq!(fs::read(broken).unwrap(), b"{broken");
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn save_copy_uses_global_undo_and_restores_conversion_selection_without_overwriting_later_choices()
{
    use deadsync_editor::generation::profiles::Loaded;
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, original, source) = custom(&path, &profiles);
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Undoable"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    let loaded = Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    let copy = loaded
        .profiles
        .values()
        .find(|f| f.profile().name() == Some("Undoable"))
        .unwrap()
        .path()
        .to_owned();
    let bytes = fs::read(&copy).unwrap();
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F3);
    for _ in 0..5 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    for _ in 0..20 {
        if has(&s, "Undoable") {
            break;
        }
        press(&mut s, KeyCode::ArrowRight);
    }
    assert!(has(&s, "Undoable"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Escape);
    let summary = expression_summary(&mut s);
    assert!(summary.contains("Undoable"), "{summary}; {:?}", texts(&s));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!copy.exists());
    assert!(original.exists());
    assert!(!s.workspace().file().session.is_dirty());
    assert!(!expression_summary(&mut s).contains("Undoable"));
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(fs::read(&copy).unwrap(), bytes);
    let summary = expression_summary(&mut s);
    assert!(summary.contains("Undoable"), "{summary}; {:?}", texts(&s));
    fs::write(&copy, b"external change").unwrap();
    let revision = s.workspace().file().session.revision();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(fs::read(&copy).unwrap(), b"external change");
    let summary = expression_summary(&mut s);
    assert!(
        summary.contains(i18n::tr("Edition", "ProfileMissing").as_ref()),
        "{summary}; {:?}",
        texts(&s)
    );
    assert_eq!(s.workspace().file().session.revision(), revision);
    fs::write(&copy, &bytes).unwrap();
    assert!(expression_summary(&mut s).contains("Undoable"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!copy.exists());
    conversion_selection(&mut s, false);
    let chosen = expression_summary(&mut s);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(fs::read(&copy).unwrap(), bytes);
    assert_eq!(expression_summary(&mut s), chosen);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
}
