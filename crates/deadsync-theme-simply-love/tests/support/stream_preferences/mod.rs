use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    audio_preferences::File,
    stream_preferences::{NoteType, Preferences},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
const SOURCE:&[u8]=b"#VERSION:0.83;#TITLE:Streams;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n1000\n1000\n1000,\n1000\n1000\n1000\n1000;";
fn init() {
    if super::FRENCH {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
fn press(s: &mut State, c: KeyCode) {
    key(s, c, true);
    key(s, c, false);
}
fn ctrl(s: &mut State, c: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, c);
    key(s, KeyCode::SuperLeft, false);
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn settings(s: &mut State) {
    press(s, KeyCode::F1);
    down(s, 28); // Stream settings; newer tools follow this menu entry.
    press(s, KeyCode::Enter);
}
fn state(song: &std::path::Path, prefs: &std::path::Path) -> State {
    fs::write(song, SOURCE).unwrap();
    let mut s = State::open(song, 0).unwrap();
    s.configure_audio_preferences(prefs).unwrap();
    s
}
fn contains(s: &State, k: &str) -> bool {
    let mut a = Vec::new();
    s.actors(&mut a);
    let t = i18n::tr("Edition", k);
    a.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..} if content.as_ref()==t.as_ref()))
}
fn finish(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(20);
    while s.is_density_loading() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn assert_readonly(s: &State) {
    assert_eq!(s.workspace().file().session.serialize().unwrap(), SOURCE);
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn stream_note_type_save_recomputes_shared_density_and_undo_redo() {
    init();
    let d = tempfile::tempdir().unwrap();
    let song = d.path().join("s.ssc");
    let path = d.path().join("prefs.json");
    let mut s = state(&song, &path);
    finish(&mut s);
    assert!(
        s.source_density_report()
            .unwrap()
            .series
            .breakdown
            .is_none()
    );
    settings(&mut s);
    assert!(contains(&s, "StreamHelp"));
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowLeft);
    }
    down(&mut s, 8);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.stream_preferences().note_type, NoteType::Quarter);
    assert_eq!(
        File::open(&path).unwrap().streams().note_type,
        NoteType::Quarter
    );
    finish(&mut s);
    assert_eq!(
        s.source_density_report()
            .unwrap()
            .series
            .breakdown
            .as_deref(),
        Some("2")
    );
    ctrl(&mut s, KeyCode::KeyZ);
    finish(&mut s);
    assert_eq!(s.stream_preferences(), Preferences::default());
    assert!(
        s.source_density_report()
            .unwrap()
            .series
            .breakdown
            .is_none()
    );
    ctrl(&mut s, KeyCode::KeyY);
    finish(&mut s);
    assert_eq!(
        s.source_density_report()
            .unwrap()
            .series
            .breakdown
            .as_deref(),
        Some("2")
    );
    let mut reopened = State::open(&song, 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    assert_eq!(reopened.stream_preferences(), s.stream_preferences());
    assert_readonly(&s);
    assert_eq!(fs::read(&song).unwrap(), SOURCE);
}
#[test]
fn every_stream_field_is_keyboard_reachable_and_bmp_marks_use_text_input() {
    init();
    let d = tempfile::tempdir().unwrap();
    let song = d.path().join("s.ssc");
    let path = d.path().join("prefs.json");
    let mut s = state(&song, &path);
    settings(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.wants_text_input());
    assert!(s.text_input("é"));
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.text_input(":"));
    assert!(!s.text_input("😀"));
    assert!(contains(&s, "StreamCharacterInvalid"));
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    let p = s.stream_preferences();
    assert_eq!(p.note_type, NoteType::SixteenthTriplet);
    assert_eq!(
        p.accumulation,
        deadsync_editor::density_preferences::Accumulation::Row
    );
    assert!(p.show_break_lengths);
    assert_eq!(p.minimum_length, 2);
    assert_eq!(p.short_break_cutoff, 5);
    assert_eq!(p.short_break, 'é');
    assert_eq!(p.long_break, ':');
    assert_eq!(File::open(&path).unwrap().streams(), p);
    assert_readonly(&s);
}
#[test]
fn cancel_defaults_and_per_field_undo_preserve_other_stream_fields() {
    init();
    let d = tempfile::tempdir().unwrap();
    let song = d.path().join("s.ssc");
    let path = d.path().join("prefs.json");
    let mut s = state(&song, &path);
    settings(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.stream_preferences(), Preferences::default());
    settings(&mut s);
    down(&mut s, 5);
    assert!(s.text_input("/"));
    down(&mut s, 3);
    press(&mut s, KeyCode::Enter);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["Stream"]["LongBreakCharacter"] = "é".into();
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    // Import the disjoint external change into the live owner before undoing
    // only the independently tracked short-break field.
    s.configure_audio_preferences(&path).unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.stream_preferences().short_break, '-');
    assert_eq!(s.stream_preferences().long_break, 'é');
    settings(&mut s);
    down(&mut s, 7);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.stream_preferences(), Preferences::default());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.stream_preferences().long_break, 'é');
    assert_readonly(&s);
}
#[test]
fn empty_editor_preference_menu_labels_match_density_dark_and_stream_routes() {
    init();
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("empty.ssc");
    fs::write(&path, b"#VERSION:0.83;#TITLE:Empty;#BPMS:0=120;").unwrap();
    let mut s = State::open(&path, 0).unwrap();
    assert!(!s.workspace().has_chart());
    for (index, help) in [
        (12, "DensitySettingsHelp"),
        (13, "DarkHelp"),
        (14, "StreamHelp"),
    ] {
        press(&mut s, KeyCode::F1);
        down(&mut s, index);
        press(&mut s, KeyCode::Enter);
        assert!(contains(&s, help), "preference menu index {index}");
        press(&mut s, KeyCode::Escape);
    }
}
