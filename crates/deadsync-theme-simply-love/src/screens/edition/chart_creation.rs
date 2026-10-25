use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::chart_creation::{Draft, LAYOUTS};
pub(super) struct Settings {
    pub(super) draft: Draft,
    layout: usize,
}
impl State {
    pub(super) fn open_new_chart(&mut self) {
        if deadsync_editor::timing_edit::migration::needed(self.workspace.file().session.document())
        {
            self.open_timing_migration();
            return;
        }
        match self.workspace.new_chart_draft() {
            Ok(draft) => {
                let kind = &self.workspace.file().session.document().charts()
                    [self.workspace.chart()]
                .steps_type;
                self.new_chart = Some(Settings {
                    draft,
                    layout: LAYOUTS.iter().position(|name| *name == kind).unwrap_or(0),
                });
                self.panel = Some(Panel::NewChart(0));
                self.status = i18n::tr("Edition", "NewChartHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn new_chart_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::NewChart(
                (index + if code == KeyCode::ArrowUp { 2 } else { 1 }) % 3,
            ));
            return;
        }
        if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            if let Some(settings) = &mut self.new_chart {
                settings.layout = (settings.layout
                    + if code == KeyCode::ArrowLeft {
                        LAYOUTS.len() - 1
                    } else {
                        1
                    })
                    % LAYOUTS.len();
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 2 {
                self.new_chart = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "Ready");
                return;
            }
            if index != 1 {
                return;
            }
            let Some(settings) = &self.new_chart else {
                return;
            };
            match self
                .workspace
                .create_chart(&settings.draft, LAYOUTS[settings.layout])
            {
                Ok(_) => {
                    self.new_chart = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "NewChartCreated");
                    self.refresh();
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }
    pub(super) fn new_chart_label(&self) -> std::sync::Arc<str> {
        let layout = self
            .new_chart
            .as_ref()
            .map_or(LAYOUTS[0], |s| LAYOUTS[s.layout]);
        i18n::tr_fmt("Edition", "NewChartLayout", &[("layout", layout)])
    }
    pub(super) fn new_chart_summary(&self) -> std::sync::Arc<str> {
        let Some(settings) = &self.new_chart else {
            return i18n::tr("Edition", "NewChartHelp");
        };
        let defaults = settings.draft.defaults();
        let key = if self.workspace.file().session.document().format
            == deadsync_editor::document::Format::Sm
        {
            "NewChartSummarySm"
        } else {
            "NewChartSummary"
        };
        i18n::tr_fmt(
            "Edition",
            key,
            &[
                ("bpm", &defaults.tempo.to_string()),
                ("signature", &defaults.signature.to_string()),
            ],
        )
    }
}
