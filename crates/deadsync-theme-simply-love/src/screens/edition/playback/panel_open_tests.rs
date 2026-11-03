//! UI openings never own the transport. Exercise the physical command registry
//! and the real scoped clock/callback observations, not synthetic dispatch calls.
use super::super::Panel;
use super::*;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::key_binding_preferences::{Action, Preferences};
use deadsync_gameplay::{EditorAssetKind, EditorAssetReadiness, EditorPreviewCursor};
use std::{fs, path::PathBuf};

fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn bind(state: &mut State, action: Action) {
    let mut preferences = Preferences::default();
    // Keep the complete canonical action map: toolbar labels read even
    // unbound actions while this fixture routes only the requested shortcut.
    for chords in preferences.commands.values_mut() {
        *chords = vec![vec![]];
    }
    preferences
        .commands
        .insert(action.id().to_owned(), vec![vec!["F12".to_owned()]]);
    preferences.validate().unwrap();
    state.configure_key_binding_preferences(Arc::new(preferences));
}
fn fixture(markers: bool, attacks: bool) -> (tempfile::TempDir, PathBuf, State) {
    crate::i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("Pack/Song");
    fs::create_dir_all(&directory).unwrap();
    for name in ["main.wav", "preview.wav"] {
        fs::write(directory.join(name), b"scoped command fixture").unwrap();
    }
    let path = directory.join("song.ssc");
    let attack = if attacks {
        // Main's observed clock advances the editing cursor to row60 at
        // 120BPM; dedicated audition leaves it at row0. Both have a real head.
        "#ATTACKS:TIME=0:LEN=1:MODS=100% Dizzy:TIME=0.625:LEN=1:MODS=100% Reverse;"
    } else {
        ""
    };
    fs::write(&path,format!("#VERSION:0.83;#TITLE:Panels;#MUSIC:main.wav;#PREVIEW:preview.wav;#SAMPLESTART:0.25;#SAMPLELENGTH:1;#OFFSET:0;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{attack}#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:5;#NOTES:0001;")).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    if markers {
        let plan = deadsync_editor::quick_add::Plan::prepare(
            &state.workspace.file().session,
            0,
            deadsync_editor::quick_add::Kind::Pattern,
            0.0,
        )
        .unwrap();
        state.workspace.apply_quick_add(&plan).unwrap();
        state.refresh();
        state.marker = None;
        state.panel = None;
    }
    (root, path, state)
}
fn start(state: &mut State, sample: bool) {
    state.take_audio_commands();
    let owner = state.editor_asset_owner();
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.editor.owner = owner;
    snapshot.editor.output.available = true;
    snapshot.editor.output.leased = true;
    snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
    snapshot.editor.output.requested_period = Some(512);
    snapshot.editor.output.negotiated_period = Some(512);
    state.update_audio(snapshot);
    state.observe_editor_assets(EditorAssetReadiness {
        owner,
        registration: state.editor_asset_registration(),
        main: true,
        dedicated_preview: true,
    });
    state.take_audio_commands();
    if sample {
        state.command_toggle_sample_audio();
        assert!(state.is_playing());
        assert!(!state.chart_transport_is_playing());
        state.observe_rendered_preview_cursor(Some(EditorPreviewCursor {
            suspended: false,
            rendered: true,
            owner,
            kind: EditorAssetKind::DedicatedPreview,
            registration: state.editor_asset_registration(),
            revision: 7,
            generation: 17,
            asset_epoch: 29,
            next_music_seconds: 0.625,
            rendered_frame: 30_000,
            source_frame: Some(30_000),
            sample_rate_hz: 48_000,
        }));
        assert_eq!(
            state
                .playback
                .as_ref()
                .unwrap()
                .sample_cursor
                .unwrap()
                .source_frame,
            Some(30_000)
        );
    } else {
        state.toggle_audio();
        assert!(state.chart_transport_is_playing());
        state.bind_decoder_generation(Some(17));
        state.bind_decoder_asset_epoch(Some(owner), Some(29));
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = state.audio_output_snapshot();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 17,
            asset_epoch: 29,
            music_nanos: 625_000_000,
        });
        snapshot.stream_clock.valid_at = Instant::now();
        state.update_audio(snapshot);
        assert_eq!(state.chart_audio_generation(), Some(17));
        assert_eq!(state.transport_music_time(), Some(0.625));
    }
    state.take_audio_commands();
}
#[derive(Debug, PartialEq)]
struct Witness {
    audio: String,
    source: Vec<u8>,
    revision: (u64, u64),
    history: (Option<String>, Option<String>),
    ids: Vec<deadsync_editor::generation::metadata::Guid>,
    cursor: (usize, u64),
}
fn witness(state: &State) -> Witness {
    let playing = state
        .playback
        .as_ref()
        .expect("opening must retain active transport");
    let session = &state.workspace.file().session;
    Witness {
        audio: format!(
            "{:?}",
            (
                state.editor_asset_owner(),
                state.editor_asset_registration(),
                state.chart_audio_generation(),
                playing.asset_epoch,
                playing.clock_generation,
                playing.issued_generation,
                playing.mapped_generation,
                playing.position.map(|p| (
                    p.music_time.to_bits(),
                    p.chart_time.to_bits(),
                    p.row.to_bits()
                )),
                &playing.sample_descriptor,
                playing.sample_cursor
            )
        ),
        source: session.serialize().unwrap(),
        revision: (session.revision(), session.content_revision()),
        history: (
            session.undo_label().map(str::to_owned),
            session.redo_label().map(str::to_owned),
        ),
        ids: session.chart_ids().to_vec(),
        cursor: (state.workspace.chart(), state.workspace.row()),
    }
}
fn unchanged(state: &mut State, before: &Witness, stage: &str) {
    assert!(state.is_playing(), "{stage}: {}", state.status);
    assert_eq!(&witness(state), before, "{stage}");
    for command in state.take_audio_commands() {
        assert!(
            !matches!(
                command,
                GameplayAudioCommand::EditorBeginAudio { .. }
                    | GameplayAudioCommand::EditorEndAudio { .. }
                    | GameplayAudioCommand::EditorPauseMusic { .. }
                    | GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorStartMusic { .. }
                    | GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
                    | GameplayAudioCommand::EditorPlayPreview { .. }
                    | GameplayAudioCommand::EditorResumePreview { .. }
                    | GameplayAudioCommand::StopMusic
                    | GameplayAudioCommand::SeekMusic(_)
            ),
            "{stage}: unexpected transport command {command:?}"
        );
    }
}
fn expected_panel(state: &State, action: Action) -> bool {
    match (action, &state.panel) {
        (Action::OpenMenu, Some(Panel::Menu(_)))
        | (Action::OpenAnalysis, Some(Panel::Analysis))
        | (Action::OpenTiming, Some(Panel::Timing(_)))
        | (Action::OpenPatternSet, Some(Panel::PatternSet(_)))
        | (Action::OpenBatch, Some(Panel::Batch(_)))
        | (Action::OpenChartProperties, Some(Panel::ChartProperties(_)))
        | (Action::OpenMarker, Some(Panel::Marker(_)))
        | (Action::OpenPattern, Some(Panel::Pattern(_)))
        | (Action::OpenGeneration, Some(Panel::Generation(_)))
        | (Action::OpenEventSelection, Some(Panel::EventSelection))
        | (Action::InspectAttack, Some(Panel::Attacks(_)))
        | (Action::OpenNoteAttributes, Some(Panel::NoteAttributes(_)))
        | (Action::OpenNoteConversion, Some(Panel::NoteConversion(_)))
        | (Action::OpenNoteOwners, Some(Panel::NoteOwners(_)))
        | (Action::OpenTransforms, Some(Panel::Transforms(_))) => true,
        _ => false,
    }
}
#[test]
fn physical_readonly_actions_and_escape_preserve_main_and_dedicated_preview() {
    deadlib_present::space::with_metrics(
        deadlib_present::space::Metrics::centered(1100., 760.),
        || {
            for sample in [false, true] {
                for action in [
                    Action::OpenMenu,
                    Action::OpenAnalysis,
                    Action::OpenTiming,
                    Action::OpenPatternSet,
                    Action::OpenBatch,
                    Action::OpenChartProperties,
                    Action::OpenMarker,
                    Action::OpenPattern,
                    Action::OpenGeneration,
                    Action::OpenEventSelection,
                    Action::InspectAttack,
                    Action::OpenNoteAttributes,
                    Action::OpenNoteConversion,
                    Action::OpenNoteOwners,
                    Action::OpenTransforms,
                ] {
                    let (_root, path, mut state) = fixture(true, true);
                    let disk = fs::read(&path).unwrap();
                    bind(&mut state, action);
                    start(&mut state, sample);
                    let before = witness(&state);
                    press(&mut state, KeyCode::F12);
                    assert!(
                        expected_panel(&state, action),
                        "{action:?}: {}",
                        state.status
                    );
                    unchanged(&mut state, &before, &format!("{action:?} opened"));
                    press(&mut state, KeyCode::Escape);
                    let parent = match action {
                        Action::OpenNoteAttributes => Some(11),
                        Action::OpenNoteConversion => Some(9),
                        Action::OpenNoteOwners => Some(10),
                        _ => None,
                    };
                    if let Some(index) = parent {
                        assert!(
                            matches!(state.panel, Some(Panel::Transforms(actual)) if actual == index)
                        );
                        unchanged(
                            &mut state,
                            &before,
                            &format!("{action:?} returned to tools"),
                        );
                        press(&mut state, KeyCode::Escape);
                    }
                    assert!(state.panel.is_none(), "{action:?} Cancel: {}", state.status);
                    unchanged(&mut state, &before, &format!("{action:?} cancelled"));
                    assert_eq!(fs::read(path).unwrap(), disk);
                }
            }
        },
    );
}
#[test]
fn refused_marker_attack_and_pending_timing_openings_do_not_touch_audio() {
    deadlib_present::space::with_metrics(
        deadlib_present::space::Metrics::centered(1100., 760.),
        || {
            for sample in [false, true] {
                for action in [
                    Action::OpenPatternSet,
                    Action::OpenMarker,
                    Action::InspectAttack,
                    Action::OpenTiming,
                ] {
                    let (_root, _path, mut state) = fixture(false, false);
                    if action == Action::OpenTiming {
                        let mut draft = state.workspace.chart_properties().unwrap();
                        draft.set_music_offset(0.25).unwrap();
                        state.chart_properties = Some(draft);
                    }
                    bind(&mut state, action);
                    start(&mut state, sample);
                    let before = witness(&state);
                    press(&mut state, KeyCode::F12);
                    assert!(
                        state.panel.is_none(),
                        "{action:?} should refuse: {}",
                        state.status
                    );
                    unchanged(&mut state, &before, &format!("{action:?} refused"));
                }
            }
        },
    );
}
#[test]
fn actual_f1_menu_routes_keep_transport_through_navigation_and_cancel() {
    deadlib_present::space::with_metrics(
        deadlib_present::space::Metrics::centered(1100., 760.),
        || {
            for sample in [false, true] {
                // Real default F1 and menu cursor movement, including helper-owned stops.
                for (index, label) in [
                    (14, "Analysis"),
                    (17, "ChartEventCopy"),
                    (18, "PackProperties"),
                    (24, "OutputSettings"),
                    (25, "FileMenu"),
                    (26, "DensitySettings"),
                    (29, "DisplayOptions"),
                    (30, "GeneralOptions"),
                ] {
                    let (_root, _path, mut state) = fixture(true, true);
                    start(&mut state, sample);
                    let before = witness(&state);
                    for _ in 0..if index == 18 { 2 } else { 1 } {
                        press(&mut state, KeyCode::F1);
                        assert!(matches!(state.panel, Some(Panel::Menu(0))));
                        unchanged(&mut state, &before, "F1 opened");
                        for _ in 0..index {
                            press(&mut state, KeyCode::ArrowDown);
                            unchanged(&mut state, &before, "Menu moved");
                        }
                        assert!(matches!(state.panel,Some(Panel::Menu(current)) if current==index));
                        press(&mut state, KeyCode::Enter);
                        assert!(
                            matches!(
                                (index, state.panel.as_ref()),
                                (14, Some(Panel::Analysis))
                                    | (17, Some(Panel::ChartEventCopy))
                                    | (18, Some(Panel::PackProperties))
                                    | (24, Some(Panel::OutputSettings(_)))
                                    | (25, Some(Panel::FileMenu(_)))
                                    | (26, Some(Panel::DensitySettings(_)))
                                    | (29, Some(Panel::OptionsSettings(_)))
                                    | (30, Some(Panel::GeneralOptions(_)))
                            ),
                            "{label}: {}",
                            state.status
                        );
                        unchanged(&mut state, &before, label);
                        press(&mut state, KeyCode::Escape);
                        assert!(state.panel.is_none(), "{label} Cancel: {}", state.status);
                        unchanged(&mut state, &before, "Menu child cancelled");
                    }
                }
            }
        },
    );
}

mod migration_tests;

mod chart_offset_tests;
mod timing_live_tests;

mod marker_metadata_tests;

mod hotbar_tests;

mod pack_history_tests;
