//! Content-bound mixed inventory shared by native widgets and region selection.
use super::{
    geometry::{self, Range},
    misc_layout::{Kind, Rect},
};
use crate::{
    Error,
    chart_order::Binding,
    event_clipboard::{Event, Selection, TimingKind},
    history::Session,
    options_preferences::RenderPolicy,
    playback::Clock,
};
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub event: Event,
    pub chart_time: f64,
    pub phase: u8,
    pub widget: Option<Kind>,
}
pub struct Inventory {
    binding: Binding,
    selection: Selection,
    entries: Vec<Entry>,
    values: std::collections::BTreeMap<Event, String>,
    event_count: usize,
    rate_count: usize,
    clock: Clock,
    index: crate::spacing_index::Index,
    misc_indices: Vec<usize>,
}
impl Inventory {
    pub fn capture(session: &mut Session, chart: usize) -> Result<Self, Error> {
        Self::capture_internal(session, chart, true)
    }
    pub fn capture_misc(session: &mut Session, chart: usize) -> Result<Self, Error> {
        Self::capture_internal(session, chart, false)
    }
    fn capture_internal(
        session: &mut Session,
        chart: usize,
        include_notes: bool,
    ) -> Result<Self, Error> {
        let mut selection = Selection::capture_inventory(session, chart, include_notes)?;
        selection.include_song_widgets(session)?;
        let clock = Clock::capture(session, chart)?;
        let entries: Vec<Entry> = selection
            .events()
            .iter()
            .copied()
            .map(|event| {
                let (phase, widget) = match event {
                    Event::Note(p) => (
                        match selection.note_kind(p) {
                            Some(crate::notes::Kind::Hold | crate::notes::Kind::Roll) => 10,
                            Some(
                                crate::notes::Kind::Tap
                                | crate::notes::Kind::Fake
                                | crate::notes::Kind::Lift,
                            ) => 8,
                            _ => 12,
                        },
                        None,
                    ),
                    Event::Attack { .. } => (6, Some(Kind::Attack)),
                    Event::Pattern { .. } => (9, Some(Kind::Pattern)),
                    Event::SongWidget(w) => (
                        0,
                        Some(match w.kind() {
                            crate::event_clipboard::WidgetKind::Preview => Kind::Preview,
                            _ => Kind::EndHint,
                        }),
                    ),
                    Event::Timing { kind, .. } => match kind {
                        TimingKind::Signature => (0, Some(Kind::Signature)),
                        TimingKind::Tempo => (1, Some(Kind::Tempo)),
                        TimingKind::Tick => (2, Some(Kind::Tick)),
                        TimingKind::Fake => (3, Some(Kind::Fake)),
                        TimingKind::Combo => (4, Some(Kind::Combo)),
                        TimingKind::Label => (5, Some(Kind::Label)),
                        TimingKind::Delay => (7, Some(Kind::Delay)),
                        TimingKind::Scroll => (13, Some(Kind::Scroll)),
                        TimingKind::Speed => (14, Some(Kind::Speed)),
                        TimingKind::Stop => (15, Some(Kind::Stop)),
                        TimingKind::Warp => (17, Some(Kind::Warp)),
                    },
                };
                let chart_time = match event {
                    Event::SongWidget(w) => w.chart_time(),
                    _ => clock.event_phase_time(event.row(), phase)?,
                };
                Ok(Entry {
                    event,
                    chart_time,
                    phase,
                    widget,
                })
            })
            .collect::<Result<_, Error>>()?;
        let scope = if crate::timing_edit::supports_resident_chart_timing(session.document()) {
            crate::timing_edit::Scope::Chart
        } else {
            crate::timing_edit::Scope::Song
        };
        let timing = crate::timing_edit::clipboard::Clipboard::capture(
            session,
            chart,
            scope,
            0,
            i32::MAX as u64,
        )?;
        let mut values: std::collections::BTreeMap<_, _> = timing.display_values().collect();
        let attack_scope = if crate::attacks::supports_resident_chart_attacks(session.document()) {
            crate::timing_edit::Scope::Chart
        } else {
            crate::timing_edit::Scope::Song
        };
        let attacks = crate::attacks::inspector::Draft::capture(session, chart, attack_scope)?;
        for event in attacks.attacks().events() {
            values.insert(
                Event::Attack { row: event.row },
                event
                    .modifiers
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        let note_events = session
            .grid(chart)?
            .iter()
            .map(|_| usize::from(!include_notes))
            .sum::<usize>();
        let event_count = selection.events().len() + note_events;
        let misc_indices = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.widget.is_some().then_some(i))
            .collect();
        let rate_count = clock.spacing_sections().count();
        let index = crate::spacing_index::Index::new(clock.spacing_sections().collect())?;
        Ok(Self {
            misc_indices,
            values,
            event_count,
            rate_count,
            clock,
            index,
            binding: Binding::capture(session, chart)?,
            selection,
            entries,
        })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.binding.is_current(session, chart)
    }
    pub fn clock(&self) -> &Clock {
        &self.clock
    }
    pub fn spacing_index(&self) -> &crate::spacing_index::Index {
        &self.index
    }
    pub fn display_value(&self, event: Event) -> Option<&str> {
        self.values.get(&event).map(String::as_str)
    }
    pub fn event_count(&self) -> usize {
        self.event_count
    }
    pub fn rate_count(&self) -> usize {
        self.rate_count
    }
    pub fn miscellaneous_indices(&self) -> &[usize] {
        &self.misc_indices
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn choices(&self) -> Selection {
        self.selection.clone()
    }
    /// Misc rectangles must be the same final layout consumed by rendering.
    /// Render budgets never truncate this collection or Shift's interval.
    pub fn collect(
        &self,
        note_axis: Range,
        by_time: bool,
        lanes: (i32, i32),
        columns: u16,
        policy: RenderPolicy,
        alt: bool,
        misc: &[(Event, Rect)],
        screen: Rect,
        mode: crate::selection_preferences::Overlap,
    ) -> Vec<Event> {
        let fully_outside = lanes.1 < 0 || lanes.0 >= i32::from(columns);
        let partially_outside = !fully_outside && (lanes.0 < 0 || lanes.1 >= i32::from(columns));
        let mut select_misc = alt || fully_outside;
        let mut result = Vec::new();
        if !select_misc && policy.notes {
            for entry in &self.entries {
                let Event::Note(p) = entry.event else {
                    continue;
                };
                if geometry::contains_head(
                    note_axis,
                    if by_time {
                        entry.chart_time
                    } else {
                        p.row as f64
                    },
                ) && i32::from(p.lane) >= lanes.0
                    && i32::from(p.lane) <= lanes.1
                {
                    result.push(entry.event);
                }
            }
        }
        if result.is_empty() && partially_outside {
            select_misc = true;
        }
        if select_misc && policy.misc_events {
            result.extend(
                misc.iter()
                    .filter(|(_, r)| geometry::contains_rect(mode, screen, *r))
                    .map(|(event, _)| *event),
            );
        }
        result
    }
}
