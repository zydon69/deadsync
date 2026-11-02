//! Publish the immutable MAIN source through the same boundary as the shell.
use deadsync_theme_simply_love::screens::edition::State;

pub fn publish_main(state: &mut State) {
    let _ = state.take_audio_commands();
    let path = state.editor_asset_paths()[0].clone().expect("MAIN path");
    let source = deadlib_audio_decode::prepared::PreparedAudioSource::load_editor(
        &path,
        deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .expect("prepare MAIN source");
    let registration = state.editor_asset_registration();
    state.observe_editor_main_source(
        state.editor_asset_owner(),
        registration,
        Some((registration, 0)),
        Some(source),
    );
}
