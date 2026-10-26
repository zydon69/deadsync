//! Resident GUID-owned autoplay/receptor state. Audio positions come only from
//! the validated transport gate; animation seconds are the host UI frame clock.
use super::{State, lane_for_key, receptor_render, view_layout::Geometry, view_render::Viewport};
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error, autoplay,
    chart_order::{Binding, SessionBinding},
    generation::metadata::Guid,
    history::Session,
    notes::Position,
    playback::Clock,
    receptors,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
struct Slot {
    binding: Binding,
    chart: usize,
    layout: String,
    clock: Arc<Clock>,
    index: Result<autoplay::Timeline, String>,
    autoplay: autoplay::State,
    animations: Vec<receptors::State>,
    visuals: Vec<receptors::Visual>,
}
#[derive(Default)]
pub(super) struct Runtime {
    slots: BTreeMap<Guid, Slot>,
    owner: Option<SessionBinding>,
    order: Vec<Guid>,
    focus: Option<Guid>,
    transport: Option<(Option<u64>, f64)>,
    now: f64,
    modifiers: u8,
}
pub(super) struct Policy<'a> {
    timeline: &'a autoplay::Timeline,
    session: &'a Session,
    chart: usize,
    chart_time: f64,
}
impl Policy<'_> {
    pub fn presentation(&self, position: Position) -> Option<autoplay::Presentation> {
        self.timeline
            .presentation(
                self.session,
                self.chart,
                position,
                self.chart_time,
                true,
                true,
                true,
            )
            .ok()
            .flatten()
    }
}
fn idle() -> receptors::Visual {
    receptors::Visual {
        brightness: 0.8,
        tap_scale: 1.0,
        rim_scale: 1.0,
        rim_alpha: 0.0,
        glow_alpha: 0.0,
    }
}
impl Runtime {
    fn prepare(
        &mut self,
        session: &Session,
        sources: Vec<(usize, Guid, Arc<Clock>)>,
        focused: Option<Guid>,
    ) -> Result<(), Error> {
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| !owner.belongs_to(session))
        {
            self.slots.clear();
            self.order.clear();
            self.focus = None;
            self.transport = None;
        }
        self.owner = Some(SessionBinding::capture(session));
        let keep: BTreeSet<_> = sources.iter().map(|(_, id, _)| id.clone()).collect();
        self.slots.retain(|id, _| keep.contains(id));
        self.order = sources.iter().map(|(_, id, _)| id.clone()).collect();
        for (chart, id, clock) in sources {
            if self
                .slots
                .get(&id)
                .is_some_and(|slot| slot.binding.is_current(session, chart))
            {
                continue;
            }
            let columns = deadsync_editor::notes::layout_columns(
                &session.document().charts()[chart].steps_type,
            )
            .ok_or_else(|| Error::Unsupported("Unsupported receptor chart".into()))?;
            let layout = session.document().charts()[chart].steps_type.clone();
            let mut animations = vec![receptors::State::default(); usize::from(columns)];
            let mut visuals = vec![idle(); usize::from(columns)];
            if let Some(mut previous) = self.slots.remove(&id).filter(|slot| {
                slot.layout == layout && slot.animations.len() == usize::from(columns)
            }) {
                // Editing the note just pressed must retain manual feedback on
                // this resident chart; invalidate only its autoplay sequence.
                for action in previous.autoplay.reset() {
                    let _ = previous.animations[usize::from(action.lane)].autoplay(
                        action.kind,
                        self.now,
                        0.0,
                        receptors::Preferences::default(),
                    );
                }
                animations = previous.animations;
                visuals = previous.visuals;
            }
            self.slots.insert(
                id,
                Slot {
                    binding: Binding::capture(session, chart)?,
                    chart,
                    layout,
                    clock,
                    index: autoplay::Timeline::capture(session, chart)
                        .map_err(|error| error.to_string()),
                    autoplay: autoplay::State::new(columns)?,
                    animations,
                    visuals,
                },
            );
        }
        if self.focus != focused {
            self.release_manual();
            self.focus = focused;
        }
        if let Some(error) = self
            .order
            .iter()
            .filter_map(|id| self.slots.get(id))
            .find_map(|slot| slot.index.as_ref().err())
        {
            return Err(Error::InvalidEdit(error.clone()));
        }
        Ok(())
    }
    fn slot(&self, session: &Session, chart: usize) -> Option<&Slot> {
        session
            .chart_ids()
            .get(chart)
            .and_then(|id| self.slots.get(id))
            .filter(|slot| slot.binding.is_current(session, chart))
    }
    fn slot_mut(&mut self, session: &Session, chart: usize) -> Option<&mut Slot> {
        session
            .chart_ids()
            .get(chart)
            .and_then(|id| self.slots.get_mut(id))
            .filter(|slot| slot.binding.is_current(session, chart))
    }
    pub fn reset(&mut self) {
        let preferences = receptors::Preferences::default();
        for slot in self.slots.values_mut() {
            for action in slot.autoplay.reset() {
                let _ = slot.animations[usize::from(action.lane)].autoplay(
                    action.kind,
                    self.now,
                    action.time_delta,
                    preferences,
                );
            }
        }
        self.transport = None;
    }
    pub fn release_input(&mut self) {
        self.modifiers = 0;
        self.release_manual();
    }
    fn observe_modifier(&mut self, event: &RawKeyboardEvent) {
        let bit = match event.code {
            KeyCode::ControlLeft => 1,
            KeyCode::ControlRight => 2,
            KeyCode::SuperLeft => 4,
            KeyCode::SuperRight => 8,
            KeyCode::ShiftLeft => 16,
            KeyCode::ShiftRight => 32,
            KeyCode::AltLeft => 64,
            KeyCode::AltRight => 128,
            _ => return,
        };
        if event.pressed {
            self.modifiers |= bit;
        } else {
            self.modifiers &= !bit;
        }
    }
    pub fn release_manual(&mut self) {
        for slot in self.slots.values_mut() {
            for animation in &mut slot.animations {
                let _ = animation.manual(false, self.now, receptors::Preferences::default());
            }
        }
    }
    fn update(
        &mut self,
        session: &Session,
        transport: Option<(Option<u64>, f64)>,
        music_time: f64,
        now: f64,
    ) {
        // A new/paused/stopped decoder identity and backwards seek cancel old
        // lane transitions. Explicit navigation also resets synchronously.
        let discontinuity = match (self.transport, transport) {
            (Some((old_generation, old_time)), Some((generation, time))) => {
                old_generation != generation || time < old_time
            }
            (Some(_), None) => true,
            _ => false,
        };
        if discontinuity {
            // Cancel at the last observed UI instant. Advancing the frame clock
            // first would restart the release fade after an entire paused gap.
            self.reset();
        }
        self.now = now;
        self.transport = transport;
        let preferences = receptors::Preferences::default();
        for id in &self.order {
            let Some(slot) = self.slots.get_mut(id) else {
                continue;
            };
            if !slot.binding.is_current(session, slot.chart) {
                continue;
            }
            if let Some((_, music)) = transport {
                let actions = slot.clock.music_to_chart_time(music).and_then(|time| {
                    slot.index
                        .as_ref()
                        .map_err(|error| Error::InvalidEdit(error.clone()))
                        .and_then(|index| index.next_inputs(session, slot.chart, time))
                        .and_then(|next| slot.autoplay.update(time, &next))
                });
                match actions {
                    Ok(actions) => {
                        for action in actions {
                            let _ = slot.animations[usize::from(action.lane)].autoplay(
                                action.kind,
                                now,
                                action.time_delta,
                                preferences,
                            );
                        }
                    }
                    Err(_) => {
                        for action in slot.autoplay.reset() {
                            let _ = slot.animations[usize::from(action.lane)].autoplay(
                                action.kind,
                                now,
                                0.0,
                                preferences,
                            );
                        }
                    }
                }
            }
            let row = slot.clock.position(music_time).unwrap_or(0.0);
            for (animation, visual) in slot.animations.iter_mut().zip(&mut slot.visuals) {
                *visual = animation
                    .sample(now, transport.is_some(), row, preferences)
                    .unwrap_or_else(|_| idle());
            }
        }
    }
    fn policy<'a>(
        &'a self,
        session: &'a Session,
        chart: usize,
        transport: Option<(Option<u64>, f64)>,
    ) -> Option<Policy<'a>> {
        let (_, music) = transport?;
        let slot = self.slot(session, chart)?;
        Some(Policy {
            timeline: slot.index.as_ref().ok()?,
            session,
            chart,
            chart_time: slot.clock.music_to_chart_time(music).ok()?,
        })
    }
}
impl State {
    pub(super) fn prepare_receptors(&mut self) {
        let session = &self.workspace.file().session;
        let mut sources = Vec::new();
        if let Some(views) = self
            .active_views
            .as_ref()
            .filter(|views| views.belongs_to(session))
        {
            for view in views.entries() {
                if let Ok(clock) = view.clock(session) {
                    sources.push((view.source_index(), view.identity().clone(), clock));
                }
            }
        }
        if let Some(chart) = self
            .workspace
            .active_chart()
            .filter(|chart| !sources.iter().any(|(source, _, _)| source == chart))
        {
            if let Ok(clock) = Clock::capture(session, chart) {
                sources.push((chart, session.chart_ids()[chart].clone(), Arc::new(clock)));
            }
        }
        let focused = self
            .workspace
            .active_chart()
            .and_then(|chart| session.chart_ids().get(chart))
            .cloned();
        if let Err(error) = self.receptor_views.prepare(session, sources, focused) {
            self.status =
                crate::i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
        }
    }
    pub(super) fn update_receptors(&mut self, now: f64) {
        let transport = self.chart_autoplay_transport();
        let session = &self.workspace.file().session;
        let music_time = transport
            .map(|(_, music)| music)
            .or_else(|| {
                let slot = self
                    .receptor_views
                    .slot(session, self.workspace.active_chart()?)?;
                if let Some((chart_time, _)) = self.playback_position() {
                    slot.clock.chart_time_to_music(chart_time).ok()
                } else {
                    slot.clock.music_time(self.workspace.row()).ok()
                }
            })
            .unwrap_or(0.0);
        self.receptor_views
            .update(session, transport, music_time, now);
    }
    pub(super) fn receptor_modifiers(&mut self, event: &RawKeyboardEvent) {
        self.receptor_views.observe_modifier(event);
    }
    pub(super) fn receptor_manual_input(&mut self, event: &RawKeyboardEvent) {
        let Some(lane) = lane_for_key(event.code) else {
            return;
        };
        let now = self.receptor_views.now;
        // Releases always clear existing feedback, including after opening a
        // panel or pressing a modifier while the lane key was held.
        if !event.pressed {
            for slot in self.receptor_views.slots.values_mut() {
                if let Some(animation) = slot.animations.get_mut(usize::from(lane)) {
                    let _ = animation.manual(false, now, receptors::Preferences::default());
                }
            }
            return;
        }
        if event.repeat
            || self.help
            || self.panel.is_some()
            || self.control
            || self.shift
            || self.receptor_views.modifiers != 0
            || self.minimap_focused()
        {
            return;
        }
        let Some(chart) = self.workspace.active_chart() else {
            return;
        };
        if let Some(slot) = self
            .receptor_views
            .slot_mut(&self.workspace.file().session, chart)
        {
            if let Some(animation) = slot.animations.get_mut(usize::from(lane)) {
                let _ = animation.manual(true, now, receptors::Preferences::default());
            }
        }
    }
    pub(super) fn receptor_policy(&self, chart: usize) -> Option<Policy<'_>> {
        // A generation change before the next UI update must not consume notes
        // using stale autoplay state or a merely queued play request.
        if self.preview.is_some() && self.workspace.active_chart() == Some(chart) {
            return None;
        }
        self.receptor_views.policy(
            &self.workspace.file().session,
            chart,
            self.chart_autoplay_transport(),
        )
    }
    pub(super) fn receptor_actors(
        &self,
        actors: &mut Vec<Actor>,
        geometry: Geometry,
        viewport: Viewport,
    ) {
        let session = &self.workspace.file().session;
        let preview = self.preview.as_ref().filter(|_| geometry.focused);
        let layout = preview
            .map(super::GenerationPreview::layout)
            .unwrap_or_else(|| &session.document().charts()[geometry.source].steps_type);
        let columns = deadsync_editor::notes::layout_columns(layout).unwrap_or(0);
        if let Some(slot) = self
            .receptor_views
            .slot(session, geometry.source)
            .filter(|slot| preview.is_none() && slot.visuals.len() == usize::from(columns))
        {
            let _ = receptor_render::actors(
                actors,
                layout,
                geometry,
                viewport,
                self.zoom.size_zoom(),
                &slot.visuals,
            );
        } else {
            let fallback = vec![idle(); usize::from(columns)];
            let _ = receptor_render::actors(
                actors,
                layout,
                geometry,
                viewport,
                self.zoom.size_zoom(),
                &fallback,
            );
        }
    }
}
