use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::{
    analysis::task::{self, Report, Task},
    workspace::Command,
};
impl State {
    pub fn is_analyzing(&self) -> bool {
        self.analysis_job.is_some()
    }
    pub fn analysis_report(&self) -> Option<&Report> {
        self.analysis_report.as_ref().filter(|report| {
            report.is_current(&self.workspace.file().session, self.workspace.chart())
        })
    }
    pub(super) fn open_analysis(&mut self) {
        self.chart_details.active = false;
        self.panel = Some(Panel::Analysis);
        if let Some(report) = self.analysis_report() {
            let length = report.summary().measure_densities.len();
            self.analysis_bin =
                ((self.workspace.row() / 192) as usize).min(length.saturating_sub(1));
            self.status = i18n::tr("Edition", "AnalysisHelp");
            return;
        }
        self.analysis_report = None;
        self.start_analysis();
    }
    fn start_analysis(&mut self) {
        if let Some(job) = &self.analysis_job {
            self.status = i18n::tr(
                "Edition",
                if job.is_cancelled() {
                    "AnalysisCancelling"
                } else {
                    "AnalysisLoading"
                },
            );
            return;
        }
        if task::worker_busy() {
            self.status = i18n::tr("Edition", "AnalysisBusy");
            return;
        }
        match Task::capture(&self.workspace.file().session, self.workspace.chart())
            .and_then(Task::spawn)
        {
            Ok(job) => {
                self.analysis_job = Some(job);
                self.status = i18n::tr("Edition", "AnalysisLoading");
            }
            Err(error) => self.status = self.analysis_error(&error),
        }
    }
    pub(super) fn poll_analysis(&mut self) {
        let Some(job) = self.analysis_job.as_mut() else {
            return;
        };
        let cancelled = job.is_cancelled();
        let Some(result) = job.poll() else {
            return;
        };
        self.analysis_job = None;
        let visible = !self.chart_details.active && matches!(self.panel, Some(Panel::Analysis));
        match result {
            Ok(report)
                if report.is_current(&self.workspace.file().session, self.workspace.chart()) =>
            {
                self.analysis_bin = (self.workspace.row() / 192) as usize;
                self.analysis_bin = self
                    .analysis_bin
                    .min(report.summary().measure_densities.len().saturating_sub(1));
                self.analysis_report = Some(report);
                if visible {
                    self.status = i18n::tr("Edition", "AnalysisHelp");
                }
            }
            Ok(_) => {
                if visible {
                    self.status = i18n::tr("Edition", "AnalysisStale");
                }
            }
            Err(error) => {
                if visible {
                    self.status = if cancelled {
                        i18n::tr("Edition", "AnalysisCancelled")
                    } else {
                        self.analysis_error(&error)
                    };
                }
            }
        }
    }
    pub(super) fn analysis_key(&mut self, code: KeyCode) -> bool {
        if self.chart_details.active {
            self.chart_details_key(code);
            return false;
        }
        if code == KeyCode::F2 {
            self.open_chart_details(None);
            return false;
        }
        if code == KeyCode::F6 {
            self.open_density_settings();
            return false;
        }
        if code == KeyCode::Escape {
            if let Some(job) = &self.analysis_job {
                job.cancel();
            }
            self.panel = None;
            self.status = i18n::tr("Edition", "Ready");
            return false;
        }
        if code == KeyCode::F12 {
            if self.analysis_job.is_none() {
                self.analysis_report = None;
            }
            self.start_analysis();
            return false;
        }
        let Some(report) = self.analysis_report() else {
            return false;
        };
        let length = report.summary().measure_densities.len();
        if length == 0 {
            return false;
        }
        match code {
            KeyCode::ArrowLeft => self.analysis_bin = self.analysis_bin.saturating_sub(1),
            KeyCode::ArrowRight => self.analysis_bin = (self.analysis_bin + 1).min(length - 1),
            KeyCode::Home => self.analysis_bin = 0,
            KeyCode::End => self.analysis_bin = length - 1,
            KeyCode::Enter | KeyCode::NumpadEnter => match report.row_for_bin(self.analysis_bin) {
                Ok(row) => {
                    self.panel = None;
                    self.command(Command::Seek(row));
                }
                Err(error) => self.status = self.analysis_error(&error),
            },
            _ => {}
        }
        false
    }
    fn analysis_error(&self, error: &deadsync_editor::Error) -> std::sync::Arc<str> {
        if matches!(error, deadsync_editor::Error::Unsupported(_)) {
            let layout = &self.workspace.file().session.document().charts()[self.workspace.chart()]
                .steps_type;
            i18n::tr_fmt("Edition", "AnalysisUnsupported", &[("layout", layout)])
        } else {
            i18n::tr_fmt(
                "Edition",
                "AnalysisError",
                &[("error", &crate::i18n::editor_error(&error))],
            )
        }
    }
    pub(super) fn analysis_actors(&self, actors: &mut Vec<Actor>) {
        if self.chart_details.active {
            self.chart_details_actors(actors);
            return;
        }
        let width = (screen_width() - 30.0).min(760.0).max(1.0);
        let height = (screen_height() - 30.0).min(420.0).max(1.0);
        let x = screen_width() * 0.5;
        let top = (screen_height() - height) * 0.5;
        actors.push(act!(quad: xy(x,top+height*0.5): setsize(width,height): diffuse(0.05,0.08,0.13,0.98): z(8020)));
        let mut line = |label: std::sync::Arc<str>, y: f32| {
            actors.push(act!(text: font("miso"): settext(label): align(0.0,0.0): xy(x-width*0.5+14.0,top+y): zoom(0.6): wrapwidthpixels((width-28.0)/0.6): maxwidth(width-28.0): maxheight(32.0): z(8021)));
        };
        line(i18n::tr("Edition", "AnalysisTitle"), 12.0);
        let Some(report) = self.analysis_report() else {
            line(self.status.clone(), 55.0);
            line(i18n::tr("Edition", "AnalysisWaitingHelp"), height - 40.0);
            return;
        };
        let s = report.summary();
        let stats = &s.stats;
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisNotes",
                &[
                    ("arrows", &stats.total_arrows.to_string()),
                    ("steps", &stats.total_steps.to_string()),
                    ("jumps", &stats.jumps.to_string()),
                    ("hands", &stats.hands.to_string()),
                ],
            ),
            44.0,
        );
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisSustains",
                &[
                    ("holds", &stats.holds.to_string()),
                    ("rolls", &stats.rolls.to_string()),
                    ("mines", &stats.mines.to_string()),
                    ("lifts", &stats.lifts.to_string()),
                    ("fakes", &stats.fakes.to_string()),
                ],
            ),
            73.0,
        );
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisNps",
                &[
                    ("peak", &format!("{:.2}", s.max_nps)),
                    ("median", &format!("{:.2}", s.median_nps)),
                    ("duration", &format!("{:.2}", s.duration_seconds)),
                    ("hash", &s.short_hash),
                ],
            ),
            102.0,
        );
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisTech",
                &[
                    ("crossovers", &s.tech_counts.crossovers.to_string()),
                    ("switches", &s.tech_counts.footswitches.to_string()),
                    ("brackets", &s.tech_counts.brackets.to_string()),
                    ("jacks", &s.tech_counts.jacks.to_string()),
                ],
            ),
            131.0,
        );
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisStreams",
                &[
                    ("streams", &s.total_streams.to_string()),
                    ("breaks", &s.stream_counts.total_breaks.to_string()),
                    (
                        "breakdown",
                        &if s.total_streams == 0 {
                            i18n::tr("Edition", "AnalysisNoStreams").to_string()
                        } else {
                            short_breakdown(&s.simple_breakdown)
                        },
                    ),
                ],
            ),
            160.0,
        );
        line(i18n::tr("Edition", "AnalysisDensity"), 188.0);
        let count = s
            .measure_densities
            .get(self.analysis_bin)
            .copied()
            .unwrap_or(0);
        let nps = s
            .measure_nps_vec
            .get(self.analysis_bin)
            .map_or_else(|| "--".to_owned(), |v| format!("{v:.2}"));
        line(
            i18n::tr_fmt(
                "Edition",
                "AnalysisInterval",
                &[
                    (
                        "index",
                        &if s.measure_densities.is_empty() {
                            0
                        } else {
                            self.analysis_bin + 1
                        }
                        .to_string(),
                    ),
                    ("count", &s.measure_densities.len().to_string()),
                    ("steps", &count.to_string()),
                    ("nps", &nps),
                ],
            ),
            height - 83.0,
        );
        line(self.status.clone(), height - 43.0);
        drop(line);
        let columns = report.columns();
        let peak = columns.iter().map(|c| c.steps).max().unwrap_or(1).max(1) as f32;
        let graph_top = top + 203.0;
        let graph_height = (height - 300.0).max(1.0);
        let bar_width = (width - 32.0) / columns.len().max(1) as f32;
        for (index, column) in columns.iter().enumerate() {
            let h = (column.steps as f32 / peak * graph_height).max(1.0);
            let selected = column.first <= self.analysis_bin && self.analysis_bin < column.end;
            let color = if selected {
                [1.0, 0.85, 0.25, 1.0]
            } else {
                [0.3, 0.7, 1.0, 1.0]
            };
            let bx = x - width * 0.5 + 16.0 + (index as f32 + 0.5) * bar_width;
            actors.push(act!(quad: xy(bx,graph_top+graph_height-h*0.5): setsize((bar_width-1.0).max(1.0),h): diffuse(color[0],color[1],color[2],color[3]): z(8021)));
        }
    }
}

fn short_breakdown(value: &str) -> String {
    let mut chars = value.chars();
    let mut text = chars.by_ref().take(140).collect::<String>();
    if chars.next().is_some() {
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod status_owner_tests;
