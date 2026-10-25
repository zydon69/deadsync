//! Background waveform decoding. No audio device or playback stream is opened.
use deadlib_audio_decode::{DecodeOptions, open_file};
use deadsync_editor::waveform::{Builder, Waveform};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
};

/// One worker per editor loader, including workers still cancelling. Replacing
/// a song cannot accumulate concurrent full-song waveform allocations.
pub struct Loader {
    busy: Arc<AtomicBool>,
    max_bytes: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Busy,
    Cancelled,
    Decode(String),
    WorkerStopped,
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("A previous waveform worker is still stopping"),
            Self::Cancelled => f.write_str("Waveform loading cancelled"),
            Self::Decode(message) => f.write_str(message),
            Self::WorkerStopped => f.write_str("Waveform worker stopped unexpectedly"),
        }
    }
}
impl std::error::Error for LoadError {}

pub struct Job {
    cancelled: Arc<AtomicBool>,
    result: Receiver<Result<Waveform, LoadError>>,
    finished: bool,
}
impl Loader {
    pub fn new(max_packed_bytes: usize) -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            max_bytes: max_packed_bytes,
        }
    }
    pub fn start(&self, path: PathBuf) -> Result<Job, LoadError> {
        let permit = Permit::acquire(&self.busy)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancelled);
        let budget = self.max_bytes;
        let (send, result) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("editor-waveform".into())
            .spawn(move || {
                let value = decode(path, budget, &worker_cancel);
                // Free the worker slot before publishing completion, so a caller
                // consuming the result may immediately start the next request.
                drop(permit);
                let _ = send.send(value);
            })
            .map_err(|error| LoadError::Decode(error.to_string()))?;
        Ok(Job {
            cancelled,
            result,
            finished: false,
        })
    }
}
impl Job {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    /// Consume completion once without blocking the editor update loop.
    pub fn poll(&mut self) -> Option<Result<Waveform, LoadError>> {
        if self.finished {
            return None;
        }
        let value = match self.result.try_recv() {
            Ok(value) => value,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(LoadError::WorkerStopped),
        };
        self.finished = true;
        // Even if decoding completed just before cancellation, its result must
        // not become the newly selected song's waveform.
        Some(if self.cancelled.load(Ordering::Relaxed) {
            Err(LoadError::Cancelled)
        } else {
            value
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct Permit(Arc<AtomicBool>);
impl Permit {
    fn acquire(busy: &Arc<AtomicBool>) -> Result<Self, LoadError> {
        busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| LoadError::Busy)?;
        Ok(Self(Arc::clone(busy)))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn decode(path: PathBuf, budget: usize, cancelled: &AtomicBool) -> Result<Waveform, LoadError> {
    let convert = |error: deadsync_editor::Error| LoadError::Decode(error.to_string());
    if cancelled.load(Ordering::Relaxed) {
        return Err(LoadError::Cancelled);
    }
    let mut source = open_file(&path, DecodeOptions::STEP_MANIA_TIMELINE)
        .map_err(|error| LoadError::Decode(error.to_string()))?;
    let mut builder = Builder::with_memory_limit(source.channels, source.sample_rate_hz, budget)
        .map_err(convert)?;
    let mut packet = Vec::new();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(LoadError::Cancelled);
        }
        packet.clear();
        if !source
            .reader
            .read_dec_packet_into(&mut packet)
            .map_err(|error| LoadError::Decode(error.to_string()))?
        {
            break;
        }
        // Bound the cancellation latency even for decoders with large packets.
        for frames in packet.chunks(source.channels * 4096) {
            if cancelled.load(Ordering::Relaxed) {
                return Err(LoadError::Cancelled);
            }
            builder.push_i16(frames).map_err(convert)?;
        }
    }
    builder
        .finish_cancellable(cancelled)
        .map_err(convert)?
        .ok_or(LoadError::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_slot_refuses_overlap_and_releases_on_guard_drop() {
        let loader = Loader::new(1024);
        let permit = Permit::acquire(&loader.busy).unwrap();
        assert!(matches!(
            loader.start("unopened.wav".into()),
            Err(LoadError::Busy)
        ));
        drop(permit);
        assert!(Permit::acquire(&loader.busy).is_ok());
    }
    #[test]
    fn cancellation_discards_already_queued_success_and_completion_is_consumed_once() {
        let (send, result) = mpsc::sync_channel(1);
        assert!(
            send.send(Ok(Builder::new(1, 48000).unwrap().finish().unwrap()))
                .is_ok()
        );
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            result,
            finished: false,
        };
        job.cancel();
        assert!(matches!(job.poll(), Some(Err(LoadError::Cancelled))));
        assert!(job.poll().is_none());
    }
    #[test]
    fn unexpected_worker_disconnect_is_a_single_visible_error() {
        let (send, result) = mpsc::sync_channel(1);
        drop(send);
        let mut job = Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            result,
            finished: false,
        };
        assert!(matches!(job.poll(), Some(Err(LoadError::WorkerStopped))));
        assert!(job.poll().is_none());
    }
}
