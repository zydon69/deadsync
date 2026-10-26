//! Latest decoder acknowledgement, independent of callback playback position.
//! Never accessed by the audio callback. Producer EOF is not audible EOF.
use std::sync::{Arc, Mutex};

const MAX_FAILURE_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderPhase {
    Pending,
    Opened,
    Producing,
    Exhausted,
    Stopped,
    Failed,
}

#[derive(Clone, Debug)]
pub struct DecoderSnapshot {
    pub command_generation: u64,
    pub sequence: u64,
    pub phase: DecoderPhase,
    pub opened_successfully: bool,
    pub requested_start_seconds: f64,
    /// Last source position queued by this generation, not the audible position.
    pub final_music_seconds: Option<f64>,
    pub failure: Option<Arc<str>>,
}

#[derive(Clone)]
pub struct DecoderStatusReader {
    shared: Arc<Mutex<DecoderSnapshot>>,
}

/// Application-owned latest-track monitor. Remains valid across play/stop/replace.
/// Reads are for control/UI threads; the render callback must not access it.
#[derive(Clone, Default)]
pub struct DecoderStatusMonitor {
    current: Arc<Mutex<Option<DecoderStatusReader>>>,
}
impl DecoderStatusMonitor {
    pub fn snapshot(&self) -> Option<DecoderSnapshot> {
        let current = self
            .current
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        current.map(|reader| reader.snapshot())
    }
    pub(super) fn set(&self, reader: DecoderStatusReader) {
        *self
            .current
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(reader);
    }
}
impl DecoderStatusReader {
    pub fn snapshot(&self) -> DecoderSnapshot {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    pub(super) fn new(generation: u64, start: f64) -> Self {
        Self {
            shared: Arc::new(Mutex::new(DecoderSnapshot {
                command_generation: generation,
                sequence: 0,
                phase: DecoderPhase::Pending,
                opened_successfully: false,
                requested_start_seconds: start,
                final_music_seconds: None,
                failure: None,
            })),
        }
    }
    pub(super) fn begin(&self, generation: u64, start: Option<f64>) {
        let mut state = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.command_generation = generation;
        state.sequence = state.sequence.wrapping_add(1);
        state.phase = DecoderPhase::Pending;
        if let Some(start) = start {
            state.requested_start_seconds = start;
        }
        state.final_music_seconds = None;
        state.failure = None;
    }
    pub(super) fn publish(
        &self,
        generation: u64,
        phase: DecoderPhase,
        position: Option<f64>,
        failure: Option<&str>,
    ) {
        let mut state = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.command_generation != generation || state.phase == DecoderPhase::Stopped {
            return;
        }
        state.sequence = state.sequence.wrapping_add(1);
        state.phase = phase;
        if matches!(phase, DecoderPhase::Opened | DecoderPhase::Producing) {
            state.opened_successfully = true;
        }
        if let Some(position) = position.filter(|position| position.is_finite()) {
            state.final_music_seconds = Some(position);
        }
        state.failure = failure.map(|detail| {
            let mut end = detail.len().min(MAX_FAILURE_BYTES);
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            Arc::from(&detail[..end])
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_completions_and_stop_cannot_overwrite_current_command() {
        let status = DecoderStatusReader::new(1, -1.0);
        status.publish(1, DecoderPhase::Opened, None, None);
        status.begin(2, Some(10.0));
        status.publish(1, DecoderPhase::Exhausted, Some(20.0), None);
        assert_eq!(status.snapshot().phase, DecoderPhase::Pending);
        assert_eq!(status.snapshot().requested_start_seconds, 10.0);
        status.publish(2, DecoderPhase::Stopped, None, None);
        status.publish(2, DecoderPhase::Failed, None, Some("late"));
        assert_eq!(status.snapshot().phase, DecoderPhase::Stopped);
    }
    #[test]
    fn failures_are_bounded_utf8_and_do_not_invent_success() {
        let status = DecoderStatusReader::new(1, 5.0);
        status.publish(1, DecoderPhase::Failed, None, Some(&"é".repeat(4096)));
        let state = status.snapshot();
        assert!(!state.opened_successfully);
        assert!(state.failure.unwrap().len() <= MAX_FAILURE_BYTES);
    }
}
