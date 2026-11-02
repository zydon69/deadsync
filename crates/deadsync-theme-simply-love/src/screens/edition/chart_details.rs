//! Source chart readouts, prepared on the existing bounded density worker.
use super::*;
use deadsync_editor::{
    chart_order::Binding,
    density::{
        StreamSettings,
        task::{Report, Task},
    },
    generation::job::Job,
};
#[derive(Default)]
pub(super) struct Runtime {
    pub active: bool,
    back: Option<Panel>,
    row: usize,
    report: Option<Report>,
    binding: Option<Binding>,
    settings: Option<StreamSettings>,
    job: Option<Job<Report>>,
    failed: bool,
}
impl State {
    fn chart_details_current(&self) -> Option<&Report> {
        self.chart_details.report.as_ref().filter(|report| {
            self.workspace.has_chart()
                && self.chart_details.settings == Some(self.source_density.stream)
                && report.is_current(&self.workspace.file().session, self.workspace.chart())
        })
    }
    #[cfg(test)]
    pub(super) fn is_chart_details_loading_for_test(&self) -> bool {
        self.chart_details_current().is_none()
    }
    pub(super) fn open_chart_details(&mut self, back: Option<Panel>) {
        if !self.workspace.has_chart() {
            return;
        }
        self.chart_details.active = true;
        self.chart_details.back = back;
        self.chart_details.row = 0;
        self.panel = Some(Panel::Analysis);
        self.status = i18n::tr("Edition", "ChartDetailsHelp");
        self.prepare_chart_details();
    }
    fn prepare_chart_details(&mut self) {
        if self.chart_details_current().is_some() {
            return;
        }
        let chart = self.workspace.chart();
        let session = &self.workspace.file().session;
        let current = self
            .chart_details
            .binding
            .as_ref()
            .is_some_and(|b| b.is_current(session, chart))
            && self.chart_details.settings == Some(self.source_density.stream);
        if current && (self.chart_details.job.is_some() || self.chart_details.failed) {
            return;
        }
        if let Some(job) = self.chart_details.job.take() {
            job.cancel();
        }
        self.chart_details.report = None;
        self.chart_details.failed = false;
        self.chart_details.binding = match Binding::capture(session, chart) {
            Ok(binding) => Some(binding),
            Err(error) => {
                self.chart_details.failed = true;
                self.status = i18n::editor_error(&error).into();
                return;
            }
        };
        self.chart_details.settings = Some(self.source_density.stream);
        match Task::capture(session, chart, self.source_density.stream)
            .and_then(|task| task.spawn())
        {
            Ok(job) => self.chart_details.job = Some(job),
            Err(error) => {
                self.chart_details.failed = true;
                self.status = i18n::editor_error(&error).into();
            }
        }
    }
    pub(super) fn poll_chart_details(&mut self) {
        if !self.chart_details.active || !matches!(self.panel, Some(Panel::Analysis)) {
            return;
        }
        if !self.workspace.has_chart() {
            self.chart_details.active = false;
            self.panel = None;
            return;
        }
        self.prepare_chart_details();
        let Some(job) = self.chart_details.job.as_mut() else {
            return;
        };
        let Some(result) = job.poll() else {
            return;
        };
        self.chart_details.job = None;
        match result {
            Ok(report)
                if report.is_current(&self.workspace.file().session, self.workspace.chart())
                    && self.chart_details.settings == Some(self.source_density.stream) =>
            {
                self.chart_details.report = Some(report);
                self.status = i18n::tr("Edition", "ChartDetailsHelp");
            }
            Ok(_) => {
                self.chart_details.binding = None;
            }
            Err(error) => {
                self.chart_details.failed = true;
                self.status = i18n::editor_error(&error).into();
            }
        }
    }
    pub(super) fn chart_details_key(&mut self, code: KeyCode) {
        if code == KeyCode::Escape {
            if let Some(job) = self.chart_details.job.take() {
                job.cancel();
            }
            self.chart_details.active = false;
            self.panel = self.chart_details.back.take();
            self.status = i18n::tr("Edition", "Ready");
            return;
        }
        if code == KeyCode::F12 {
            self.chart_details.binding = None;
            self.chart_details.report = None;
            self.chart_details.failed = false;
            self.prepare_chart_details();
            return;
        }
        let Some(report) = self.chart_details_current() else {
            return;
        };
        let last = 4 + report.totals.lanes.len();
        match code {
            KeyCode::ArrowDown | KeyCode::PageDown => {
                self.chart_details.row = (self.chart_details.row + 1).min(last)
            }
            KeyCode::ArrowUp | KeyCode::PageUp => {
                self.chart_details.row = self.chart_details.row.saturating_sub(1)
            }
            KeyCode::Home => self.chart_details.row = 0,
            KeyCode::End => self.chart_details.row = last,
            _ => {}
        }
    }
    pub(super) fn chart_details_actors(&self, actors: &mut Vec<Actor>) {
        let width = (screen_width() - 24.).min(850.).max(1.);
        let height = (screen_height() - 24.).min(530.).max(1.);
        let x = screen_width() * 0.5;
        let top = (screen_height() - height) * 0.5;
        actors.push(act!(quad:xy(x,top+height*0.5):zoomto(width,height):diffuse(0.04,0.06,0.1,0.98):z(8070)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","ChartDetailsTitle")):xy(x,top+20.):zoom(0.65):maxwidth(width-24.):z(8071)));
        let Some(report) = self.chart_details_current() else {
            actors.push(act!(text:font("miso"):settext(if self.chart_details.failed{self.status.clone()}else{i18n::tr("Edition","ChartDetailsWaiting")}):xy(x,top+65.):zoom(0.5):maxwidth(width-24.):z(8071)));
            return;
        };
        let totals = &report.totals;
        let fmt = |key: &str, values: &[(&str, String)]| {
            let values: Vec<_> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
            i18n::tr_fmt("Edition", key, &values)
        };
        let mut lines = vec![
            fmt(
                "ChartDetailsNps",
                &[
                    ("step", format!("{:.2}", report.step_plot.peak)),
                    ("row", format!("{:.2}", report.row_plot.peak)),
                ],
            ),
            fmt(
                "ChartDetailsCounts",
                &[
                    ("notes", totals.notes.to_string()),
                    ("rows", totals.rows.to_string()),
                ],
            ),
            fmt(
                "ChartDetailsSustains",
                &[
                    ("holds", totals.holds.to_string()),
                    ("rolls", totals.rolls.to_string()),
                    ("lifts", totals.lifts.to_string()),
                ],
            ),
            fmt(
                "ChartDetailsOther",
                &[
                    ("mines", totals.mines.to_string()),
                    ("fakes", totals.fakes.to_string()),
                ],
            ),
            fmt(
                "ChartDetailsStream",
                &[("value", report.series.breakdown.clone().unwrap_or_default())],
            ),
        ];
        for (lane, count) in totals.lanes.iter().enumerate() {
            let (weight, percent) = totals.distribution(lane).expect("bounded lane");
            lines.push(fmt(
                "ChartDetailsLane",
                &[
                    ("lane", (lane + 1).to_string()),
                    ("count", count.to_string()),
                    ("percent", percent.to_string()),
                    ("weight", weight.to_string()),
                ],
            ));
        }
        let capacity = (((height - 104.) / 32.).floor() as usize).max(1);
        let selected = self.chart_details.row.min(lines.len().saturating_sub(1));
        let first = selected / capacity * capacity;
        for (index, label) in lines.into_iter().enumerate().skip(first).take(capacity) {
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(label):align(0.,0.):xy(x-width*0.5+12.,top+52.+(index-first)as f32*32.):zoom(0.48):maxwidth(width-24.):maxheight(28.):z(8071)));
            self.ui_row(actors, start, index == selected, 32.);
        }
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","ChartDetailsHelp")):xy(x,top+height-22.):zoom(0.4):maxwidth(width-24.):z(8071)));
    }
}

#[cfg(test)]
mod tests;
