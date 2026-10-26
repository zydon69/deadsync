//! Exact head query in row order, with separate crossing sustains and budgets.
//! Represented Tap/Fake/Lift, Hold/Roll and Mine heads follow the pinned
//! SMEventComparer order: row, lane, event class, player. Extension kinds
//! Keysound/Attack remain deterministic without a source parity claim.
use super::{Grid, Kind, Note, Position};
use crate::{Error, playback::Clock};
use std::{cmp::Reverse, collections::BinaryHeap};
fn type_rank(kind: Kind) -> u8 {
    // Editor Fake/Lift wrap LaneTapNote; Mine wraps LaneNote. The library
    // inserts Pattern after LaneTapNote but it is not part of this grid query.
    match kind {
        Kind::Tap | Kind::Fake | Kind::Lift => 0,
        Kind::Hold | Kind::Roll => 1,
        Kind::Mine => 2,
        Kind::Keysound | Kind::Attack => 3,
    }
}
pub struct Query<'a> {
    pub crossings: Vec<(Position, &'a Note)>,
    pub heads: Vec<(Position, &'a Note)>,
    pub complete: bool,
    pub tree_queries: usize,
}
impl Grid {
    /// One predecessor and one ordered range per occupied track, then a heap
    /// merge. No scan of notes before start, and no track-major head starvation.
    pub fn minimap_chronological(
        &self,
        start: u64,
        end: u64,
        head_limit: usize,
        track_limit: usize,
    ) -> Result<Query<'_>, Error> {
        if start > end
            || head_limit > 65536
            || track_limit > 4096
            || self.tracks.len() > track_limit
        {
            return Err(Error::InvalidEdit(
                "Invalid minimap interval or query budget".into(),
            ));
        }
        let mut result = Query {
            crossings: Vec::new(),
            heads: Vec::new(),
            complete: true,
            tree_queries: 0,
        };
        let mut ranges = Vec::with_capacity(self.tracks.len());
        let mut queue = BinaryHeap::new();
        for (&(player, lane), track) in &self.tracks {
            result.tree_queries += 2;
            if let Some((&row, note)) = track
                .range(..start)
                .next_back()
                .filter(|(_, note)| note.end.is_some_and(|tail| tail >= start))
            {
                result
                    .crossings
                    .push((Position { player, lane, row }, note));
            }
            let mut range = track.range(start..=end);
            if let Some((&row, note)) = range.next() {
                queue.push(Reverse((
                    row,
                    lane,
                    type_rank(note.kind),
                    player,
                    ranges.len(),
                )));
            }
            ranges.push(range);
        }
        result
            .crossings
            .sort_unstable_by_key(|(position, _)| (position.row, position.player, position.lane));
        while result.heads.len() < head_limit {
            let Some(Reverse((row, lane, _rank, player, index))) = queue.pop() else {
                break;
            };
            let position = Position { player, lane, row };
            let note = self
                .get(position)
                .expect("heap head belongs to the immutable grid");
            result.heads.push((position, note));
            result.tree_queries += 2;
            if let Some((&next, note)) = ranges[index].next() {
                queue.push(Reverse((next, lane, type_rank(note.kind), player, index)));
            }
        }
        result.complete = queue.is_empty();
        Ok(result)
    }
}
/// Inclusive note-row interval for a chart-time window. Binary-search the note
/// phase directly: inverse cursor time at a warp can omit equal-time note heads.
/// The editor timeline's note phase is nondecreasing, including warp/negative
/// stop plateaus. Uses at most2*log2(last_row+2) clock queries, not all note rows.
pub fn rows_for_chart_time(
    clock: &Clock,
    start: f64,
    end: f64,
    last_row: u64,
) -> Result<Option<(u64, u64)>, Error> {
    if !start.is_finite() || !end.is_finite() || start > end || last_row > i32::MAX as u64 {
        return Err(Error::InvalidEdit("Invalid minimap time window".into()));
    }
    fn bound(clock: &Clock, last: u64, time: f64, strict: bool) -> Result<u64, Error> {
        let mut low = 0;
        let mut high = last + 1;
        while low < high {
            let middle = low + (high - low) / 2;
            let candidate = clock.note_time(middle)?;
            if candidate < time || (strict && candidate == time) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }
    let first = bound(clock, last_row, start, false)?;
    let after = bound(clock, last_row, end, true)?;
    Ok((first < after).then(|| (first, after - 1)))
}
