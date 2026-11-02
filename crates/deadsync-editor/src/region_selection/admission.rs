//! UpdateChartEvents admission policy, separate from spatial candidate discovery.
//! A next-rate transition bypasses the ordinary terminal quota check. Existing
//! crossing holds have already entered the source's visible-event list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Lane,
    Misc,
    Other,
    NextRate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub admit: bool,
    pub stop: bool,
}
pub struct Admission {
    max: i32,
    count: usize,
    misc: bool,
}
impl Admission {
    pub fn new(max: i32, crossing_holds: usize, misc: bool) -> Self {
        Self {
            max,
            count: crossing_holds,
            misc,
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    /// Call only after source far-edge handling. In particular an event may be
    /// rejected geometrically before it has a chance to stop the quota scan.
    pub fn offer(&mut self, kind: Kind) -> Decision {
        let admit = match kind {
            Kind::Misc | Kind::NextRate => self.misc,
            _ => true,
        };
        if admit {
            self.count = self.count.saturating_add(1);
        }
        Decision {
            admit,
            stop: kind != Kind::NextRate && (self.max < 0 || self.count > self.max as usize),
        }
    }
}
