use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn event(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    event(state, code, true);
    event(state, code, false);
}
fn focus(state: &mut State) {
    event(state, KeyCode::SuperLeft, true);
    press(state, KeyCode::KeyM);
    event(state, KeyCode::SuperLeft, false);
    assert!(state.minimap_focused());
}
fn setup(stop: bool) -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("music.wav"), b"command-only asset").unwrap();
    fs::write(directory.path().join("song.ssc"),b"#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#OFFSET:0;#SAMPLESTART:0;#SAMPLELENGTH:1;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let path = directory.path().join("preferences.json");
    fs::write(
        &path,
        format!("{{\"Version\":1,\"Scroll\":{{\"StopPlaybackWhenDraggingScrollBars\":{stop}}}}}"),
    )
    .unwrap();
    let mut state = State::open(&directory.path().join("song.ssc"), 0).unwrap();
    state.configure_audio_preferences(&path).unwrap();
    (directory, state)
}
#[test]
fn keyboard_capture_preserves_note_selection_and_resumes_exact_destination_on_release() {
    let (_dir, mut state) = setup(false);
    press(&mut state, KeyCode::Space);
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().selection(), Some((0, 48)));
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 626_000_000;
    state.update_audio(snapshot);
    assert_eq!(state.workspace().selection(), Some((0, 48)));
    focus(&mut state);
    event(&mut state, KeyCode::ArrowDown, true);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    let destination = state.chart_focus_time().unwrap();
    assert!(destination > 0.626);
    assert_eq!(state.workspace().selection(), Some((0, 48)));
    press(&mut state, KeyCode::Digit2);
    assert!(!state.workspace().file().session.is_dirty());
    event(&mut state, KeyCode::ArrowDown, false);
    assert!(state.is_playing());
    let commands = state.take_audio_commands();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(&commands[0],GameplayAudioCommand::PlayMusic {cut,..} if (cut.start_sec-destination).abs()<1e-10)
    );
    assert_eq!(state.workspace().selection(), Some((0, 48)));
}
#[test]
fn stop_preference_and_initially_stopped_transport_never_resume_on_release() {
    for playing in [false, true] {
        let (_dir, mut state) = setup(true);
        if playing {
            press(&mut state, KeyCode::Enter);
            state.take_audio_commands();
        }
        focus(&mut state);
        event(&mut state, KeyCode::ArrowDown, true);
        state.take_audio_commands();
        event(&mut state, KeyCode::ArrowDown, false);
        assert!(!state.is_playing());
        assert!(state.take_audio_commands().is_empty());
        assert!(!state.workspace().file().session.is_dirty());
    }
}
#[test]
fn focus_loss_does_not_resume_and_minimap_presentation_is_bounded() {
    let (_dir, mut state) = setup(false);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    focus(&mut state);
    event(&mut state, KeyCode::ArrowDown, true);
    state.take_audio_commands();
    state.release_input();
    event(&mut state, KeyCode::ArrowDown, false);
    assert!(!state.is_playing());
    assert!(!state.minimap_focused());
    assert!(state.take_audio_commands().is_empty());
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let bars = actors
        .iter()
        .filter(|actor| {
            matches!(
                actor,
                deadlib_present::actors::Actor::Sprite { z: 8011, .. }
            )
        })
        .count();
    assert!(bars > 0 && bars <= 64);
    assert!(actors.iter().any(|actor| matches!(
        actor,
        deadlib_present::actors::Actor::Sprite { z: 8012, .. }
    )));
}

#[test]
fn distinct_drag_preference_is_reviewable_saved_and_reloaded_without_song_changes() {
    let (directory, mut state) = setup(false);
    event(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::KeyS);
    event(&mut state, KeyCode::ShiftLeft, false);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    assert!(!state.scroll_preferences().stop_playback_when_dragging());
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert!(state.scroll_preferences().stop_playback_when_dragging());
    assert!(!state.scroll_preferences().stop_playback());
    let preferences: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.path().join("preferences.json")).unwrap())
            .unwrap();
    assert_eq!(
        preferences["Scroll"]["StopPlaybackWhenDraggingScrollBars"],
        true
    );
    let mut reopened = State::open(&directory.path().join("song.ssc"), 0).unwrap();
    reopened
        .configure_audio_preferences(&directory.path().join("preferences.json"))
        .unwrap();
    assert!(reopened.scroll_preferences().stop_playback_when_dragging());
    assert!(!state.workspace().file().session.is_dirty());
}

#[test]
fn overview_batches_are_bounded_complete_and_invalidate_after_note_edits() {
    let (directory, _state) = setup(false);
    let measures = std::iter::repeat_n("1000\n".repeat(48), 125)
        .collect::<Vec<_>>()
        .join(",\n");
    fs::write(
        directory.path().join("large.ssc"),
        format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{measures};"),
    )
    .unwrap();
    let mut state = State::open(&directory.path().join("large.ssc"), 0).unwrap();
    let render = |state: &State| {
        let mut actors = Vec::new();
        state.actors(&mut actors);
        actors
    };
    let actors = render(&state);
    assert_eq!(state.minimap_density_progress(), Some((4096, false)));
    let loading = i18n::tr("Edition", "MinimapLoading");
    assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains(loading.as_ref()))));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((6000, true)));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((6000, true)));
    press(&mut state, KeyCode::Digit1);
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((4096, false)));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((5999, true)));
}

#[test]
fn leaving_preview_audition_does_not_arm_chart_resume_for_minimap_capture() {
    let (_directory, mut state) = setup(false);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::F4);
    assert!(matches!(
        state.take_audio_commands().as_slice(),
        [GameplayAudioCommand::PlayMusic { looping: true, .. }]
    ));
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    assert!(!state.is_playing());
    state.take_audio_commands();
    focus(&mut state);
    event(&mut state, KeyCode::ArrowDown, true);
    event(&mut state, KeyCode::ArrowDown, false);
    assert!(!state.is_playing());
    assert!(state.take_audio_commands().is_empty());
}
