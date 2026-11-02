//! Density-only access to the initialized editor tree. Rendering clocks keep
//! their existing query phases; measure starts use the active same-row rate.
use super::*;
impl EditorTimeline {
    pub(crate) fn density_clock(
        timing: &GenerationTiming,
        grid: &Grid,
        attacks: &[u64],
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        Self::build_internal(timing, grid, None, attacks, true, cancelled)
    }
    pub(crate) fn density_last_event(&self) -> Result<(u64, u64), Error> {
        let Some(entry) = self.entries.last().copied() else {
            return Ok((0, 0));
        };
        let end = match entry.kind {
            EntryKind::Head { end: Some(end), .. } | EntryKind::Region { end } => end,
            EntryKind::Timing(EventKind::Warp(length) | EventKind::Fake(length)) => entry
                .row
                .checked_add(length as u64)
                .ok_or_else(|| Error::InvalidEdit("Density region endpoint overflow".into()))?,
            EntryKind::Timing(EventKind::Stop(length)) if length < 0.0 => {
                // Original negative-stop RefreshEndChartPosition takes the last subsequent
                // rate with time <= the stop start time; GetEndRow then truncates it.
                let mut end = entry.row as f64;
                for rate in self.rates.iter().filter(|r| r.row >= entry.row) {
                    if rate.time > entry.time {
                        break;
                    }
                    end = crate::rate_mapping::rate_position(
                        rate.row as f64,
                        rate.time,
                        rate.warp as f64,
                        rate.stop,
                        1.0 / rate.seconds_per_row,
                        entry.time,
                    );
                }
                if !end.is_finite() || end < 0.0 || end > i32::MAX as f64 {
                    return Err(Error::InvalidEdit(
                        "Density negative-stop endpoint out of range".into(),
                    ));
                }
                end as u64
            }
            _ => entry.row,
        };
        Ok((entry.row, end))
    }
    pub(crate) fn density_measure_times(
        &self,
        count: usize,
        cancelled: &AtomicBool,
    ) -> Result<Vec<f64>, Error> {
        let mut times = Vec::with_capacity(count);
        let mut cursor = 0;
        for m in 0..count {
            check(cancelled)?;
            let row = (m as u64)
                .checked_mul(192)
                .ok_or_else(|| Error::InvalidEdit("Density measure row overflow".into()))?;
            while cursor + 1 < self.rates.len() && self.rates[cursor + 1].row <= row {
                cursor += 1;
            }
            let time = self.rates.get(cursor).map_or(0.0, |r| r.position_time(row));
            finite(time)?;
            times.push(time);
        }
        Ok(times)
    }
}
