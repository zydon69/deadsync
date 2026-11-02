//! One physical Main callback's source domain. Never inferred from telemetry.
use std::sync::atomic::{AtomicU64, Ordering};
#[derive(Clone, Copy, Debug)]
pub struct SourceTickClock {
    pub generation: u64,
    pub asset_epoch: u64,
    pub source_frame: i64,
    pub rate: f64,
    pub physical_start: u64,
    pub frames: usize,
}
impl SourceTickClock {
    pub(crate) fn scaled(self, offset: usize) -> i64 {
        self.source_frame
            .saturating_add((offset as f64 * self.rate) as i64)
    }
    pub(crate) fn end(self) -> i64 {
        self.source_frame
            .saturating_add(i64::from((self.frames as f64 * self.rate) as i32))
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Receipt {
    pub timeline: u64,
    pub schedule: u64,
    pub generation: u64,
    pub asset: u64,
    pub physical_end: u64,
    pub last_scaled: i64,
    pub next: usize,
}
#[derive(Debug)]
pub(crate) struct ReceiptCell {
    sequence: AtomicU64,
    timeline: AtomicU64,
    schedule: AtomicU64,
    generation: AtomicU64,
    asset: AtomicU64,
    end: AtomicU64,
    last: AtomicU64,
    next: AtomicU64,
}
impl ReceiptCell {
    pub(crate) fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            timeline: AtomicU64::new(0),
            schedule: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            asset: AtomicU64::new(0),
            end: AtomicU64::new(0),
            last: AtomicU64::new(0),
            next: AtomicU64::new(0),
        }
    }
    pub(crate) fn read(&self) -> Option<Receipt> {
        for _ in 0..2 {
            let s = self.sequence.load(Ordering::Acquire);
            if s == 0 || s & 1 != 0 {
                continue;
            }
            let r = Receipt {
                timeline: self.timeline.load(Ordering::Relaxed),
                schedule: self.schedule.load(Ordering::Relaxed),
                generation: self.generation.load(Ordering::Relaxed),
                asset: self.asset.load(Ordering::Relaxed),
                physical_end: self.end.load(Ordering::Relaxed),
                last_scaled: self.last.load(Ordering::Relaxed) as i64,
                next: self.next.load(Ordering::Relaxed) as usize,
            };
            std::sync::atomic::fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Relaxed) == s {
                return Some(r);
            }
        }
        None
    }
    pub(crate) fn publish(&self, r: Receipt) {
        let s = self.sequence.load(Ordering::Acquire);
        let Some(next) = s.checked_add(2) else {
            return;
        };
        if s & 1 != 0
            || self
                .sequence
                .compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        self.timeline.store(r.timeline, Ordering::Relaxed);
        self.schedule.store(r.schedule, Ordering::Relaxed);
        self.generation.store(r.generation, Ordering::Relaxed);
        self.asset.store(r.asset, Ordering::Relaxed);
        self.end.store(r.physical_end, Ordering::Relaxed);
        self.last.store(r.last_scaled as u64, Ordering::Relaxed);
        self.next.store(r.next as u64, Ordering::Relaxed);
        self.sequence.store(next, Ordering::Release);
    }
}
