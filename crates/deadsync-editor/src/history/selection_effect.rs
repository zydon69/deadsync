//! Typed selection outcomes carried by the exact history entry, never its label.
use super::{RevisionToken, Session};
use crate::{event_clipboard::Event, generation::metadata::Guid};
#[derive(Clone, Debug)]
pub struct SelectionEffect {
    token: RevisionToken,
    chart: Guid,
    events: Vec<Event>,
}
impl SelectionEffect {
    pub(crate) fn capture(session: &Session, chart: usize, events: Vec<Event>) -> Self {
        Self {
            token: session.revision_token(),
            chart: session.chart_ids()[chart].clone(),
            events,
        }
    }
    pub fn chart(&self) -> &Guid {
        &self.chart
    }
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_token(&self.token) && session.chart_ids().contains(&self.chart)
    }
}
#[derive(Debug)]
pub(crate) struct SelectionHistory {
    pub before: SelectionEffect,
    pub after: SelectionEffect,
}
impl Session {
    pub fn take_event_selection_update(&mut self) -> Option<SelectionEffect> {
        self.event_selection_update.take()
    }
    /// Called immediately after a successful atomic action. Changed=true is the
    /// action's proof that it committed exactly one history entry.
    pub(crate) fn record_event_selection(&mut self, receipt: SelectionHistory, changed: bool) {
        self.event_selection_update = Some(receipt.after.clone());
        if changed {
            let entry = self
                .undo
                .last_mut()
                .expect("changed event transaction committed a history entry");
            assert_eq!(
                entry.after_revision, self.revision,
                "selection receipt belongs to the just-committed entry"
            );
            entry.selection = Some(receipt);
        }
    }
}
