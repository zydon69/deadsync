//! Bridge from shared Session/view state into immutable visible-chart actors.
use super::{
    GenerationPreview, State, text,
    view_layout::{self, Geometry},
    view_render::{self, Viewport},
};
use crate::i18n;
use deadsync_editor::Error;
use deadlib_present::{
    actors::Actor,
    space::{screen_center_x, screen_height, screen_width},
};
impl State {
    pub(super) fn visible_view_geometry(&self)->Vec<Geometry>{self.chart_geometries(false)}
    pub(super) fn first_chart_canvas_left(&self)->Option<f32>{self.chart_geometries(true).first().map(|g|g.left)}
    fn chart_geometries(&self,unclipped:bool) -> Vec<Geometry> {
        if !self.workspace.has_chart() {
            return Vec::new();
        }
        let session = &self.workspace.file().session;
        let mut charts: Vec<_> = self
            .active_views
            .as_ref()
            .filter(|views| views.belongs_to(session))
            .map(|views| {
                views
                    .entries()
                    .iter()
                    .filter_map(|view| {
                        (session.chart_ids().get(view.source_index()) == Some(view.identity()))
                            .then_some(view.source_index())
                            .and_then(|chart| {
                                deadsync_editor::notes::layout_columns(
                                    &session.document().charts()[chart].steps_type,
                                )
                                .map(|columns| (chart, columns))
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let focused = self.workspace.chart();
        let focused_columns = self
            .preview
            .as_ref()
            .map(GenerationPreview::grid)
            .unwrap_or_else(|| self.workspace.grid())
            .columns();
        if let Some((_, columns)) = charts.iter_mut().find(|(chart, _)| *chart == focused) {
            *columns = focused_columns;
        } else {
            charts.push((focused, focused_columns));
        }
        let width = screen_width();
        let side = self.pattern_set_side_panel();
        let center = self.receptor_canvas_position().0;
        let available = if side { width - 570.0 } else { width - 210.0 };
        let right = if side {
            (width - 390.0).max(1.0)
        } else {
            width
        };
        view_layout::layout(&charts, focused, center, available, if unclipped{-f32::MAX}else{0.0}, if unclipped{f32::MAX}else{right}).unwrap_or_default()
    }

    // Detached edit overlays retain their own bounded geometry resources.
    pub(super) fn prepare_view_render(&mut self) {
        let mut charts: Vec<_> = self
            .visible_view_geometry()
            .into_iter()
            .map(|view| view.source)
            .collect();
        if let Some(source)=self.preview_source_chart(){if !charts.contains(&source){charts.push(source);}}
        if let Err(error) = self.workspace.prepare_view_grids(&charts) {
            self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]);
        }
        self.view_inputs.prepare(
            &self.workspace.file().session,
            &charts,
            self.active_views.as_ref(),
        );
        self.prepare_waveform_draw_policy();
    }
    pub(super) fn preserve_chart_canvas_left(&mut self,previous:Option<f32>)->Result<(),Error>{
        let Some(previous)=previous else{return Ok(())};let Some(current)=self.first_chart_canvas_left()else{return Ok(())};let mut preferences=self.receptor_preferences();let delta=previous-current;if delta.round()==0.{return Ok(())}
        let width=screen_width();let side=self.pattern_set_side_panel();let baseline=if side{(width-390.)*0.5}else{screen_center_x()};let right=if side{(width-390.).max(1.)}else{width.max(1.)};let destination=(self.receptor_canvas_position().0+delta).clamp(0.,right);
        let raw=(842.+f64::from(destination-baseline)).round().clamp(i32::MIN as f64,i32::MAX as f64)as i32;
        if raw==preferences.placement.x{return Ok(())}preferences.placement.x=raw;
        // SetChartFocused changes this application preference without an Undo
        // action and ignores user axis locks. Keep the real owner for flush and
        // preference-history authentication; never reset audio/receptor clocks.
        if let Some(file)=self.audio_preferences_file.as_mut(){file.set_receptors(preferences);}else{if self.source_preferences_import.memory.is_none(){self.source_preferences_import.memory=Some(deadsync_editor::audio_preferences::File::memory()?);}self.source_preferences_import.memory.as_mut().unwrap().set_receptors(preferences);}
        self.configure_receptor_preferences(preferences);Ok(())
    }
    pub(super) fn receptor_canvas_position(&self)->(f32,f32){
        let width=screen_width();let side=self.pattern_set_side_panel();let center=if side{(width-390.)*0.5}else{screen_center_x()};let right=if side{(width-390.).max(1.)}else{width.max(1.)};let top=105.;let bottom=(screen_height()-85.).max(top);
        self.receptor_preferences().placement.project(center,0.,right,top,bottom,self.reverse_display)
    }
    pub(super) fn chart_viewport(&self) -> Viewport {
        let top = 105.0;
        let bottom = (screen_height() - 85.0).max(top);
        let cursor=self.receptor_canvas_position().1;
        Viewport {
            top,
            bottom,
            cursor,
        }
    }
    pub(super) fn inactive_view_actors(&self, actors: &mut Vec<Actor>) {
        let Ok(music_time) = self.chart_focus_time() else {
            return;
        };
        let geometries = self.visible_view_geometry();

        let viewport = self.chart_viewport();
        let session = &self.workspace.file().session;
        for geometry in geometries.into_iter().filter(|view| !view.focused) {
            let result = (|| -> Result<(), deadsync_editor::Error> {
                let grid = session.loaded_grid(geometry.source).ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("View grid is unavailable".into())
                })?;
                let data = self.view_inputs.get(session, geometry.source)?;
                let projection = data.projection(
                    music_time,
                    self.spacing_mode,
                    self.reverse_display,
                    self.zoom.spacing_zoom(),
                    viewport,
                    self.current_spacing_speeds(),
                )?;
                let cursor = data.position_row(music_time)?.max(0.0).min(i32::MAX as f64) as u64;
                let last_row = grid
                    .last_row()
                    .max(cursor.saturating_add(768))
                    .max(data.last_row());
                let ranges = projection.visible_ranges(viewport.top, viewport.bottom, last_row)?;
                view_render::lanes(actors, grid, geometry, viewport);
                self.receptor_actors(actors, geometry, viewport);
                view_render::markers(
                    actors,
                    data,
                    &projection,
                    geometry,
                    viewport,
                    &ranges,
                    ((viewport.bottom - viewport.top).ceil() as usize).clamp(1, 4096),
                    self.selection_marker_budget(geometry.source),
                    self.render_policy(),
                );
                let policy = self.receptor_policy(geometry.source);
                let padding = geometry.lane_width * self.zoom.size_zoom() as f32;
                let note_ranges = projection.visible_ranges(viewport.top-padding, viewport.bottom+padding,last_row)?;
                let source_plan=self.source_viewport_plan(geometry,viewport)?;
                let admitted=source_plan.admission.notes().collect::<Vec<_>>();
                self.misc_event_actors_from_plan(actors,geometry,viewport,&source_plan);
                if self.render_policy().notes {
                let mut stats = view_render::notes(
                    actors,
                    grid,
                    &session.document().charts()[geometry.source].steps_type,
                    &projection,
                    geometry,
                    viewport,
                    &note_ranges,
                    self.zoom.size_zoom() as f32,
                    admitted.len(),
                    policy.as_ref(),
                    self.reverse_display,
                    self.active_views.as_ref().and_then(|views|views.entries().iter().find(|view|view.source_index()==geometry.source)).and_then(|view|view.selection()),
                    Some((&self.hold_colors,data.signatures.as_ref())),
                    None,
                    Some(&admitted),
                    self.selected_note_positions_for_render(geometry.source).as_ref(),
                );
                stats.partial|=source_plan.admission.stopped_by_quota;
                view_render::partial_notice(actors, geometry, viewport, stats);
                }
                view_render::cursor(actors, geometry, viewport);
                Ok(())
            })();
            if let Err(error) = result {
                text(
                    actors,
                    &i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]),
                    geometry.left.max(geometry.clip_left),
                    viewport.top,
                    0.45,
                    [1.0, 0.5, 0.5, 1.0],
                );
            }
        }
    }
}
