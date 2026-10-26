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
