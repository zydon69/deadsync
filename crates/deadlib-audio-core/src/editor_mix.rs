//! Single editor owner's coherent gain tuple. Gameplay controls stay independent.
//! Sequentially consistent cell fields make odd/write/even validation explicit;
//! reads remain bounded to two attempts once per physical callback.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorMixGains {
    pub main: f32,
    pub music: f32,
    pub assist: f32,
    pub beat: f32,
}
impl EditorMixGains {
    pub fn valid(self) -> bool {
        [self.main, self.music, self.assist, self.beat]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Snapshot {
    pub owner: u64,
    pub gains: EditorMixGains,
}
pub(crate) struct Cell {
    #[cfg(test)]
    pub(crate) gate: std::sync::Mutex<
        Option<(
            std::sync::Arc<std::sync::Barrier>,
            std::sync::Arc<std::sync::Barrier>,
        )>,
    >,
    sequence: AtomicU64,
    owner: AtomicU64,
    main: AtomicU32,
    music: AtomicU32,
    assist: AtomicU32,
    beat: AtomicU32,
}
impl Cell {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            gate: std::sync::Mutex::new(None),
            sequence: AtomicU64::new(0),
            owner: AtomicU64::new(0),
            main: AtomicU32::new(1f32.to_bits()),
            music: AtomicU32::new(1f32.to_bits()),
            assist: AtomicU32::new(1f32.to_bits()),
            beat: AtomicU32::new(1f32.to_bits()),
        }
    }
    #[cfg(test)]pub(crate)fn exhaust_for_test(&self){self.sequence.store(u64::MAX-1,Ordering::SeqCst);}
    pub fn can_publish(&self) -> bool {
        let n = self.sequence.load(Ordering::SeqCst);
        n & 1 == 0 && n.checked_add(2).is_some()
    }
    /// The audio application thread is the sole editor publisher. No callback
    /// writes this cell. Exhaustion/invalid gains refuse before any mutation.
    pub fn publish(&self, owner: u64, gains: EditorMixGains) -> bool {
        if !gains.valid() {
            return false;
        }
        let n = self.sequence.load(Ordering::SeqCst);
        let Some(end) = n.checked_add(2) else {
            return false;
        };
        if n & 1 != 0
            || self
                .sequence
                .compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return false;
        }
        self.owner.store(owner, Ordering::SeqCst);
        self.main.store(gains.main.to_bits(), Ordering::SeqCst);
        #[cfg(test)]
        if let Some((enter, release)) = self.gate.lock().unwrap().take() {
            enter.wait();
            release.wait();
        }
        self.music.store(gains.music.to_bits(), Ordering::SeqCst);
        self.assist.store(gains.assist.to_bits(), Ordering::SeqCst);
        self.beat.store(gains.beat.to_bits(), Ordering::SeqCst);
        self.sequence.store(end, Ordering::SeqCst);
        true
    }
    pub fn read(&self) -> Option<Snapshot> {
        for _ in 0..2 {
            let n = self.sequence.load(Ordering::SeqCst);
            if n == 0 || n & 1 != 0 {
                continue;
            }
            let value = Snapshot {
                owner: self.owner.load(Ordering::SeqCst),
                gains: EditorMixGains {
                    main: f32::from_bits(self.main.load(Ordering::SeqCst)),
                    music: f32::from_bits(self.music.load(Ordering::SeqCst)),
                    assist: f32::from_bits(self.assist.load(Ordering::SeqCst)),
                    beat: f32::from_bits(self.beat.load(Ordering::SeqCst)),
                },
            };
            std::sync::atomic::fence(Ordering::SeqCst);
            if self.sequence.load(Ordering::SeqCst) == n {
                return Some(value);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_or_exhausted_gain_tuple_refuses_without_partial_publication() {
        let cell = Cell::new();
        let old = EditorMixGains {
            main: 0.61,
            music: 0.43,
            assist: 0.71,
            beat: 0.79,
        };
        assert!(cell.publish(7, old));
        let mut bad = old;
        bad.main = f32::NAN;
        assert!(!cell.publish(7, bad));
        assert_eq!(cell.read().unwrap().gains, old);
        cell.sequence.store(u64::MAX - 1, Ordering::SeqCst);
        assert!(!cell.can_publish());
        assert!(!cell.publish(8, EditorMixGains { main: 1., ..old }));
        assert_eq!(cell.owner.load(Ordering::SeqCst), 7);
        assert_eq!(cell.main.load(Ordering::SeqCst), old.main.to_bits());
        assert_eq!(cell.sequence.load(Ordering::SeqCst), u64::MAX - 1);
    }
}
