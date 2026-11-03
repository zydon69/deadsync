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

// The simplified native editor fits short charts into the overview. A configured
// smaller range still zooms long charts; persisted source preferences are intact.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_short_and_long_chart_overviews_share_bounds_at_all_native_sizes_and_dpi() {
        i18n::init_for_tests();
        let directory=tempfile::tempdir().unwrap();
        for (measures,short) in [(32,true),(256,false)] {
            let path=directory.path().join(format!("{measures}.ssc"));
            let notes=std::iter::repeat_n("1000\n0100\n0010\n0001",measures).collect::<Vec<_>>().join(",\n");
            std::fs::write(&path,format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};"),
            ).unwrap();
            for (width,height) in [(854,480),(1100,760),(3440,1440)] {
                for dpi in [1.0,2.0] {
                    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(width as f32,height as f32),
                    );
                    let mut state=State::open(&path,0).unwrap();
                    state.ui_dpi=Some(deadsync_editor::options_preferences::dpi::Capture::new(&state.general_options,dpi,
                    ));
                    state.ui_dpi_render_scale=dpi as f32;
                    state.set_ui_dpi_viewport((width as f64*dpi) as u32,(height as f64*dpi) as u32,
                    );
                    let preferences=state.minimap_preferences();
                    let before=state.workspace.file().session.serialize().unwrap();
                    for mode in [deadsync_editor::zoom::SpacingMode::ConstantTime,deadsync_editor::zoom::SpacingMode::ConstantRow,
                    ] {
                        state.spacing_mode=mode;
                        for reverse in [false,true] {
                            state.reverse_display=reverse;
                            let (geometry,clock)=state.minimap_projection().unwrap();
                            let p=geometry.projection;
                            let viewport=state.chart_viewport();
                            let mount=state.minimap_mount_bounds().unwrap();
                            assert_eq!(mount.y,p.bounds_y);
                            assert_eq!(mount.visible_height,p.visible_height as u32);
                            assert_eq!(p.bounds_y,state.overview_vertical_layout().top);
                            assert_eq!(p.bounds_y+p.visible_height,viewport.bottom.floor() as i32);
                            assert_eq!(p.reverse,reverse);
                            let configured=preferences.visible_range(geometry.axis);
                            let last=state.workspace.grid().last_row();
                            let content=match geometry.axis {
                                Axis::Time=> {
                                    (clock.chart_time(0).unwrap(),clock.note_time(last).unwrap())
                                }
                                Axis::Row=>(0.,last as f64),
                            };
                            if short {
                                let start = p.project_editor(content.0);
                                let end = p.project_editor(content.1);
                                assert!((start - 1.0).abs() < 1e-8);
                                assert!((end - f64::from(p.visible_height - 2)).abs() < 1e-8);
                            } else {
                                assert_eq!(p.mini_range, configured);
                            }
                            assert_eq!(state.minimap_preferences(),preferences);
                            assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn short_chart_fills_overview_without_rewriting_configured_ranges(){
        i18n::init_for_tests();
        for (size,dpi)in[([854.0,480.0],1.0),([1100.0,760.0],2.0),([3440.0,1440.0],2.0),
        ]{
            deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(size[0],size[1],
            ));
            for (mode,index,low,high,coordinate)in[(deadsync_editor::zoom::SpacingMode::ConstantTime,1,30.0,300.0,2.0,
                ),(deadsync_editor::zoom::SpacingMode::ConstantRow,2,3072.0,28800.0,192.0,
                ),
            ]{
                let root=tempfile::tempdir().unwrap();let path=root.path().join("short.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001,1000\n0100\n0010\n0001;").unwrap();
                let mut state=State::open(&path,0).unwrap();state.configure_spacing_preference(deadsync_editor::scroll_preferences::spacing::Preference::from_effective(mode),
                );state.ui_dpi=Some(deadsync_editor::options_preferences::dpi::Capture::new(&state.general_options,dpi,
                ));state.ui_dpi_render_scale=dpi as f32;state.set_ui_dpi_viewport((size[0]as f64*dpi)as u32,(size[1]as f64*dpi)as u32,
                );
                let mut projected=Vec::new();
                for(right,expected)in[(false,low),(true,high)]{
                    state.open_minimap_settings();for _ in 0..200{state.minimap_settings_key(index,if right{KeyCode::ArrowRight}else{KeyCode::ArrowLeft},
                        );}state.minimap_settings_key(5,KeyCode::Enter);
                    let(geometry,clock)=state.minimap_projection().unwrap();let p=geometry.projection;assert_eq!(
                        state.minimap_preferences().visible_range(geometry.axis),expected);projected.push((p.project_editor(coordinate),p.project_editor(coordinate+1.0)-p.project_editor(coordinate),
                    ));let mount=state.minimap_mount_bounds().unwrap();assert_eq!(mount.y,p.bounds_y);assert_eq!(mount.visible_height,p.visible_height as u32);
                    let mut actors=Vec::new();state.minimap_content_actors(&mut actors,&geometry,&clock);let mut meshes=0;
                    for actor in &actors{if let Actor::Mesh{vertices,offset,..}=actor{meshes+=1;assert_eq!(*offset,[mount.x as f32,p.bounds_y as f32]);for vertex in vertices.iter(){assert!(vertex.pos[0]>=0.0&&vertex.pos[0]<=state.minimap.content.options.width as f32);assert!(vertex.pos[1]>=0.0&&vertex.pos[1]<=p.visible_height as f32);}}}
                    assert!(meshes>0,"actual content raster must produce a bounded mesh");
                }
                assert_eq!(projected[0],projected[1],
                    "short chart fits both oversized ranges"
                );
            }
        }
    }
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
            let viewport=state.chart_viewport();
            let geometry=state.minimap_viewport_geometry().unwrap();
            let bounds=state.minimap_mount_bounds().unwrap();
            assert!(geometry.projection.bounds_y as f32 >= viewport.top);
            assert!((geometry.projection.bounds_y+geometry.projection.visible_height) as f32 <= viewport.bottom);
            assert_eq!(bounds.y,geometry.projection.bounds_y);
            assert_eq!(bounds.visible_height,geometry.projection.visible_height as u32);
            assert_eq!(state.minimap_preferences().visible_range(geometry.axis), Preferences::default().visible_range(geometry.axis));

            let commands=state.take_audio_commands();
            let owners:Vec<_>=commands.iter().filter_map(|c|match c{deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..}=> {
                        Some(*owner)
                    }
                    _=>None,
                }).collect();
            assert_eq!(owners.len(),1);let owner=owners[0];assert_ne!(owner,0);
            let mut snapshot=deadsync_gameplay::GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);snapshot.editor.assets=deadsync_gameplay::EditorAssetReadiness{owner,registration:state.editor_asset_registration(),main:true,dedicated_preview:true,
            };state.update_audio(snapshot);
            assert!(state.take_audio_commands().is_empty());
            state.open_song_properties();
            state.toggle_sample_audio();
            assert!(state.is_playing());
            assert!(!state.chart_transport_is_playing());
            assert!(matches!(
                state.take_audio_commands().as_slice(),
                [deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview { owner:actual,looping: true, .. }] if *actual==owner
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
    pub(super) position: deadsync_editor::minimap_position_preferences::Preferences,
    pub(super) options_draft: Option<super::minimap_options_draft::Draft>,
    selection: deadsync_editor::minimap_select::Capture,
    pointer_y: Option<i32>,
    pub(super) draft: Option<Preferences>,
    pub(super) visual_draft: Option<deadsync_editor::visual_preferences::MiniMap>,
    held: u8,
    gesture: Gesture,
    binding: Option<Binding>,
    clock: RefCell<Option<(Binding, Arc<Clock>)>>,
    pub(super) content: super::minimap_content_state::Content,
}
impl State {
    /// Compatibility diagnostic: content is now queried exactly, not aggregated.
    pub fn minimap_density_progress(&self) -> Option<(u64, bool)> {
        self.minimap_content_progress()
            .map(|(count, notes, markers, mesh)| (count as u64, notes && markers && mesh))
    }
    pub(super) fn minimap_capture_active(&self) -> bool {
        self.minimap.gesture.active()
    }
    pub fn minimap_focused(&self) -> bool {
        self.minimap.focused
    }
    fn minimap_clock(&self) -> Result<Arc<Clock>, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        let chart=self.preview_source_chart().unwrap_or(self.workspace.chart());
        if let Some(view) = self.active_views.as_ref().and_then(|views| {
            views
                .entries()
                .iter()
                .find(|view| view.source_index() == chart)
        }) {
            return view.clock(session);
        }
        let mut cached = self.minimap.clock.borrow_mut();
        if cached
            .as_ref()
            .is_none_or(|(binding, _)| !binding.is_current(session, chart))
        {
            *cached = Some((
                Binding::capture(session, chart)?,
                Arc::new(Clock::capture(session, chart)?),
            ));
        }
        Ok(Arc::clone(&cached.as_ref().unwrap().1))
    }
    fn minimap_position_with_clock(
        &self,
        clock: &Clock,
    ) -> Result<Position, deadsync_editor::Error> {
        if let Some(position)=self.preview_position()?{return Ok(Position{chart_time:position.chart_time(),music_time:position.music_time(),row:position.row(),
            });}
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
        let last_row = self.preview.as_ref().map_or_else(||self.workspace.grid().last_row(),|preview|preview.grid().last_row(),
        );
        let (focus, content) = match axis {
            Axis::Time => (
                position.chart_time,
                (clock.chart_time(0)?, clock.note_time(last_row)?),
            ),
            Axis::Row => (position.row, (0.0, last_row as f64)),
        };
        let overview = self.overview_vertical_layout();
        let mut geometry = Geometry::build(
            axis,
            viewport,
            scale,
            focus,
            content,
            self.minimap.preferences.visible_range(axis),
            overview.top,
            (overview.bottom - overview.top).max(1),
            self.reverse_display,
            self.minimap.preferences.quantize(),
        )?;
        let span = content.1 - content.0;
        let projection = &mut geometry.projection;
        if span > 0.0 && span <= projection.mini_range && projection.visible_height > 3 {
            // Reserve one raster pixel at each rim. Use the same effective
            // projection for notes, overlays, cursor and navigation, rather
            // than stretching just the note mesh after rendering.
            let units_per_pixel = span / f64::from(projection.visible_height - 3);
            projection.mini_range = units_per_pixel * f64::from(projection.visible_height);
            projection.mini_start = content.0 - units_per_pixel;
            projection.mini_unquantized = projection.mini_start;
            projection.full_start = projection.mini_start;
            projection.full_end = projection.mini_start + projection.mini_range;
        }
        Ok((geometry, clock))
    }
    fn minimap_effect(&mut self, effect: Option<Effect>) -> Result<(), deadsync_editor::Error> {
        match effect {
            Some(Effect::Pause(_)) => self.pause_for_minimap(),
            Some(Effect::Focus(position)) => {
                // Source navigation switch has no Variable/unknown case, even
                // though UpdateMiniMap renders these raw modes using time.
                if self.spacing_mode()==deadsync_editor::zoom::SpacingMode::Variable && !self.minimap.preferences.variable_spacing().supports_navigation(){return Ok(());}
                self.receptor_views.reset();
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
                self.retarget_paused_scroll_position(&focus);
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
        self.minimap.selection.clear();
        self.minimap.pointer_y = None;
        self.minimap.held = 0;
        self.minimap.binding = None;
    }
    pub(super) fn minimap_release_input(&mut self) {
        self.minimap.gesture.clear();
        self.minimap.focused = false;
        self.minimap.selection.clear();
        self.minimap.pointer_y = None;
        self.minimap.held = 0;
        self.minimap.binding = None;
    }
    /// Called before the raw-key release guard, after modifier tracking.
    /// Returns true when this navigation mode consumed the key event.
    pub(super) fn command_toggle_minimap_focus(&mut self) {
        if !self.minimap.content.options.visible {
            return;
        }
        if self.minimap.focused {
            self.minimap_end();
        }
        self.minimap.focused = !self.minimap.focused;
        self.status = if self.minimap.focused {
            i18n::tr_fmt(
                "Edition",
                "MinimapHelp",
                &[(
                    "focus",
                    &self.command_binding_label(
                        deadsync_editor::key_binding_preferences::Action::ToggleMinimapFocus,
                    ),
                )],
            )
        } else {
            i18n::tr("Edition", "Ready")
        };
    }
    pub(super) fn minimap_key(&mut self, event: &RawKeyboardEvent) -> bool {
        if !self.minimap.content.options.visible {
            self.minimap_release_input();
            return false;
        }
        if !self.minimap.focused {
            return false;
        }
        if self.panel.is_some() || !self.workspace.has_chart() {
            self.minimap.gesture.clear();
            self.minimap.focused = false;
            self.minimap.selection.clear();
            self.minimap.pointer_y = None;
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
            self.minimap.selection.clear();
            self.minimap.pointer_y = None;
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
        if self.minimap.preferences.visible_range(self.minimap_axis())==0.0{return true;}
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let beginning = !self.minimap.gesture.active();
            if beginning {
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
            let current = if self.minimap.position.select_mode
                == deadsync_editor::minimap_position_preferences::SelectMode::MoveToSelectedArea
            {
                self.minimap.pointer_y.unwrap_or(current)
            } else {
                current
            };
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
            if self.minimap.position.select_mode
                == deadsync_editor::minimap_position_preferences::SelectMode::MoveToSelectedArea
            {
                self.minimap.pointer_y = Some(selected);
                if beginning {
                    let cursor = projection.editor_start
                        + (projection.editor_end - projection.editor_start)
                            * projection.grabbed_percentage;
                    let target = projection.reflect_y(selected).ok_or_else(|| {
                        deadsync_editor::Error::InvalidEdit("Minimap pixel overflow".into())
                    })?;
                    let relative = f64::from(target) - f64::from(projection.bounds_y);
                    let inside = relative >= projection.project_editor(projection.editor_start)
                        && relative <= projection.project_editor(projection.editor_end);
                    // Small keyboard steps grab the existing editor rectangle at
                    // the cursor, then move. Distant keys click the chosen area.
                    let begin_pixel = if inside { current } else { selected };
                    self.minimap
                        .selection
                        .begin(
                            projection,
                            begin_pixel,
                            cursor,
                            self.minimap.position.select_mode,
                        )
                        .ok_or_else(|| {
                            deadsync_editor::Error::InvalidEdit("Invalid minimap selection".into())
                        })?;
                    if inside {
                        self.minimap
                            .selection
                            .move_to(projection, selected)
                            .ok_or_else(|| {
                                deadsync_editor::Error::InvalidEdit(
                                    "Invalid minimap selection motion".into(),
                                )
                            })?;
                    }
                } else {
                    self.minimap
                        .selection
                        .move_to(projection, selected)
                        .ok_or_else(|| {
                            deadsync_editor::Error::InvalidEdit(
                                "Invalid minimap selection motion".into(),
                            )
                        })?;
                }
            } else {
                projection.drag(selected).ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("Invalid minimap drag".into())
                })?;
            }
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
        if (self.panel.is_some() && !matches!(self.panel, Some(super::Panel::GenerationPreview(_))))
            || !self.workspace.has_chart()
        {
            return;
        }
        let Ok((geometry, clock)) = self.minimap_projection() else {
            return;
        };
        if let Ok(bounds) = self.minimap_mount_bounds() {
            self.overview_label(actors,bounds.x as f32,bounds.width as f32,0,i18n::tr("Edition","MinimapOverview"),
            );
        }
        self.minimap_content_actors(actors, &geometry, &clock);
    }
}

#[cfg(test)]mod source_raw_minimap_consumers{
 use super::*;use deadsync_editor::minimap_preferences::VariableSpacing;
 fn state(root:&std::path::Path)->State{let path=root.join("source.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100,0010,0001;").unwrap();State::open(&path,0).unwrap()}
 #[test]fn zero_range_is_owned_but_renders_nothing_and_cannot_arm_capture(){i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0,
        ));let root=tempfile::tempdir().unwrap();let mut state=state(root.path());let mut preferences=state.minimap_preferences();for axis in[Axis::Time,Axis::Row]{preferences.set_visible_range(axis,0.0).unwrap();}state.configure_minimap_preferences(preferences);state.take_audio_commands();state.minimap.focused=true;assert!(state.minimap_viewport_geometry().is_err());let mut actors=Vec::new();state.minimap_actors(&mut actors);assert!(actors.is_empty());let row=state.workspace.row();assert!(state.minimap_key(&RawKeyboardEvent{code:KeyCode::ArrowDown,pressed:true,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0}));assert!(!state.minimap_capture_active());assert_eq!(state.workspace.row(),row);assert!(state.take_audio_commands().is_empty());}
 #[test]fn source_variable_and_unknown_render_time_but_do_not_set_position_while_known_modes_do(){i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0,
        ));for raw in[VariableSpacing::ConstantTime,VariableSpacing::ConstantRow,VariableSpacing::Variable,VariableSpacing::Unknown(99),
        ]{let root=tempfile::tempdir().unwrap();let mut state=state(root.path());state.spacing_mode=deadsync_editor::zoom::SpacingMode::Variable;let mut preferences=state.minimap_preferences();preferences.set_variable_spacing(raw);state.configure_minimap_preferences(preferences);assert_eq!(state.minimap_projection().unwrap().0.axis,raw.axis());let before=state.workspace.row();let before_focus=state.focus_position.as_ref().map(|p|(p.music_time(),p.row()));let clock=state.minimap_clock().unwrap();state.minimap_effect(Some(Effect::Focus(Position{chart_time:clock.chart_time(192).unwrap(),music_time:clock.chart_time_to_music(clock.note_time(192).unwrap()).unwrap(),row:192.0,
                }))).unwrap();if raw.supports_navigation(){let focus=state.focus_position.as_ref().expect("navigation must publish the exact display position");assert_eq!(state.workspace.row(),focus.edit_row());if raw.axis()==Axis::Row{assert_eq!(focus.row(),192.0);}else{let time=clock.chart_time_to_music(clock.note_time(192).unwrap()).unwrap();assert_eq!(focus.music_time(),time);assert_eq!(focus.row(),clock.position(time).unwrap());assert!((focus.row()-192.0).abs()<1e-9);}}else{assert_eq!(state.workspace.row(),before);assert_eq!(state.focus_position.as_ref().map(|p|(p.music_time(),p.row())),before_focus);}}}
 #[test]fn both_panels_restore_raw_spacing_and_zero_ranges_and_cancel_preserves_raw(){i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0,
        ));for options in[false,true]{let root=tempfile::tempdir().unwrap();let mut state=state(root.path());let path=root.path().join("preferences.json");std::fs::write(&path,"{\"Version\":1,\"MiniMap\":{\"MiniMapSpacingModeForVariable\":\"Variable\",\"MiniMapVisibleTimeRange\":0,\"MiniMapVisibleRowRange\":0}}").unwrap();state.configure_audio_preferences(&path).unwrap();let before=state.minimap_preferences();assert_eq!(before.variable_spacing(),VariableSpacing::Variable);if options{state.open_minimap_options();state.minimap_options_key(13,KeyCode::Enter);state.minimap_options_key(15,KeyCode::Enter);}else{state.open_minimap_settings();state.minimap_settings_key(4,KeyCode::Enter);state.minimap_settings_key(6,KeyCode::Enter);}assert_eq!(state.minimap_preferences(),before);if options{state.open_minimap_options();state.minimap_options_key(13,KeyCode::Enter);state.minimap_options_key(14,KeyCode::Enter);}else{state.open_minimap_settings();state.minimap_settings_key(4,KeyCode::Enter);state.minimap_settings_key(5,KeyCode::Enter);}assert_eq!(state.minimap_preferences(),Preferences::default());assert!(state.minimap_viewport_geometry().is_ok());state.command(Command::Undo);assert_eq!(state.minimap_preferences(),before);assert!(state.minimap_viewport_geometry().is_err());state.command(Command::Redo);assert_eq!(state.minimap_preferences(),Preferences::default());}}
}

#[cfg(test)]
mod exact_pause_navigation_tests {
    use super::*;
    use deadsync_gameplay::{EditorLogicalMusicClock, GameplayAudioCommand, GameplayAudioSnapshot};
    use std::time::Instant;

    fn fixture(axis: Axis) -> (tempfile::TempDir, State, u64) {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(root.path().join("music.wav"), b"command-only asset").unwrap();
        std::fs::write(&path, "#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state
            .take_audio_commands()
            .iter()
            .find_map(|command| match command {
                GameplayAudioCommand::EditorBeginAudio { owner, .. } => Some(*owner),
                _ => None,
            })
            .unwrap();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        state.take_audio_commands();
        state.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::from_effective(match axis {
                Axis::Time => deadsync_editor::zoom::SpacingMode::ConstantTime,
                Axis::Row => deadsync_editor::zoom::SpacingMode::ConstantRow,
            }),
        );
        (root, state, owner)
    }
    fn position(state: &State, row: f64) -> Position {
        let exact = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            state.workspace.chart(),
            row,
        )
        .unwrap();
        Position {
            music_time: exact.music_time(),
            chart_time: exact.chart_time(),
            row: exact.row(),
        }
    }
    fn start(state: &mut State, owner: u64, row: f64) {
        state.navigate_exact_position(row, true);
        state.toggle_audio();
        assert!(
            matches!(state.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorStartMusic {owner:actual,..}] if *actual == owner)
        );
        state.bind_decoder_generation(Some(7));
        state.bind_decoder_asset_epoch(Some(owner), Some(13));
        let target = position(state, row);
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = state.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(EditorLogicalMusicClock {
            generation: 7,
            asset_epoch: 13,
            music_nanos: (target.music_time * 1e9) as i64,
        });
        state.update_audio(snapshot);
        state.take_audio_commands();
    }
    fn begin(state: &mut State, stop: bool) {
        state.minimap.focused = true;
        let position = state.minimap_position().unwrap();
        let playing = state.chart_transport_is_playing();
        let effect = state.minimap.gesture.input(Input::Begin {
            position,
            playing,
            stop_playback_on_release: stop,
        });
        state.minimap_effect(effect).unwrap();
        assert!(!state.is_playing());
        assert!(matches!(
            state.take_audio_commands().as_slice(),
            [GameplayAudioCommand::EditorPauseMusic { .. }]
        ));
    }
    fn move_to(state: &mut State, row: f64) -> Position {
        let target = position(state, row);
        let effect = state.minimap.gesture.input(Input::Move(target));
        state.minimap_effect(effect).unwrap();
        let expected = match state.minimap_axis() {
            Axis::Row => deadsync_editor::chart_focus::Position::at_fractional_row(
                &state.workspace.file().session,
                state.workspace.chart(),
                target.row,
            )
            .unwrap(),
            Axis::Time => deadsync_editor::chart_focus::Position::at_time(
                &state.workspace.file().session,
                state.workspace.chart(),
                target.music_time,
            )
            .unwrap(),
        };
        assert_eq!(state.workspace.row(), expected.edit_row());
        assert_eq!(
            state.playback_position(),
            Some((expected.chart_time(), expected.row()))
        );
        assert_eq!(
            state.chart_focus_time().unwrap().to_bits(),
            expected.music_time().to_bits()
        );
        Position {
            music_time: expected.music_time(),
            chart_time: expected.chart_time(),
            row: expected.row(),
        }
    }
    fn assert_resume(state: &mut State, owner: u64, target: Position) {
        assert!(state.chart_transport_is_playing());
        assert_eq!(
            state.playback_position(),
            Some((target.chart_time, target.row))
        );
        assert!(
            matches!(state.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorResumeMusic { owner:actual, seconds,.. }] if *actual == owner && seconds.to_bits() == (target.music_time + state.audio_preferences.effective().offset).to_bits())
        );
    }
    #[test]
    fn fractional_drag_and_return_to_initial_integral_row_replace_exact_paused_resume_in_both_axes()
    {
        for axis in [Axis::Time, Axis::Row] {
            for stop in [false, true] {
                for path in [&[50.75][..], &[60.75, 50.5][..], &[-0.75][..]] {
                    let (root, mut state, owner) = fixture(axis);
                    let initial = if path[0] < 0.0 { -0.25 } else { 50.25 };
                    start(&mut state, owner, initial);
                    let before = state.workspace.file().session.serialize().unwrap();
                    begin(&mut state, stop);
                    let mut target = position(&state, initial);
                    for row in path {
                        target = move_to(&mut state, *row);
                    }
                    std::fs::remove_file(root.path().join("music.wav")).unwrap();
                    state.minimap_end();
                    if stop {
                        assert!(!state.is_playing());
                        assert!(state.take_audio_commands().is_empty());
                        assert_eq!(
                            state.playback_position(),
                            Some((target.chart_time, target.row))
                        );
                        state.toggle_audio();
                    }
                    assert_resume(&mut state, owner, target);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                    assert!(state.workspace.file().session.undo_label().is_none());
                }
            }
        }
    }
    #[test]
    fn capture_from_user_paused_transport_retargets_without_auto_playing_or_reopening() {
        let (_root, mut state, owner) = fixture(Axis::Row);
        start(&mut state, owner, 50.25);
        state.toggle_audio();
        assert!(!state.is_playing());
        assert!(matches!(state.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorPauseMusic {..}]));
        let position = state.minimap_position().unwrap();
        let effect = state.minimap.gesture.input(Input::Begin {position, playing:false, stop_playback_on_release:false,
        });
        state.minimap_effect(effect).unwrap();
        let target = move_to(&mut state, 50.75);
        let effect = state.minimap.gesture.input(Input::Move(Position {row:f64::NAN, ..target}));
        assert!(effect.is_none());
        state.minimap_effect(effect).unwrap();
        state.minimap_end();
        assert!(!state.is_playing());
        assert_eq!(state.playback_position(), Some((target.chart_time, target.row)));
        assert!(state.take_audio_commands().is_empty());
        state.toggle_audio();
        assert_resume(&mut state, owner, target);
    }
    #[test]
    fn escape_releases_capture_and_resumes_exact_destination_but_lifecycle_clear_never_auto_resumes()
     {
        for lifecycle in [false, true] {
            let (_root, mut state, owner) = fixture(Axis::Row);
            start(&mut state, owner, 50.25);
            begin(&mut state, false);
            let target = move_to(&mut state, 50.75);
            if lifecycle {
                state.minimap_release_input();
                assert!(!state.minimap.gesture.active());
                assert!(!state.is_playing());
                assert!(state.take_audio_commands().is_empty());
                state.minimap_end();
                assert!(!state.is_playing());
                assert!(state.take_audio_commands().is_empty());
                state.toggle_audio();
            } else {
                assert!(state.minimap_key(&RawKeyboardEvent {
                    code: KeyCode::Escape,
                    pressed: true,
                    repeat: false,
                    timestamp: Instant::now(),
                    host_nanos: 0
                }));
                assert!(!state.minimap.gesture.active());
                assert!(!state.minimap.focused);
            }
            assert_resume(&mut state, owner, target);
        }
    }
}
