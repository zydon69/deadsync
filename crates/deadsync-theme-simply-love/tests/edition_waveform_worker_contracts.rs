// Compile the worker's own unit contracts in isolation. The complete theme unit
// target also includes unrelated native SongLua fixtures absent from this checkout.
// Real decoder and State integration are covered by edition_waveform_audio_workflow.
#[path = "../src/editor_audio.rs"]
mod editor_audio;
