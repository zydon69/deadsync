use super::{Panel, State};
use crate::i18n;
use deadsync_editor::chart_order::{Binding, Orderer};

pub(super) struct Request {
    binding: Binding,
    backwards: bool,
    opening_list: bool,
    source_row: u64,
    music_time: Option<f64>,
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
            let Ok(music_time) = self.chart_focus_music_time() else {
                self.status = i18n::tr("Edition", "ChartOrderFailed");
                return;
            };
            self.chart_order_request = Some(Request {
                binding,
                backwards,
                opening_list: false,
                source_row: self.workspace.row(),
                music_time: Some(music_time),
                language_revision: i18n::revision(),
            });
        }
    }
    pub(super) fn queue_chart_list(&mut self) {
        if !self.workspace.has_chart() {
            self.chart_list_cache.clear();
            self.chart_list_selection = Default::default();
            self.panel = Some(Panel::ChartList);
            return;
        }
        if let Ok(binding) =
            Binding::capture(&self.workspace.file().session, self.workspace.chart())
        {
            self.chart_order_request = Some(Request {
                binding,
                backwards: false,
                opening_list: true,
                source_row: self.workspace.row(),
                music_time: None,
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
                && request.source_row == state.workspace.row()
                && state.label_preedit.is_empty()
                && request.language_revision == i18n::revision()
                && request
                    .binding
                    .is_current(&state.workspace.file().session, state.workspace.chart())
        };
        if !current(self) {
            return false;
        }
        let result = self
            .chart_list_cache
            .refresh(&mut self.workspace, request.language_revision, orderer)
            .map(|_| ());
        // The native boundary may reenter the application or change locale.
        // Never apply results belonging to the former song/focus/language.
        if !current(self) {
            return false;
        }
        match result {
            Ok(()) if request.opening_list => {
                self.chart_list_selection
                    .reconcile(self.chart_list_cache.rows(), self.workspace.chart());
                self.panel = Some(Panel::ChartList);
                self.status = i18n::tr("Edition", "ChartListHelp");
            }
            Ok(()) => {
                let Some(next) = self
                    .chart_list_cache
                    .adjacent(self.workspace.chart(), request.backwards)
                else {
                    self.status = i18n::tr("Edition", "ChartOrderFailed");
                    return true;
                };
                self.focus_chart_at_time(
                    next,
                    request.music_time.expect("navigation captures music time"),
                );
            }
            Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
        }
        true
    }
}
