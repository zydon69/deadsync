//! Scheduling helpers from ActionAutoGeneratePatterns (GrooveAuthor 48e18f71).
//! Immediate-next overlap and incremental counters are observable contracts:
//! an empty next region does not clip, and the last captured row is rescanned.
use super::{context::ContextEvent, editor_timing::EditorTimeline, pattern_job::Span};
use crate::Error;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub span: Span,
    pub spacing: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub region: Region,
    /// First, last and full requested step count before overlap clipping.
    pub steps: Option<(u64, u64, u64)>,
    /// First generated row of the immediate next region, when it cuts this one.
    pub cutoff: Option<u64>,
    pub retained_steps: u64,
}
pub struct Schedule {
    entries: Vec<Entry>,
    total_steps: u64,
    last_step: Option<u64>,
}
impl Schedule {
    /// Stable row ordering matches the action's chronological processing. The
    /// caller owns selection normalization; repeated markers remain repeated.
    pub fn new(
        regions: impl IntoIterator<Item = Region>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        let mut entries = Vec::new();
        for region in regions {
            check(cancelled)?;
            entries.push(Entry {
                steps: region.span.optional_steps(region.spacing)?,
                region,
                cutoff: None,
                retained_steps: 0,
            });
        }
        entries.sort_by_key(|entry| entry.region.span.start);
        let mut total_steps = 0u64;
        for index in 0..entries.len() {
            check(cancelled)?;
            let next_first = entries
                .get(index + 1)
                .and_then(|next| next.steps)
                .map(|steps| steps.0);
            let entry = &mut entries[index];
            if let Some((first, last, count)) = entry.steps {
                entry.cutoff = next_first.filter(|next| *next <= last);
                entry.retained_steps = match entry.cutoff {
                    Some(next) if next <= first => 0,
                    Some(next) => (next - first - 1) / entry.region.spacing + 1,
                    None => count,
                };
                total_steps = total_steps
                    .checked_add(entry.retained_steps)
                    .filter(|count| *count <= i32::MAX as u64)
                    .ok_or_else(|| invalid("Scheduled pattern steps exceed the reference range"))?;
            }
        }
        let last_step = entries
            .iter()
            .rev()
            .find_map(|entry| entry.steps.map(|steps| steps.1));
        check(cancelled)?;
        Ok(Self {
            entries,
            total_steps,
            last_step,
        })
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn total_steps(&self) -> u64 {
        self.total_steps
    }
    /// This is the final nonempty marker's last step, not the maximum row of
    /// every marker; a later short overlapping marker supersedes an earlier one.
    pub fn last_step(&self) -> Option<u64> {
        self.last_step
    }
    pub fn average_nps(&self, retained: &EditorTimeline) -> Result<f64, Error> {
        retained.patterns_nps(self.total_steps, self.last_step)
    }
}
impl Entry {
    pub fn keeps_row(&self, row: u64) -> bool {
        self.cutoff.is_none_or(|next| row < next)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneCounts {
    lanes: Vec<u64>,
    total: i64,
    last_row: Option<u64>,
}
impl LaneCounts {
    pub fn new(columns: usize) -> Result<Self, Error> {
        if columns == 0 || columns > u16::MAX as usize {
            return Err(invalid("Invalid lane-counter width"));
        }
        Ok(Self {
            lanes: vec![0; columns],
            total: 0,
            last_row: None,
        })
    }
    pub fn lanes(&self) -> &[u64] {
        &self.lanes
    }
    pub fn total(&self) -> i64 {
        self.total
    }
    pub fn last_row(&self) -> Option<u64> {
        self.last_row
    }
    /// Timing and marker events advance the captured row too. Keep the inclusive
    /// lower bound of the original enumerator, including its boundary recount.
    /// Validation/cancellation cannot leave partially advanced counters.
    pub fn advance(
        &mut self,
        events: &[ContextEvent],
        first_step: u64,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        check(cancelled)?;
        if first_step > i32::MAX as u64 {
            return Err(invalid("Pattern counter row exceeds the reference range"));
        }
        if self.last_row.is_some_and(|last| last >= first_step) {
            return Ok(());
        }
        let mut next = self.clone();
        let mut previous = None;
        for event in events {
            check(cancelled)?;
            if event.row > i32::MAX as u64
                || previous.is_some_and(|row| row > event.row)
                || event
                    .lane
                    .is_some_and(|lane| usize::from(lane) >= self.lanes.len())
                || (event.is_step && event.lane.is_none())
            {
                return Err(invalid("Invalid ordered pattern counter event"));
            }
            previous = Some(event.row);
            if event.row >= first_step {
                continue;
            }
            if self.last_row.is_some_and(|last| event.row < last) {
                continue;
            }
            next.last_row = Some(event.row);
            if event.is_step {
                next.total = next
                    .total
                    .checked_add(1)
                    .filter(|count| *count <= i64::from(i32::MAX))
                    .ok_or_else(|| invalid("Pattern counter exceeds the reference range"))?;
                next.lanes[usize::from(event.lane.unwrap())] += 1;
            }
        }
        check(cancelled)?;
        *self = next;
        Ok(())
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Pattern scheduling cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
