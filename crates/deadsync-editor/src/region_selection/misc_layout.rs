//! Shared rectangles for drawing and selecting miscellaneous chart widgets.
//!
//! The insertion algorithm intentionally follows GrooveAuthor's widget manager:
//! positioning a widget can move widgets inserted earlier, and a second widget
//! of the same kind replaces only the row's reference, not the old rectangle.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Signature,
    Stop,
    Delay,
    Warp,
    Tick,
    Combo,
    Label,
    Attack,
    Preview,
    EndHint,
    Pattern,
    Tempo,
    Scroll,
    Speed,
    Fake,
}
impl Kind {
    fn side_and_order(self) -> (bool, usize) {
        match self {
            Self::Signature => (false, 0),
            Self::Stop => (false, 1),
            Self::Delay => (false, 2),
            Self::Warp => (false, 3),
            Self::Tick => (false, 4),
            Self::Combo => (false, 5),
            Self::Label => (false, 6),
            Self::Attack => (false, 7),
            Self::Preview => (true, 0),
            Self::EndHint => (true, 1),
            Self::Pattern => (true, 2),
            Self::Tempo => (true, 3),
            Self::Scroll => (true, 4),
            Self::Speed => (true, 5),
            Self::Fake => (true, 6),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidGeometry {
    NonFinite,
    NonPositiveSize,
    NegativePadding,
}

/// A frame owns all rectangles. Callers must read them after all insertions,
/// rather than retaining a rectangle returned by an earlier insertion.
#[derive(Clone, Debug)]
pub struct Layout {
    left: f64,
    right: f64,
    max_width: i32,
    padding: f64,
    rows: BTreeMap<u64, ([Option<u64>; 8], [Option<u64>; 8])>,
    rectangles: BTreeMap<u64, Rect>,
}
impl Layout {
    pub fn new(
        left: f64,
        right: f64,
        max_width: i32,
        padding: f64,
    ) -> Result<Self, InvalidGeometry> {
        if !left.is_finite() || !right.is_finite() || !padding.is_finite() {
            return Err(InvalidGeometry::NonFinite);
        }
        if padding < 0.0 {
            return Err(InvalidGeometry::NegativePadding);
        }
        Ok(Self {
            left,
            right,
            max_width,
            padding,
            rows: BTreeMap::new(),
            rectangles: BTreeMap::new(),
        })
    }
    pub fn clear(&mut self) {
        self.rows.clear();
        self.rectangles.clear();
    }
    pub fn rectangle(&self, id: u64) -> Option<Rect> {
        self.rectangles.get(&id).copied()
    }
    pub fn rectangles(&self) -> impl Iterator<Item = (u64, Rect)> + '_ {
        self.rectangles.iter().map(|(&id, &rect)| (id, rect))
    }
    pub fn maximum_single_row_height(&self, height: f64) -> Result<f64, InvalidGeometry> {
        if !height.is_finite() {
            return Err(InvalidGeometry::NonFinite);
        }
        if height <= 0.0 {
            return Err(InvalidGeometry::NonPositiveSize);
        }
        Ok(8.0 * height + 7.0 * self.padding)
    }
    fn advance(&self, current: Rect, previous: Rect, right: bool) -> (f64, f64) {
        let mut x = if right {
            previous.x + previous.width + self.padding
        } else {
            previous.x - current.width - self.padding
        };
        let mut y = previous.y;
        if right && x + current.width > self.right + f64::from(self.max_width) {
            x = self
                .right
                .min(self.right + f64::from(self.max_width) - current.width);
            y += current.height + self.padding;
        } else if !right && x < self.left - f64::from(self.max_width) {
            x = (self.left - current.width).max(self.left - f64::from(self.max_width));
            y += current.height + self.padding;
        }
        (x, y)
    }
    /// `key` is chart time in ConstantTime, chart position otherwise. `row_y`
    /// must come from the same projection used by the chart renderer.
    pub fn position(
        &mut self,
        id: u64,
        kind: Kind,
        key: f64,
        row_y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), InvalidGeometry> {
        if ![key, row_y, width, height].iter().all(|n| n.is_finite()) {
            return Err(InvalidGeometry::NonFinite);
        }
        if width <= 0.0 || height <= 0.0 {
            return Err(InvalidGeometry::NonPositiveSize);
        }
        // C# Dictionary<double,...> considers -0 and +0 equal.
        let key = if key == 0.0 { 0 } else { key.to_bits() };
        let (right, order) = kind.side_and_order();
        let row = self.rows.entry(key).or_insert(([None; 8], [None; 8]));
        let mut events = if right { row.1 } else { row.0 };
        self.rectangles.insert(
            id,
            Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
        );
        let mut x = if right {
            self.right
                .min(self.right + f64::from(self.max_width) - width)
        } else {
            (self.left - width).max(self.left - f64::from(self.max_width))
        };
        let mut y = row_y;
        let mut y_min = y;
        let mut y_max = y;
        let mut added = false;
        let mut previous = None;
        for i in 0..if right { 7 } else { 8 } {
            let Some(current_id) = events[i] else {
                continue;
            };
            if i < order {
                let current = self.rectangles[&current_id];
                previous = Some(current_id);
                (x, y) = self.advance(self.rectangles[&id], current, right);
                y_min = current.y.min(y_min);
                y_max = (current.y + current.height).max(y_max);
            } else if i > order {
                if !added {
                    let rect = self
                        .rectangles
                        .get_mut(&id)
                        .expect("positioned widget exists");
                    rect.x = x;
                    rect.y = y;
                    events[order] = Some(id);
                    added = true;
                    previous = Some(id);
                    y_min = y.min(y_min);
                    y_max = (y + height).max(y_max);
                }
                let current = self.rectangles[&current_id];
                // In the source, inserting the new widget always establishes
                // a predecessor before any later kind is shifted.
                let prior = self.rectangles[&previous.expect("preceding widget established")];
                (x, y) = self.advance(current, prior, right);
                let rect = self
                    .rectangles
                    .get_mut(&current_id)
                    .expect("row reference exists");
                rect.x = x;
                rect.y = y;
                previous = Some(current_id);
                y_min = y.min(y_min);
                y_max = (y + rect.height).max(y_max);
            }
        }
        if !added {
            let rect = self
                .rectangles
                .get_mut(&id)
                .expect("positioned widget exists");
            rect.x = x;
            rect.y = y;
            events[order] = Some(id);
            y_min = y.min(y_min);
            y_max = (y + height).max(y_max);
        }
        let top = row_y - (y_max - y_min) * 0.5;
        for current in events.iter().flatten() {
            let rect = self
                .rectangles
                .get_mut(current)
                .expect("row reference exists");
            rect.y = rect.y - y_min + top;
        }
        let row = self.rows.get_mut(&key).expect("row established");
        if right {
            row.1 = events;
        } else {
            row.0 = events;
        }
        Ok(())
    }
}
