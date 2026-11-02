//! Validated chart snapshot for source assist/beat scheduling.
use super::*;
use crate::{
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    history::Session,
    notes::{Grid, Kind},
    playback,
};
use std::sync::atomic::AtomicBool;
pub struct ChartTicks {
    clock: playback::Clock,
    timeline: Timeline,
}
impl ChartTicks {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let document = session.document();
        let descriptor = document.charts().get(chart).ok_or_else(invalid)?;
        let parsed;
        let grid = if let Some(grid) = session.loaded_grid(chart) {
            grid
        } else {
            parsed = Grid::parse_layout(
                document.value(descriptor.notes.clone()),
                &descriptor.steps_type,
            )?;
            &parsed
        };
        let timing = GenerationTiming::from_document(document, chart)?;
        let editor = EditorTimeline::density_clock(&timing, grid, &[], &AtomicBool::new(false))?;
        let timeline = Timeline::new(editor.heads().iter().map(|head| Head {
            row: head.position.row as i64,
            chart_time: head.time,
            step: matches!(head.kind, Kind::Tap | Kind::Hold | Kind::Roll | Kind::Lift),
            fake: head.fake,
        }))?;
        Ok(Self {
            clock: playback::Clock::capture(session, chart)?,
            timeline,
        })
    }
    pub fn window(&self, audio: EffectiveAudio, input: Input) -> Result<Window, Error> {
        self.timeline.window(&self.clock, audio, input)
    }
}
impl Clock for playback::Clock {
    fn beat_span_end(
        &self,
        row: i64,
        expected: i64,
        attack: f64,
        rate: u32,
        window_end: f64,
    ) -> Result<i64, Error> {
        let boundary = self.tick_mapping_end(row)?.max(row);
        let limit =
            u64::try_from((i128::from(boundary) - i128::from(row)) / 48).map_err(|_| invalid())?;
        let at = |step: u64| -> Result<i64, Error> {
            i64::try_from(i128::from(row) + i128::from(step) * 48).map_err(|_| invalid())
        };
        let valid = |step: u64| -> Result<bool, Error> {
            // A probe beyond the finite timeline is outside this finite
            // window; do not reject a valid near cursor because the section
            // extends to an unrepresentable distant extrapolation.
            let Ok(time) = self.fractional_music_time(at(step)? as f64) else {
                return Ok(false);
            };
            Ok(time < window_end && sample(time - attack, rate)? == expected)
        };
        if limit == 0 {
            return Ok(row);
        }
        if valid(limit)? {
            return at(limit);
        }
        let (mut lo, mut hi) = (0u64, limit);
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if valid(mid)? {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        at(lo)
    }
    fn chart_from_song(&self, seconds: f64) -> Result<f64, Error> {
        self.music_to_chart_time(seconds)
    }
    fn song_from_chart(&self, seconds: f64) -> Result<f64, Error> {
        self.chart_time_to_music(seconds)
    }
    fn row_from_song(&self, seconds: f64) -> Result<f64, Error> {
        self.position(seconds)
    }
    fn song_from_row(&self, row: i64) -> Result<f64, Error> {
        self.chart_time_to_music(self.fractional_chart_time(row as f64)?)
    }
}
