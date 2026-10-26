//! Time/row viewport overview; keyboard capture adapts the original drag policy.
use super::*;
use deadsync_editor::{
    chart_order::Binding,
    minimap_navigation::{Effect, Gesture, Input, Position},
    minimap_preferences::Preferences,
    minimap_viewport::{Axis, Geometry, Viewport},
    playback::Clock,
};
use std::cell::RefCell;

struct Density {
    axis: Axis,
    binding: Binding,
    bins: [u32; 64],
    full_start: f64,
    range: f64,
    cursor: Option<deadsync_editor::notes::Position>,
    done: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_completion_during_capture_cannot_resume_chart_transport() {
        // Current UI stops audition when leaving its panel. Exercise the native
        // state-machine boundary directly as well, so a future nonmodal panel or
        // asynchronous completion cannot accidentally arm chart resume.
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
                854.0, 480.0,
            ));
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(directory.path().join("music.wav"), b"command-only asset").unwrap();
            let path = directory.path().join("song.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#MUSIC:music.wav;#SAMPLESTART:0;#SAMPLELENGTH:1;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_song_properties();
            state.toggle_sample_audio();
            assert!(state.is_playing());
            assert!(!state.chart_transport_is_playing());
            assert!(matches!(
                state.take_audio_commands().as_slice(),
                [deadsync_gameplay::GameplayAudioCommand::PlayMusic { looping: true, .. }]
            ));
            state.panel = None;
            state.minimap.focused = true;
            let key = |pressed| RawKeyboardEvent {
                code: KeyCode::ArrowDown,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            };
            assert!(state.minimap_key(&key(true)));
            assert!(
                state.take_audio_commands().is_empty(),
                "navigation does not pause audition"
            );
            state.stop_audio();
            state.take_audio_commands();
            assert!(state.minimap_key(&key(false)));
            assert!(!state.is_playing());
            assert!(
                state.take_audio_commands().is_empty(),
                "sample completion never starts chart playback"
            );
        }
    }
}
#[derive(Default)]
pub(super) struct Navigation {
    focused: bool,
    pub(super) preferences: Preferences,
    pub(super) draft: Option<Preferences>,
    held: u8,
    gesture: Gesture,
    binding: Option<Binding>,
    clock: RefCell<Option<(Binding, Arc<Clock>)>>,
    density: RefCell<Option<Density>>,
}
impl State {
    pub fn minimap_density_progress(&self) -> Option<(u64, bool)> {
        self.minimap.density.borrow().as_ref().map(|cache| {
            (
                cache.bins.iter().map(|count| u64::from(*count)).sum(),
                cache.done,
            )
        })
    }
    pub fn minimap_focused(&self) -> bool {
        self.minimap.focused
    }
    fn minimap_clock(&self) -> Result<Arc<Clock>, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        if let Some(view) = self.active_views.as_ref().and_then(|views| {
            views
                .entries()
                .iter()
                .find(|view| view.source_index() == self.workspace.chart())
        }) {
            return view.clock(session);
        }
        let mut cached = self.minimap.clock.borrow_mut();
        if cached
            .as_ref()
            .is_none_or(|(binding, _)| !binding.is_current(session, self.workspace.chart()))
        {
            *cached = Some((
                Binding::capture(session, self.workspace.chart())?,
                Arc::new(Clock::capture(session, self.workspace.chart())?),
            ));
        }
        Ok(Arc::clone(&cached.as_ref().unwrap().1))
    }
    fn minimap_position_with_clock(
        &self,
        clock: &Clock,
    ) -> Result<Position, deadsync_editor::Error> {
        let (chart_time, row) = match self.playback_position() {
            Some(position) => position,
            None => (
                clock.chart_time(self.workspace.row())?,
                self.workspace.row() as f64,
            ),
        };
        Ok(Position {
            chart_time,
            music_time: clock.chart_time_to_music(chart_time)?,
            row,
        })
    }
    fn minimap_position(&self) -> Result<Position, deadsync_editor::Error> {
        let clock = self.minimap_clock()?;
        self.minimap_position_with_clock(&clock)
    }
    pub(super) fn configure_minimap_preferences(&mut self, preferences: Preferences) {
        self.minimap.preferences = preferences;
        self.minimap.draft = None;
    }
    pub fn minimap_preferences(&self) -> Preferences {
        self.minimap.preferences
    }
    pub fn minimap_viewport_geometry(&self) -> Result<Geometry, deadsync_editor::Error> {
        self.minimap_projection().map(|(geometry, _)| geometry)
    }
    fn minimap_axis(&self) -> Axis {
        match self.spacing_mode() {
            deadsync_editor::zoom::SpacingMode::ConstantTime => Axis::Time,
            deadsync_editor::zoom::SpacingMode::ConstantRow => Axis::Row,
            deadsync_editor::zoom::SpacingMode::Variable => {
                self.minimap.preferences.variable_axis()
            }
        }
    }
    fn minimap_projection(&self) -> Result<(Geometry, Arc<Clock>), deadsync_editor::Error> {
        let clock = self.minimap_clock()?;
        let position = self.minimap_position_with_clock(&clock)?;
        let axis = self.minimap_axis();
        let mode = match axis {
            Axis::Time => deadsync_editor::zoom::SpacingMode::ConstantTime,
            Axis::Row => deadsync_editor::zoom::SpacingMode::ConstantRow,
        };
        let scale = self.zoom.spacing(mode) * self.zoom.spacing_zoom();
        let native = self.chart_viewport();
        let viewport = Viewport {
            top: native.top.into(),
            bottom: native.bottom.into(),
            focal: native.cursor.into(),
        };
        let last_row = self.workspace.grid().last_row();
        let (focus, content) = match axis {
            Axis::Time => (
                position.chart_time,
                (clock.chart_time(0)?, clock.note_time(last_row)?),
            ),
            Axis::Row => (position.row, (0.0, last_row as f64)),
        };
        let geometry = Geometry::build(
            axis,
            viewport,
            scale,
            focus,
            content,
            self.minimap.preferences.visible_range(axis),
            96,
            (screen_height() - 192.0).max(40.0) as i32,
            self.reverse_display,
            self.minimap.preferences.quantize(),
        )?;
        Ok((geometry, clock))
    }
    fn minimap_effect(&mut self, effect: Option<Effect>) -> Result<(), deadsync_editor::Error> {
        match effect {
            Some(Effect::Pause(_)) => self.pause_for_minimap(),
            Some(Effect::Focus(position)) => {
                let session = &self.workspace.file().session;
                let focus = match self.active_views.as_ref().and_then(|views| {
                    views
                        .entries()
                        .iter()
                        .find(|view| view.source_index() == self.workspace.chart())
                }) {
                    Some(view) if self.minimap_axis() == Axis::Row => {
                        view.position_at_row(session, position.row)?
                    }
                    Some(view) => view.position_at_time(session, position.music_time)?,
                    None if self.minimap_axis() == Axis::Row => {
                        deadsync_editor::chart_focus::Position::at_fractional_row(
                            session,
                            self.workspace.chart(),
                            position.row,
                        )?
                    }
                    None => deadsync_editor::chart_focus::Position::at_time(
                        session,
                        self.workspace.chart(),
                        position.music_time,
                    )?,
                };
                self.workspace.seek_preserving_selection(focus.edit_row())?;
                self.focus_position = Some(focus);
                self.refresh();
            }
            Some(Effect::Resume(_)) => {
                if !self.is_playing() {
                    self.toggle_audio();
                }
            }
            None => {}
        }
        Ok(())
    }
    fn minimap_end(&mut self) {
        let effect = self.minimap.gesture.input(Input::End);
        if self.minimap_effect(effect).is_err() {
            self.status = i18n::tr("Edition", "MinimapFailed");
        }
        self.minimap.held = 0;
        self.minimap.binding = None;
    }
    pub(super) fn minimap_release_input(&mut self) {
        self.minimap.gesture.clear();
        self.minimap.focused = false;
        self.minimap.held = 0;
        self.minimap.binding = None;
    }
    /// Called before the raw-key release guard, after modifier tracking.
    /// Returns true when this navigation mode consumed the key event.
    pub(super) fn minimap_key(&mut self, event: &RawKeyboardEvent) -> bool {
        if self.control
            && event.code == KeyCode::KeyM
            && self.panel.is_none()
            && self.workspace.has_chart()
        {
            if !event.pressed || event.repeat {
                return true;
            }
            if self.shift {
                self.open_minimap_settings();
                return true;
            }
            if self.minimap.focused {
                self.minimap_end();
            }
            self.minimap.focused = !self.minimap.focused;
            self.status = i18n::tr(
                "Edition",
                if self.minimap.focused {
                    "MinimapHelp"
                } else {
                    "Ready"
                },
            );
            return true;
        }
        if !self.minimap.focused {
            return false;
        }
        if self.panel.is_some() || !self.workspace.has_chart() {
            self.minimap.gesture.clear();
            self.minimap.focused = false;
            self.minimap.held = 0;
            return false;
        }
        if self.control {
            if event.pressed {
                self.minimap_release_input();
            }
            return false;
        }
        if self.minimap.binding.as_ref().is_some_and(|binding| {
            !binding.is_current(&self.workspace.file().session, self.workspace.chart())
        }) {
            self.minimap.gesture.clear();
            self.minimap.held = 0;
            self.minimap.binding = None;
        }
        if event.code == KeyCode::Escape {
            if event.pressed {
                self.minimap_end();
                self.minimap.focused = false;
                self.status = i18n::tr("Edition", "Ready");
            }
            return true;
        }
        let bit = match event.code {
            KeyCode::ArrowUp => 1,
            KeyCode::ArrowDown => 2,
            KeyCode::PageUp => 4,
            KeyCode::PageDown => 8,
            KeyCode::Home => 16,
            KeyCode::End => 32,
            _ => return true,
        };
        if !event.pressed {
            self.minimap.held &= !bit;
            if self.minimap.held == 0 {
                self.minimap_end();
            }
            return true;
        }
        let result = (|| -> Result<(), deadsync_editor::Error> {
            if !self.minimap.gesture.active() {
                let position = self.minimap_position()?;
                let playing = self.chart_transport_is_playing();
                let effect = self.minimap.gesture.input(Input::Begin {
                    position,
                    playing,
                    stop_playback_on_release: self.scroll_preferences.stop_playback_when_dragging(),
                });
                let session = &self.workspace.file().session;
                self.minimap.binding = Some(Binding::capture(session, self.workspace.chart())?);
                self.minimap_effect(effect)?;
            }
            self.minimap.held |= bit;
            let (mut geometry, clock) = self.minimap_projection()?;
            let projection = &mut geometry.projection;
            let focus = self.minimap_position()?;
            let unreflected = (f64::from(projection.bounds_y)
                + projection.project_editor(match geometry.axis {
                    Axis::Time => focus.chart_time,
                    Axis::Row => focus.row,
                }))
            .round() as i32;
            let current = projection.reflect_y(unreflected).ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Minimap pixel overflow".into())
            })?;
            let step = if self.shift { 10 } else { 1 };
            let selected = match event.code {
                KeyCode::ArrowUp => current.saturating_sub(step),
                KeyCode::ArrowDown => current.saturating_add(step),
                KeyCode::PageUp => current.saturating_sub(projection.visible_height / 4),
                KeyCode::PageDown => current.saturating_add(projection.visible_height / 4),
                KeyCode::Home => projection.bounds_y,
                KeyCode::End => projection.bounds_y + projection.visible_height,
                _ => current,
            };
            projection.drag(selected).ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Invalid minimap drag".into())
            })?;
            let position = geometry.position(&clock)?;
            let effect = self.minimap.gesture.input(Input::Move(position));
            self.minimap_effect(effect)
        })();
        if result.is_err() {
            self.status = i18n::tr("Edition", "MinimapFailed");
        }
        true
    }
    pub(super) fn minimap_actors(&self, actors: &mut Vec<Actor>) {
        if self.panel.is_some() || !self.workspace.has_chart() {
            return;
        }
        let Ok((geometry, clock)) = self.minimap_projection() else {
            return;
        };
        let projection = &geometry.projection;
        let full_range = projection.full_end - projection.full_start;
        let session = &self.workspace.file().session;
        let mut cache = self.minimap.density.borrow_mut();
        if cache.as_ref().is_none_or(|cache| {
            !cache.binding.is_current(session, self.workspace.chart())
                || cache.full_start != projection.full_start
                || cache.range != full_range
                || cache.axis != geometry.axis
        }) {
            *cache = Some(Density {
                axis: geometry.axis,
                binding: match Binding::capture(session, self.workspace.chart()) {
                    Ok(binding) => binding,
                    Err(_) => return,
                },
                bins: [0; 64],
                full_start: projection.full_start,
                range: full_range,
                cursor: None,
                done: false,
            });
        }
        if let Some(cache) = cache.as_mut().filter(|cache| !cache.done) {
            let mut count = 0;
            for (position, _) in self
                .workspace
                .grid()
                .iter_heads_after(cache.cursor)
                .take(4096)
            {
                cache.cursor = Some(position);
                count += 1;
                let coordinate = match geometry.axis {
                    Axis::Time => clock.note_time(position.row),
                    Axis::Row => Ok(position.row as f64),
                };
                if let Ok(coordinate) = coordinate {
                    let fraction =
                        ((coordinate - projection.full_start) / full_range).clamp(0.0, 1.0);
                    let bin = ((fraction * 64.0) as usize).min(63);
                    cache.bins[bin] = cache.bins[bin].saturating_add(1);
                }
            }
            cache.done = count < 4096;
        }
        let x = screen_width() - 24.0;
        if cache.as_ref().is_some_and(|cache| !cache.done) {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","MinimapLoading")): xy(x,82.0): zoom(0.5): maxwidth(90.0): z(8013)));
        }
        let y = projection.bounds_y as f32;
        let height = projection.visible_height as f32;
        actors.push(act!(quad: xy(x,y+height*0.5): setsize(32.0,height): diffuse(0.06,0.08,0.12,0.95): z(8010)));
        if let Some(cache) = cache.as_ref() {
            let maximum = cache.bins.iter().copied().max().unwrap_or(1).max(1) as f32;
            for (index, count) in cache
                .bins
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
            {
                let coordinate = cache.full_start + (index as f64 + 0.5) / 64.0 * cache.range;
                let fraction =
                    projection.project_editor(coordinate) / f64::from(projection.visible_height);
                if !(0.0..=1.0).contains(&fraction) {
                    continue;
                }
                let yy = if self.reverse_display {
                    1.0 - fraction as f32
                } else {
                    fraction as f32
                };
                actors.push(act!(quad: xy(x,y+yy*height): setsize(24.0*(*count as f32/maximum).sqrt(),height/64.0): diffuse(0.25,0.75,0.95,0.9): z(8011)));
            }
        }
        let start = (projection.project_editor(projection.editor_start)
            / f64::from(projection.visible_height))
        .clamp(0.0, 1.0) as f32;
        let end = (projection.project_editor(projection.editor_end)
            / f64::from(projection.visible_height))
        .clamp(0.0, 1.0) as f32;
        let center = if self.reverse_display {
            1.0 - (start + end) * 0.5
        } else {
            (start + end) * 0.5
        };
        actors.push(act!(quad: xy(x,y+center*height): setsize(30.0,((end-start)*height).max(2.0)): diffuse(0.95,0.75,0.25,if self.minimap.focused {0.5}else{0.2}): z(8012)));
    }
}
