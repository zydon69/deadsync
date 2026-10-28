//! Reviewed global row shifts through the existing mixed-event engine.
use crate::{
    Error,
    event_clipboard::Selection,
    event_shift,
    generation::metadata::Guid,
    history::{Session, file_intent::IntentGuard},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Stale,
    MissingReview,
    Invalid,
}
#[derive(Debug)]
pub struct Failure {
    pub reason: Refusal,
    pub error: Error,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            reason: Refusal::Invalid,
            error,
        }
    }
}
fn refuse(reason: Refusal, message: &str) -> Failure {
    Failure {
        reason,
        error: invalid(message),
    }
}
pub struct Plan {
    guard: IntentGuard,
    chart: usize,
    identity: Guid,
    shift: event_shift::Plan,
}
impl Plan {
    pub fn prepare(session: &mut Session, chart: usize, rows: i64) -> Result<Self, Failure> {
        let identity = session
            .chart_ids()
            .get(chart)
            .ok_or_else(|| invalid("ShiftAll chart does not exist"))?
            .clone();
        let mut all = Selection::capture(session, chart, 0, i32::MAX as u64)?;
        all.select_all(true);
        let shift = event_shift::Plan::capture(session, &all, rows)?;
        Ok(Self {
            guard: IntentGuard::capture(session),
            chart,
            identity,
            shift,
        })
    }
    pub fn report(&self) -> &event_shift::Report {
        self.shift.report()
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && self.guard.is_current(session)
    }
    pub fn apply(
        self,
        session: &mut Session,
        chart: usize,
        reviewed: &event_shift::Report,
    ) -> Result<event_shift::Report, Failure> {
        if !self.is_current(session, chart) {
            return Err(refuse(
                Refusal::Stale,
                "ShiftAll belongs to another chart or history lifetime",
            ));
        }
        if reviewed != self.report() {
            return Err(refuse(
                Refusal::MissingReview,
                "Review the exact global shift counts before applying",
            ));
        }
        Ok(self.shift.apply(session)?)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
