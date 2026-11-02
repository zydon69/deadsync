//! Circular timing recorder ported from GrooveAuthor PerformanceMonitor
//! (48e18f71655d0f0d86144c63239331833501fda2, MIT).
//!
//! The host supplies monotonic ticks. Unfinished frames are not published;
//! pausing retains completed history and still closes the current frame.
pub const CAPACITY: usize = 1024;
pub const TIMINGS: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Timing {
    Frame,
    EditorCpu,
    Update,
    ChartEvents,
    MiniMap,
    Waveform,
    Draw,
    Present,
    PresentWait,
}

impl Timing {
    pub const ALL: [Self; TIMINGS] = [
        Self::Frame,
        Self::EditorCpu,
        Self::Update,
        Self::ChartEvents,
        Self::MiniMap,
        Self::Waveform,
        Self::Draw,
        Self::Present,
        Self::PresentWait,
    ];
}

#[derive(Clone, Copy, Debug, Default)]
struct Counter {
    start: i64,
    total: i64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    counters: [Counter; TIMINGS],
}

impl Frame {
    pub fn ticks(&self, timing: Timing) -> i64 {
        self.counters[timing as usize].total
    }
}

/// Bounded storage; allocation occurs once, never while recording a frame.
pub struct Monitor {
    frames: Box<[Frame]>,
    current: Option<usize>,
    oldest: Option<usize>,
    enabled: bool,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl Monitor {
    /// A zero-capacity recorder is meaningless and is rejected at construction.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            frames: vec![Frame::default(); capacity].into_boxed_slice(),
            current: None,
            oldest: None,
            enabled: true,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn begin_frame(&mut self, ticks: i64) {
        if let Some(current) = self.current {
            let frame = &mut self.frames[current];
            if frame.ticks(Timing::Frame) == 0 {
                let counter = &mut frame.counters[Timing::Frame as usize];
                counter.total = counter
                    .total
                    .wrapping_add(ticks.wrapping_sub(counter.start));
                if self.oldest.is_none() {
                    self.oldest = Some(current);
                }
            }
        }
        if !self.enabled {
            return;
        }
        let len = self.frames.len();
        if let (Some(current), Some(oldest)) = (self.current, self.oldest) {
            let used = (current + len - oldest) % len;
            if used == len - 1 {
                self.oldest = Some((oldest + 1) % len);
            }
        }
        let current = self.current.map_or(0, |index| (index + 1) % len);
        self.current = Some(current);
        // Source Reset retains start timestamps and clears accumulated totals.
        for counter in &mut self.frames[current].counters {
            counter.total = 0;
        }
        self.frames[current].counters[Timing::Frame as usize].start = ticks;
    }

    pub fn start(&mut self, timing: Timing, ticks: i64) {
        if !self.enabled {
            return;
        }
        if let Some(index) = self.current {
            self.frames[index].counters[timing as usize].start = ticks;
        }
    }

    pub fn end(&mut self, timing: Timing, ticks: i64) {
        if !self.enabled {
            return;
        }
        if let Some(index) = self.current {
            let counter = &mut self.frames[index].counters[timing as usize];
            counter.total = counter
                .total
                .wrapping_add(ticks.wrapping_sub(counter.start));
        }
    }

    /// Like source SetTime, this accumulates rather than replaces the timing.
    pub fn add(&mut self, timing: Timing, ticks: i64) {
        if !self.enabled {
            return;
        }
        if let Some(index) = self.current {
            let counter = &mut self.frames[index].counters[timing as usize];
            counter.total = counter.total.wrapping_add(ticks);
        }
    }

    /// Most recent completed frame first, exactly as the source enumerator.
    pub fn frames(&self) -> Frames<'_> {
        let next = self.current.and_then(|index| {
            if self.frames[index].ticks(Timing::Frame) != 0 {
                Some(index)
            } else if self.oldest.is_none() || self.oldest == Some(index) {
                None
            } else {
                Some((index + self.frames.len() - 1) % self.frames.len())
            }
        });
        Frames {
            monitor: self,
            next,
            remaining: self.frames.len(),
        }
    }
}

pub struct Frames<'a> {
    monitor: &'a Monitor,
    next: Option<usize>,
    remaining: usize,
}

impl<'a> Iterator for Frames<'a> {
    type Item = &'a Frame;
    fn next(&mut self) -> Option<Self::Item> {
        // Keep iteration bounded even if a caller explicitly records Frame
        // before BeginFrame has established the source's oldest-frame marker.
        if self.remaining == 0 {
            return None;
        }
        let index = self.next?;
        self.remaining -= 1;
        self.next = if self.monitor.oldest == Some(index) {
            None
        } else {
            Some((index + self.monitor.frames.len() - 1) % self.monitor.frames.len())
        };
        Some(&self.monitor.frames[index])
    }
}
