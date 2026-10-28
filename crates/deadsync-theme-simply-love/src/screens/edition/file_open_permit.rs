//! One strict simfile parser worker shared by File Open and Pack Open.
//! Cancellation is cooperative: the worker, never its UI receiver, owns Permit.
use deadsync_editor::Error;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
#[derive(Default)]
struct Gate {
    running: Arc<AtomicBool>,
}
pub(super) struct Permit {
    running: Arc<AtomicBool>,
}
impl Gate {
    fn acquire(&self) -> Result<Permit, Error> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::InvalidEdit("Another song is still loading; wait for it to finish".into())
            })?;
        Ok(Permit {
            running: Arc::clone(&self.running),
        })
    }
}
impl Permit {
    pub(super) fn acquire() -> Result<Self, Error> {
        static GATE: OnceLock<Gate> = OnceLock::new();
        GATE.get_or_init(Gate::default).acquire()
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    #[test]
    fn dropped_cancelled_receiver_cannot_release_worker_capacity() {
        let gate = Gate::default();
        let permit = gate.acquire().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let (release, wait) = mpsc::sync_channel(0);
        let (result, receiver) = mpsc::sync_channel::<()>(1);
        let worker = std::thread::spawn(move || {
            let _permit = permit;
            wait.recv().unwrap();
            assert!(signal.load(Ordering::Acquire));
            let _ = result.send(());
        });
        cancelled.store(true, Ordering::Release);
        drop(receiver);
        for _ in 0..32 {
            assert!(
                gate.acquire().is_err(),
                "cancelling UI must not allow overlapping parsers"
            );
        }
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(gate.acquire().is_ok());
    }
    #[test]
    fn spawn_failure_or_worker_unwind_releases_owned_permit() {
        let gate = Gate::default();
        let permit = gate.acquire().unwrap();
        drop(permit);
        assert!(gate.acquire().is_ok());
        let permit = gate.acquire().unwrap();
        let result = std::panic::catch_unwind(|| {
            let _permit = permit;
            panic!("worker failed");
        });
        assert!(result.is_err());
        assert!(gate.acquire().is_ok());
    }
}
