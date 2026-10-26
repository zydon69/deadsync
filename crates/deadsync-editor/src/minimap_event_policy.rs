//! Event enumeration decisions from Editor.UpdateMiniMap at 48e18f71.
//! Caller owns chronological iteration and the bounded crossing-sustain query.
use crate::notes::Kind;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Short,
    Mine,
    Sustain { roll: bool },
}
pub fn note_shape(kind: Kind) -> Option<Shape> {
    match kind {
        Kind::Tap | Kind::Fake | Kind::Lift => Some(Shape::Short),
        Kind::Mine => Some(Shape::Mine),
        Kind::Hold => Some(Shape::Sustain { roll: false }),
        Kind::Roll => Some(Shape::Sustain { roll: true }),
        Kind::Keysound | Kind::Attack => None,
    }
}
/// Source includes all crossing sustains first, then checks `count > maximum`
/// after adding each ordinary event. Thus it can draw maximum+1 normal notes;
/// callers must report incompleteness rather than silently dropping this fact.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    count: usize,
    maximum: usize,
}
impl Budget {
    pub fn new(maximum: usize, crossing_sustains: usize) -> Self {
        Self {
            count: crossing_sustains,
            maximum,
        }
    }
    pub fn count(self) -> usize {
        self.count
    }
    /// Call after the event's Add operation. `below_bottom` refers to that
    /// operation's result, not a row/time guess (warps can share coordinates).
    pub fn stop_after(&mut self, kind: Kind, below_bottom: bool) -> bool {
        let rendered = note_shape(kind).is_some();
        if rendered {
            self.count = self.count.saturating_add(1);
        }
        (rendered && below_bottom) || self.count > self.maximum
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    pub start_row: u64,
    pub end_row: u64,
    pub start_time: f64,
    pub end_time: f64,
}
/// Input must follow the original chart's pattern ordering. Only exact row
/// adjacency merges; overlap and equal timestamps do not imply adjacency.
pub fn merge_adjacent(patterns: impl IntoIterator<Item = Pattern>) -> Vec<Pattern> {
    let mut output: Vec<Pattern> = Vec::new();
    for pattern in patterns {
        if let Some(previous) = output
            .last_mut()
            .filter(|previous| previous.end_row == pattern.start_row)
        {
            previous.end_row = pattern.end_row;
            previous.end_time = pattern.end_time;
        } else {
            output.push(pattern);
        }
    }
    output
}
pub fn show_label(enabled: bool, row: u64) -> bool {
    enabled && row != 0
}
