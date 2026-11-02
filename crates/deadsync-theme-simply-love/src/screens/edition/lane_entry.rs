//! Semantic ownership for source press/release note entry. Detached gestures
//! commit on release; source playback/auto-advance and explicit native tools
//! retain their immediate press semantics. Cancellation never commits a draft.
use super::{State, spacing_view::Projection, view_layout::Geometry, view_render::Viewport};
use crate::i18n;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    key_binding_preferences::Action,
    lane_entry::{Inputs, Press},
    notes::Kind,
    workspace::{Command, SNAP_ROWS},
};
#[derive(Default)]
pub(super) struct Runtime {
    pub(super) model: Option<Inputs>,
    pub(super) fake_cache: Option<deadsync_editor::draft_note_policy::Cache>,
    pub(super) fake_error: Option<String>,
    modified: bool,
    owned: std::collections::BTreeSet<u16>,
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
        let owned = !self.lane_input.owned.is_empty();
        self.lane_input.owned.clear();
        let cancelled = self
            .lane_input
            .model
            .take()
            .is_some_and(|mut input| input.cancel() != 0);
        if owned || cancelled {
            self.command_binding_quarantine_held();
            self.receptor_views.release_manual();
        }
        owned || cancelled
    }
    pub(super) fn lane_entry_release_input(&mut self) {
        if !self.lane_input.owned.is_empty() || self.lane_input.modified {
            self.command_binding_quarantine_held();
        }
        self.lane_input = Runtime::default();
        self.receptor_views.release_manual();
    }
    fn lane_entry_error(&mut self, error: Error) {
        self.lane_entry_cancel();
        self.status = i18n::tr_fmt(
            "Edition",
            "Error",
            &[("message", &crate::i18n::editor_error(&error))],
        );
    }
    /// Run before modifier/modal early returns so release cannot remain queued
    /// behind a newly opened panel. True means the key has been consumed.
    pub(super) fn lane_entry_before_key(&mut self, event: &RawKeyboardEvent) -> bool {
        if self.panel.is_some() || self.help || self.minimap_focused() || self.preview.is_some() {
            self.lane_entry_release_input();
        }
        if event.pressed
            && (event.code == KeyCode::Escape || self.command_binding_close_final(event.code))
            && self.lane_entry_cancel()
        {
            // The consumed cancel must never become a later timer activation.
            self.command_binding_quarantine(event.code);
            return true;
        }
        // Arbitrary prefixes are supported by the source registry. An
        // unrelated raw key alone is not a semantic cancellation intent.
        false
    }
    /// One logical activation after native alias coalescence. Physical key
    /// ownership belongs to Registry; this layer owns the detached lane draft.
    pub(super) fn lane_entry_semantic_press(&mut self, action: Action) {
        if action == Action::ArrowModification {
            self.lane_input.modified = true;
            if let Some(input) = self.lane_input.model.as_mut() {
                if let Err(error) = input.modifier(&self.workspace.file().session, true) {
                    self.lane_entry_error(error);
                    return;
                }
            }
            self.prepare_lane_draft_policy();
            return;
        }
        let Some(lane) = action.lane() else {
            return;
        };
        if !self.workspace.has_chart() || self.preview.is_some() {
            return;
        }
        if lane >= self.workspace.grid().columns() {
            return;
        }
        // Preserve native Ctrl/Alt+plain-digit no-op; explicitly configured
        // modifier chords and the original source profile remain executable.
        if !self.command_binding_entry_modifiers_allowed(action) {
            return;
        }
        if self.workspace.tool() != Kind::Tap {
            self.stop_audio();
            self.command(Command::Place(lane));
            self.lane_input.owned.insert(lane);
            self.receptor_manual_lane(lane, true);
            return;
        }
        if !self.lane_input.owned.insert(lane) {
            return;
        }
        self.receptor_manual_lane(lane, true);
        let row = self.lane_entry_row();
        let request = Press {
            lane,
            player: self.workspace.player(),
            row,
            snap_rows: SNAP_ROWS[self.workspace.snap()],
            modified: self.lane_input.modified,
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
                return;
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
    }
    /// Release is never discarded by the command dispatcher. Modal/focus
    /// cancellation clears ownership first, so later releases cannot commit.
    pub(super) fn lane_entry_semantic_release(&mut self, action: Action) {
        if action == Action::ArrowModification {
            self.lane_input.modified = false;
            if let Some(input) = self.lane_input.model.as_mut() {
                if let Err(error) = input.modifier(&self.workspace.file().session, false) {
                    self.lane_entry_error(error);
                    return;
                }
            }
            self.prepare_lane_draft_policy();
            return;
        }
        let Some(lane) = action.lane() else {
            return;
        };
        self.receptor_manual_lane(lane, false);
        self.lane_entry_reconcile();
        if !self.lane_input.owned.remove(&lane) {
            return;
        }
        if self.panel.is_some() || self.help || self.minimap_focused() || self.preview.is_some() {
            self.lane_entry_cancel();
            return;
        }
        let Some(mut input) = self.lane_input.model.take() else {
            return;
        };
        match self.workspace.release_lane_input(&mut input, lane) {
            Ok(changed) => {
                self.lane_input.model = Some(input);
                if changed {
                    self.lane_entry_rebind_position();
                    self.refresh();
                }
            }
            Err(error) => self.lane_entry_error(error),
        }
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
        let modified = self.lane_input.modified;
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
    /// Draw only source-admitted edited identities, in their admitted order.
    pub(super) fn lane_entry_actors(
        &self,
        actors: &mut Vec<Actor>,
        projection: &Projection<'_>,
        geometry: Geometry,
        viewport: Viewport,
        drafts: &[deadsync_editor::lane_entry::Draft],
    ) -> bool {
        let layout =
            &self.workspace.file().session.document().charts()[self.workspace.chart()].steps_type;
        let size = self.zoom.size_zoom() as f32;
        let mut strips = drafts.len();
        let mut partial = false;
        for &draft in drafts {
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
