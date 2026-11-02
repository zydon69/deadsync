//! Shared region interpretation/search used by individual and batch requests.
use super::*;
pub(crate) struct RegionGenerator<'a> {
    pub source: &'a source::SourceChart,
    pub graph: &'a StepGraph,
    pub pad: &'a Pad,
}
impl RegionGenerator<'_> {
    /// Full ActionAutoGeneratePatterns contract, including its second enumeration.
    /// The first identical marker's notes are all clipped by its successor, but
    /// its interpretation still updates the inclusive preceding counters.
    pub fn generate_action(
        &self,
        options: &Options,
        grid: &Grid,
        timeline: &EditorTimeline,
        average_nps: f64,
        counts: &mut LaneCounts,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize, usize),
    ) -> Result<pattern::PatternPreview, Error> {
        drop(self.generate(options, grid, timeline, average_nps, counts, cancelled, |_, _| {}));
        check(cancelled)?;
        self.generate(options, grid, timeline, average_nps, counts, cancelled, progress)
    }

    pub fn generate(
        &self,
        options: &Options,
        grid: &Grid,
        timeline: &EditorTimeline,
        average_nps: f64,
        counts: &mut LaneCounts,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize, usize),
    ) -> Result<pattern::PatternPreview, Error> {
        let spacing = ROWS_PER_BEAT / u64::from(options.pattern.beat_sub_division);
        let (start, end, num_steps) = options.span.steps(spacing)?;
        let events = timeline.context_events();
        counts.advance(&events, start, cancelled)?;
        let extension = if options.performed.transitions.enabled == Some(true) {
            options
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
        let following = timeline.prepare_region(grid, Some((end + 1, range_end)), cancelled)?;
        let after = expressed::interpret(
            self.graph,
            self.pad,
            &following.request(self.source.difficulty),
            &options.expressed,
            cancelled,
            |_, _, _| {},
        )?;
        let preceding = timeline.prepare_region(
            grid,
            start.checked_sub(1).map(|last| (range_start, last)),
            cancelled,
        )?;
        let before = expressed::interpret(
            self.graph,
            self.pad,
            &preceding.request(self.source.difficulty),
            &options.expressed,
            cancelled,
            |_, _, _| {},
        )?;
        let total_steps_before = counts.total();
        let mut lane_counts = counts.lanes().to_vec();
        if options.ignore_preceding_distribution {
            lane_counts.fill(0);
        }
        let context = context::derive(
            self.graph,
            self.pad,
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
                config: &options.performed,
            },
            lane_counts,
            cancelled,
        )?;
        pattern::generate(
            &PatternRequest {
                layout: self.source.layout.clone(),
                start,
                end,
                seed: options.seed,
                previous_foot: context.previous_foot,
                previous_footing: context.previous_footing,
                following_footing: context.following_footing,
                context: context.root,
                config: options.performed.clone(),
                pattern: options.pattern.clone(),
                average_nps,
            },
            &self.source.timing,
            cancelled,
            progress,
        )
    }
}
