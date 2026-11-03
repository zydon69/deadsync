#![forbid(unsafe_code)]

pub mod mixer;
pub mod output;
pub mod output_buffer;
pub mod position;
pub mod preview_envelope;
pub mod render;
pub mod rendered_cursor;
pub use preview_envelope::{
    editor_preview_envelope, editor_preview_envelope_enabled, set_editor_preview_envelope,
};
pub use rendered_cursor::{RenderedMusicCursor, freeze_rendered_music_cursor, rendered_music_cursor,
};
pub mod ring;
pub mod sfx_ring;
pub mod telemetry;

pub use mixer::{
    ActiveSfx, MAX_ACTIVE_SFX, MAX_MIX_BUSES, MAX_SCHEDULE_AHEAD_FRAMES, MixBus, MixControls,
    QueuedSfx, ScheduledOnset, SfxOnset, SfxPan, SfxVoiceControl, exact_onset_decision, f32_to_i16, i16_to_f32, mix_active_sfx,
    push_queued_sfx,
    scheduled_onset_decision,
};
pub use output::{AudioOutputMode, OutputBackendReady, OutputDeviceInfo, OutputTimingSnapshot};
pub use position::{
    CallbackClockSource, CallbackClockWindow, MUSIC_POS_MAP_BACKLOG_FRAMES,
    MusicStreamClockSnapshot, PlaybackPosMap, activate_music_track, active_music_track_id,
    advance_music_asset_epoch, advance_music_preview_asset_epoch, bump_music_map_generation,
    clear_music_stream_clock_seed, fallback_music_position, fallback_stream_position_frames,
    load_callback_clock_snapshot_now, mark_music_track_started, music_asset_epoch,
    music_clock_seed_enabled, music_gain_snap_generation, music_map_generation,
    music_nanos_from_seconds, music_pause_epoch, music_target_gain, music_total_frames,
    music_track_active, music_track_active_relaxed, music_track_has_started, music_track_paused,
    music_track_start_frame, next_music_track_id, normalized_music_rate,
    note_timing_diag_callback_gap, pause_music_track, publish_callback_window_end,
    publish_callback_window_start_nanos, reset_music_stream_clock_state, reset_music_target_gain,
    resume_music_track, seed_music_asset_clock, seed_music_stream_clock,
    seeded_music_position, set_music_clock_rate, set_music_target_gain, snap_music_gain_generation,
    stop_music_track, stream_position_frames_from_window, timing_diag_last_callback_gap_ns,
};
pub use render::{CallbackInfo, OutputBufferMut, RenderReport, RenderState};
pub use ring::{
    AudioRenderHandle, AudioStreamHandle, MusicBlockTiming, MusicBlockWriter, PlayedMapReader,
    music_transport, played_map_drops,
};
pub use sfx_ring::{SfxDrain, SfxReceiver, SfxScheduleRetirer, SfxSender, sfx_transport};
pub use telemetry::{
    AUDIO_STUTTER_DIAG_EVENT_COUNT, OutputTelemetryBackend, OutputTelemetryClock,
    OutputTimingQuality, StutterDiagAudioEvent, StutterDiagAudioEventKind,
    collect_stutter_diag_events, current_output_timing_quality, get_output_timing_snapshot,
    note_output_clock_fallback, note_output_timing_sanity_failure, note_output_underrun,
    publish_output_backend_ready, publish_output_timing, publish_output_timing_quality,
    record_stutter_diag_event, report_audio_render_callback,
    stutter_diag_callback_gap_threshold_ns, stutter_diag_trigger_seq,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct MusicMapSeg {
    pub stream_frame_start: i64,
    pub frames: i64,
    pub music_start_sec: f64,
    pub music_sec_per_frame: f64,
}

pub mod scheduled_sfx;
pub use scheduled_sfx::{ScheduledTrigger, SfxSchedule};
pub mod scheduled_voice;
pub use scheduled_voice::SfxScheduleVoice;

pub use output_buffer::{BufferCapabilities, BufferControl, BufferNegotiation, BufferRequest, OutputBufferState, ReconfigureError, ReconfigureFailure, ReconfigureOutcome,
};

mod preview_source;

mod preview_transport;
pub use preview_envelope::{initialize_editor_preview_clock, set_editor_preview_region};
pub use preview_source::{PreviewCallbackCursor, preview_callback_cursor};
pub use preview_transport::PreviewSource;
pub use preview_envelope::{editor_preview_initial_clock, editor_preview_initial_frame};

mod editor_main;
pub mod editor_time_stretch;

mod hermite;
pub use editor_main::{Configuration as EditorMainConfiguration, EditorMainCursor, configure_editor_main,
    editor_main_can_configure, editor_main_configuration,editor_main_cursor, thaw_editor_main,
};

mod source_tick;
pub use source_tick::SourceTickClock;

mod editor_mix;
pub use editor_mix::EditorMixGains;

#[cfg(test)]
fn test_audio_guard() -> std::sync::MutexGuard<'static, ()> {
    // A single process-wide player backs every renderer and cursor test.
    // The lease includes worker teardown, not just individual callbacks.
    static PLAYER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = PLAYER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reset_music_stream_clock_state();
    resume_music_track();
    reset_music_target_gain();
    advance_music_asset_epoch();
    guard
}
