//! Index for visible pattern regions. Built when document metadata changes,
//! never by the render loop. Subtrees entirely outside the viewport are skipped.
use crate::Error;

#[derive(Clone, Debug, Default)]
pub struct RegionIndex {
    regions: Vec<(u64, u64)>,
    maximum_end: Vec<u64>,
    base: usize,
}
impl RegionIndex {
    pub fn new(mut regions: Vec<(u64, u64)>) -> Result<Self, Error> {
        if regions.iter().any(|(start, end)| start > end) {
            return Err(Error::InvalidEdit("Inverted pattern region".into()));
        }
        regions.sort_unstable();
        let base = regions
            .len()
            .max(1)
            .checked_next_power_of_two()
            .ok_or_else(|| Error::InvalidEdit("Too many pattern regions".into()))?;
        let size = base
            .checked_mul(2)
            .ok_or_else(|| Error::InvalidEdit("Too many pattern regions".into()))?;
        let mut maximum_end = vec![0; size];
        for (index, &(_, end)) in regions.iter().enumerate() {
            maximum_end[base + index] = end;
        }
        for node in (1..base).rev() {
            maximum_end[node] = maximum_end[node * 2].max(maximum_end[node * 2 + 1]);
        }
        Ok(Self {
            regions,
            maximum_end,
            base,
        })
    }
    pub fn last_row(&self) -> u64 {
        self.maximum_end.get(1).copied().unwrap_or(0)
    }
    /// Sorted, deterministic visible regions, including crossing boundaries.
    /// At most `limit` regions are returned, bounding actor work in overviews.
    /// Overlapping regions beyond this presentation budget remain in metadata.
    pub fn visible(&self, start: u64, end: u64, limit: usize) -> Result<Vec<(u64, u64)>, Error> {
        if start > end || !(1..=4096).contains(&limit) {
            return Err(Error::InvalidEdit(
                "Invalid region viewport or budget".into(),
            ));
        }
        let mut result = Vec::new();
        if self.regions.is_empty() {
            return Ok(result);
        }
        let mut stack = vec![(1, 0, self.base)];
        while let Some((node, left, right)) = stack.pop() {
            if left >= self.regions.len()
                || self.regions[left].0 > end
                || self.maximum_end[node] < start
            {
                continue;
            }
            if right - left == 1 {
                result.push(self.regions[left]);
                if result.len() == limit {
                    break;
                }
            } else {
                let middle = left + (right - left) / 2;
                stack.push((node * 2 + 1, middle, right));
                stack.push((node * 2, left, middle));
            }
        }
        Ok(result)
    }
}
