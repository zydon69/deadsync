//! Pattern start is the Fumen.Pattern event phase (after LaneTapNote);
//! EditorPatternEvent.GetEndChartTime uses chart-position conversion before
//! every equal-row rate event, as in the original EditorChart method.
use crate::{Error, minimap_event_policy::Pattern, playback::Clock};
pub fn capture(clock: &Clock, start: u64, length: u64) -> Result<Pattern, Error> {
    let end = start
        .checked_add(length)
        .ok_or_else(|| Error::InvalidEdit("Minimap pattern end overflow".into()))?;
    Ok(Pattern {
        start_row: start,
        end_row: end,
        start_time: clock.note_time(start)?,
        end_time: clock.fractional_chart_time(end as f64)?,
    })
}
