use super::*;
use deadsync_editor::{
    key_binding_preferences::scroll_modifiers::Kind,
    scroll_preferences::wheel::{self, FrameInput, Input, Intent, Interpolation},
};
use std::time::Instant;
#[derive(Default)]
pub(super) struct Runtime {
    input: FrameInput,
    spacing_active: bool,
    dirty: bool,
    previous_frame: Option<Instant>,
    interpolation: Option<(bool, Interpolation)>,
}
impl Runtime {
    pub(super) fn cancel(&mut self) {
        self.input.cancel();
        self.interpolation = None;
    }
}

#[cfg(test)]
mod navigation_regression_tests {
    use super::*;
    use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};

    fn fixture(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("wheel.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state.editor_asset_owner();
        state.take_audio_commands();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        state.take_audio_commands();
        (root, state)
    }

    #[test]
    fn snapped_wheel_matches_source_alignment_for_fractional_negative_and_reverse_positions() {
        for language in ["en", "fr"] {
            for reverse in [false, true] {
                for (initial, earlier, later) in [
                    (50.75, 48.0, 96.0),
                    (-0.75, -48.0, 48.0),
                    (-50.75, -48.0, 0.0),
                ] {
                    for (forward, expected) in [(false, earlier), (true, later)] {
                        let (_root, mut state) = fixture(language);
                        state.scroll_preferences.set_reverse(reverse);
                        state.reverse_display = reverse;
                        assert_eq!(
                            deadsync_editor::workspace::SNAP_ROWS[state.workspace.snap()],
                            48
                        );
                        state.navigate_exact_position(initial, false);
                        let bytes = state.workspace.file().session.serialize().unwrap();
                        let now = Instant::now();
                        state.update_with_time(now);
                        let detents = if forward != reverse { -1.0 } else { 1.0 };
                        assert!(state.wheel_lines(detents));
                        state.update_with_time(now + std::time::Duration::from_millis(16));
                        assert_eq!(
                            state.playback_position().unwrap().1,
                            expected,
                            "initial={initial}, reverse={reverse}, forward={forward}"
                        );
                        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
                        assert!(state.workspace.file().session.undo_label().is_none());
                    }
                }
            }
        }
    }

    #[test]
    fn changing_snap_with_physical_arrows_keeps_chart_transport_playing() {
        for language in ["en", "fr"] {
            let (_root, mut state) = fixture(language);
            state.toggle_audio();
            assert!(state.chart_transport_is_playing(), "{}", state.status);
            state.take_audio_commands();
            let position = state.playback_position();
            for (code, expected) in [(KeyCode::ArrowRight, 1), (KeyCode::ArrowLeft, 0)] {
                for pressed in [true, false] {
                    state.raw_key(&RawKeyboardEvent {
                        code,
                        pressed,
                        repeat: false,
                        timestamp: Instant::now(),
                        host_nanos: 0,
                    });
                }
                assert_eq!(state.workspace.snap(), expected, "{}", state.status);
                assert!(state.chart_transport_is_playing());
                assert_eq!(state.playback_position(), position);
                assert!(!state.take_audio_commands().iter().any(|command| matches!(
                    command,
                    GameplayAudioCommand::EditorStopMusic { .. }
                        | GameplayAudioCommand::EditorPauseMusic { .. }
                        | GameplayAudioCommand::EditorSeekMusic { .. }
                        | GameplayAudioCommand::EditorStartMusic { .. }
                )));
            }
        }
    }
}

impl State {
    /// Window-local input; both editor hosts call this after live-input admission.
    pub fn wheel_lines(&mut self, vertical: f64) -> bool {
        if !self.wheel_allowed() {
            self.wheel.cancel();
            return false;
        }
        self.wheel.input.lines(vertical);
        true
    }
    pub fn wheel_pixels(&mut self, vertical: f64, window_scale: f64) -> bool {
        if !self.wheel_allowed() {
            self.wheel.cancel();
            return false;
        }
        self.wheel.input.physical_pixels(vertical, window_scale);
        true
    }
    pub(super) fn update_wheel(&mut self, now: Instant) {
        let previous = self.wheel.previous_frame.replace(now).unwrap_or(now);
        if !self.wheel_allowed() {
            self.wheel.cancel();
            return;
        }
        let current_time = now.saturating_duration_since(self.zoom_epoch).as_secs_f64();
        let start = previous
            .saturating_duration_since(self.zoom_epoch)
            .as_secs_f64();
        // Editor.UpdateMusicAndPosition only interpolates while !Playing;
        // each playing frame resets both Desired axes to the transport cursor.
        // Do this at frame admission, not on Play, so a start/pause pair without
        // an intervening playback frame retains the original pending target.
        if self.chart_transport_is_playing() {
            self.wheel.interpolation = None;
        }
        let detents = self.wheel.input.take();
        let intent = wheel::decide(
            self.scroll_preferences,
            Input {
                detents,
                zoom_held: self.scroll_modifier_down(Kind::Zoom),
                spacing_held: self.scroll_modifier_down(Kind::Spacing),
                playing: self.chart_transport_is_playing(),
                has_chart: self.has_song() && self.workspace.has_chart(),
                snapped: deadsync_editor::workspace::SNAP_ROWS[self.workspace.snap()] != 0,
                spacing_zoom: self.zoom.spacing_zoom(),
                spacing_mode: self.spacing_preference,
            },
        );
        let result = (|| -> Result<(), deadsync_editor::Error> {
            match intent {
                Intent::Zoom => {
                    self.zoom
                        .process_input(start, detents, true, false, self.spacing_mode())?;
                }
                Intent::Spacing(Some(mode)) => {
                    self.wheel.spacing_active = true;
                    self.zoom.process_input(start, detents, false, true, mode)?;
                }
                Intent::Spacing(None) | Intent::None => {}
                Intent::SnapEarlier | Intent::SnapLater => {
                    self.wheel.interpolation = None;
                    // The original wheel calls the same snapped navigation as
                    // Up/Down, including fractional and negative positions.
                    self.navigate_snap(matches!(intent, Intent::SnapLater));
                }
                Intent::PlayingTime { delta, .. } => {
                    self.wheel.interpolation = None;
                    let music = self.capture_song_property_presentation()?.music_time();
                    self.navigate_exact_time(music + delta, true)?;
                }
                Intent::InterpolateTime(delta) | Intent::InterpolateRows(delta) => {
                    let position = self.capture_song_property_presentation()?;
                    let is_time = matches!(intent, Intent::InterpolateTime(_));
                    let current = if is_time {
                        position.music_time()
                    } else {
                        position.row()
                    };
                    if !self
                        .wheel
                        .interpolation
                        .is_some_and(|(axis, _)| axis == is_time)
                    {
                        self.wheel.interpolation = Some((is_time, Interpolation::new(current)));
                    }
                    self.wheel
                        .interpolation
                        .as_mut()
                        .expect("initialized axis")
                        .1
                        .begin(start, current, delta);
                }
            }
            if let Some((time, mut interpolation)) = self.wheel.interpolation {
                let active = !interpolation.is_complete(interpolation.current());
                let position = interpolation.value(current_time, self.zoom_preferences.duration());
                self.wheel.interpolation = Some((time, interpolation));
                if active {
                    if time {
                        self.navigate_exact_time(position, true)?;
                    } else {
                        self.navigate_wheel_position(false, position, true)?;
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.wheel.cancel();
            self.status = i18n::editor_error(&error).into();
        }
    }
}

impl State {
    pub(super) fn sync_wheel_spacing(&mut self) -> Result<(), deadsync_editor::Error> {
        if !self.wheel.spacing_active {
            return Ok(());
        }
        let mut settings = self.scroll_preferences;
        for (index, mode) in [
            deadsync_editor::zoom::SpacingMode::ConstantTime,
            deadsync_editor::zoom::SpacingMode::ConstantRow,
            deadsync_editor::zoom::SpacingMode::Variable,
        ]
        .into_iter()
        .enumerate()
        {
            settings.set_speed(index, self.zoom.spacing(mode))?;
        }
        if settings != self.scroll_preferences {
            self.scroll_preferences = settings;
            if let Some(file) = self.application_preferences_owner_mut() {
                file.set_scroll(settings);
            }
            self.wheel.dirty = true;
        }
        self.wheel.spacing_active = self.zoom.spacing_is_interpolating();
        Ok(())
    }
    pub(super) fn flush_wheel_settings(&mut self) -> Result<(), deadsync_editor::Error> {
        self.workspace.retry_profile_durability()?;
        self.sync_wheel_spacing()?;
        // This is an explicit publication/departure barrier, never a frame loop.
        // MRU and advanced output options share this real mutable File owner with
        // wheel settings. Keep it on failure: save may already have published
        // preferences while directory durability still requires a retry.
        if let Some(file) = self.audio_preferences_file.as_mut() {
            if file.has_changes() {
                file.save()?;
            }
        }
        self.flush_application_memory_preferences()?;
        self.wheel.dirty = false;
        Ok(())
    }
    pub(super) fn wheel_intent_guard(&mut self) -> bool {
        match self.flush_wheel_settings() {
            Ok(()) => true,
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "ApplicationPreferencesSaveFailed",
                    &[("error", &i18n::editor_error(&error))],
                );
                false
            }
        }
    }
    pub(super) fn freeze_wheel_spacing(&mut self) {
        for mode in [
            deadsync_editor::zoom::SpacingMode::ConstantTime,
            deadsync_editor::zoom::SpacingMode::ConstantRow,
            deadsync_editor::zoom::SpacingMode::Variable,
        ] {
            let value = self.zoom.spacing(mode);
            self.zoom
                .set_spacing(mode, value)
                .expect("current bounded spacing");
        }
        self.wheel.spacing_active = false;
        self.wheel.cancel();
    }
    pub(super) fn transfer_wheel_spacing(&self, next: &mut State) {
        next.zoom.inherit_spacing_animation(&self.zoom);
        next.wheel.spacing_active = self.wheel.spacing_active;
    }
}
