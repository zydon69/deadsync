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
fn setup(stop: bool) -> (tempfile::TempDir, State,u64) {
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
        format!("{{\"Version\":1,\"SnapIndex\":1,\"Scroll\":{{\"StopPlaybackWhenDraggingScrollBars\":{stop}}}}}"),
    )
    .unwrap();
    let mut state = State::open(&directory.path().join("song.ssc"), 0).unwrap();
    state.configure_audio_preferences(&path).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
    let owner=publish_audio_fixture(&mut state);
    (directory, state,owner)
}
#[test]
fn keyboard_capture_extends_active_source_range_and_resumes_exact_destination_on_release() {
    let (_dir, mut state,owner)=setup(false);
    press(&mut state, KeyCode::Space);
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(),
        48,
        "explicit quarter-note fixture snap"
    );
    assert_eq!(
        state.workspace().selection(), Some((0, state.workspace().row())));
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 626_000_000;
    state.update_audio(snapshot);
    assert_eq!(state.workspace().selection(), Some((0, state.workspace().row())));
    focus(&mut state);
    event(&mut state, KeyCode::ArrowDown, true);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::EditorPauseMusic {owner,
            seconds: 0.626
        }]
    );
    let destination = state.chart_focus_time().unwrap();
    assert!(destination > 0.626);
    assert_eq!(state.workspace().selection(), Some((0, state.workspace().row())));
    press(&mut state, KeyCode::Digit2);
    assert!(!state.workspace().file().session.is_dirty());
    event(&mut state, KeyCode::ArrowDown, false);
    assert!(state.is_playing());
    let commands = state.take_audio_commands();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(&commands[0],GameplayAudioCommand::EditorResumeMusic {owner:actual,seconds,..} if *actual==owner && (*seconds-destination).abs()<1e-10)
    );
    assert_eq!(state.workspace().selection(), Some((0, state.workspace().row())));
}
#[test]
fn stop_preference_and_initially_stopped_transport_never_resume_on_release() {
    for playing in [false, true] {
        let (_dir, mut state, _owner)=setup(true);
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
    let (_dir, mut state, _owner)=setup(false);
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
    let meshes: Vec<_> = actors
        .iter()
        .filter_map(|actor| match actor {
            deadlib_present::actors::Actor::Mesh {
                z: 8011, vertices, ..
            } => Some(vertices),
            _ => None,
        })
        .collect();
    assert_eq!(
        meshes.len(),
        1,
        "one retained minimap mesh replaces density quads"
    );
    assert!(!meshes[0].is_empty());
    assert!(
        meshes[0].len() <= 16384 * 6,
        "explicit rectangle/vertex budget"
    );
    let (count, notes, markers, mesh) = state.minimap_content_progress().unwrap();
    assert_eq!(count, 4);
    assert!(
        notes && markers && mesh,
        "bounded frame is complete for the fixture"
    );
    let geometry = state.minimap_viewport_geometry().unwrap();
    assert!(geometry.projection.editor_end > geometry.projection.editor_start);
}

#[test]
fn distinct_drag_preference_is_reviewable_saved_and_reloaded_without_song_changes() {
    let (directory, mut state, _owner)=setup(false);
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
fn exact_overview_is_complete_and_invalidates_after_note_edits() {
    let (directory, _state, _owner)=setup(false);
    let measures = std::iter::repeat_n("1000\n".repeat(48), 125)
        .collect::<Vec<_>>()
        .join(",\n");
    fs::write(
        directory.path().join("large.ssc"),
        format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{measures};"),
    )
    .unwrap();
    let mut state = State::open(&directory.path().join("large.ssc"), 0).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
    let render = |state: &State| {
        let mut actors = Vec::new();
        state.actors(&mut actors);
        actors
    };
    let actors = render(&state);
    assert_eq!(state.minimap_density_progress(), Some((6000, true)));
    let partial = i18n::tr("Edition", "MinimapPartial");
    assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains(partial.as_ref()))));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((6000, true)));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((6000, true)));
    press(&mut state, KeyCode::Digit1);
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((5999, true)));
    render(&state);
    assert_eq!(state.minimap_density_progress(), Some((5999, true)));
}

#[test]
fn minimap_capture_preserves_preview_without_chart_pause_or_resume() {
    let (_directory, mut state,owner)=setup(false);
    // Preview commands need an acknowledged output lease as well as prepared
    // MAIN data; silent chart navigation intentionally does not require one.
    let mut output = GameplayAudioSnapshot::default();
    output.editor.owner = owner;
    output.editor.output.available = true;
    output.editor.output.leased = true;
    output.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner,
        registration: state.editor_asset_registration(),
        main: true,
        dedicated_preview: false,
    };
    state.update_audio(output);
    state.take_audio_commands();
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::F4);
    let preview_commands = state.take_audio_commands();
    assert!(matches!(
            preview_commands.as_slice(),
        [GameplayAudioCommand::EditorPlayPreview {owner:actual, looping: true, .. }] if *actual==owner
    ),
        "{preview_commands:?}"
    );
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    assert!(state.is_playing());
    // Read-only panel closure intentionally preserves sample audition. The
    // minimap must not treat that independent bus as chart transport.
    assert!(state.take_audio_commands().is_empty());
    focus(&mut state);
    event(&mut state, KeyCode::ArrowDown, true);
    event(&mut state, KeyCode::ArrowDown, false);
    assert!(state.is_playing());
    assert!(state.take_audio_commands().is_empty());
}
fn chord(state: &mut State, code: KeyCode, shift: bool) {
    event(state, KeyCode::SuperLeft, true);
    if shift {
        event(state, KeyCode::ShiftLeft, true);
    }
    press(state, code);
    if shift {
        event(state, KeyCode::ShiftLeft, false);
    }
    event(state, KeyCode::SuperLeft, false);
}
#[test]
fn native_viewport_uses_render_bounds_focal_reverse_and_time_row_axes() {
    use deadsync_editor::minimap_viewport::Axis;
    let (_dir, mut state, _owner)=setup(false);
    for reversed in [false, true] {
        if reversed {
            event(&mut state, KeyCode::ShiftLeft, true);
            press(&mut state, KeyCode::KeyV);
            event(&mut state, KeyCode::ShiftLeft, false);
        }
        for axis in [Axis::Time, Axis::Row] {
            let mode = match axis {
                Axis::Time => deadsync_editor::zoom::SpacingMode::ConstantTime,
                Axis::Row => deadsync_editor::zoom::SpacingMode::ConstantRow,
            };
            if state.spacing_mode() != mode {
                chord(&mut state, KeyCode::KeyT, false);
            }
            assert_eq!(state.spacing_mode(), mode);
            let geometry = state.minimap_viewport_geometry().unwrap();
            let p = &geometry.projection;
            assert_eq!(geometry.axis, axis);
            assert_eq!(p.reverse, reversed);
            // Actual render viewport is [105,395], cursor 160 or340. In both
            // directions the earlier distance is55, not one quarter screen.
            assert!((p.grabbed_percentage - 55.0 / 290.0).abs() < 1e-12);
            assert!((geometry.focused_coordinate()).abs() < 1e-12);
            // The configured range is preserved; a short chart is fitted
            // into the overview with a narrower effective range.
            let configured = if axis == Axis::Time { 240.0 } else { 24576.0 };
            assert_eq!(state.minimap_preferences().visible_range(axis), configured);
            assert!(p.mini_range > 0.0 && p.mini_range < configured);
        }
    }
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn minimap_settings_cancel_save_reopen_and_variable_axis_are_independent() {
    use deadsync_editor::minimap_viewport::Axis;
    let (dir, mut state, _owner)=setup(false);
    let original = state.minimap_preferences();
    chord(&mut state, KeyCode::KeyM, true);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::Escape);
    assert_eq!(state.minimap_preferences(), original);
    chord(&mut state, KeyCode::KeyM, true);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowLeft);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowLeft);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    let prefs = state.minimap_preferences();
    assert_eq!(prefs.variable_axis(), Axis::Row);
    assert_eq!(prefs.visible_range(Axis::Time), 230.0);
    assert_eq!(prefs.visible_range(Axis::Row), 24384.0);
    assert!(prefs.quantize());
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened
        .configure_audio_preferences(&dir.path().join("preferences.json"))
        .unwrap();
    assert_eq!(reopened.minimap_preferences(), prefs);
    chord(&mut reopened, KeyCode::KeyT, true);
    assert_eq!(
        reopened.minimap_viewport_geometry().unwrap().axis,
        Axis::Row
    );
    assert!(!reopened.workspace().file().session.is_dirty());
}
#[test]
fn row_drag_preserves_fractional_warp_destination_and_offsets_in_both_directions() {
    use deadsync_editor::{minimap_viewport::Axis, playback::Clock};
    for reverse in [false, true] {
        let (dir, _, _owner)=setup(false);
        let song = dir.path().join("song.ssc");
        fs::write(&song,b"#VERSION:0.83;#MUSIC:music.wav;#OFFSET:0.25;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0.1};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#STOPS:1=1;#DELAYS:2=0.25;#WARPS:3=1;#NOTES:1000\n0100\n0010\n0001\n,\n0000\n0000\n0000\n1000;").unwrap();
        let mut state = State::open(&song, 0).unwrap();
        let path = dir.path().join("preferences.json");
        let mut file = deadsync_editor::audio_preferences::File::open(&path).unwrap();
        let mut prefs = file.minimap();
        prefs.set_visible_range(Axis::Row, 512.0).unwrap();
        file.set_minimap(prefs);
        file.save().unwrap();
        state.configure_audio_preferences(&path).unwrap();
        state.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
        );
        if reverse {
            event(&mut state, KeyCode::ShiftLeft, true);
            press(&mut state, KeyCode::KeyV);
            event(&mut state, KeyCode::ShiftLeft, false);
        }
        press(&mut state, KeyCode::Space);
        press(&mut state, KeyCode::ArrowDown);
        let selection = state.workspace().selection();
        assert_eq!(
            state.spacing_mode(),
            deadsync_editor::zoom::SpacingMode::ConstantRow
        );
        focus(&mut state);
        let clock = Clock::capture(&state.workspace().file().session, 0).unwrap();
        let code = if reverse {
            KeyCode::ArrowUp
        } else {
            KeyCode::ArrowDown
        };
        let mut found = false;
        for _ in 0..150 {
            press(&mut state, code);
            let geometry = state.minimap_viewport_geometry().unwrap();
            let row = geometry.focused_coordinate();
            if row > 145.0 && row < 191.0 {
                assert_eq!(geometry.axis, Axis::Row);
                assert!(
                    (state.chart_focus_time().unwrap() - clock.fractional_music_time(row).unwrap())
                        .abs()
                        < 1e-10
                );
                assert!(
                    (clock.position(state.chart_focus_time().unwrap()).unwrap() - row).abs() > 0.1,
                    "inverse timing would lose the requested warp row"
                );
                assert_eq!(state.workspace().row(), row.floor() as u64);
                assert_eq!(state.workspace().selection(),
                    Some((selection.unwrap().0, state.workspace().row()))
                );
                found = true;
                break;
            }
        }
        assert!(found, "keyboard drag reached warp interval");
        assert!(!state.workspace().file().session.is_dirty());
    }
}
#[test]
fn numeric_settings_accept_key_repeats_only_while_panel_is_open() {
    use deadsync_editor::minimap_viewport::Axis;
    let (_dir, mut state, _owner)=setup(false);
    let repeat = |state: &mut State, code| {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed: true,
            repeat: true,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    };
    chord(&mut state, KeyCode::KeyM, true);
    // Boolean/enum toggles keep their single-press policy.
    repeat(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    for _ in 0..3 {
        repeat(&mut state, KeyCode::ArrowRight);
    }
    event(&mut state, KeyCode::ArrowRight, false);
    press(&mut state, KeyCode::ArrowDown);
    for _ in 0..2 {
        repeat(&mut state, KeyCode::ArrowRight);
    }
    event(&mut state, KeyCode::ArrowRight, false);
    for _ in 0..3 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    let settings = state.minimap_preferences();
    assert_eq!(settings.variable_axis(), Axis::Time);
    assert_eq!(settings.visible_range(Axis::Time), 280.0);
    assert_eq!(settings.visible_range(Axis::Row), 24960.0);
    let before = state.workspace().file().session.serialize().unwrap();
    repeat(&mut state, KeyCode::ArrowRight);
    event(&mut state, KeyCode::ArrowRight, false);
    assert_eq!(state.minimap_preferences(), settings);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        before
    );
    assert!(!state.workspace().file().session.is_dirty());
}


fn publish_audio_fixture(state: &mut State)->u64 {
    let commands=state.take_audio_commands();
    let owners:Vec<_> =commands.iter().filter_map(|c|match c{deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..}=>Some(*owner),_=>None,
        }).collect();
    assert_eq!(owners.len(),1,"one published lazy session");
    assert_ne!(owners[0],0);
    let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
    let paths = state.editor_asset_paths();
    snapshot.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner: owners[0],
        registration: state.editor_asset_registration(),
        main: paths[0].is_some(),
        dedicated_preview: paths[1].is_some(),
    };
    state.update_audio(snapshot);
    assert!(!state.take_audio_commands().iter().any(|c|matches!(c,deadsync_gameplay::GameplayAudioCommand::EditorInitializeOutput{..})),"unavailable command-only host needs no DSP request");
    owners[0]
}
