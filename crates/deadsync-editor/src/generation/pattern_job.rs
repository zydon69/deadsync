//! Snapshot-bound single-region pattern preparation. Contexts retain the
//! editor's assigned times; newly generated steps use the reference Fumen clock.
mod region;
pub(crate) use region::RegionGenerator;

use super::{
    config::{ExpressedConfig, PatternConfig, PerformedConfig},
    context::{self, ContextRequest},
    editor_timing::EditorTimeline,
    expressed,
    graph::StepGraph,
    pattern::{self, PatternRequest},
    pattern_region::{self, Deletions},
    pattern_schedule::{LaneCounts, Region, Schedule},
    source,
};
use crate::{
    Error,
    document::Format,
    history::{RevisionToken, Session},
    notes::{Grid, ROWS_PER_BEAT},
    pad::Pad,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start: u64,
    pub end: u64,
    pub include_start: bool,
    pub include_end: bool,
}
impl Span {
    /// GrooveAuthor's pattern markers are anchored to their start row, rather
    /// than rounded to the global subdivision grid.
    pub fn steps(self, spacing: u64) -> Result<(u64, u64, u64), Error> {
        self.optional_steps(spacing)?
            .ok_or_else(|| Error::InvalidEdit("Pattern range contains no steps".into()))
    }
    /// Empty regions are valid for clear actions, which skip them like the reference.
    pub fn optional_steps(self, spacing: u64) -> Result<Option<(u64, u64, u64)>, Error> {
        if spacing == 0 || self.start > self.end || self.end > i32::MAX as u64 {
            return invalid("Invalid pattern marker range");
        }
        let first = self
            .start
            .checked_add(if self.include_start { 0 } else { spacing })
            .ok_or_else(|| Error::InvalidEdit("Pattern range overflow".into()))?;
        let mut last = self.start + (self.end - self.start) / spacing * spacing;
        if !self.include_end && last == self.end {
            let Some(previous) = last.checked_sub(spacing) else {
                return Ok(None);
            };
            last = previous;
        }
        if first > last {
            return Ok(None);
        }
        Ok(Some((first, last, (last - first) / spacing + 1)))
    }
}
#[derive(Clone, Debug)]
pub struct Options {
    pub span: Span,
    pub seed: i32,
    pub expressed: ExpressedConfig,
    pub performed: PerformedConfig,
    pub pattern: PatternConfig,
    pub ignore_preceding_distribution: bool,
}
pub struct Task {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    chart: usize,
    options: Options,
    persistence: Option<Persistence>,
}
struct Persistence {
    directory: Option<std::path::PathBuf>,
    inherit_expression: bool,
}
pub struct Preview {
    token: RevisionToken,
    chart: usize,
    generated: pattern::PatternPreview,
    grid: Grid,
    deletions: Deletions,
    average_nps: f64,
    profile_issues: Vec<super::profiles::Issue>,
    persistence: Option<super::pattern_persistence::Plan>,
}
impl Task {
    pub fn capture(session: &Session, chart: usize, options: Options) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return invalid("Pattern source chart does not exist");
        }
        source::require_reference_source(session.document(), chart)?;
        options.pattern.validate()?;
        options.performed.validate()?;
        options.expressed.validate()?;
        options
            .span
            .steps(ROWS_PER_BEAT / u64::from(options.pattern.beat_sub_division))?;
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            chart,
            options,
            persistence: None,
        })
    }
    pub fn capture_persistent(
        session: &Session,
        chart: usize,
        options: Options,
        directory: Option<&std::path::Path>,
    ) -> Result<Self, Error> {
        let mut task = Self::capture(session, chart, options)?;
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        task.persistence = Some(Persistence {
            directory,
            inherit_expression: false,
        });
        Ok(task)
    }
    pub fn with_chart_expression(mut self) -> Self {
        if let Some(persistence) = &mut self.persistence {
            persistence.inherit_expression = true;
        }
        self
    }

    pub fn capture_saved(
        session: &Session,
        chart: usize,
        row: u64,
        registry: &super::metadata::Registry,
    ) -> Result<Self, Error> {
        let data = super::metadata::ChartData::read(session.document(), chart)?
            .ok_or_else(|| Error::InvalidEdit("No saved pattern markers in this chart".into()))?;
        Self::capture(session, chart, data.options(row, registry)?)
    }

    pub fn spawn(self) -> Result<super::job::Job<Preview>, Error> {
        use super::job::{Job, Phase, Progress};
        Job::spawn_with("editor-pattern-generation", move |cancelled, progress| {
            let result = self.run(cancelled, |completed, total| {
                progress(Progress {
                    phase: Phase::Converting,
                    completed,
                    total,
                })
            })?;
            progress(Progress {
                phase: Phase::Ready,
                completed: 1,
                total: 1,
            });
            Ok(result)
        })
    }
    /// Runs against owned snapshot bytes and never mutates a live session.
    pub fn run(
        mut self,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize, usize),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let source = source::read(self.bytes.clone(), self.format, self.chart, cancelled)?;
        let mut profile_issues = Vec::new();
        let persistence = if let Some(request) = &self.persistence {
            let loaded = if let Some(path) = &request.directory {
                super::profiles::Loaded::directory_checked(path, cancelled)?
            } else {
                super::profiles::Loaded {
                    registry: super::metadata::Registry::builtin(),
                    profiles: std::collections::BTreeMap::new(),
                    issues: Vec::new(),
                }
            };
            let data = source.initial_editor_metadata()?;
            if request.inherit_expression {
                self.options.expressed = loaded
                    .registry
                    .expressed
                    .get(&data.expressed_chart_config)
                    .cloned()
                    .ok_or_else(|| {
                        Error::Unsupported(format!(
                            "Missing expressed generation profile {}",
                            data.expressed_chart_config
                        ))
                    })?;
            }
            let plan = super::pattern_persistence::Plan::create(
                data,
                &self.options,
                &loaded,
                request.directory.clone(),
            )?;
            profile_issues = loaded.issues;
            Some(plan)
        } else {
            None
        };
        check(cancelled)?;
        let graph = StepGraph::builtin(&source.layout)?;
        let pad = Pad::builtin(&source.layout)?;
        let spacing = ROWS_PER_BEAT / u64::from(self.options.pattern.beat_sub_division);
        let (start, end, _) = self.options.span.steps(spacing)?;
        let regions = source.pattern_regions(cancelled)?;
        if regions
            .iter()
            .any(|&(row, end)| row == self.options.span.start && end != self.options.span.end)
        {
            return invalid("Existing marker bounds differ; edit the marker before regeneration");
        }
        let timeline = source.editor_timeline(cancelled)?
            .with_markers(&regions, cancelled)?;
        let (edits, deletions) =
            pattern_region::deletion_edits(&source.grid, 0, start, end, spacing, cancelled)?;
        let mut retained = source.grid.clone();
        for (position, note) in edits {
            check(cancelled)?;
            retained.set(position, note)?;
        }
        let projected = timeline.project(
            &retained,
            Some((self.options.span.start, self.options.span.end)),
            cancelled,
        )?;
        let schedule = Schedule::new(
            [Region {
                span: self.options.span,
                spacing,
            }],
            cancelled,
        )?;
        let average_nps = schedule.average_nps(&projected)?;
        let mut counts = LaneCounts::new(graph.columns())?;
        let generated = RegionGenerator {
            source: &source,
            graph: &graph,
            pad,
        }
        .generate(
            &self.options,
            &retained,
            &projected,
            average_nps,
            &mut counts,
            cancelled,
            progress,
        )?;
        check(cancelled)?;
        // Assemble via the same edit path as final acceptance, in an isolated
        // session. This checks collisions and keeps preview and apply identical.
        let mut snapshot = Session::new(crate::document::Document::parse(self.bytes, self.format)?);
        let revision = snapshot.revision();
        generated.apply(&mut snapshot, self.chart, 0, revision)?;
        let grid = snapshot.grid(self.chart)?.clone();
        check(cancelled)?;
        Ok(Preview {
            token: self.token,
            chart: self.chart,
            generated,
            grid,
            deletions,
            average_nps,
            profile_issues,
            persistence,
        })
    }
}
impl Preview {
    pub fn persists_marker(&self) -> bool {
        self.persistence.is_some()
    }
    pub fn profile_issues(&self) -> &[super::profiles::Issue] {
        &self.profile_issues
    }
    pub fn grid(&self) -> &Grid {
        &self.grid
    }
    pub fn deletions(&self) -> Deletions {
        self.deletions
    }
    pub fn average_nps(&self) -> f64 {
        self.average_nps
    }
    pub fn generated(&self) -> &pattern::PatternPreview {
        &self.generated
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_token(&self.token)
    }
    pub fn apply(&self, session: &mut Session, selected_chart: usize) -> Result<bool, Error> {
        if selected_chart != self.chart || !self.is_current(session) {
            return invalid("Pattern preview belongs to another chart or revision");
        }
        if let Some(plan) = &self.persistence {
            session.edit_chart_atomic(self.chart, "Generate pattern", |temporary| {
                self.generated
                    .apply(temporary, self.chart, 0, temporary.revision())?;
                plan.data.save(temporary, self.chart)?;
                plan.ensure_profiles()?;
                Ok(())
            })
        } else {
            self.generated
                .apply(session, self.chart, 0, session.revision())
        }
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        return invalid("Pattern preparation cancelled");
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}

/// Profile files are loaded on the generation worker, never in a key handler.
pub struct SavedTask {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    chart: usize,
    row: u64,
    directory: std::path::PathBuf,
}
impl SavedTask {
    pub fn capture(
        session: &Session,
        chart: usize,
        row: u64,
        directory: &std::path::Path,
    ) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return invalid("Pattern source chart does not exist");
        }
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            chart,
            row,
            directory: if directory.is_absolute() {
                directory.to_owned()
            } else {
                std::env::current_dir()?.join(directory)
            },
        })
    }
    pub fn run(
        self,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize, usize),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let loaded = super::profiles::Loaded::directory_checked(&self.directory, cancelled)?;
        check(cancelled)?;
        let document = crate::document::Document::parse(self.bytes.clone(), self.format)?;
        let metadata = super::metadata::ChartData::read(&document, self.chart)?
            .ok_or_else(|| Error::InvalidEdit("No saved pattern markers in this chart".into()))?;
        let options = metadata.options(self.row, &loaded.registry)?;
        let issues = loaded.issues;
        drop(loaded.profiles);
        drop(loaded.registry);
        let mut result = Task {
            bytes: self.bytes,
            format: self.format,
            token: self.token,
            chart: self.chart,
            options,
            persistence: None,
        }
        .run(cancelled, progress)?;
        result.profile_issues = issues;
        check(cancelled)?;
        Ok(result)
    }
    pub fn spawn(self) -> Result<super::job::Job<Preview>, Error> {
        use super::job::{Job, Phase, Progress};
        Job::spawn_with(
            "editor-saved-pattern-generation",
            move |cancelled, progress| {
                let result = self.run(cancelled, |completed, total| {
                    progress(Progress {
                        phase: Phase::Converting,
                        completed,
                        total,
                    })
                })?;
                progress(Progress {
                    phase: Phase::Ready,
                    completed: 1,
                    total: 1,
                });
                Ok(result)
            },
        )
    }
}
