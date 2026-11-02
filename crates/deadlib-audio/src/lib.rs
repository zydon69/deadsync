//! Audio output backends, realtime mixing, and buffered stream decoding.

mod launch;

pub use deadlib_audio_core::*;
pub use launch::LinuxAudioBackend;
#[cfg(target_os = "linux")]
pub use launch::available_linux_backends;
pub use launch::{InitConfig, OutputPlan, prepare_output, prepare_output_with_buffers};

pub mod stream;

// Tests in this crate drive one process-wide transport/clock, just like the
// single game player. Hold the lease until all worker runtimes have dropped.
#[cfg(test)]
fn test_audio_guard() -> std::sync::MutexGuard<'static, ()> {
    static PLAYER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = PLAYER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    deadlib_audio_core::reset_music_stream_clock_state();
    deadlib_audio_core::resume_music_track();
    deadlib_audio_core::reset_music_target_gain();
    guard
}
