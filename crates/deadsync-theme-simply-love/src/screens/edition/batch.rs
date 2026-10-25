//! Batch conversion uses the same options and profile chooser as single conversion.
//! Only the source type and result navigation belong to this panel.
use super::*;

impl State {
    pub fn batch_preview(&self) -> Option<(&batch_job::Preview, usize)> {
        match self.preview.as_ref()? {
            GenerationPreview::Batch { preview, entry } => Some((preview, *entry)),
            _ => None,
        }
    }

    fn batch_sources(&self) -> Vec<&str> {
        let charts = self.workspace.file().session.document().charts();
        TARGET_LAYOUTS
            .iter()
            .copied()
            .filter(|layout| charts.iter().any(|chart| chart.steps_type == *layout))
            .collect()
    }

    pub(super) fn open_batch(&mut self) {
        if self.is_generating() {
            return;
        }
        let sources = self.batch_sources();
        let current =
            &self.workspace.file().session.document().charts()[self.workspace.chart()].steps_type;
        let source = sources
            .iter()
            .copied()
            .find(|source| *source == current)
            .or_else(|| {
                self.batch_source
                    .as_deref()
                    .filter(|source| sources.contains(source))
            })
            .or_else(|| sources.first().copied());
        let Some(source) = source else {
            self.status = i18n::tr("Edition", "BatchNoSources");
            return;
        };
        self.batch_source = Some(source.to_owned());
        self.panel = Some(Panel::Batch(0));
        self.status = i18n::tr("Edition", "BatchOptionsHelp");
    }

    pub(super) fn batch_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::Batch(
                (index + if code == KeyCode::ArrowUp { 7 } else { 1 }) % 8,
            ));
        } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            let forward = code == KeyCode::ArrowRight;
            match index {
                0 => {
                    let sources = self.batch_sources();
                    if !sources.is_empty() {
                        let current = sources
                            .iter()
                            .position(|source| Some(*source) == self.batch_source.as_deref())
                            .unwrap_or(0);
                        self.batch_source = Some(
                            sources[(current + if forward { 1 } else { sources.len() - 1 })
                                % sources.len()]
                            .to_owned(),
                        );
                    }
                }
                1..=3 => self.adjust_generation_setting(index - 1, forward),
                _ => {}
            }
        } else if code == KeyCode::Enter {
            match index {
                4 => self.open_conversion_profiles_for(ProfileTarget::Batch),
                5 => self.start_batch(),
                6 => self.panel = None,
                _ => {}
            }
        }
    }

    fn start_batch(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let Some(source) = self.batch_source.as_deref() else {
            return;
        };
        match self
            .workspace
            .batch_generation_task(
                source,
                self.generation_options(),
                self.profile_directory.as_deref(),
                self.generation.profiles.clone(),
            )
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::Batch(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    pub(super) fn change_batch_entry(&mut self, forward: bool) {
        if let Some(GenerationPreview::Batch { preview, entry }) = &mut self.preview {
            let count = preview.entries().len();
            if count > 0 {
                *entry = (*entry + if forward { 1 } else { count - 1 }) % count;
            }
        }
    }

    pub(super) fn batch_label(&self, index: usize, key: &str) -> Arc<str> {
        match index {
            0 => {
                let source = self.batch_source.as_deref().unwrap_or("");
                let count = self
                    .workspace
                    .file()
                    .session
                    .document()
                    .charts()
                    .iter()
                    .filter(|chart| chart.steps_type == source)
                    .count();
                i18n::tr_fmt(
                    "Edition",
                    key,
                    &[("value", source), ("count", &count.to_string())],
                )
            }
            1..=3 => self.generation_label(index - 1, key),
            7 => self.generation_label(6, key),
            _ => i18n::tr("Edition", key),
        }
    }

    pub(super) fn batch_summary(&self, preview: &batch_job::Preview, index: usize) -> Arc<str> {
        let report = preview.report();
        let mut summary = i18n::tr_fmt(
            "Edition",
            "BatchSummary",
            &[
                ("generated", &report.generated.to_string()),
                ("failed", &report.failed.to_string()),
                ("total", &report.total.to_string()),
                ("layout", preview.target_layout()),
            ],
        )
        .to_string();
        if let Some(entry) = preview.entries().get(index) {
            summary.push('\n');
            summary.push_str(&i18n::tr_fmt(
                "Edition",
                "BatchEntry",
                &[
                    ("index", &(index + 1).to_string()),
                    ("total", &report.total.to_string()),
                    ("source", &(entry.source_chart + 1).to_string()),
                    ("difficulty", &entry.difficulty),
                    ("meter", &entry.meter),
                ],
            ));
            summary.push('\n');
            if let Some(result) = entry.preview() {
                let report = result.report();
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "BatchEntryReady",
                    &[
                        ("notes", &report.notes.to_string()),
                        ("mines", &report.mines.to_string()),
                        ("skipped", &report.skipped_events.to_string()),
                        ("dropped", &report.dropped_mines.to_string()),
                        ("issues", &result.profile_issues().len().to_string()),
                    ],
                ));
            } else if let Some(error) = entry.error() {
                summary.push_str(&i18n::tr_fmt(
                    "Edition",
                    "BatchEntryFailed",
                    &[("message", error)],
                ));
            }
        }
        summary.into()
    }
}
