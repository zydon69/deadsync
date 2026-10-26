use super::State;
use crate::i18n;
use deadsync_editor::{
    chart_order::{Binding, Orderer},
    workspace::Command,
};

pub(super) struct Request {
    binding: Binding,
    backwards: bool,
    language_revision: u64,
}
impl State {
    pub(super) fn queue_chart_order(&mut self, backwards: bool) {
        if self.workspace.file().session.document().charts().len() < 2 {
            return;
        }
        if let Ok(binding) =
            Binding::capture(&self.workspace.file().session, self.workspace.chart())
        {
            self.chart_order_request = Some(Request {
                binding,
                backwards,
                language_revision: i18n::revision(),
            });
        }
    }
    pub fn process_chart_order(&mut self, orderer: &mut impl Orderer) -> bool {
        let Some(request) = self.chart_order_request.take() else {
            return false;
        };
        let current = |state: &Self| {
            state.panel.is_none()
                && state.label_preedit.is_empty()
                && request.language_revision == i18n::revision()
                && request
                    .binding
                    .is_current(&state.workspace.file().session, state.workspace.chart())
        };
        if !current(self) {
            return false;
        }
        let result = self.workspace.chart_order_snapshot().and_then(|snapshot| {
            let mut expected = snapshot.source_indices().to_vec();
            if expected.len() < 2 {
                return Ok(expected);
            }
            let indices = orderer.order(&snapshot)?;
            let mut received = indices.clone();
            expected.sort_unstable();
            received.sort_unstable();
            if received != expected {
                return Err(deadsync_editor::Error::InvalidEdit(
                    "Chart order is not an exhaustive source-index permutation".into(),
                ));
            }
            Ok(indices)
        });
        // The native boundary may reenter the application or change locale.
        // Never apply results belonging to the former song/focus/language.
        if !current(self) {
            return false;
        }
        match result {
            Ok(indices) => {
                let Some(position) = indices
                    .iter()
                    .position(|index| *index == self.workspace.chart())
                else {
                    self.status = i18n::tr("Edition", "ChartOrderFailed");
                    return true;
                };
                let delta = if request.backwards {
                    indices.len() - 1
                } else {
                    1
                };
                let next = indices[(position + delta) % indices.len()];
                self.command(Command::SelectChart(next));
            }
            Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
        }
        true
    }
}
