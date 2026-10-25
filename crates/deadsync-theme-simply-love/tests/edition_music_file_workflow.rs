use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_text::Field,
    music_file::{Browse, Picker},
};
use deadsync_gameplay::GameplayAudioCommand;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, path::PathBuf, time::Instant};
#[derive(Default)]
struct Choose {
    next: Option<Result<Option<PathBuf>, Error>>,
    calls: usize,
    last: Option<(PathBuf, String)>,
}
impl Picker for Choose {
    fn choose(&mut self, browse: &Browse) -> Result<Option<PathBuf>, Error> {
        self.calls += 1;
        self.last = Some((browse.directory.clone(), browse.current.clone()));
        self.next.take().unwrap_or(Ok(None))
    }
}
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
fn has(s: &State, value: &str) -> bool {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.iter()
        .any(|a| matches!(a,Actor::Text{content,..} if content.to_string().contains(value)))
}
const SONG: &str =
    "#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
fn setup() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    fs::write(
        dir.path().join("song.ogg"),
        b"fixture: command test, no decoder",
    )
    .unwrap();
    (dir, State::open(&path, 0).unwrap())
}
fn open_music(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
    for _ in 0..4 {
        press(s, KeyCode::Tab);
    }
}
fn apply(s: &mut State) {
    press(s, KeyCode::Escape);
    for _ in 0..6 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn play(s: &mut State) -> PathBuf {
    assert!(!s.is_playing());
    s.take_audio_commands();
    press(s, KeyCode::Enter);
    let commands = s.take_audio_commands();
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        GameplayAudioCommand::PlayMusic { path, .. } => path.clone(),
        other => panic!("{other:?}"),
    }
}
#[test]
fn picker_and_clear_stage_then_apply_undo_redo_save_and_drive_real_audio_commands() {
    let (dir, mut s) = setup();
    let base = s.workspace().file().path().parent().unwrap().to_path_buf();
    let folder = base.join("audio");
    fs::create_dir(&folder).unwrap();
    let asset = folder.join("Été #50%20.wav");
    fs::write(&asset, b"selected").unwrap();
    assert_eq!(play(&mut s), base.join("song.ogg"));
    open_music(&mut s);
    assert!(!s.is_playing());
    assert!(has(&s, "Music: | (song music)"));
    assert!(
        s.take_audio_commands()
            .iter()
            .any(|c| matches!(c, GameplayAudioCommand::StopMusic))
    );
    let mut picker = Choose {
        next: Some(Ok(Some(asset.clone()))),
        ..Choose::default()
    };
    press(&mut s, KeyCode::F2);
    assert!(s.process_music_picker(&mut picker));
    assert_eq!(picker.last, Some((base.clone(), String::new())));
    assert!(has(&s, "Music: audio/Été #50%20.wav|"));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    apply(&mut s);
    assert_eq!(play(&mut s), asset);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(play(&mut s), base.join("song.ogg"));
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(play(&mut s), asset);
    ctrl(&mut s, KeyCode::KeyS);
    let mut reopen = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    assert_eq!(play(&mut reopen), asset);
    open_music(&mut reopen);
    press(&mut reopen, KeyCode::F3);
    assert!(!reopen.process_music_picker(&mut picker));
    assert!(has(&reopen, "Chart music override cleared"));
    apply(&mut reopen);
    assert_eq!(play(&mut reopen), base.join("song.ogg"));
    assert_eq!(fs::read(&asset).unwrap(), b"selected");
}
#[test]
fn cancel_invalid_and_stale_picker_requests_keep_draft_history_and_do_not_open_dialogs() {
    let (dir, mut s) = setup();
    open_music(&mut s);
    assert!(s.text_input("existing.ogg"));
    let mut picker = Choose::default();
    press(&mut s, KeyCode::F2);
    assert!(s.process_music_picker(&mut picker));
    assert!(has(&s, "selection cancelled"));
    assert!(has(&s, "Music: existing.ogg|"));
    picker.next = Some(Ok(Some(dir.path().join("missing.ogg"))));
    press(&mut s, KeyCode::F2);
    assert!(s.process_music_picker(&mut picker));
    assert!(has(&s, "music file cannot be selected"));
    assert!(has(&s, "Music: existing.ogg|"));
    let calls = picker.calls;
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::Tab);
    assert!(!s.process_music_picker(&mut picker));
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Tab);
    key(&mut s, KeyCode::ShiftLeft, false);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("new.ogg"));
    assert!(!s.process_music_picker(&mut picker));
    press(&mut s, KeyCode::F2);
    s.text_preedit("未確定");
    assert!(!s.process_music_picker(&mut picker));
    s.text_preedit("");
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::F3);
    assert!(!s.process_music_picker(&mut picker));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Music: [new.ogg]|"));
    assert_eq!(picker.calls, calls);
    apply(&mut s);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "MUSIC")
            .as_deref(),
        Some("new.ogg")
    );
}
#[test]
fn keyboard_path_entry_keeps_missing_paths_editable_while_playback_reports_them() {
    let (_dir, mut s) = setup();
    open_music(&mut s);
    assert!(s.text_input("later.ogg"));
    apply(&mut s);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "MUSIC")
            .as_deref(),
        Some("later.ogg")
    );
    s.take_audio_commands();
    press(&mut s, KeyCode::Enter);
    assert!(!s.is_playing());
    assert!(s.take_audio_commands().is_empty());
    assert!(has(&s, "Music file not found"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "MUSIC"),
        None
    );
    let mut draft = s.workspace().chart_properties().unwrap();
    draft.set_text(Field::Music, "typed.ogg").unwrap();
    assert!(draft.has_changes());
}
