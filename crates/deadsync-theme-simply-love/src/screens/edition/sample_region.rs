use super::*;

pub(super) struct Cache {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    values: [u64; 5],
    positions: Option<[f64; 2]>,
    times: Option<[f64; 2]>,
}
impl State {
    pub(super) fn refresh_sample_region(&mut self) {
        self.refresh_live_preview_region();
        self.refresh_misc_draft_widgets();
        self.refresh_waveform();
        if !self.workspace.has_chart() {
            self.sample_region = None;
            return;
        }
        let session = &self.workspace.file().session;
        let captured;
        let draft = if let Some(draft) = self.song_properties.as_ref() {
            draft
        } else {
            captured = deadsync_editor::song_properties::Draft::capture(session);
            let Ok(draft) = &captured else {
                self.sample_region = None;
                return;
            };
            draft
        };
        let main_preview = draft
            .text(deadsync_editor::song_text::Field::PreviewMusic)
            .is_empty();
        let values = [
            draft.sample_start(),
            draft.sample_length(),
            draft.music_offset(),
            draft.sync_offset(),
            if main_preview { 1.0 } else { 0.0 },
        ]
        .map(f64::to_bits);
        let chart = self
            .preview_source_chart()
            .unwrap_or_else(|| self.workspace.chart());
        if self.sample_region.as_ref().is_some_and(|cache| {
            Arc::ptr_eq(&cache.source, session.document().source())
                && cache.revision == session.revision()
                && cache.chart == chart
                && cache.values == values
        }) {
            return;
        }
        let interval = if main_preview && draft.sample_length() > 0.0 {
            draft
                .sample_end()
                .ok()
                .and_then(|end| draft.clock(session, chart).ok().map(|clock| (clock, end)))
        } else {
            None
        };
        let times = interval.as_ref().and_then(|(clock, end)| {
            Some([
                clock.music_to_chart_time(draft.sample_start()).ok()?,
                clock.music_to_chart_time(*end).ok()?,
            ])
        });
        let positions = interval.as_ref().and_then(|(clock, end)| {
            Some([
                clock.position(draft.sample_start()).ok()?,
                clock.position(*end).ok()?,
            ])
        });
        self.sample_region = Some(Cache {
            source: Arc::clone(session.document().source()),
            revision: session.revision(),
            chart,
            values,
            times,
            positions,
        });
    }
    pub(super) fn sample_region_actors(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        top: f32,
        bottom: f32,
        projection: &spacing_view::Projection<'_>,
    ) {
        let Some(cache) = &self.sample_region else {
            return;
        };
        let Some(times) = cache.times else {
            return;
        };
        let positions = cache.positions.unwrap_or([f64::NAN; 2]);
        let Some(start) = projection.sample_y(positions[0], times[0]) else {
            return;
        };
        let Some(end) = projection.sample_y(positions[1], times[1]) else {
            return;
        };
        let a = start.min(end).clamp(f64::from(top), f64::from(bottom)) as f32;
        let b = start.max(end).clamp(f64::from(top), f64::from(bottom)) as f32;
        if b <= a {
            return;
        }
        if self.render_policy().regions {
            actors.push(
            act!(quad: xy(x,(a+b)*0.5): zoomto(width,b-a): diffuse(0.2,0.85,0.65,0.14): z(8001)),
        );
            for boundary in [start, end] {
                let position = boundary;
                if position >= f64::from(top) && position <= f64::from(bottom) {
                    actors.push(act!(quad: xy(x,position as f32): zoomto(width,1.0): diffuse(0.2,0.85,0.65,0.8): z(8002)));
                }
            }
        }
    }
}

#[cfg(test)]
mod resident_preview_consumer_tests {
    use super::*;
    use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
    use deadsync_editor::{song_text::Field, workspace::Command};
    use deadsync_gameplay::{
        EditorAssetKind as Kind, EditorAssetReadiness, GameplayAudioCommand as Audio,
    };
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn fixture(language: &str) -> (tempfile::TempDir, PathBuf, Vec<u8>, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        for name in ["main.wav", "ignored.wav", "authored.wav"] {
            std::fs::write(
                root.path().join(name),
                b"command fixture; readiness explicitly acknowledged",
            )
            .unwrap();
        }
        let path = root.path().join("raw.sm");
        let bytes=b"#TITLE:Resident preview;#MUSIC:main.wav;#PREVIEW:ignored.wav;#SAMPLESTART:0.375;#SAMPLELENGTH:1.25;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTES:dance-single::Edit:9:0,0,0,0,0:1000\n0100\n0010\n0001;".to_vec();
        std::fs::write(&path, &bytes).unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, path, bytes, state)
    }
    fn acknowledge(state: &mut State) {
        state.take_audio_commands();
        let owner = state.editor_asset_owner();
        let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
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
    }
    fn preview_widget(state: &State) -> bool {
        let captured;
        let draft = match state.song_properties.as_ref() {
            Some(draft) => draft,
            None => {
                captured = state.workspace.song_properties().unwrap();
                &captured
            }
        };
        draft
            .widgets(&state.workspace.file().session, 0)
            .unwrap()
            .iter()
            .any(|widget| widget.kind() == deadsync_editor::event_clipboard::WidgetKind::Preview)
    }
    fn inspector_press(state: &mut State, code: KeyCode) {
        let playing = state.is_playing();
        let main = state.chart_transport_is_playing();
        press(state, code);
        assert_eq!(
            state.is_playing(),
            playing,
            "inspector key {code:?} changed playing state; {}",
            state.status
        );
        assert_eq!(
            state.chart_transport_is_playing(),
            main,
            "inspector key {code:?} changed main/sample role; {}",
            state.status
        );
    }
    fn open_preview_field(state: &mut State) {
        inspector_press(state, KeyCode::F11);
        assert!(
            matches!(state.panel, Some(Panel::SongProperties(0))),
            "{}",
            state.status
        );
        for _ in 0..3 {
            inspector_press(state, KeyCode::ArrowDown);
        }
        inspector_press(state, KeyCode::Enter);
        assert!(matches!(state.panel, Some(Panel::SongText(0))));
        for _ in 0..10 {
            inspector_press(state, KeyCode::ArrowDown);
        }
        assert!(matches!(state.panel, Some(Panel::SongText(10))));
    }
    fn apply_text(state: &mut State) {
        press(state, KeyCode::Escape);
        assert!(matches!(state.panel, Some(Panel::SongProperties(3))));
        press(state, KeyCode::ArrowUp);
        press(state, KeyCode::ArrowUp);
        press(state, KeyCode::Enter);
        assert!(state.panel.is_none(), "{}", state.status);
        assert!(state.song_properties.is_none());
    }
    fn assert_main(state: &mut State) {
        state.refresh_sample_region();
        assert!(state.sample_region.as_ref().unwrap().times.is_some());
        assert!(preview_widget(state));
        acknowledge(state);
        state.command_toggle_sample_audio();
        assert!(state.is_playing(), "{}", state.status);
        assert!(state.take_audio_commands().iter().any(|command|matches!(command,Audio::EditorPlayPreview{kind:Kind::Main,path,cut,..}if path.ends_with("main.wav")&&cut.start_sec==0.375&&cut.length_sec==1.25)));
        state.stop_audio();
        state.take_audio_commands();
    }
    #[test]
    fn physical_f11_authored_sm_preview_apply_undo_redo_clear_reopen_en_fr() {
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(1100., 760.),
            || {
                for language in ["en", "fr"] {
                    let (root, path, source_bytes, mut state) = fixture(language);
                    assert!(
                        state.editor_asset_paths()[1].is_none(),
                        "raw SM PREVIEW is not a registered source"
                    );
                    assert_eq!(
                        state
                            .workspace
                            .song_properties()
                            .unwrap()
                            .text(Field::PreviewMusic),
                        ""
                    );
                    assert_main(&mut state);
                    let old_owner = state.editor_asset_owner();
                    open_preview_field(&mut state);
                    assert!(state.text_input("authored.wav"));
                    assert!(!preview_widget(&state));
                    assert!(
                        state.sample_region.as_ref().unwrap().times.is_none(),
                        "staged dedicated source invalidates region cache before Apply"
                    );
                    let mut actors = Vec::new();
                    state.actors(&mut actors);
                    assert!(!actors.iter().any(|actor|matches!(actor,Actor::Text{content,..}if content.contains("Song widget preview is unavailable"))));
                    apply_text(&mut state);
                    assert!(
                        state.editor_asset_paths()[1]
                            .as_ref()
                            .unwrap()
                            .ends_with("authored.wav")
                    );
                    assert_eq!(
                        state
                            .workspace
                            .file()
                            .session
                            .document()
                            .text(None, "PREVIEW")
                            .as_deref(),
                        Some("ignored.wav")
                    );
                    acknowledge(&mut state);
                    state.command_toggle_sample_audio();
                    assert!(state.is_playing());
                    assert!(state.take_audio_commands().iter().any(|command|matches!(command,Audio::EditorPlayPreview{kind:Kind::DedicatedPreview,path,cut,..}if path.ends_with("authored.wav")&&cut.start_sec==0.&&cut.length_sec.is_infinite())));
                    let authored_bytes = state.workspace.file().session.serialize().unwrap();
                    state.command(Command::Undo);
                    assert!(
                        !state.is_playing(),
                        "undo configured preview change stops old audition"
                    );
                    assert!(state.editor_asset_paths()[1].is_none());
                    assert_main(&mut state);
                    state.command(Command::Redo);
                    assert!(
                        state.editor_asset_paths()[1]
                            .as_ref()
                            .unwrap()
                            .ends_with("authored.wav")
                    );
                    assert!(!preview_widget(&state));
                    // Old registration cannot certify the restored dedicated source.
                    state.observe_editor_assets(EditorAssetReadiness {
                        owner: old_owner,
                        registration: state.editor_asset_registration().wrapping_sub(1),
                        main: true,
                        dedicated_preview: true,
                    });
                    state.command_toggle_sample_audio();
                    assert!(!state.is_playing());
                    state.take_audio_commands();
                    acknowledge(&mut state);
                    state.command_toggle_sample_audio();
                    assert!(state.is_playing());
                    state.take_audio_commands();
                    open_preview_field(&mut state);
                    press(&mut state, KeyCode::F3);
                    assert!(preview_widget(&state));
                    assert!(state.sample_region.as_ref().unwrap().times.is_some());
                    apply_text(&mut state);
                    assert!(
                        !state.is_playing(),
                        "Clear must stop obsolete dedicated audition"
                    );
                    assert!(state.editor_asset_paths()[1].is_none());
                    assert_main(&mut state);
                    let cleared_bytes = state.workspace.file().session.serialize().unwrap();
                    for (bytes, dedicated) in [(authored_bytes, true), (cleared_bytes, false)] {
                        let reopened = root.path().join(if dedicated {
                            "authored.sm"
                        } else {
                            "cleared.sm"
                        });
                        std::fs::write(&reopened, &bytes).unwrap();
                        let mut fresh = State::open(&reopened, 0).unwrap();
                        assert_ne!(fresh.editor_asset_owner(), old_owner);
                        fresh.take_audio_commands();
                        fresh.observe_editor_assets(EditorAssetReadiness {
                            owner: old_owner,
                            registration: fresh.editor_asset_registration(),
                            main: true,
                            dedicated_preview: true,
                        });
                        fresh.command_toggle_sample_audio();
                        assert!(
                            !fresh.is_playing(),
                            "reopened owner rejects stale readiness"
                        );
                        fresh.take_audio_commands();
                        assert_eq!(fresh.editor_asset_paths()[1].is_some(), dedicated);
                        assert_eq!(preview_widget(&fresh), !dedicated);
                        if dedicated {
                            acknowledge(&mut fresh);
                            fresh.command_toggle_sample_audio();
                            assert!(fresh.take_audio_commands().iter().any(|command|matches!(command,Audio::EditorPlayPreview{kind:Kind::DedicatedPreview,path,..}if path.ends_with("authored.wav"))));
                        } else {
                            assert_main(&mut fresh);
                        }
                    }
                    assert_eq!(
                        std::fs::read(path).unwrap(),
                        source_bytes,
                        "fixture original never published or overwritten"
                    );
                }
            },
        );
    }
    #[test]
    fn physical_preview_apply_and_clear_keep_main_transport_owner_phase_generation_en_fr() {
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(1100., 760.),
            || {
                for language in ["en", "fr"] {
                    let (_root, _path, _bytes, mut state) = fixture(language);
                    acknowledge(&mut state);
                    state.toggle_audio();
                    assert!(state.chart_transport_is_playing());
                    state.take_audio_commands();
                    let owner = state.editor_asset_owner();
                    state.bind_decoder_generation(Some(17));
                    state.bind_decoder_asset_epoch(Some(owner), Some(29));
                    let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
                    snapshot.editor.owner = owner;
                    snapshot.editor.output.available = true;
                    snapshot.editor.output.leased = true;
                    snapshot.editor.output.period_control =
                        deadsync_gameplay::EditorBufferControl::Adjustable;
                    snapshot.editor.output.requested_period = Some(512);
                    snapshot.editor.output.negotiated_period = Some(512);
                    snapshot.editor.logical_music =
                        Some(deadsync_gameplay::EditorLogicalMusicClock {
                            generation: 17,
                            asset_epoch: 29,
                            music_nanos: 625_000_000,
                        });
                    snapshot.stream_clock.valid_at = std::time::Instant::now();
                    state.update_audio(snapshot);
                    state.take_audio_commands();
                    let instant = state
                        .transport_music_time()
                        .expect("scoped actual logical music clock");
                    for clear in [false, true] {
                        open_preview_field(&mut state);
                        if clear {
                            inspector_press(&mut state, KeyCode::F3);
                        } else {
                            assert!(state.text_input("authored.wav"));
                        }
                        assert!(
                            state.chart_transport_is_playing(),
                            "staging preview path does not affect main"
                        );
                        apply_text(&mut state);
                        assert!(
                            state.chart_transport_is_playing(),
                            "source OnMusicPreviewChanged stops preview only"
                        );
                        assert_eq!(state.editor_asset_owner(), owner);
                        assert_eq!(state.chart_audio_generation(), Some(17));
                        assert_eq!(state.playback_asset_epoch(), Some(29));
                        assert_eq!(
                            state.transport_music_time().unwrap().to_bits(),
                            instant.to_bits()
                        );
                        assert!(!state.take_audio_commands().iter().any(|command| matches!(
                            command,
                            Audio::EditorStopMusic { .. }
                                | Audio::EditorPauseMusic { .. }
                                | Audio::EditorStartMusic { .. }
                                | Audio::EditorSwapMusicAsset { .. }
                                | Audio::EditorClearMusicAsset { .. }
                                | Audio::SeekMusic(_)
                        )));
                    }
                }
            },
        );
    }
}
