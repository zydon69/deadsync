//! Snapshot-bound single-region pattern preparation. Contexts retain the
//! editor's assigned times; newly generated steps use the reference Fumen clock.
use super::{
    config::{ExpressedConfig, PatternConfig, PerformedConfig},
    context::{self, ContextRequest},
    editor_timing::EditorTimeline,
    expressed,
    graph::StepGraph,
    pattern::{self, PatternRequest},
    pattern_region::{self, Deletions},
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
        if spacing == 0 || self.start > self.end || self.end > i32::MAX as u64 {
            return invalid("Invalid pattern marker range");
        }
        let first = self
            .start
            .checked_add(if self.include_start { 0 } else { spacing })
            .ok_or_else(|| Error::InvalidEdit("Pattern range overflow".into()))?;
        let mut last = self.start + (self.end - self.start) / spacing * spacing;
        if !self.include_end && last == self.end {
            last = last
                .checked_sub(spacing)
                .ok_or_else(|| Error::InvalidEdit("Pattern range contains no steps".into()))?;
        }
        if first > last {
            return invalid("Pattern range contains no steps");
        }
        Ok((first, last, (last - first) / spacing + 1))
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
}
pub struct Preview {
    token: RevisionToken,
    chart: usize,
    generated: pattern::PatternPreview,
    grid: Grid,
    deletions: Deletions,
    average_nps: f64,
}
impl Task {
    pub fn capture(session: &Session, chart: usize, options: Options) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return invalid("Pattern source chart does not exist");
        }
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
        })
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
        self,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize, usize),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let source = source::read(self.bytes.clone(), self.format, self.chart, cancelled)?;
        let graph = StepGraph::builtin(&source.layout)?;
        let pad = Pad::builtin(&source.layout)?;
        let spacing = ROWS_PER_BEAT / u64::from(self.options.pattern.beat_sub_division);
        let (start, end, num_steps) = self.options.span.steps(spacing)?;
        let timeline = EditorTimeline::build(&source.timing, &source.grid, None, cancelled)?;
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
        let average_nps = projected.pattern_nps(num_steps, end)?;
        let events = projected.context_events();
        let extension = if self.options.performed.transitions.enabled == Some(true) {
            self.options
                .performed
                .transitions
                .steps_per_transition_min
                .max(0) as usize
        } else {
            0
        };
        let (range_start, range_end) =
            pattern_region::bounds(&events, start, end, extension, cancelled)?;
        // The original action interprets following events first, independently.
        let following =
            projected.prepare_region(&retained, Some((end + 1, range_end)), cancelled)?;
        let after = expressed::interpret(
            &graph,
            pad,
            &following.request(source.difficulty),
            &self.options.expressed,
            cancelled,
            |_, _, _| {},
        )?;
        let preceding = projected.prepare_region(
            &retained,
            start.checked_sub(1).map(|last| (range_start, last)),
            cancelled,
        )?;
        let before = expressed::interpret(
            &graph,
            pad,
            &preceding.request(source.difficulty),
            &self.options.expressed,
            cancelled,
            |_, _, _| {},
        )?;
        let mut lane_counts = vec![0; graph.columns()];
        let mut total_steps_before = 0;
        for event in events
            .iter()
            .filter(|event| event.row < start && event.is_step)
        {
            check(cancelled)?;
            lane_counts[usize::from(event.lane.unwrap())] += 1;
            total_steps_before += 1;
        }
        if self.options.ignore_preceding_distribution {
            lane_counts.fill(0);
        }
        let context = context::derive(
            &graph,
            pad,
            &ContextRequest {
                start,
                end,
                num_steps,
                total_steps_before,
                range_starts_after_chart: range_start > 0,
                preceding: &before.last,
                following: &after.last,
                preceding_events: &preceding.editor_events,
                following_events: &following.editor_events,
                config: &self.options.performed,
            },
            lane_counts,
            cancelled,
        )?;
        let generated = pattern::generate(
            &PatternRequest {
                layout: source.layout,
                start,
                end,
                seed: self.options.seed,
                previous_foot: context.previous_foot,
                previous_footing: context.previous_footing,
                following_footing: context.following_footing,
                context: context.root,
                config: self.options.performed,
                pattern: self.options.pattern,
                average_nps,
            },
            &source.timing,
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
        })
    }
}
impl Preview {
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
        self.generated
            .apply(session, self.chart, 0, session.revision())
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
