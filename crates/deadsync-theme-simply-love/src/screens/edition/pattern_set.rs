//! Optional saved-pattern selection and review; all generation stays in the core.
use super::*;
use std::collections::BTreeSet;

pub(super) struct Settings {
    markers: Vec<(u64, u64)>,
    current: usize,
    selected: BTreeSet<u64>,
}
impl State {
    pub(super) fn pattern_set_side_panel(&self) -> bool {
        screen_width() >= 800.0
            && matches!(self.panel, Some(Panel::GenerationPreview(_)))
            && matches!(self.preview, Some(GenerationPreview::PatternSet { .. }))
    }
    pub fn pattern_set_preview(&self) -> Option<(&pattern_batch::Preview, usize)> {
        match self.preview.as_ref()? {
            GenerationPreview::PatternSet { preview, entry } => Some((preview, *entry)),
            _ => None,
        }
    }
    pub(super) fn open_pattern_set(&mut self) {
        if self.is_generating() {
            return;
        }
        let result = (|| {
            let data = deadsync_editor::generation::metadata::ChartData::read(
                self.workspace.file().session.document(),
                self.workspace.chart(),
            )?
            .ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit(
                    i18n::tr("Edition", "PatternSetNoMarkers").to_string(),
                )
            })?;
            let current = data.marker_at(self.workspace.row()).map(|(row, _)| row);
            let markers: Vec<_> = data
                .patterns
                .unwrap_or_default()
                .into_iter()
                .map(|(row, definition)| (row, definition.length))
                .collect();
            if markers.is_empty() {
                return Err(deadsync_editor::Error::InvalidEdit(
                    i18n::tr("Edition", "PatternSetNoMarkers").to_string(),
                ));
            }
            let current = current
                .and_then(|row| markers.iter().position(|marker| marker.0 == row))
                .unwrap_or(0);
            let selected = markers.iter().map(|marker| marker.0).collect();
            Ok(Settings {
                markers,
                current,
                selected,
            })
        })();
        match result {
            Ok(settings) => {
                self.pattern_set = Some(settings);
                self.panel = Some(Panel::PatternSet(0));
                self.status = i18n::tr("Edition", "PatternSetOptionsHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn pattern_set_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::PatternSet(
                (index + if code == KeyCode::ArrowUp { 6 } else { 1 }) % 7,
            ));
        } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            if let Some(settings) = &mut self.pattern_set {
                if index == 0 {
                    settings.current = (settings.current
                        + if code == KeyCode::ArrowRight {
                            1
                        } else {
                            settings.markers.len() - 1
                        })
                        % settings.markers.len();
                } else if index == 1 {
                    toggle(settings);
                }
            }
        } else if code == KeyCode::Enter {
            match index {
                1 => {
                    if let Some(settings) = &mut self.pattern_set {
                        toggle(settings);
                    }
                }
                2 => {
                    if let Some(settings) = &mut self.pattern_set {
                        settings.selected =
                            settings.markers.iter().map(|marker| marker.0).collect();
                    }
                }
                3 => {
                    if let Some(settings) = &mut self.pattern_set {
                        settings.selected.clear();
                    }
                }
                4 => {
                    if let Some((start, end)) = self.workspace.selection() {
                        if let Some(settings) = &mut self.pattern_set {
                            settings.selected = settings
                                .markers
                                .iter()
                                .filter(|marker| start <= marker.0 && marker.0 <= end)
                                .map(|marker| marker.0)
                                .collect();
                        }
                    } else {
                        self.status = i18n::tr("Edition", "PatternSetNoArea");
                    }
                }
                5 => self.start_pattern_set(),
                6 => self.panel = None,
                _ => {}
            }
        }
    }
    fn start_pattern_set(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let Some(settings) = &self.pattern_set else {
            return;
        };
        if settings.selected.is_empty() {
            self.status = i18n::tr("Edition", "PatternSetNoSelection");
            return;
        }
        match self
            .workspace
            .pattern_set_task(
                settings.selected.iter().copied(),
                self.profile_directory.as_deref(),
            )
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::PatternSet(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn change_pattern_set_entry(&mut self, forward: bool) {
        if let Some(GenerationPreview::PatternSet { preview, entry }) = &mut self.preview {
            let count = preview.entries().len();
            if count > 0 {
                *entry = (*entry + if forward { 1 } else { count - 1 }) % count;
                let row = preview.entries()[*entry].row;
                if let Err(error) = self.workspace.execute(Command::Seek(row)) {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
                self.refresh();
            }
        }
    }
    pub(super) fn pattern_set_label(&self, index: usize, key: &str) -> Arc<str> {
        let Some(settings) = &self.pattern_set else {
            return i18n::tr("Edition", key);
        };
        let (row, length) = settings.markers[settings.current];
        match index {
            0 => i18n::tr_fmt(
                "Edition",
                key,
                &[
                    ("index", &(settings.current + 1).to_string()),
                    ("total", &settings.markers.len().to_string()),
                    ("beat", &format!("{:.3}", row as f64 / 48.0)),
                    ("length", &format!("{:.3}", length as f64 / 48.0)),
                ],
            ),
            1 => i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "value",
                    &i18n::tr(
                        "Common",
                        if settings.selected.contains(&row) {
                            "Yes"
                        } else {
                            "No"
                        },
                    ),
                )],
            ),
            5 => i18n::tr_fmt(
                "Edition",
                key,
                &[("count", &settings.selected.len().to_string())],
            ),
            _ => i18n::tr("Edition", key),
        }
    }
    pub(super) fn pattern_set_summary(
        &self,
        preview: &pattern_batch::Preview,
        index: usize,
    ) -> Arc<str> {
        let report = preview.report();
        let mut summary = i18n::tr_fmt(
            "Edition",
            "PatternSetSummary",
            &[
                ("generated", &report.generated.to_string()),
                ("empty", &report.empty.to_string()),
                ("failed", &report.failed.to_string()),
                ("total", &report.total.to_string()),
                ("issues", &preview.profile_issues().len().to_string()),
            ],
        )
        .to_string();
        summary.push('\n');
        summary.push_str(&i18n::tr_fmt(
            "Edition",
            "PatternSetCounts",
            &[
                ("removed", &report.deletions.removed.to_string()),
                ("holds", &report.deletions.shortened_holds.to_string()),
                ("nps", &format!("{:.3}", preview.average_nps())),
                ("notes", &report.notes.to_string()),
                ("repeated", &report.repeated_taps.to_string()),
                ("trimmed", &report.trimmed_steps.to_string()),
            ],
        ));
        if let Some(entry) = preview.entries().get(index) {
            summary.push('\n');
            summary.push_str(&i18n::tr_fmt(
                "Edition",
                "PatternSetResult",
                &[
                    ("index", &(index + 1).to_string()),
                    ("total", &report.total.to_string()),
                    ("beat", &format!("{:.3}", entry.row as f64 / 48.0)),
                    ("seed", &entry.seed.to_string()),
                ],
            ));
            summary.push('\n');
            if let Some(generated) = entry.generated() {
                let warnings = if generated.warnings.is_empty() {
                    i18n::tr("Edition", "PatternNoWarnings").to_string()
                } else {
                    generated.warnings.iter().map(|warning|i18n::tr("Edition",match warning {deadsync_editor::generation::pattern::EndWarning::RelaxedToOneFoot=>"PatternRelaxed",deadsync_editor::generation::pattern::EndWarning::FollowingFootingUnreachable=>"PatternUnreachable"}).to_string()).collect::<Vec<_>>().join(" ")
                };
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "PatternSetReady",
                    &[
                        ("steps", &generated.steps.len().to_string()),
                        ("warnings", &warnings),
                    ],
                ));
            } else if let Some(error) = entry.error() {
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "BatchEntryFailed",
                    &[("message", error)],
                ));
            } else {
                summary.push_str(&i18n::tr("Edition", "PatternSetEmpty"));
            }
        }
        if report.failed > 0 {
            summary.push('\n');
            summary.push_str(&i18n::tr("Edition", "PatternSetFailedRegions"));
        }
        summary.into()
    }
}
fn toggle(settings: &mut Settings) {
    let row = settings.markers[settings.current].0;
    if !settings.selected.remove(&row) {
        settings.selected.insert(row);
    }
}
