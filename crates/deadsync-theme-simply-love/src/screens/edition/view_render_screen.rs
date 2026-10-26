//! Bridge from shared Session/view state into immutable visible-chart actors.
use super::{
    GenerationPreview, State, text,
    view_layout::{self, Geometry},
    view_render::{self, Viewport},
};
use crate::i18n;
use deadlib_present::{
    actors::Actor,
    space::{screen_center_x, screen_height, screen_width},
};
impl State {
    pub(super) fn visible_view_geometry(&self) -> Vec<Geometry> {
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
        let center = if side {
            (width - 390.0) * 0.5
        } else {
            screen_center_x()
        };
        let available = if side { width - 570.0 } else { width - 210.0 };
        let right = if side {
            (width - 390.0).max(1.0)
        } else {
            width
        };
        view_layout::layout(&charts, focused, center, available, 0.0, right).unwrap_or_default()
    }
    pub(super) fn view_note_budget(&self) -> usize {
        (view_render::NOTE_BUDGET / self.visible_view_geometry().len().max(1)).max(1)
    }
    pub(super) fn prepare_view_render(&mut self) {
        let charts: Vec<_> = self
            .visible_view_geometry()
            .into_iter()
            .map(|view| view.source)
            .collect();
        if let Err(error) = self.workspace.prepare_view_grids(&charts) {
            self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
        }
        self.view_inputs.prepare(
            &self.workspace.file().session,
            &charts,
            self.active_views.as_ref(),
        );
    }
    pub(super) fn chart_viewport(&self) -> Viewport {
        let top = 105.0;
        let bottom = (screen_height() - 85.0).max(top);
        let cursor = if self.reverse_display {
            (bottom - 55.0).max(top)
        } else {
            (top + 55.0).min(bottom)
        };
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
        let budget = (view_render::NOTE_BUDGET / geometries.len().max(1)).max(1);
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
                );
                let policy = self.receptor_policy(geometry.source);
                let padding = geometry.lane_width * self.zoom.size_zoom() as f32;
                let note_ranges = projection.visible_ranges(viewport.top-padding, viewport.bottom+padding,last_row)?;
                let stats = view_render::notes(
                    actors,
                    grid,
                    &session.document().charts()[geometry.source].steps_type,
                    &projection,
                    geometry,
                    viewport,
                    &note_ranges,
                    self.zoom.size_zoom() as f32,
                    budget,
                    policy.as_ref(),
                    self.reverse_display,
                    self.active_views.as_ref().and_then(|views|views.entries().iter().find(|view|view.source_index()==geometry.source)).and_then(|view|view.selection()),
                );
                view_render::cursor(actors, geometry, viewport);
                view_render::partial_notice(actors, geometry, viewport, stats);
                self.end_hint_actors(actors, &projection, geometry, viewport);
                Ok(())
            })();
            if let Err(error) = result {
                text(
                    actors,
                    &i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]),
                    geometry.left.max(geometry.clip_left),
                    viewport.top,
                    0.45,
                    [1.0, 0.5, 0.5, 1.0],
                );
            }
        }
    }
}
