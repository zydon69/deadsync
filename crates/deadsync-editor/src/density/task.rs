//! Background source density snapshot, independent of RSSP's analysis worker.
use super::{Event, Series, StreamSettings};
use crate::{
    Error,
    document::{Document, Format},
    generation::metadata::{ChartData, Guid},
    generation::{
        editor_timing::EditorTimeline,
        job::{Job, Phase, Progress},
        timing::GenerationTiming,
    },
    history::{RevisionToken, Session},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Task {
    preview_grid: Option<crate::notes::Grid>,
    bytes: Arc<[u8]>,
    format: Format,
    load_context: crate::document::LoadContext,
    chart: usize,
    identity: Guid,
    token: RevisionToken,
    stream: StreamSettings,
}
/// Source StepTotals, using the same fake-aware event classification as density.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub notes: u64,
    pub rows: u64,
    pub holds: u64,
    pub rolls: u64,
    pub mines: u64,
    pub lifts: u64,
    pub fakes: u64,
    pub lanes: Vec<u64>,
    max_lane: u64,
}
impl Totals {
    fn collect(heads: &[crate::generation::editor_timing::TimedHead], columns: usize) -> Self {
        use crate::notes::Kind;
        let mut result = Self {
            lanes: vec![0; columns],
            ..Self::default()
        };
        let mut rows = std::collections::BTreeSet::new();
        for head in heads {
            if head.fake || head.kind == Kind::Fake {
                result.fakes += 1;
                continue;
            }
            match head.kind {
                Kind::Tap | Kind::Hold | Kind::Roll | Kind::Lift => {
                    result.notes += 1;
                    rows.insert(head.position.row);
                    if let Some(lane) = result.lanes.get_mut(usize::from(head.position.lane)) {
                        *lane += 1;
                        result.max_lane = result.max_lane.max(*lane);
                    }
                    match head.kind {
                        Kind::Hold => result.holds += 1,
                        Kind::Roll => result.rolls += 1,
                        Kind::Lift => result.lifts += 1,
                        _ => {}
                    }
                }
                Kind::Mine => result.mines += 1,
                _ => {}
            }
        }
        result.rows = rows.len() as u64;
        result
    }
    /// Original float percentages, banker's rounding, separate maximum normalization.
    pub fn distribution(&self, lane: usize) -> Option<(u32, u32)> {
        let count = *self.lanes.get(lane)?;
        let max = self.max_lane;
        if max == 0 {
            return Some((0, 0));
        }
        let percent = |total: u64| {
            ((100.0f32 * count as f32 / total as f32).round_ties_even() as u32).min(100)
        };
        Some((percent(max), percent(self.notes)))
    }
}
pub struct Report {
    pub totals: Totals,
    pub series: Series,
    pub step_plot: super::Plot,
    pub row_plot: super::Plot,
    clock: crate::playback::Clock,
    chart: usize,
    identity: Guid,
    token: RevisionToken,
}
impl Report {
    /// The original Use Stream action writes the source stream notation, including
    /// the empty string when there is no stream. Never substitute RSSP analysis.
    pub fn description_draft(
        &self,
        session: &Session,
        chart: usize,
    ) -> Result<crate::chart_properties::Draft, Error> {
        if !self.is_current(session, chart) {
            return Err(invalid(
                "Stream description report is stale or belongs to another chart",
            ));
        }
        let mut draft = crate::chart_properties::Draft::capture(session, chart)?;
        draft.set_text(
            crate::chart_text::Field::Description,
            self.series.breakdown.as_deref().unwrap_or(""),
        )?;
        Ok(draft)
    }
    pub fn chart_time(&self, row: u64) -> Result<f64, Error> {
        self.clock.chart_time(row)
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && session.accepts_token(&self.token)
    }
}
impl Task {
    pub fn capture(session: &Session, chart: usize, stream: StreamSettings) -> Result<Self, Error> {
        Ok(Self {
            preview_grid: None,
            bytes: session.serialize()?.into(),
            format: session.document().format,
            load_context: session.document().load_context(),
            chart,
            identity: session
                .chart_ids()
                .get(chart)
                .ok_or_else(|| invalid("Density chart does not exist"))?
                .clone(),
            token: session.revision_token(),
            stream,
        })
    }
    /// Immutable generated notes with timing/metadata bound to their source chart.
    pub fn capture_preview_grid(
        session: &Session,
        chart: usize,
        stream: StreamSettings,
        grid: &crate::notes::Grid,
    ) -> Result<Self, Error> {
        let mut task = Self::capture(session, chart, stream)?;
        task.preview_grid = Some(grid.clone());
        Ok(task)
    }
    pub fn spawn(self) -> Result<Job<Report>, Error> {
        Job::spawn_with("editor-source-density", move |cancelled, progress| {
            progress(Progress {
                phase: Phase::Preparing,
                completed: 0,
                total: 1,
            });
            let report = self.run(cancelled)?;
            progress(Progress {
                phase: Phase::Ready,
                completed: 1,
                total: 1,
            });
            Ok(report)
        })
    }
    pub fn run(self, cancelled: &AtomicBool) -> Result<Report, Error> {
        check(cancelled)?;
        let mut session = Session::new(Document::parse_with_context(
            self.bytes,
            self.format,
            self.load_context,
        )?);
        let timing = GenerationTiming::from_document(session.document(), self.chart)?;
        let attacks = crate::attacks::placement::Timeline::capture(&session, self.chart)?;
        let attack_rows: Vec<_> = attacks.markers().iter().map(|m| m.row).collect();
        let patterns = ChartData::read(session.document(), self.chart)?
            .and_then(|d| d.patterns)
            .unwrap_or_default();
        let regions: Vec<_> = patterns
            .iter()
            .map(|(row, p)| (*row, *row + p.length))
            .collect();
        let grid = match self.preview_grid.as_ref() {
            Some(grid) => grid,
            None => session.grid(self.chart)?,
        };
        if grid.iter().take(2_000_001).count() > 2_000_000 {
            return Err(invalid("Density note budget exceeded"));
        }
        let editor = EditorTimeline::density_clock(&timing, grid, &attack_rows, cancelled)?
            .with_markers(&regions, cancelled)?;
        let totals = Totals::collect(editor.heads(), usize::from(grid.columns()));
        let heads: BTreeMap<_, _> = editor
            .heads()
            .iter()
            .map(|h| {
                (
                    h.position,
                    (!h.fake
                        && matches!(
                            h.kind,
                            crate::notes::Kind::Tap
                                | crate::notes::Kind::Hold
                                | crate::notes::Kind::Roll
                                | crate::notes::Kind::Lift
                        )),
                )
            })
            .collect();
        let mut events: Vec<_> = editor
            .timed_events()
            .map(|e| Event {
                row: e.row,
                step: e.head.is_some_and(|p| heads.get(&p) == Some(&true)),
                fake: false,
            })
            .collect();
        let (last_start, mut end) = editor.density_last_event()?;
        let song = crate::song_properties::Draft::capture(&session)?;
        let clock = crate::playback::Clock::capture(&session, self.chart)?;
        let row_at = |seconds: f64| -> Result<u64, Error> {
            let row = clock.position(seconds)?;
            if row > i32::MAX as f64 {
                return Err(invalid("Density widget row exceeds source limit"));
            }
            Ok(row.max(0.0) as u64)
        };
        if song.text(crate::song_text::Field::PreviewMusic).is_empty() {
            events.push(Event {
                row: row_at(song.sample_start())?,
                step: false,
                fake: false,
            });
        }
        if song.supports_last_second_hint() && song.last_second_hint() > 0.0 {
            let row = row_at(song.last_second_hint())?;
            events.push(Event {
                row,
                step: false,
                fake: false,
            });
            if row > last_start {
                end = row;
            }
        }
        // A far preview is a count boundary but is excluded from GetEndPosition.
        // Do not allocate it beyond the source chart's final measure.
        let count =
            usize::try_from(end / 192 + 1).map_err(|_| invalid("Density measure overflow"))?;
        if count > 262144 {
            return Err(invalid("Density measure budget exceeded"));
        }
        events.retain(|e| e.row / 192 < count as u64);
        events.sort_by_key(|e| e.row);
        check(cancelled)?;
        let times = editor.density_measure_times(count, cancelled)?;
        let final_row = (count as u64)
            .checked_mul(192)
            .filter(|r| *r <= i32::MAX as u64)
            .ok_or_else(|| invalid("Density final row exceeds source limit"))?;
        let final_time = editor.position_time(final_row)?;
        let series =
            Series::build_checked(events, times, final_time, self.stream, 2_100_000, || {
                check(cancelled)
            })?;
        check(cancelled)?;
        let step_plot = series.plot(crate::density_preferences::Accumulation::Step, 2048)?;
        check(cancelled)?;
        let row_plot = series.plot(crate::density_preferences::Accumulation::Row, 2048)?;
        check(cancelled)?;
        Ok(Report {
            totals,
            series,
            step_plot,
            row_plot,
            clock,
            chart: self.chart,
            identity: self.identity,
            token: self.token,
        })
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn check(c: &AtomicBool) -> Result<(), Error> {
    if c.load(Ordering::Relaxed) {
        Err(invalid("Source density cancelled"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod totals_tests {
    use super::*;
    use crate::{
        generation::editor_timing::TimedHead,
        notes::{Kind, Position},
    };
    #[test]
    fn source_totals_count_players_lanes_fake_heads_and_bankers_percentages() {
        let head = |row, lane, player, kind, fake| TimedHead {
            position: Position { row, lane, player },
            kind,
            fake,
            time: row as f64 / 96.,
        };
        let heads = [
            head(0, 0, 0, Kind::Tap, false),
            head(0, 0, 1, Kind::Tap, false),
            head(0, 1, 0, Kind::Hold, false),
            head(48, 0, 0, Kind::Roll, false),
            head(96, 1, 0, Kind::Lift, false),
            head(144, 0, 0, Kind::Mine, false),
            head(192, 0, 0, Kind::Tap, true),
            head(192, 1, 0, Kind::Mine, true),
            head(240, 0, 0, Kind::Fake, false),
        ];
        let totals = Totals::collect(&heads, 4);
        assert_eq!(
            (
                totals.notes,
                totals.rows,
                totals.holds,
                totals.rolls,
                totals.lifts,
                totals.mines,
                totals.fakes
            ),
            (5, 3, 1, 1, 1, 1, 3)
        );
        assert_eq!(totals.lanes, vec![3, 2, 0, 0]);
        assert_eq!(totals.distribution(0), Some((100, 60)));
        assert_eq!(totals.distribution(1), Some((67, 40)));
        assert_eq!(totals.distribution(3), Some((0, 0)));
        assert_eq!(totals.distribution(4), None);
        let ties = Totals {
            notes: 8,
            lanes: vec![1, 7],
            max_lane: 7,
            ..Totals::default()
        };
        assert_eq!(ties.distribution(0), Some((14, 12)));
        assert_eq!(ties.distribution(1), Some((100, 88)));
    }
}
