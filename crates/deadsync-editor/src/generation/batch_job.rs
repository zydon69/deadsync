//! One conversion per source chart of a selected type. Failures are retained for
//! review while successful charts can be applied together as one history action.
use super::job::{
    ChartOptions, ChartPreview, ChartTask, Job, Phase, ProfileOverrides, ProfileSnapshot, Progress,
    TARGET_LAYOUTS,
};
use crate::{
    Error,
    history::{ChartAddition, RevisionToken, Session},
    notes::{Grid, layout_columns},
};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub struct Task {
    token: RevisionToken,
    source_layout: String,
    target_layout: String,
    tasks: Vec<ChartTask>,
    sources: Vec<(usize, String, String)>,
    directory: Option<std::path::PathBuf>,
    profiles: ProfileOverrides,
}
pub struct Entry {
    pub source_chart: usize,
    pub difficulty: String,
    pub meter: String,
    outcome: Result<ChartPreview, String>,
}
impl Entry {
    pub fn preview(&self) -> Option<&ChartPreview> {
        self.outcome.as_ref().ok()
    }
    pub fn error(&self) -> Option<&str> {
        self.outcome.as_ref().err().map(String::as_str)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub total: usize,
    pub generated: usize,
    pub failed: usize,
    pub notes: usize,
    pub mines: usize,
}
pub struct Preview {
    token: RevisionToken,
    source_layout: String,
    target_layout: String,
    entries: Vec<Entry>,
    empty: Grid,
}
/// The actual source family seen by the reference engine. In particular, raw
/// pump-double StepF2 co-op is effectively pump-routine and is not a candidate.
/// UI callers cache this inventory by content token rather than decoding notes
/// in presentation code; capture uses the same admission below.
pub fn source_charts(session: &Session) -> Vec<(usize, &str)> {
    (0..session.document().charts().len())
        .filter_map(|chart| {
            let layout = super::source::effective_layout(session.document(), chart).ok()?;
            TARGET_LAYOUTS.contains(&layout).then_some((chart, layout))
        })
        .collect()
}
impl Task {
    pub fn capture(
        session: &Session,
        source_layout: &str,
        options: ChartOptions,
        directory: Option<&Path>,
        profiles: ProfileOverrides,
    ) -> Result<Self, Error> {
        if !TARGET_LAYOUTS.contains(&source_layout) {
            return Err(invalid("Unsupported source type for batch generation"));
        }
        let sources: Vec<_> = source_charts(session)
            .into_iter()
            .filter(|(_, layout)| *layout == source_layout)
            .map(|(index, _)| {
                let chart = &session.document().charts()[index];
                (index, chart.difficulty.clone(), chart.meter.clone())
            })
            .collect();
        if sources.is_empty() {
            return Err(invalid("No source charts of the selected type"));
        }
        let indices = sources.iter().map(|source| source.0).collect::<Vec<_>>();
        let target_layout = options.target_layout.clone();
        let tasks = ChartTask::capture_many(session, &indices, options)?;
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        Ok(Self {
            token: session.revision_token(),
            source_layout: source_layout.into(),
            target_layout,
            tasks,
            sources,
            directory,
            profiles,
        })
    }
    pub fn spawn(self) -> Result<Job<Preview>, Error> {
        Job::spawn_with("editor-chart-set-generation", move |cancelled, progress| {
            self.run(cancelled, progress)
        })
    }
    pub fn run(
        self,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(Progress),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let snapshot = ProfileSnapshot::load(self.directory.as_deref(), cancelled)?;
        let total = self.tasks.len();
        let mut entries = Vec::with_capacity(total);
        for (index, (task, (source_chart, difficulty, meter))) in
            self.tasks.into_iter().zip(self.sources).enumerate()
        {
            check(cancelled)?;
            progress(Progress {
                phase: Phase::Preparing,
                completed: index,
                total,
            });
            let outcome = task
                .with_profile_snapshot(snapshot.clone(), self.profiles.clone())
                .run(cancelled, |current| {
                    progress(Progress {
                        phase: if current.phase == Phase::Ready {
                            Phase::Converting
                        } else {
                            current.phase
                        },
                        completed: index,
                        total,
                    })
                });
            check(cancelled)?;
            entries.push(Entry {
                source_chart,
                difficulty,
                meter,
                outcome: outcome.map_err(|error| error.to_string()),
            });
            progress(Progress {
                phase: Phase::Converting,
                completed: index + 1,
                total,
            });
        }
        check(cancelled)?;
        let empty = Grid::new(
            layout_columns(&self.target_layout)
                .ok_or_else(|| invalid("Unknown batch destination layout"))?,
            1,
        )?;
        progress(Progress {
            phase: Phase::Ready,
            completed: total,
            total,
        });
        check(cancelled)?;
        Ok(Preview {
            token: self.token,
            source_layout: self.source_layout,
            target_layout: self.target_layout,
            entries,
            empty,
        })
    }
}
impl Preview {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn source_layout(&self) -> &str {
        &self.source_layout
    }
    pub fn target_layout(&self) -> &str {
        &self.target_layout
    }
    pub fn grid(&self, index: usize) -> &Grid {
        self.entries
            .get(index)
            .and_then(Entry::preview)
            .map(ChartPreview::grid)
            .unwrap_or(&self.empty)
    }
    pub fn report(&self) -> Report {
        let mut report = Report {
            total: self.entries.len(),
            ..Report::default()
        };
        for entry in &self.entries {
            if let Some(preview) = entry.preview() {
                report.generated += 1;
                report.notes += preview.report().notes;
                report.mines += preview.report().mines;
            } else {
                report.failed += 1;
            }
        }
        report
    }
    pub fn apply(&self, session: &mut Session) -> Result<Vec<usize>, Error> {
        if !session.accepts_token(&self.token) {
            return Err(invalid(
                "Chart-set preview belongs to another session or revision",
            ));
        }
        let additions = self
            .entries
            .iter()
            .filter_map(Entry::preview)
            .map(|preview| ChartAddition {
                source_chart: preview.source_chart(),
                layout: preview.target_layout(),
                grid: preview.grid(),
            })
            .collect::<Vec<_>>();
        session.append_charts(&additions, session.revision(), "Generate chart set")
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Chart-set generation cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
