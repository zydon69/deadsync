//! Chart-wide replacement of the 13 event types exposed by original GrooveAuthor.
//! Lane notes are deliberately outside this operation. A captured plan is bound
//! to one session revision and publishes all destinations as one history gesture.
use crate::{
    Error,
    event_clipboard::{Clipboard, Event, Selection, TimingKind},
    history::{RevisionToken, Session},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    Signature,
    Tempo,
    Stop,
    Delay,
    Warp,
    Scroll,
    Speed,
    Fake,
    Combo,
    Tick,
    Label,
    Attack,
    Pattern,
}
impl EventType {
    pub const ALL: [Self; 13] = [
        Self::Signature,
        Self::Tempo,
        Self::Stop,
        Self::Delay,
        Self::Warp,
        Self::Scroll,
        Self::Speed,
        Self::Fake,
        Self::Combo,
        Self::Tick,
        Self::Label,
        Self::Attack,
        Self::Pattern,
    ];
    fn of(event: Event) -> Option<Self> {
        Some(match event {
            Event::Pattern { .. } => Self::Pattern,
            Event::Attack { .. } => Self::Attack,
            Event::Note(_) | Event::SongWidget(_) => return None,
            Event::Timing { kind, .. } => match kind {
                TimingKind::Signature => Self::Signature,
                TimingKind::Tempo => Self::Tempo,
                TimingKind::Stop => Self::Stop,
                TimingKind::Delay => Self::Delay,
                TimingKind::Warp => Self::Warp,
                TimingKind::Scroll => Self::Scroll,
                TimingKind::Speed => Self::Speed,
                TimingKind::Fake => Self::Fake,
                TimingKind::Combo => Self::Combo,
                TimingKind::Tick => Self::Tick,
                TimingKind::Label => Self::Label,
            },
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub changed: bool,
    pub destinations_changed: usize,
    pub patterns_pasted: usize,
    pub patterns_skipped: usize,
    /// Selected chart-independent types already shared by source and destinations
    /// in SM / legacy SSC. Their source values are never rewritten as chart data.
    pub shared_types_unchanged: BTreeSet<EventType>,
}
#[derive(Clone)]
pub struct Plan {
    token: RevisionToken,
    source: usize,
    destinations: Vec<usize>,
    types: BTreeSet<EventType>,
    shared: BTreeSet<EventType>,
    clipboard: Option<Clipboard>,
    origin: u64,
}
impl Plan {
    pub fn capture(
        session: &mut Session,
        source: usize,
        destinations: impl IntoIterator<Item = usize>,
        types: impl IntoIterator<Item = EventType>,
    ) -> Result<Self, Error> {
        let mut types: BTreeSet<_> = types.into_iter().collect();
        if types.is_empty() {
            return Err(invalid("Choose at least one event type"));
        }
        let destinations: BTreeSet<_> = destinations.into_iter().collect();
        if destinations.is_empty() {
            return Err(invalid("Choose at least one destination chart"));
        }
        let charts = session.document().charts().len();
        if source >= charts
            || destinations
                .iter()
                .any(|chart| *chart >= charts || *chart == source)
        {
            return Err(invalid(
                "Chart-copy source and destinations must be distinct existing charts",
            ));
        }
        let shared:BTreeSet<_>=types.iter().copied().filter(|kind| {
            if *kind==EventType::Pattern {false}
            else if *kind==EventType::Attack {!crate::attacks::supports_resident_chart_attacks(session.document())}
            else {!crate::timing_edit::supports_resident_chart_timing(session.document())}
        }).collect();
        types.retain(|kind| !shared.contains(kind));
        let selected = selected(session, source, &types)?;
        let origin = selected
            .events()
            .iter()
            .enumerate()
            .filter(|(index, _)| selected.is_selected(*index))
            .map(|(_, event)| event.row())
            .min()
            .unwrap_or(0);
        let clipboard = selected.copy(session)?;
        Ok(Self {
            token: session.revision_token(),
            source,
            destinations: destinations.into_iter().collect(),
            types,
            shared,
            clipboard,
            origin,
        })
    }
    pub fn apply(self, session: &mut Session) -> Result<Report, Error> {
        if !session.accepts_token(&self.token) {
            return Err(invalid(
                "Chart-copy plan belongs to another session or revision",
            ));
        }
        let mut report = Report {
            shared_types_unchanged: self.shared.clone(),
            ..Report::default()
        };
        if self.types.is_empty() {
            return Ok(report);
        }
        report.changed = session.edit_event_properties_atomic(
            self.source,
            "Copy event types between charts",
            |staged| {
                for &chart in &self.destinations {
                    let changed = staged.edit_event_properties_atomic(
                        chart,
                        "Replace chart event types",
                        |staged| {
                            // Initial BPM/signature nodes are protected by typed deletion.
                            selected(staged, chart, &self.types)?.delete(staged)?;
                            if let Some(clipboard) = &self.clipboard {
                                let pasted =
                                    clipboard.paste_with_report(staged, chart, self.origin, 0)?;
                                report.patterns_pasted += pasted.patterns_pasted;
                                report.patterns_skipped += pasted.patterns_skipped;
                            }
                            Ok(())
                        },
                    )?;
                    if changed {
                        report.destinations_changed += 1;
                    }
                }
                Ok(())
            },
        )?;
        if report.changed{session.wrap_chart_event_copy();}
        Ok(report)
    }
}
fn selected(
    session: &mut Session,
    chart: usize,
    types: &BTreeSet<EventType>,
) -> Result<Selection, Error> {
    let mut selection = Selection::capture(session, chart, 0, i32::MAX as u64)?;
    selection.select_all(false);
    for index in 0..selection.events().len() {
        if EventType::of(selection.events()[index]).is_some_and(|kind| types.contains(&kind)) {
            selection.set_selected(index, true)?;
        }
    }
    Ok(selection)
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

/// Closed prospective presentation, independent of the focused source chart.
pub mod live { pub use crate::history::chart_copy_live::{Plan,Application,Receipt}; }
