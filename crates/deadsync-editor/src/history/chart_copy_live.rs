//! Closed chart-copy presentation: source chart and focused chart are independent.
use super::{ApplicationReviewToken, Change, ContentToken, RevisionToken, Session};
use crate::{
    Error,
    chart_event_copy::{EventType, Report},
    chart_focus::Position,
    generation::{
        metadata::Guid,
        timing::{EventKind, GenerationTiming},
    },
    timing_edit::live::Anchor,
};
enum Operation {
    Copy(crate::chart_event_copy::Plan),
    History(bool),
}
pub struct Plan {
    review: ApplicationReviewToken,
    revision: RevisionToken,
    focus: Option<usize>,
    position: Option<Position>,
    operation: Operation,
}
/// Content-only receipt. It does not claim unchanged chart timing or audio.
pub struct Receipt {
    before: ContentToken,
    after: ContentToken,
    charts: Vec<Guid>,
}
impl Receipt {
    pub(crate) fn rebind_token(
        &self,
        before: &ContentToken,
        session: &Session,
    ) -> Result<ContentToken, Error> {
        if !std::sync::Arc::ptr_eq(&before.identity, &self.before.identity)
            || before.revision != self.before.revision
            || !session.accepts_content_token(&self.after)
            || self.charts != session.chart_ids()
        {
            return Err(Error::ExternalChange);
        }
        Ok(self.after.clone())
    }
}
pub struct Application {
    report: Report,
    position: Option<Position>,
    receipt: Receipt,
}
impl Application {
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    pub fn into_position(self) -> Option<Position> {
        self.position
    }
}
fn invalid() -> Error {
    Error::ExternalChange
}
fn copy(change: &Change) -> Result<Change, Error> {
    Ok(match change {
        Change::Value {
            range,
            before,
            after,
        } => Change::Value {
            range: range.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AddedProperty {
            chart,
            name,
            before,
            after,
        } => Change::AddedProperty {
            chart: *chart,
            name: name.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AttackIdentity(value) => Change::AttackIdentity(value.clone()),
        Change::PatternReadd { chart } => Change::PatternReadd { chart: *chart },
        Change::MarkerSeedTouch { chart } => Change::MarkerSeedTouch { chart: *chart },
        Change::ChartEventCopy(change) => Change::ChartEventCopy(Box::new(copy(change)?)),
        Change::Group(changes) => {
            Change::Group(changes.iter().map(copy).collect::<Result<_, _>>()?)
        }
        _ => return Err(invalid()),
    })
}
fn time_events(s: &Session, chart: usize) -> Result<Vec<crate::generation::timing::Event>, Error> {
    Ok(GenerationTiming::from_document(s.document(), chart)?
        .editor_events()
        .iter()
        .copied()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::Tempo(_) | EventKind::Stop(_) | EventKind::Delay(_) | EventKind::Warp(_)
            )
        })
        .collect())
}
fn unchanged_clock(before: &Session, after: &Session, chart: usize) -> Result<bool, Error> {
    Ok(time_events(before, chart)? == time_events(after, chart)?
        && crate::chart_properties::Draft::capture(before, chart)?
            .effective_music_offset()
            .to_bits()
            == crate::chart_properties::Draft::capture(after, chart)?
                .effective_music_offset()
                .to_bits()
        && crate::song_properties::sync_offset(before.document())?.to_bits()
            == crate::song_properties::sync_offset(after.document())?.to_bits())
}
// This is a retention API, not an independent navigation request. Keeping
// the current canonical Position lets unchanged clocks preserve a stop-interior
// time or a selected equal-time warp row. An Anchor payload must describe that
// same current Position; silently substituting it would misrepresent the API.
fn validate_focus(before: &Session, focus: Option<&(Position, Anchor)>) -> Result<(), Error> {
    if let Some((old, anchor)) = focus {
        if !old.is_bound_to(before, old.chart()) {
            return Err(invalid());
        }
        let (requested, current) = match anchor {
            Anchor::MusicTime(time) => (*time, old.music_time()),
            Anchor::Row(row) => (*row, old.row()),
        };
        if !requested.is_finite() || requested.to_bits() != current.to_bits() {
            return Err(invalid());
        }
    }
    Ok(())
}
fn position(
    before: &Session,
    after: &Session,
    focus: Option<&(Position, Anchor)>,
) -> Result<Option<Position>, Error> {
    validate_focus(before, focus)?;
    focus
        .map(|(old, anchor)| {
            let time = || Position::at_time(after, old.chart(), old.music_time());
            let row = || Position::at_fractional_row(after, old.chart(), old.row());
            if unchanged_clock(before, after, old.chart())? {
                // Preserve stop-interior time or a chosen equal-time warp row through actual
                // clock constructors, never by minting an unchecked row/time tuple.
                let inverse = time()?;
                if inverse.row().to_bits() == old.row().to_bits()
                    && inverse.chart_time().to_bits() == old.chart_time().to_bits()
                {
                    return Ok(inverse);
                }
                let forward = row()?;
                if forward.music_time().to_bits() == old.music_time().to_bits()
                    && forward.chart_time().to_bits() == old.chart_time().to_bits()
                {
                    return Ok(forward);
                }
                return Err(invalid());
            }
            match anchor {
                Anchor::MusicTime(_) => {
                    let forward = row()?;
                    if forward.music_time().to_bits() == old.music_time().to_bits() {
                        Ok(forward)
                    } else {
                        time()
                    }
                }
                Anchor::Row(_) => row(),
            }
        })
        .transpose()
}
impl Plan {
    /// Copy while retaining the supplied current focus. The Anchor selects
    /// music-time or row retention; its finite payload must match that Position
    /// bit-for-bit. A different payload is navigation and is refused.
    pub fn prepare(
        session: &mut Session,
        source: usize,
        destinations: impl IntoIterator<Item = usize>,
        types: impl IntoIterator<Item = EventType>,
        focus: Option<(Position, Anchor)>,
    ) -> Result<Self, Error> {
        validate_focus(session, focus.as_ref())?;
        let plan = crate::chart_event_copy::Plan::capture(session, source, destinations, types)?;
        let mut staged = super::timing_live::presentation_candidate(session)?;
        plan.clone().apply(&mut staged)?;
        let projected = position(session, &staged, focus.as_ref())?;
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            focus: focus.as_ref().map(|(position, _)| position.chart()),
            position: projected,
            operation: Operation::Copy(plan),
        })
    }
    /// Undo/Redo with the same current-focus retention contract as `prepare`.
    /// A divergent Anchor payload is refused before consuming history.
    pub fn prepare_history(
        session: &Session,
        focus: Option<(Position, Anchor)>,
        redo: bool,
    ) -> Result<Self, Error> {
        validate_focus(session, focus.as_ref())?;
        let entry = (if redo {
            session.redo.last()
        } else {
            session.undo.last()
        })
        .filter(|entry| matches!(entry.change, Change::ChartEventCopy(_)))
        .ok_or_else(invalid)?;
        let mut staged = super::timing_live::presentation_candidate(session)?;
        staged.apply(&mut copy(&entry.change)?, redo)?;
        staged.revision = if redo {
            entry.after_revision
        } else {
            entry.before_revision
        };
        staged.content_revision = if redo {
            entry.after_content_revision
        } else {
            entry.before_content_revision
        };
        let projected = position(session, &staged, focus.as_ref())?;
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            focus: focus.as_ref().map(|(position, _)| position.chart()),
            position: projected,
            operation: Operation::History(redo),
        })
    }
    pub fn chart(&self) -> Option<usize> {
        self.focus
    }
    pub fn apply(self, session: &mut Session) -> Result<Application, Error> {
        if !session.accepts_application_review(&self.review)
            || !session.accepts_token(&self.revision)
        {
            return Err(invalid());
        }
        let before = session.content_token();
        let charts = session.chart_ids().to_vec();
        let report = match self.operation {
            Operation::Copy(plan) => plan.apply(session)?,
            Operation::History(redo) => Report {
                changed: if redo {
                    session.redo()?
                } else {
                    session.undo()?
                },
                ..Report::default()
            },
        };
        debug_assert!(
            self.position
                .as_ref()
                .is_none_or(|p| p.is_bound_to(session, p.chart()))
        );
        Ok(Application {
            report,
            position: self.position,
            receipt: Receipt {
                before,
                after: session.content_token(),
                charts,
            },
        })
    }
}
impl Session {
    pub(crate) fn wrap_chart_event_copy(&mut self) {
        let entry = self.undo.last_mut().expect("successful chart copy");
        let change = std::mem::replace(&mut entry.change, Change::Group(Vec::new()));
        entry.change = Change::ChartEventCopy(Box::new(change));
    }
    pub fn history_changes_chart_event_copy(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::ChartEventCopy(_)))
    }
}
