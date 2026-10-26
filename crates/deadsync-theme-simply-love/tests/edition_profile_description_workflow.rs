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
fn description_copy_and_in_place_save_clear_and_noop_keep_name_config_and_song_stable() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F3);
    assert!(s.wants_text_input());
    assert!(has(&s, "Enter: stage description"));
    assert!(s.text_input("Appuis équilibrés 日本語"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!has(&s, "Appuis équilibrés 日本語"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(has(&s, "Appuis équilibrés 日本語"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert_eq!(fs::read(&file).unwrap(), original);
    let loaded = deadsync_editor::generation::profiles::Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 2);
    let copy = loaded
        .profiles
        .values()
        .find(|f| {
            f.profile()
                .name()
                .is_some_and(|n| n.starts_with("Custom -"))
        })
        .unwrap();
    assert_eq!(
        copy.profile().description(),
        Some("Appuis équilibrés 日本語")
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    // Reopen the original chart/profile, update its description without changing its name.
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F3);
    assert!(s.text_input("Shared description"));
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    let saved = deadsync_editor::generation::profiles::ProfileFile::open(&file).unwrap();
    assert_eq!(saved.profile().description(), Some("Shared description"));
    assert_eq!(saved.profile().name(), Some("Original"));
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::Backspace);
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    assert_eq!(
        deadsync_editor::generation::profiles::ProfileFile::open(&file)
            .unwrap()
            .profile()
            .description(),
        Some("")
    );
    let before = fs::read(&file).unwrap();
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn cancelled_description_and_stale_clipboard_between_name_and_description_do_not_write() {
    use deadsync_editor::clipboard::{Error, TextClipboard};
    struct Board {
        reads: usize,
    }
    impl TextClipboard for Board {
        fn read_text(&mut self) -> Result<String, Error> {
            self.reads += 1;
            Ok("From clipboard".into())
        }
        fn write_text(&mut self, _: &str) -> Result<(), Error> {
            Ok(())
        }
    }
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let before = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F2);
    ctrl(&mut s, KeyCode::KeyV);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F3);
    let mut board = Board { reads: 0 };
    assert!(!s.process_text_clipboard(&mut board));
    assert_eq!(board.reads, 0);
    ctrl(&mut s, KeyCode::KeyV);
    assert!(s.process_text_clipboard(&mut board));
    assert!(has(&s, "From clipboard"));
    press(&mut s, KeyCode::Escape);
    assert!(has(&s, "Description changes discarded"));
    original_save(&mut s);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn existing_multiline_description_is_visually_single_line_but_noop_retains_original_text() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (_, file, source) = custom(&path, &profiles);
    let mut saved = deadsync_editor::generation::profiles::ProfileFile::open(&file).unwrap();
    let mut profile = saved.profile().clone();
    profile
        .set_description(Some(
            "Line one
Line two",
        ))
        .unwrap();
    saved.save(&profile).unwrap();
    let before = fs::read(&file).unwrap();
    let mut s = State::open_with_profiles(&path, 0, &profiles).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F3);
    assert!(has(&s, "Line one\\nLine two"), "{:?}", texts(&s));
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn accepting_absent_description_without_typing_does_not_insert_an_empty_member() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let before = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(
        deadsync_editor::generation::profiles::ProfileFile::open(&file)
            .unwrap()
            .profile()
            .description(),
        None
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
