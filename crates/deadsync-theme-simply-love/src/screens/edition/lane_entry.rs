//! Native draft ownership for source press/release note entry. State/view hooks
//! are supplied separately; this module never edits document bytes on key-down.
use super::{
    State, lane_for_key, spacing_view::Projection, view_layout::Geometry, view_render::Viewport,
};
use crate::i18n;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    lane_entry::{Inputs, Press},
    notes::Kind,
    workspace::{Command, SNAP_ROWS},
};
#[derive(Default)]
pub(super) struct Runtime {
    pub(super) model: Option<Inputs>,
    pub(super) fake_cache: Option<deadsync_editor::draft_note_policy::Cache>,
    pub(super) fake_error: Option<String>,
    shift_keys: u8,
    alt_keys: u8,
    last_row: Option<f64>,
}
impl State {
    fn lane_entry_row(&self) -> f64 {
        if self.chart_autoplay_transport().is_some() {
            if let Some((_, row)) = self.playback_position() {
                return row;
            }
        }
        self.focus_position
            .as_ref()
            .filter(|p| p.is_current(&self.workspace))
            .map_or(self.workspace.row() as f64, |p| p.row())
    }
    pub(super) fn lane_entry_cancel(&mut self) -> bool {
        self.lane_input.last_row = None;
        self.lane_input
            .model
            .take()
            .is_some_and(|mut input| input.cancel() != 0)
    }
    pub(super) fn lane_entry_release_input(&mut self) {
        self.lane_input = Runtime::default();
    }
    fn lane_entry_error(&mut self, error: Error) {
        self.lane_entry_cancel();
        self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
    }
    /// Run before modifier/modal early returns so release cannot remain queued
    /// behind a newly opened panel. True means the key has been consumed.
    pub(super) fn lane_entry_before_key(&mut self, event: &RawKeyboardEvent) -> bool {
        match event.code {
            KeyCode::AltLeft | KeyCode::AltRight => {
                let bit = if event.code == KeyCode::AltLeft { 1 } else { 2 };
                if event.pressed {
                    self.lane_input.alt_keys |= bit;
                    self.lane_entry_cancel();
                } else {
                    self.lane_input.alt_keys &= !bit;
                }
            }
            KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight => {
                if event.pressed {
                    self.lane_entry_cancel();
                }
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                let bit = if event.code == KeyCode::ShiftLeft { 1 } else { 2 };
                if event.pressed {
                    self.lane_input.shift_keys |= bit;
                } else {
                    self.lane_input.shift_keys &= !bit;
                }
                let modified = self.lane_input.shift_keys != 0;
                if let Some(input) = self.lane_input.model.as_mut() {
                    if let Err(e) = input.modifier(&self.workspace.file().session, modified) {
                        self.lane_entry_error(e);
                    }
                }
            }
            _ => {}
        }
        if matches!(event.code, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
            self.prepare_lane_draft_policy();
        }
        if self.panel.is_some() || self.help || self.minimap_focused() || self.preview.is_some() {
            self.lane_entry_cancel();
        }
        if !event.pressed {
            let Some(lane) = lane_for_key(event.code) else {
                return false;
            };
            if !self
                .lane_input
                .model
                .as_ref()
                .is_some_and(|input| input.is_held(lane))
            {
                return false;
            }
            let mut input = self.lane_input.model.take().expect("held lane input");
            match self.workspace.release_lane_input(&mut input, lane) {
                Ok(changed) => {
                    self.lane_input.model = Some(input);
                    if changed {
                        self.lane_entry_rebind_position();
                        self.refresh();
                    }
                }
                Err(e) => self.lane_entry_error(e),
            }
            return true;
        }
        if event.code == KeyCode::Escape && self.lane_entry_cancel() {
            return true;
        }
        let keeps_gesture = lane_for_key(event.code).is_some()
            || matches!(
                event.code,
                KeyCode::ShiftLeft
                    | KeyCode::ShiftRight
                    | KeyCode::ArrowUp
                    | KeyCode::ArrowDown
                    | KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::KeyP
                    | KeyCode::Space
                    | KeyCode::KeyA
            );
        if !keeps_gesture {
            self.lane_entry_cancel();
        }
        false
    }
    /// Called before the legacy stop_audio/Place route. Explicit tools retain
    /// that route; source entry gestures start from the default Tap tool.
    pub(super) fn lane_entry_key(&mut self, event: &RawKeyboardEvent) -> bool {
        let Some(lane) = lane_for_key(event.code) else {
            return false;
        };
        if self.workspace.tool() != Kind::Tap
            || self.control
            || !self.workspace.has_chart()
            || self.preview.is_some()
        {
            return false;
        }
        if lane >= self.workspace.grid().columns() {
            return true;
        }
        if self.lane_input.alt_keys != 0 || self.shift && self.lane_input.shift_keys == 0 {
            return true;
        }
        if event.repeat {
            return true;
        }
        let row = self.lane_entry_row();
        let request = Press {
            lane,
            player: self.workspace.player(),
            row,
            snap_rows: SNAP_ROWS[self.workspace.snap()],
            modified: self.lane_input.shift_keys != 0,
            playing: self.is_playing(),
            advance_by_snap: self.workspace.auto_advance(),
        };
        let current = self.lane_input.model.take().filter(|input| {
            input.chart() == self.workspace.chart()
                && input.is_current(&self.workspace.file().session)
        });
        let prepared = match current {
            Some(input) => Ok(input),
            None => self.workspace.capture_lane_input(),
        };
        let mut input = match prepared {
            Ok(input) => input,
            Err(e) => {
                self.lane_entry_error(e);
                return true;
            }
        };
        match self.workspace.press_lane_input(&mut input, request) {
            Ok(outcome) => {
                self.lane_input.model = Some(input);
                self.lane_input.last_row = Some(row);
                if outcome.changed {
                    self.lane_entry_rebind_position();
                }
                if let Some(row) = outcome.advance_to {
                    self.command(Command::Seek(row));
                } else {
                    self.refresh();
                }
            }
            Err(e) => self.lane_entry_error(e),
        }
        true
    }
    fn lane_entry_rebind_position(&mut self) {
        self.focus_position = self.lane_input.last_row.and_then(|row| {
            deadsync_editor::chart_focus::Position::at_fractional_row(
                &self.workspace.file().session,
                self.workspace.chart(),
                row,
            )
            .ok()
        });
    }
    pub(super) fn lane_entry_reconcile(&mut self) {
        let Some(input) = self.lane_input.model.as_ref() else {
            return;
        };
        if self.panel.is_some()
            || self.help
            || self.minimap_focused()
            || self.preview.is_some()
            || self.workspace.active_chart() != Some(input.chart())
            || !input.is_current(&self.workspace.file().session)
        {
            self.lane_entry_cancel();
            return;
        }
        let row = self.lane_entry_row();
        if self.lane_input.last_row == Some(row) {
            return;
        }
        let player = self.workspace.player();
        let modified = self.lane_input.shift_keys != 0;
        let result = self
            .lane_input
            .model
            .as_mut()
            .expect("checked lane model")
            .move_to(&self.workspace.file().session, row, player, modified);
        if let Err(e) = result {
            self.lane_entry_error(e);
        } else {
            self.lane_input.last_row = Some(row);
        }
    }
    pub(super) fn lane_entry_command(&mut self, command: Command) {
        if !matches!(
            command,
            Command::Move(_)
                | Command::Seek(_)
                | Command::PreviousMeasure
                | Command::NextMeasure
                | Command::PreviousLabel
                | Command::NextLabel
                | Command::SelectPlayer(_)
                | Command::SetSnap(_)
                | Command::ToggleSelection
                | Command::ToggleAutoAdvance
        ) {
            self.lane_entry_cancel();
        }
    }
}

impl State {
    pub(super) fn lane_entry_model(&self) -> Option<&Inputs> {
        self.lane_input.model.as_ref().filter(|input| {
            self.workspace.active_chart() == Some(input.chart())
                && input.is_current(&self.workspace.file().session)
        })
    }
    pub(super) fn lane_entry_overlay_budget(&self) -> usize {
        self.lane_entry_model()
            .filter(|input| {
                input.drafts().next().is_some() || input.deleted_heads().next().is_some()
            })
            .map_or(0, |_| self.view_note_budget().min(512))
    }
    /// A bounded detached overlay. True requests the existing partial-view notice.
    pub(super) fn lane_entry_actors(
        &self,
        actors: &mut Vec<Actor>,
        projection: &Projection<'_>,
        geometry: Geometry,
        viewport: Viewport,
        budget: usize,
    ) -> bool {
        let Some(input) = self.lane_entry_model() else {
            return false;
        };
        let layout =
            &self.workspace.file().session.document().charts()[self.workspace.chart()].steps_type;
        let size = self.zoom.size_zoom() as f32;
        let mut strips = budget;
        let mut partial = false;
        for draft in input.drafts() {
            let Some(fake) = self.lane_draft_fake(draft) else {
                partial = true;
                continue;
            };
            let x = geometry.left + (f32::from(draft.position.lane) + 0.5) * geometry.lane_width;
            if !geometry.visible_x(x) {
                continue;
            }
            let Some(y) = projection.note_y(draft.position.row) else {
                partial = true;
                continue;
            };
            if let Some(tail) = draft.end {
                let Some(tail_y) = projection.note_y(tail) else {
                    partial = true;
                    continue;
                };
                match super::hold_render::chart_render_draft(
                    super::hold_render::ChartRequest {
                        layout,
                        row: draft.position.row,
                        lane: draft.position.lane,
                        player: draft.position.player as usize,
                        selected: false,
                        roll: draft.kind == Kind::Roll,
                        active: false,
                        center_x: x,
                        head_center: y,
                        tail_center: tail_y,
                        width: geometry.lane_width * size,
                        reverse: self.reverse_display,
                        viewport: [
                            geometry.clip_left,
                            viewport.top,
                            geometry.clip_right,
                            viewport.bottom,
                        ],
                        cutoff: None,
                        size_zoom: self.zoom.size_zoom(),
                        body_budget: strips,
                    },
                    &self.hold_colors,
                    self.signatures.as_ref(),
                ) {
                    Ok(output) => {
                        strips = strips.saturating_sub(output.body_strips);
                        partial |= output.partial;
                        actors.extend(output.actors);
                        if fake {
                            match super::short_note_render::fake_marker(
                                [x, y as f32],
                                geometry.lane_width * size,
                                0.8,
                            ) {
                                Ok(mut actor) => match super::hold_render::clip_world_sprite(
                                    &mut actor,
                                    [
                                        geometry.clip_left,
                                        viewport.top,
                                        geometry.clip_right,
                                        viewport.bottom,
                                    ],
                                ) {
                                    Ok(true) => actors.push(actor),
                                    Ok(false) => {}
                                    Err(_) => partial = true,
                                },
                                Err(_) => partial = true,
                            }
                        }
                    }
                    Err(_) => partial = true,
                }
            } else {
                let color = if draft.kind == Kind::Mine {
                    self.hold_colors.mine(layout, draft.position.player, false)
                } else {
                    self.hold_colors
                        .color_row(draft.position.row, self.signatures.as_ref())
                        .and_then(|row| {
                            self.hold_colors.tap(
                                layout,
                                row,
                                draft.position.lane,
                                draft.position.player,
                                false,
                            )
                        })
                };
                let result = color
                    .map_err(|_| "Invalid draft note color")
                    .and_then(|fill| {
                        super::short_note_render::render(super::short_note_render::Request {
                            layout,
                            lane: draft.position.lane,
                            kind: if draft.kind == Kind::Mine {
                                super::short_note_render::Kind::Mine
                            } else {
                                super::short_note_render::Kind::Tap
                            },
                            selected: false,
                            fake,
                            center: [x, y as f32],
                            width: geometry.lane_width * size,
                            fill,
                            alpha: 0.8,
                        })
                    });
                match result {
                    Ok(output) => {
                        for mut actor in output {
                            match super::hold_render::clip_world_sprite(
                                &mut actor,
                                [
                                    geometry.clip_left,
                                    viewport.top,
                                    geometry.clip_right,
                                    viewport.bottom,
                                ],
                            ) {
                                Ok(true) => actors.push(actor),
                                Ok(false) => {}
                                Err(_) => partial = true,
                            }
                        }
                    }
                    Err(_) => partial = true,
                }
            }
        }
        partial
    }
}
