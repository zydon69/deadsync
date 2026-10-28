//! Semantic registry, persistence, modal routing and keyboard-only capture.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::space::{Metrics, set_current_metrics};
use deadsync_editor::{audio_preferences::File, key_binding_preferences::Preferences};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
static SERIAL: Mutex<()> = Mutex::new(());
fn event(s: &mut State, code: KeyCode, pressed: bool, repeat: bool, time: Instant) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat,
        timestamp: time,
        host_nanos: 0,
    });
}
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    event(s, code, pressed, false, Instant::now());
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode, shift: bool) {
    key(s, KeyCode::SuperLeft, true);
    if shift {
        key(s, KeyCode::ShiftLeft, true);
    }
    press(s, code);
    if shift {
        key(s, KeyCode::ShiftLeft, false);
    }
    key(s, KeyCode::SuperLeft, false);
}
fn open(
    locale: &str,
    prefs: Preferences,
) -> (
    tempfile::TempDir,
    State,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    i18n::init(deadsync_assets::language::load_for_tests(locale));
    set_current_metrics(Metrics::centered(854., 480.));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
    )
    .unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    file.set_key_bindings(Arc::new(prefs)).unwrap();
    file.save().unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&path).unwrap();
    (dir, s, song, path)
}
#[test]
fn remapped_save_executes_existing_operation_and_old_shortcut_cannot_double_dispatch() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("Save".into(), vec![vec!["F13".into()]]);
        let (_d, mut s, song, _) = open(locale, prefs);
        let source = fs::read(&song).unwrap();
        press(&mut s, KeyCode::Digit1);
        assert!(s.workspace().file().session.is_dirty());
        ctrl(&mut s, KeyCode::KeyS, false);
        assert_eq!(fs::read(&song).unwrap(), source);
        assert!(s.workspace().file().session.is_dirty());
        press(&mut s, KeyCode::F13);
        assert!(!s.workspace().file().session.is_dirty());
        assert_ne!(fs::read(&song).unwrap(), source);
    }
}
#[test]
fn capture_enter_binding_is_separate_from_confirmation_and_saved_as_one_global_action() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, song, path) = open(locale, Preferences::default());
        let source = fs::read(&song).unwrap();
        ctrl(&mut s, KeyCode::KeyK, true);
        for _ in 0..2 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::Enter);
        for _ in 0..4 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        assert_eq!(
            File::open(&path).unwrap().key_bindings().commands["Save"],
            vec![vec!["Enter"]]
        );
        assert_eq!(fs::read(&song).unwrap(), source);
        assert!(!s.workspace().file().session.is_dirty());
        assert!(
            s.workspace()
                .file()
                .session
                .undo_changes_key_binding_preferences()
        );
        ctrl(&mut s, KeyCode::KeyZ, false);
        assert_eq!(
            File::open(&path).unwrap().key_bindings(),
            Arc::new(Preferences::default())
        );
        ctrl(&mut s, KeyCode::KeyY, false);
        assert_eq!(
            File::open(&path).unwrap().key_bindings().commands["Save"],
            vec![vec!["Enter"]]
        );
    }
}
#[test]
fn source_repeat_comes_from_frame_time_and_os_repeat_does_not_add_an_action() {
    let _g = SERIAL.lock().unwrap();
    let mut prefs = Preferences::default();
    prefs
        .commands
        .insert("MoveToNextMeasure".into(), vec![vec!["F14".into()]]);
    let (_d, mut s, _, _) = open("en", prefs);
    let time = Instant::now();
    event(&mut s, KeyCode::F14, true, false, time);
    let first = s.workspace().row();
    assert!(first > 0);
    event(
        &mut s,
        KeyCode::F14,
        true,
        true,
        time + Duration::from_millis(100),
    );
    assert_eq!(s.workspace().row(), first);
    s.update_with_time(time + Duration::from_millis(501));
    assert_eq!(s.workspace().row(), first * 2);
    event(
        &mut s,
        KeyCode::F14,
        false,
        false,
        time + Duration::from_millis(510),
    );
    s.update_with_time(time + Duration::from_secs(2));
    assert_eq!(s.workspace().row(), first * 2);
}
#[test]
fn modal_and_focus_cleanup_quarantine_held_shortcuts_until_release() {
    let _g = SERIAL.lock().unwrap();
    let mut prefs = Preferences::default();
    prefs
        .commands
        .insert("MoveToNextMeasure".into(), vec![vec!["F14".into()]]);
    let (_d, mut s, _, _) = open("en", prefs);
    let time = Instant::now();
    event(&mut s, KeyCode::F14, true, false, time);
    let first = s.workspace().row();
    press(&mut s, KeyCode::F1);
    s.update_with_time(time + Duration::from_secs(1));
    assert_eq!(s.workspace().row(), first);
    press(&mut s, KeyCode::Escape);
    s.update_with_time(time + Duration::from_secs(2));
    assert_eq!(s.workspace().row(), first);
    key(&mut s, KeyCode::F14, false);
    press(&mut s, KeyCode::F14);
    assert_eq!(s.workspace().row(), first * 2);
    s.release_input();
    event(
        &mut s,
        KeyCode::F14,
        true,
        true,
        time + Duration::from_secs(3),
    );
    s.update_with_time(time + Duration::from_secs(4));
    assert_eq!(s.workspace().row(), first * 2);
}

#[test]
fn paged_help_uses_configured_binding_in_both_languages() {
    use deadlib_present::actors::Actor;
    fn texts(actor: &Actor, out: &mut Vec<String>) {
        match actor {
            Actor::Text { content, .. } => out.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => children.iter().for_each(|child| texts(child, out)),
            _ => {}
        }
    }
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut preferences = Preferences::default();
        preferences
            .commands
            .insert("Save".into(), vec![vec!["F13".into()]]);
        let (_d, mut state, _, _) = open(locale, preferences);
        press(&mut state, KeyCode::F1);
        for _ in 0..12 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter);
        let mut all = Vec::new();
        for _ in 0..20 {
            let mut actors = Vec::new();
            state.actors(&mut actors);
            for actor in &actors {
                texts(actor, &mut all);
            }
            press(&mut state, KeyCode::PageDown);
        }
        let action = if locale == "en" {
            "Save"
        } else {
            "Enregistrer"
        };
        assert!(
            all.iter()
                .any(|text| text.contains("F13") && text.contains(action)),
            "{all:?}"
        );
        assert!(
            !all.iter()
                .any(|text| text.contains("Ctrl/Cmd+S:") || text.contains("Ctrl/Cmd+S :"))
        );
    }
}

#[test]
fn second_increment_remapped_snap_and_end_do_not_keep_old_dispatch_routes() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("IncreaseSnap".into(), vec![vec!["F13".into()]]);
        prefs
            .commands
            .insert("MoveToChartEnd".into(), vec![vec!["F14".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        let snap = s.workspace().snap();
        press(&mut s, KeyCode::ArrowRight);
        assert_eq!(s.workspace().snap(), snap);
        press(&mut s, KeyCode::F13);
        assert_ne!(s.workspace().snap(), snap);
        press(&mut s, KeyCode::PageDown);
        let last_row = s.workspace().row();
        assert!(last_row > 0);
        press(&mut s, KeyCode::Digit1);
        press(&mut s, KeyCode::Home);
        press(&mut s, KeyCode::End);
        assert_eq!(s.workspace().row(), 0);
        press(&mut s, KeyCode::F14);
        assert_eq!(s.workspace().row(), last_row);
    }
}

#[test]
fn second_increment_rate_remapping_keeps_playback_settings_semantic() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("IncreaseMusicRate".into(), vec![vec!["F15".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        let baseline = s.audio_preferences().music_rate();
        key(&mut s, KeyCode::ShiftLeft, true);
        press(&mut s, KeyCode::ArrowRight);
        key(&mut s, KeyCode::ShiftLeft, false);
        assert_eq!(s.audio_preferences().music_rate(), baseline);
        press(&mut s, KeyCode::F15);
        assert!(s.audio_preferences().music_rate() > baseline);
        let changed = s.audio_preferences().music_rate();
        key(&mut s, KeyCode::ControlLeft, true);
        key(&mut s, KeyCode::ShiftRight, true);
        press(&mut s, KeyCode::ArrowRight);
        key(&mut s, KeyCode::ShiftRight, false);
        key(&mut s, KeyCode::ControlLeft, false);
        assert_eq!(s.audio_preferences().music_rate(), changed);
    }
}

#[test]
fn third_increment_remapped_menu_and_save_as_open_existing_panels_only_once() {
    use deadlib_present::actors::Actor;
    fn all_texts(actor: &Actor, out: &mut Vec<String>) {
        match actor {
            Actor::Text { content, .. } => out.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => {
                children.iter().for_each(|child| all_texts(child, out))
            }
            _ => {}
        }
    }
    fn visible(s: &mut State) -> Vec<String> {
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let mut out = Vec::new();
        for actor in actors {
            all_texts(&actor, &mut out);
        }
        out
    }
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("OpenEditorMenu".into(), vec![vec!["F13".into()]]);
        prefs
            .commands
            .insert("SaveAs".into(), vec![vec!["F14".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        let baseline = visible(&mut s);
        press(&mut s, KeyCode::F1);
        assert_eq!(visible(&mut s), baseline);
        press(&mut s, KeyCode::F13);
        assert_ne!(visible(&mut s), baseline);
        press(&mut s, KeyCode::Escape);
        press(&mut s, KeyCode::F14);
        let texts = visible(&mut s);
        let expected = i18n::tr("Edition", "SaveAsDestinationHelp").to_string();
        assert!(texts.iter().any(|s| s == &expected), "{texts:?}");
        // Modal entry quarantines the opener and never creates a second panel.
        event(&mut s, KeyCode::F14, true, true, Instant::now());
        assert_eq!(visible(&mut s), texts);
    }
}

#[test]
fn fourth_increment_navigation_and_zoom_remap_without_replaying_physical_arrows() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("MoveDown".into(), vec![vec!["F13".into()]]);
        prefs
            .commands
            .insert("IncreaseKeyboardZoom".into(), vec![vec!["F14".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        press(&mut s, KeyCode::ArrowDown);
        assert_eq!(s.workspace().row(), 0);
        press(&mut s, KeyCode::F13);
        let row = s.workspace().row();
        assert!(row > 0);
        ctrl(&mut s, KeyCode::ArrowUp, false);
        let time = Instant::now() + Duration::from_secs(1);
        s.update_with_time(time);
        assert!((s.spacing_zoom() - 1.0).abs() < 0.0001);
        event(&mut s, KeyCode::F14, true, false, time);
        event(&mut s, KeyCode::F14, false, false, time);
        s.update_with_time(time + Duration::from_millis(200));
        assert!(s.spacing_zoom() > 1.0);
        assert_eq!(s.workspace().row(), row);
        assert!(!s.workspace().file().session.is_dirty());
    }
}
#[test]
fn fourth_increment_existing_conversion_panel_is_bindable_but_unbound_by_default() {
    use deadlib_present::actors::Actor;
    fn texts(actor: &Actor, out: &mut Vec<String>) {
        match actor {
            Actor::Text { content, .. } => out.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => children.iter().for_each(|a| texts(a, out)),
            _ => {}
        }
    }
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("OpenNoteConversion".into(), vec![vec!["F15".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        let source = s.workspace().file().session.serialize().unwrap();
        press(&mut s, KeyCode::F15);
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let mut all = Vec::new();
        for actor in actors {
            texts(&actor, &mut all);
        }
        let expected = i18n::tr("Edition", "NoteConversionHelp").to_string();
        assert!(all.iter().any(|t| t == &expected), "{all:?}");
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        press(&mut s, KeyCode::Escape);
    }
}
#[test]
fn fourth_increment_ctrl_shift_w_closes_tab_without_opening_waveform_preferences() {
    use deadlib_present::actors::Actor;
    fn contains(actor: &Actor, text: &str) -> bool {
        match actor {
            Actor::Text { content, .. } => content.as_str() == text,
            Actor::Frame { children, .. } => children.iter().any(|a| contains(a, text)),
            _ => false,
        }
    }
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, _, _) = open(locale, Preferences::default());
        ctrl(&mut s, KeyCode::KeyW, true);
        assert!(!s.workspace().has_chart());
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let expected = i18n::tr("Edition", "WaveformSettingsHelp");
        assert!(!actors.iter().any(|a| contains(a, &expected)));
        assert!(!s.workspace().file().session.is_dirty());
    }
}

#[test]
fn semantic_file_new_is_a_guarded_untitled_song_not_a_new_chart() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("New".into(), vec![vec!["F19".into()]]);
        let (_d, mut s, _, _) = open(locale, prefs);
        press(&mut s, KeyCode::Digit1);
        assert!(s.workspace().file().has_unsaved_document());
        let ids = s.workspace().file().session.chart_ids().to_vec();
        press(&mut s, KeyCode::F19);
        assert!(s.workspace().has_chart());
        assert_eq!(s.workspace().file().session.chart_ids(), ids.as_slice());
        press(&mut s, KeyCode::Escape);
        assert!(s.workspace().file().has_unsaved_document());
        ctrl(&mut s, KeyCode::KeyZ, false);
        assert!(!s.workspace().file().has_unsaved_document());
        press(&mut s, KeyCode::F19);
        assert!(!s.workspace().has_chart());
        assert!(s.workspace().file().requires_save_as());
        assert!(s.workspace().file().existing_path().is_none());
        assert!(s.workspace().file().session.document().charts().is_empty());
    }
}
