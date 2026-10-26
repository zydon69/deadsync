//! Regenerate selected saved markers against one evolving immutable snapshot.
//! Delete all selected regions first, estimate density once, then generate in
//! row order with shared preceding counters and typed insertion times.
use super::{
    graph::StepGraph,
    job::{Job, Phase, Progress},
    metadata::{ChartData, Registry},
    pattern::PatternPreview,
    pattern_job::RegionGenerator,
    pattern_region::{self, Deletions, NoteEdit},
    pattern_schedule::{LaneCounts, Region, Schedule},
    source,
};
use crate::{
    Error,
    document::Format,
    history::{RevisionToken, Session},
    notes::{Grid, Note},
    pad::Pad,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Task {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    chart: usize,
    rows: Vec<u64>,
    directory: Option<PathBuf>,
}
pub struct Entry {
    pub row: u64,
    pub seed: i32,
    pub trimmed_steps: usize,
    outcome: Result<Option<PatternPreview>, String>,
}
impl Entry {
    pub fn generated(&self) -> Option<&PatternPreview> {
        self.outcome.as_ref().ok().and_then(Option::as_ref)
    }
    pub fn error(&self) -> Option<&str> {
        self.outcome.as_ref().err().map(String::as_str)
    }
    pub fn is_empty(&self) -> bool {
        matches!(self.outcome, Ok(None))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub total: usize,
    pub generated: usize,
    pub empty: usize,
    pub failed: usize,
    /// Generated tap instances before file export coalesces repeated positions.
    pub notes: usize,
    pub repeated_taps: usize,
    pub trimmed_steps: usize,
    pub deletions: Deletions,
}
pub struct Preview {
    token: RevisionToken,
    chart: usize,
    layout: String,
    grid: Grid,
    edits: Vec<NoteEdit>,
    entries: Vec<Entry>,
    average_nps: f64,
    report: Report,
    issues: Vec<super::profiles::Issue>,
}
impl Task {
    pub fn capture(
        session: &Session,
        chart: usize,
        rows: impl IntoIterator<Item = u64>,
        directory: Option<&Path>,
    ) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return Err(invalid("Pattern source chart does not exist"));
        }
        source::require_reference_source(session.document(), chart)?;
        let rows: Vec<_> = rows
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if rows.is_empty() {
            return Err(invalid("No patterns selected for generation"));
        }
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
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            chart,
            rows,
            directory,
        })
    }
    pub fn capture_all(
        session: &Session,
        chart: usize,
        directory: Option<&Path>,
    ) -> Result<Self, Error> {
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("No saved pattern markers in this chart"))?;
        Self::capture(
            session,
            chart,
            data.patterns.unwrap_or_default().into_keys(),
            directory,
        )
    }
    pub fn spawn(self) -> Result<Job<Preview>, Error> {
        Job::spawn_with(
            "editor-pattern-set-generation",
            move |cancelled, progress| self.run(cancelled, progress),
        )
    }
    pub fn run(
        self,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(Progress),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let source = source::read(self.bytes, self.format, self.chart, cancelled)?;
        let data = source.initial_editor_metadata()?;
        let (registry, issues) = if let Some(directory) = &self.directory {
            let loaded = super::profiles::Loaded::directory_checked(directory, cancelled)?;
            (loaded.registry, loaded.issues)
        } else {
            (Registry::builtin(), Vec::new())
        };
        data.validate()?;
        registry
            .expressed
            .get(&data.expressed_chart_config)
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "Missing expressed generation profile {}",
                    data.expressed_chart_config
                ))
            })?
            .validate()?;
        let mut regions = Vec::new();
        let mut options = Vec::new();
        for row in &self.rows {
            check(cancelled)?;
            let marker = data
                .patterns
                .as_ref()
                .and_then(|markers| markers.get(row))
                .ok_or_else(|| invalid("Selected pattern marker is missing"))?;
            let pattern = registry
                .patterns
                .get(&marker.pattern_config_guid)
                .ok_or_else(|| {
                    Error::Unsupported(format!(
                        "Missing pattern generation profile {}",
                        marker.pattern_config_guid
                    ))
                })?;
            pattern.validate()?;
            regions.push(Region {
                span: super::pattern_job::Span {
                    start: *row,
                    end: *row + marker.length,
                    include_start: marker.start_position_inclusive,
                    include_end: marker.end_position_inclusive,
                },
                spacing: 48 / u64::from(pattern.beat_sub_division),
            });
            options.push((
                marker.random_seed,
                data.options_allow_empty(*row, &registry)
                    .map_err(|error| error.to_string()),
            ));
        }
        let schedule = Schedule::new(regions, cancelled)?;
        let graph = StepGraph::builtin(&source.layout)?;
        let pad = Pad::builtin(&source.layout)?;
        let original = source.grid.clone();
        let mut grid = original.clone();
        let timeline = source.editor_timeline(cancelled)?
            .with_markers(&source.pattern_regions(cancelled)?, cancelled)?;
        let mut report = Report {
            total: schedule.entries().len(),
            ..Report::default()
        };
        for entry in schedule.entries() {
            check(cancelled)?;
            if let Some((start, end, _)) = entry.steps {
                let (edits, removed) = pattern_region::deletion_edits(
                    &grid,
                    0,
                    start,
                    end,
                    entry.region.spacing,
                    cancelled,
                )?;
                report.deletions.removed += removed.removed;
                report.deletions.shortened_holds += removed.shortened_holds;
                for (position, note) in edits {
                    check(cancelled)?;
                    grid.set(position, note)?;
                }
            }
        }
        let mut timeline = timeline.project(&grid, None, cancelled)?;
        let average_nps = schedule.average_nps(&timeline)?;
        let mut counts = LaneCounts::new(graph.columns())?;
        let generator = RegionGenerator {
            source: &source,
            graph: &graph,
            pad,
        };
        let mut entries = Vec::new();
        for (index, (entry, (seed, options))) in schedule.entries().iter().zip(options).enumerate()
        {
            check(cancelled)?;
            progress(Progress {
                phase: Phase::Converting,
                completed: index,
                total: report.total,
            });
            let mut trimmed_steps = 0;
            let outcome = if let Some((first, _, _)) = entry.steps {
                match options {
                    Ok(options) => generator
                        .generate(
                            &options,
                            &grid,
                            &timeline,
                            average_nps,
                            &mut counts,
                            cancelled,
                            |_, _| {
                                progress(Progress {
                                    phase: Phase::Converting,
                                    completed: index,
                                    total: report.total,
                                })
                            },
                        )
                        .and_then(|mut generated| {
                            let full = generated.steps.len();
                            generated.steps.retain(|step| entry.keeps_row(step.row));
                            trimmed_steps = full - generated.steps.len();
                            let taps = generated.tap_positions(cancelled)?;
                            let updated =
                                timeline.with_pattern_taps(taps.iter().copied(), cancelled)?;
                            let mut updated_grid = grid.clone();
                            for tap in &taps {
                                check(cancelled)?;
                                updated_grid.set(*tap, Some(Note::tap()))?;
                            }
                            report.repeated_taps +=
                                taps.iter().filter(|tap| grid.get(**tap).is_some()).count();
                            grid = updated_grid;
                            timeline = updated;
                            report.notes += taps.len();
                            Ok(Some(generated))
                        })
                        .map_err(|error| error.to_string()),
                    Err(error) => {
                        counts.advance(&timeline.context_events(), first, cancelled)?;
                        Err(error)
                    }
                }
            } else {
                Ok(None)
            };
            check(cancelled)?;
            match &outcome {
                Ok(Some(_)) => report.generated += 1,
                Ok(None) => report.empty += 1,
                Err(_) => report.failed += 1,
            }
            report.trimmed_steps += trimmed_steps;
            entries.push(Entry {
                row: entry.region.span.start,
                seed,
                trimmed_steps,
                outcome,
            });
            progress(Progress {
                phase: Phase::Converting,
                completed: index + 1,
                total: report.total,
            });
        }
        let positions: BTreeSet<_> = original
            .iter()
            .chain(grid.iter())
            .map(|(position, _)| position)
            .collect();
        let mut edits = Vec::new();
        for position in positions {
            check(cancelled)?;
            if original.get(position) != grid.get(position) {
                edits.push((position, grid.get(position).cloned()));
            }
        }
        progress(Progress {
            phase: Phase::Ready,
            completed: report.total,
            total: report.total,
        });
        check(cancelled)?;
        Ok(Preview {
            token: self.token,
            chart: self.chart,
            layout: source.layout,
            grid,
            edits,
            entries,
            average_nps,
            report,
            issues,
        })
    }
}
impl Preview {
    pub fn has_changes(&self) -> bool {
        !self.edits.is_empty()
    }
    pub fn grid(&self) -> &Grid {
        &self.grid
    }
    pub fn layout(&self) -> &str {
        &self.layout
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn average_nps(&self) -> f64 {
        self.average_nps
    }
    pub fn report(&self) -> Report {
        self.report
    }
    pub fn profile_issues(&self) -> &[super::profiles::Issue] {
        &self.issues
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid(
                "Pattern-set preview belongs to another chart or revision",
            ));
        }
        session.edit_generated_notes(chart, "Regenerate pattern set", self.edits.clone())
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Pattern-set generation cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
