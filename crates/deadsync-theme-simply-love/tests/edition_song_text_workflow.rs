#[path = "support/transport_host.rs"]
mod transport_host;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    clipboard::{Error as ClipboardError, TextClipboard},
    music_file::{Browse, Picker},
};
use deadsync_gameplay::GameplayAudioCommand;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, path::PathBuf, time::Instant};
#[derive(Default)]
struct Board {
    value: String,
    writes: usize,
}
impl TextClipboard for Board {
    fn read_text(&mut self) -> Result<String, ClipboardError> {
        Ok(self.value.clone())
    }
    fn write_text(&mut self, value: &str) -> Result<(), ClipboardError> {
        self.writes += 1;
        self.value = value.into();
        Ok(())
    }
}
struct Choose {
    path: PathBuf,
    calls: usize,
    current: String,
}
impl Picker for Choose {
    fn choose(&mut self, r: &Browse) -> Result<Option<PathBuf>, Error> {
        self.calls += 1;
        self.current = r.current.clone();
        Ok(Some(self.path.clone()))
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
fn labels(s: &State) -> Vec<String> {
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
    labels(s).iter().any(|v| v.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn setup(sm: bool, override_audio: bool) -> (tempfile::TempDir, State, String) {
    init();
    let dir = tempfile::tempdir().unwrap();
    let source = format!(
        "#VERSION:0.83;#TITLE:Old;#ARTIST:A;#CREDIT:File author;#MUSIC:song.ogg;#BPMS:0=120;{}",
        if sm {
            "#NOTES:dance-single::Hard:9::1000;"
        } else if override_audio {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.ogg;#NOTES:1000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
        }
    );
    let path = dir.path().join(if sm { "song.sm" } else { "song.ssc" });
    fs::write(&path, &source).unwrap();
    for name in ["song.ogg", "new.wav", "chart.ogg"] {
        fs::write(dir.path().join(name), b"fixture: transport command only").unwrap();
    }
    let mut state = State::open(&path, 0).unwrap();
    transport_host::publish(&mut state);
    (dir, state, source)
}
fn open(s: &mut State) {
    press(s, KeyCode::F11);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
fn music(s: &mut State) {
    if s.is_playing() {
        press(s, KeyCode::Enter);
        s.take_audio_commands();
    }
    open(s);
    press(s, KeyCode::PageDown);
    press(s, KeyCode::ArrowDown);
}
fn apply(s: &mut State, sync: bool) {
    press(s, KeyCode::Escape);
    // Song text returns to action3; Apply is action1, SyncOffset is0.
    for _ in 0..if sync { 3 } else { 2 } {
        press(s, KeyCode::ArrowUp);
    }
    if sync {
        press(s, KeyCode::ArrowRight);
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn play(s: &mut State) -> PathBuf {
    if s.is_playing() {
        press(s, KeyCode::Enter);
    }
    assert!(!s.is_playing());
    transport_host::publish(s);
    press(s, KeyCode::Enter);
    let commands = s.take_audio_commands();
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        GameplayAudioCommand::EditorStartMusic {
            path: Some(path), ..
        }
        | GameplayAudioCommand::EditorResumeMusic { path, .. } => path.clone(),
        other => panic!("{other:?}"),
    }
}
fn authoring_tags(
    bytes: Vec<u8>,
    format: deadsync_editor::document::Format,
) -> Vec<(Option<usize>, String, Vec<u8>)> {
    let document = deadsync_editor::document::Document::parse(bytes, format).unwrap();
    document
        .tags()
        .iter()
        .filter(|tag| {
            !matches!(
                tag.name.as_str(),
                "GASONGDATA" | "GASONGDATAVERSION" | "GASONGDATASONG" | "GASONGDATAVERSIONSONG"
            )
        })
        .map(|tag| {
            (
                tag.chart,
                tag.name.clone(),
                document.source()[tag.value.clone()].to_vec(),
            )
        })
        .collect()
}
fn assert_saved_authoring(state: &State, path: &std::path::Path) {
    let format = state.workspace().file().session.document().format;
    assert_eq!(
        authoring_tags(fs::read(path).unwrap(), format),
        authoring_tags(
            state.workspace().file().session.serialize().unwrap(),
            format
        )
    );
    let reopened = State::open(path, 0).unwrap();
    assert_eq!(
        deadsync_editor::song_properties::sync_offset(
            reopened.workspace().file().session.document()
        )
        .unwrap(),
        deadsync_editor::song_properties::sync_offset(state.workspace().file().session.document())
            .unwrap()
    );
}
fn audio_field(state: &mut State, key: &str) {
    let name = i18n::tr("Edition", key);
    for _ in 0..16 {
        if labels(state)
            .iter()
            .any(|label| label.contains(name.as_ref()))
        {
            press(state, KeyCode::Tab); // Field -> Value
            return;
        }
        press(state, KeyCode::ArrowRight);
    }
    panic!("audio field {key} unavailable: {:?}", labels(state));
}
fn audio_apply(state: &mut State) {
    press(state, KeyCode::ArrowDown); // Value -> Restore defaults
    press(state, KeyCode::ArrowDown); // -> Apply
    press(state, KeyCode::Enter);
}
#[test]
fn shared_song_editor_stages_unicode_clipboard_and_sync_in_one_saveable_undo_gesture_for_sm_and_ssc()
 {
    for sm in [false, true] {
        let (_dir, mut s, source) = setup(sm, false);
        let path = s
            .workspace()
            .file()
            .require_existing_path()
            .unwrap()
            .to_path_buf();
        open(&mut s);
        assert!(has(&s, "Title: [Old]|"));
        let mut board = Board::default();
        ctrl(&mut s, KeyCode::KeyC);
        assert!(s.process_text_clipboard(&mut board));
        assert_eq!(board.value, "Old");
        assert!(s.text_input("Été : ; \\ 日本語"));
        press(&mut s, KeyCode::Tab);
        press(&mut s, KeyCode::Tab);
        board.value = "Composer 日本語".into();
        ctrl(&mut s, KeyCode::KeyV);
        assert!(s.process_text_clipboard(&mut board));
        press(&mut s, KeyCode::Tab);
        assert!(s.text_input("New author"));
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            source.as_bytes()
        );
        apply(&mut s, true);
        assert_eq!(
            s.workspace().file().session.undo_label(),
            Some("Change song properties")
        );
        assert_eq!(
            s.workspace()
                .file()
                .session
                .document()
                .text(None, "TITLE")
                .as_deref(),
            Some("Été : ; \\ 日本語")
        );
        assert_eq!(
            s.workspace()
                .file()
                .session
                .document()
                .text(None, "ARTIST")
                .as_deref(),
            Some("Composer 日本語")
        );
        assert!(
            s.workspace()
                .file()
                .session
                .document()
                .text(None, if sm { "GASongDataSONG" } else { "GASongData" })
                .is_some()
        );
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            source.as_bytes()
        );
        ctrl(&mut s, KeyCode::KeyY);
        ctrl(&mut s, KeyCode::KeyS);
        let reopen = State::open(&path, 0).unwrap();
        assert_eq!(
            reopen
                .workspace()
                .file()
                .session
                .document()
                .text(None, "TITLE")
                .as_deref(),
            Some("Été : ; \\ 日本語")
        );
        assert_eq!(reopen.workspace().grid().iter().count(), 1);
    }
}
#[test]
fn global_music_picker_clear_and_undo_drive_inherited_audio_in_sm_ssc_and_preserve_chart_overrides()
{
    for (sm, override_audio) in [(true, false), (false, false), (false, true)] {
        let (_dir, mut s, _) = setup(sm, override_audio);
        let base = s
            .workspace()
            .file()
            .require_existing_path()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let original = base.join(if override_audio {
            "chart.ogg"
        } else {
            "song.ogg"
        });
        assert_eq!(play(&mut s), original);
        music(&mut s);
        assert!(has(&s, "Song music: [song.ogg]|"));
        assert!(has(&s, "Fields 2/3"));
        assert!(!s.is_playing());
        let mut picker = Choose {
            path: base.join("new.wav"),
            calls: 0,
            current: String::new(),
        };
        press(&mut s, KeyCode::F2);
        assert!(s.process_music_picker(&mut picker));
        assert_eq!(picker.current, "song.ogg");
        assert!(has(&s, "Song music chosen"));
        apply(&mut s, false);
        let expected = base.join(if override_audio {
            "chart.ogg"
        } else {
            "new.wav"
        });
        assert_eq!(play(&mut s), expected);
        music(&mut s);
        press(&mut s, KeyCode::F3);
        assert!(has(&s, "Song music: | (no song music)"));
        apply(&mut s, false);
        if override_audio {
            assert_eq!(play(&mut s), base.join("chart.ogg"));
        } else {
            s.take_audio_commands();
            press(&mut s, KeyCode::Enter);
            assert!(s.is_playing());
            assert!(matches!(
                s.take_audio_commands().as_slice(),
                [GameplayAudioCommand::EditorStartMusic { path: None, .. }]
            ));
        }
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(play(&mut s), expected);
    }
}
#[test]
fn song_chart_scope_switches_drop_pending_clipboard_and_picker_requests_without_io() {
    let (_dir, mut s, source) = setup(false, false);
    music(&mut s);
    let mut board = Board::default();
    ctrl(&mut s, KeyCode::KeyC);
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut s, KeyCode::Tab);
    }
    let mut picker = Choose {
        path: s
            .workspace()
            .file()
            .require_existing_path()
            .unwrap()
            .parent()
            .unwrap()
            .join("new.wav"),
        calls: 0,
        current: String::new(),
    };
    assert!(!s.process_music_picker(&mut picker));
    assert!(!s.process_text_clipboard(&mut board));
    assert_eq!((picker.calls, board.writes), (0, 0));
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    music(&mut s);
    assert!(!s.process_music_picker(&mut picker));
    assert_eq!(picker.calls, 0);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
}
#[test]
fn text_cancel_and_chartless_song_properties_remain_keyboard_accessible_and_persistent() {
    let (_dir, mut s, source) = setup(false, false);
    open(&mut s);
    assert!(s.text_input("Discard"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.ssc");
    fs::write(&path, "#VERSION:0.83;#TITLE:Old;").unwrap();
    let mut empty = State::open(&path, 0).unwrap();
    assert!(!empty.workspace().has_chart());
    open(&mut empty);
    assert!(has(&empty, "Title: [Old]|"));
    assert!(empty.text_input("Empty song"));
    apply(&mut empty, false);
    ctrl(&mut empty, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert!(!reopened.workspace().has_chart());
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(None, "TITLE")
            .as_deref(),
        Some("Empty song")
    );
}

#[test]
fn song_text_help_has_a_bounded_multiline_footer() {
    let (_dir, mut state, _) = setup(false, false);
    open(&mut state);
    let help = i18n::tr("Edition", "SongTextHelp");
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let footer = actors.iter().find(|actor| {
        matches!(actor,
        Actor::Text { content, .. } if content.to_string() == help.as_ref())
    });
    match footer.expect("song text help is rendered") {
        Actor::Text {
            offset,
            wrap_width_pixels,
            max_width,
            max_height,
            ..
        } => {
            assert_eq!(*offset, [18.0, 430.0]);
            assert_eq!(*wrap_width_pixels, Some(1636));
            assert_eq!(*max_width, Some(818.0));
            assert_eq!(*max_height, Some(42.0));
        }
        _ => unreachable!(),
    }
}

#[test]
fn song_music_offset_and_itg_action_apply_once_undo_and_save_in_both_formats() {
    for sm in [false, true] {
        let (dir, mut state, original) = setup(sm, false);
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::Digit0); // source ITG migration is available only at zero sync
        for _ in 0..5 {
            press(&mut state, KeyCode::ArrowDown);
        } // explicit ITG action
        press(&mut state, KeyCode::Enter);
        assert!(has(&state, "0.009000"));
        press(&mut state, KeyCode::ArrowUp); // global music offset
        press(&mut state, KeyCode::ArrowRight);
        assert!(has(&state, "0.010000"));
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        for _ in 0..5 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter); // Apply
        let offset: f64 = state
            .workspace()
            .file()
            .session
            .document()
            .text(None, "OFFSET")
            .unwrap()
            .parse()
            .unwrap();
        assert!((offset - 0.010).abs() < 1e-12);
        let applied = state.workspace().file().session.serialize().unwrap();
        ctrl(&mut state, KeyCode::KeyZ);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        ctrl(&mut state, KeyCode::KeyY);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            applied
        );
        ctrl(&mut state, KeyCode::KeyS);
        assert_saved_authoring(
            &state,
            &dir.path().join(if sm { "song.sm" } else { "song.ssc" }),
        );
    }
}

#[test]
fn preview_sample_panel_stages_clamps_applies_undoes_and_saves_without_changing_offsets() {
    for sm in [false, true] {
        let (dir, mut state, original) = setup(sm, false);
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::F6);
        assert!(has(&state, "Preview start: 0.000000 s"));
        press(&mut state, KeyCode::ArrowRight);
        press(&mut state, KeyCode::Tab);
        press(&mut state, KeyCode::ArrowLeft);
        assert!(has(&state, "Preview length: 0.000000 s"));
        key(&mut state, KeyCode::ShiftLeft, true);
        press(&mut state, KeyCode::ArrowRight);
        key(&mut state, KeyCode::ShiftLeft, false);
        assert!(has(&state, "Preview length: 0.000100 s"));
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        press(&mut state, KeyCode::Escape);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        let document = state.workspace().file().session.document();
        assert_eq!(document.text(None, "SAMPLESTART").as_deref(), Some("0.001"));
        assert_eq!(
            document.text(None, "SAMPLELENGTH").as_deref(),
            Some("0.0001")
        );
        assert!(document.text(None, "OFFSET").is_none());
        let _changed = state.workspace().file().session.serialize().unwrap();
        ctrl(&mut state, KeyCode::KeyZ);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        ctrl(&mut state, KeyCode::KeyY);
        ctrl(&mut state, KeyCode::KeyS);
        assert_saved_authoring(
            &state,
            &dir.path().join(if sm { "song.sm" } else { "song.ssc" }),
        );
    }
}

#[test]
fn sample_boundary_actions_work_without_audio_and_preserve_pending_document_until_apply() {
    let (dir, mut state, original) = setup(false, false);
    fs::remove_file(dir.path().join("song.ogg")).unwrap();
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    let end_row = state.workspace().row();
    assert!(end_row > 0);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::F2); // end from current song time
    assert!(has(&state, "Preview region staged"));
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::F3); // start is zero; negative row clamps to zero
    assert_eq!(state.workspace().row(), 0);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::F3);
    assert!(state.workspace().row().abs_diff(end_row) <= 1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

#[test]
fn sample_region_actor_tracks_staged_length_and_disappears_after_discard() {
    let (_dir, mut state, original) = setup(false, false);
    let fill = |state: &State| {
        let mut actors = Vec::new();
        state.actors(&mut actors);
        actors.iter().any(
            |actor| matches!(actor, Actor::Sprite { tint, .. } if *tint == [0.2,0.85,0.65,0.14]),
        )
    };
    assert!(!fill(&state));
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::ArrowRight);
    assert!(fill(&state));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    assert!(!fill(&state));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

#[test]
fn sample_audition_sends_looped_cut_updates_region_live_and_stops_on_toggle() {
    let (dir, mut state, original) = setup(false, false);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::ArrowRight);
    let row = state.workspace().row();
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    assert!(state.is_playing());
    match state.take_audio_commands().as_slice() {
        [
            GameplayAudioCommand::EditorPlayPreview {
                path,
                cut,
                looping,
                rate,
                ..
            },
        ] => {
            assert_eq!(*path, dir.path().join("song.ogg").canonicalize().unwrap());
            assert_eq!(cut.start_sec, 0.0);
            assert_eq!(cut.length_sec, 0.001);
            assert_eq!(cut.fade_out_sec, 1.5);
            assert!(*looping);
            assert_eq!(*rate, 1.0);
        }
        other => panic!("unexpected commands: {other:?}"),
    }
    assert_eq!(state.workspace().row(), row);
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        [GameplayAudioCommand::EditorStopMusic {
            owner: state.editor_asset_owner()
        }]
    );
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    state.take_audio_commands();
    press(&mut state, KeyCode::ArrowLeft);
    assert!(state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        [GameplayAudioCommand::EditorSetPreviewRegion {
            owner: state.editor_asset_owner(),
            start: 0.0,
            length: 0.0
        }]
    );
    press(&mut state, KeyCode::F4); // Explicit toggle ends the retained silent loop.
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        [GameplayAudioCommand::EditorStopMusic {
            owner: state.editor_asset_owner()
        }]
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

#[test]
fn sample_region_geometry_is_clipped_and_disappears_when_scrolled_out_of_view() {
    use deadlib_present::actors::SizeSpec;
    let (dir, _state, original) = setup(false, false);
    let path = dir.path().join("song.ssc");
    let source = original.replace(
        "#TITLE:Old;",
        "#TITLE:Old;#SAMPLESTART:0;#SAMPLELENGTH:100;",
    );
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let region = |state: &State| {
        let mut actors = Vec::new();
        state.actors(&mut actors);
        actors.into_iter().find_map(|actor| match actor {
            Actor::Sprite {
                offset,
                size: [SizeSpec::Px(width), SizeSpec::Px(height)],
                tint,
                scale,
                ..
            } if tint == [0.2, 0.85, 0.65, 0.14] => Some((
                offset,
                if width == 0.0 {
                    scale[0]
                } else {
                    width * scale[0]
                },
                if height == 0.0 {
                    scale[1]
                } else {
                    height * scale[1]
                },
            )),
            _ => None,
        })
    };
    let (offset, width, height) = region(&state).expect("visible preview fill");
    assert!(width > 0.0 && height > 0.0);
    assert!(offset[1] - height * 0.5 >= 105.0 - 0.001);
    assert!(offset[1] + height * 0.5 <= 395.0 + 0.001);
    for _ in 0..250 {
        press(&mut state, KeyCode::ArrowDown);
    }
    assert!(region(&state).is_none());
    assert!(!has(&state, "Music preview"));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
}

#[test]
fn dedicated_preview_picker_stages_audio_saves_and_sm_navigation_supports_preview_field() {
    let (dir, mut state, _) = setup(false, false);
    music(&mut state);
    press(&mut state, KeyCode::Tab);
    let mut picker = Choose {
        path: dir.path().join("new.wav").canonicalize().unwrap(),
        calls: 0,
        current: String::new(),
    };
    press(&mut state, KeyCode::F2);
    assert!(state.process_music_picker(&mut picker));
    assert_eq!(picker.calls, 1);
    assert!(has(&state, "Dedicated preview selected"));
    apply(&mut state, false); // Registered assets follow committed song metadata.
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    match state.take_audio_commands().as_slice() {
        [
            GameplayAudioCommand::EditorPlayPreview {
                path, cut, looping, ..
            },
        ] => {
            assert_eq!(*path, dir.path().join("new.wav").canonicalize().unwrap());
            assert_eq!(cut.start_sec, 0.0);
            assert!(cut.length_sec.is_infinite());
            assert!(*looping);
        }
        other => panic!("unexpected audio: {other:?}"),
    }
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    state.take_audio_commands();
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    ctrl(&mut state, KeyCode::KeyS);
    assert!(
        fs::read_to_string(dir.path().join("song.ssc"))
            .unwrap()
            .contains("#PREVIEW:new.wav;")
    );
    let (_dir, mut state, original) = setup(true, false);
    music(&mut state);
    press(&mut state, KeyCode::Tab);
    assert!(state.wants_text_input());
    assert!(has(&state, "Dedicated preview file: |"));
    key(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::ShiftLeft, false);
    assert!(has(&state, "Song music: [song.ogg]|"));
    assert_eq!(picker.calls, 1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

#[test]
fn last_second_hint_keyboard_field_is_staged_saved_and_undoable_in_both_formats() {
    for sm in [false, true] {
        let (dir, mut state, original) = setup(sm, false);
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::F6);
        press(&mut state, KeyCode::Tab);
        press(&mut state, KeyCode::Tab);
        press(&mut state, KeyCode::ArrowRight);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        press(&mut state, KeyCode::Escape);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state
                .workspace()
                .file()
                .session
                .document()
                .text(None, "LASTSECONDHINT")
                .as_deref(),
            Some("0.001")
        );
        ctrl(&mut state, KeyCode::KeyS);
        assert!(
            fs::read_to_string(dir.path().join(if sm { "song.sm" } else { "song.ssc" }))
                .unwrap()
                .contains("#LASTSECONDHINT:0.001;")
        );
        ctrl(&mut state, KeyCode::KeyZ);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
    }
}

#[test]
fn application_audio_preferences_reload_and_drive_audition_without_touching_song() {
    use deadsync_editor::audio_preferences::File;
    let (dir, mut state, original) = setup(false, false);
    let prefs_path = dir.path().join("editor-preferences.json");
    let mut file = File::open(&prefs_path).unwrap();
    let mut audio = file.audio();
    audio.set_fade_in(0.25).unwrap();
    audio.set_fade_out(2.0).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    state.configure_audio_preferences(&prefs_path).unwrap();
    assert_eq!(state.audio_preferences(), audio);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::ArrowRight);
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    match state.take_audio_commands().as_slice() {
        [GameplayAudioCommand::EditorPlayPreview { cut, .. }] => {
            assert_eq!(cut.fade_in_sec, 0.25);
            assert_eq!(cut.fade_out_sec, 2.0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&prefs_path).unwrap();
    assert_eq!(reopened.audio_preferences(), audio);
    fs::write(&prefs_path, b"invalid preferences").unwrap();
    assert!(reopened.configure_audio_preferences(&prefs_path).is_err());
    assert_eq!(reopened.audio_preferences(), audio);
    assert_eq!(fs::read(&prefs_path).unwrap(), b"invalid preferences");
}

#[test]
fn keyboard_audio_settings_save_reload_cancel_and_drive_audition_without_song_changes() {
    let (dir, mut state, original) = setup(false, false);
    let path = dir.path().join("editor-preferences.json");
    state.configure_audio_preferences(&path).unwrap();
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::F5);
    audio_field(&mut state, "AudioNameFadeIn");
    press(&mut state, KeyCode::ArrowRight);
    assert!(!path.exists());
    assert_eq!(state.audio_preferences().fade_in(), 0.0);
    audio_apply(&mut state);
    assert!(has(&state, "Audio preferences saved"));
    assert_eq!(state.audio_preferences().fade_in(), 0.001);
    assert_eq!(state.audio_preferences().fade_out(), 1.5);
    let saved = fs::read(&path).unwrap();
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    assert!(!state.workspace().file().session.is_dirty());
    press(&mut state, KeyCode::F5);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::Escape);
    assert_eq!(fs::read(&path).unwrap(), saved);
    assert_eq!(state.audio_preferences().fade_in(), 0.001);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::ArrowRight);
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::F4);
    match state.take_audio_commands().as_slice() {
        [GameplayAudioCommand::EditorPlayPreview { cut, .. }] => {
            assert_eq!(cut.fade_in_sec, 0.001);
            assert_eq!(cut.fade_out_sec, 1.5);
        }
        other => panic!("unexpected commands {other:?}"),
    }
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    assert_eq!(reopened.audio_preferences(), state.audio_preferences());
}

#[test]
fn keyboard_preference_save_refuses_external_or_future_files_and_keeps_active_settings() {
    for future in [false, true] {
        let (dir, mut state, original) = setup(false, false);
        let path = dir.path().join("editor-preferences.json");
        let external = if future {
            r#"{"Version":2}"#
        } else {
            r#"{"Version":1,"Audio":{"PreviewFadeInTime":0.5}}"#
        };
        if future {
            fs::write(&path, external).unwrap();
            assert!(state.configure_audio_preferences(&path).is_err());
        } else {
            state.configure_audio_preferences(&path).unwrap();
        }
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::F6);
        press(&mut state, KeyCode::F5);
        audio_field(&mut state, "AudioNameFadeIn");
        press(&mut state, KeyCode::ArrowRight);
        if !future {
            fs::write(&path, external).unwrap();
        }
        audio_apply(&mut state);
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
        if future {
            // A rejected source file uses memory-only ownership; safe live
            // settings remain editable while the future file stays untouched.
            assert_eq!(state.audio_preferences().fade_in(), 0.001);
            assert!(has(&state, "Audio preferences saved"));
        } else {
            assert_eq!(state.audio_preferences().fade_in(), 0.0);
            assert!(has(&state, "Preview fade in (seconds)"));
        }
        press(&mut state, KeyCode::Escape);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
    }
}

#[test]
fn keyboard_audio_settings_show_fine_steps_restore_defaults_and_cancel_without_creating_file() {
    let (dir, mut state, _) = setup(false, false);
    let path = dir.path().join("editor-preferences.json");
    state.configure_audio_preferences(&path).unwrap();
    for code in [KeyCode::F11, KeyCode::F6, KeyCode::F5] {
        press(&mut state, code);
    }
    audio_field(&mut state, "AudioNameOffset");
    key(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftLeft, false);
    assert!(has(&state, "0.000100"));
    press(&mut state, KeyCode::ArrowDown); // Restore defaults
    press(&mut state, KeyCode::Enter);
    assert!(has(&state, "0.000000"));
    press(&mut state, KeyCode::Escape); // discard without publication
    assert!(!path.exists());
    assert_eq!(state.audio_preferences(), Default::default());
    assert!(!state.workspace().file().session.is_dirty());
}
