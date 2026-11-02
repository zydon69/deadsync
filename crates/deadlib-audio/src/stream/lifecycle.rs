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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodedCodec {
    Flac,
    Mp3,
    Vorbis,
    Opus,
    Wav,
}

#[derive(Clone, Debug)]
pub struct DecoderSnapshot {
    /// Actual successfully opened codec, supplied by the decoder worker.
    pub codec: Option<DecodedCodec>,
    pub codec_path: Option<Arc<std::path::Path>>,
    pub command_generation: u64,
    pub asset_epoch: u64,
    /// Producer prefill was published for this exact command, not merely opened.
    pub render_ready_generation: Option<u64>,
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
    #[cfg(test)]
    pub(super) fn new(generation: u64, start: f64) -> Self {
        Self::new_asset(generation, start, deadlib_audio_core::music_asset_epoch())
    }
    pub(super) fn new_asset(generation: u64, start: f64, asset_epoch: u64) -> Self {
        Self {
            shared: Arc::new(Mutex::new(DecoderSnapshot {
                codec: None,
                codec_path: None,
                command_generation: generation,
                asset_epoch,
                render_ready_generation: None,
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
        state.render_ready_generation = None;
        if let Some(start) = start {
            state.requested_start_seconds = start;
        }
        state.final_music_seconds = None;
        state.failure = None;
    }
    pub(super) fn opened_codec(
        &self,
        generation: u64,
        codec: DecodedCodec,
        path: &std::path::Path,
    ) {
        let mut state = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.command_generation != generation || state.phase == DecoderPhase::Stopped {
            return;
        }
        state.codec = Some(codec);
        state.codec_path = Some(Arc::from(path));
        state.opened_successfully = true;
        state.phase = DecoderPhase::Opened;
        state.sequence = state.sequence.wrapping_add(1);
    }
    pub(super) fn render_ready(&self, generation: u64) {
        let mut state = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.command_generation == generation && state.phase != DecoderPhase::Stopped {
            if state.render_ready_generation != Some(generation) {
                state.sequence = state.sequence.wrapping_add(1);
            }
            state.render_ready_generation = Some(generation);
        }
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
        let _audio_guard = crate::test_audio_guard();
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
        let _audio_guard = crate::test_audio_guard();
        let status = DecoderStatusReader::new(1, 5.0);
        status.publish(1, DecoderPhase::Failed, None, Some(&"é".repeat(4096)));
        let state = status.snapshot();
        assert!(!state.opened_successfully);
        assert!(state.failure.unwrap().len() <= MAX_FAILURE_BYTES);
    }
}

#[cfg(test)]
mod notification_codec_tests {
    use super::*;
    #[test]
    fn codec_is_actual_opened_reader_and_stale_or_stopped_workers_cannot_publish() {
        let _audio_guard = crate::test_audio_guard();
        let status = DecoderStatusReader::new(11, 0.);
        assert_eq!(status.snapshot().codec, None);
        status.opened_codec(10, DecodedCodec::Opus, std::path::Path::new("actual.ogg"));
        assert_eq!(status.snapshot().codec, None);
        assert!(!status.snapshot().opened_successfully);
        status.opened_codec(11, DecodedCodec::Vorbis, std::path::Path::new("actual.ogg"));
        let opened = status.snapshot();
        assert_eq!(opened.codec, Some(DecodedCodec::Vorbis));
        assert!(opened.opened_successfully);
        status.begin(12, Some(10.));
        status.opened_codec(11, DecodedCodec::Opus, std::path::Path::new("actual.ogg"));
        assert_eq!(status.snapshot().codec, Some(DecodedCodec::Vorbis));
        status.opened_codec(12, DecodedCodec::Opus, std::path::Path::new("actual.ogg"));
        assert_eq!(status.snapshot().codec, Some(DecodedCodec::Opus));
        status.publish(12, DecoderPhase::Stopped, None, None);
        let stopped = status.snapshot();
        status.opened_codec(12, DecodedCodec::Wav, std::path::Path::new("actual.ogg"));
        let after = status.snapshot();
        assert_eq!(after.sequence, stopped.sequence);
        assert_eq!(after.phase, DecoderPhase::Stopped);
        assert_eq!(after.codec, Some(DecodedCodec::Opus));
    }
    #[test]
    fn successful_header_does_not_invent_produced_audio_and_failure_remains_failure() {
        let _audio_guard = crate::test_audio_guard();
        let status = DecoderStatusReader::new(1, 0.);
        status.opened_codec(1, DecodedCodec::Opus, std::path::Path::new("actual.ogg"));
        assert_eq!(status.snapshot().phase, DecoderPhase::Opened);
        assert_eq!(status.snapshot().final_music_seconds, None);
        status.publish(1, DecoderPhase::Failed, None, Some("real decode failed"));
        assert_eq!(status.snapshot().phase, DecoderPhase::Failed);
        assert_eq!(
            status.snapshot().failure.as_deref(),
            Some("real decode failed")
        );
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn new_command_clears_old_metrics_and_cannot_inherit_old_prefill() {
        let _audio_guard = crate::test_audio_guard();
        let status = DecoderStatusReader::new_asset(41, 0., 7);
        status.publish(41, DecoderPhase::Producing, Some(0.5), None);
        status.render_ready(41);
        assert_eq!(status.snapshot().render_ready_generation, Some(41));
        status.begin(42, None);
        let pending = status.snapshot();
        assert_eq!(pending.phase, DecoderPhase::Pending);
        assert_eq!(pending.final_music_seconds, None);
        assert_eq!(pending.render_ready_generation, None);
        status.render_ready(41);
        assert_eq!(status.snapshot().render_ready_generation, None);
        status.publish(42, DecoderPhase::Producing, Some(0.51), None);
        assert_eq!(status.snapshot().render_ready_generation, None);
        status.render_ready(42);
        assert_eq!(status.snapshot().render_ready_generation, Some(42));
    }
}
