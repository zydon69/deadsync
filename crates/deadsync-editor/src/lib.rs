//! Editor domain independent of rendering, audio playback, and gameplay state.
//!
//! Simfiles retain their original bytes. Edits replace explicit value spans,
//! leaving unknown tags, comments, encodings, and unedited charts intact.
pub mod scroll_preferences;
pub mod analysis;
pub mod attacks;
pub mod audio_preferences;
pub mod chart_properties;
pub mod chart_identity;
pub mod chart_text;
pub mod chart_creation;
pub mod chart_deletion;
pub mod chart_order;
pub mod clipboard;
pub mod event_clipboard;
pub mod document;
pub mod display_tempo;
pub mod generation;
pub mod history;
pub mod music_file;
pub mod notes;
pub mod note_attributes;
pub mod stepf2;
mod number;
pub mod pad;
pub mod playback;
pub mod projection;
pub mod rate_mapping;
pub mod region_index;
pub mod runtime;
pub mod song_assets;
pub mod song_properties;
pub mod song_text;
pub mod storage;
pub mod spacing;
pub mod spacing_geometry;
pub mod spacing_index;
pub mod spacing_search;
pub mod scroll_interpolation;
pub mod text_buffer;
pub mod timing_edit;
pub mod transform;
pub mod workspace;
pub mod zoom;
pub mod zoom_preferences;
pub mod waveform;
pub mod waveform_preferences;
pub mod waveform_spacing;
pub mod waveform_palette;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidFormat(String),
    Unsupported(String),
    InvalidEdit(String),
    ExternalChange,
    RecoveryConflict,
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(message)
            | Self::Unsupported(message)
            | Self::InvalidEdit(message) => f.write_str(message),
            Self::ExternalChange => {
                f.write_str("The simfile changed on disk; reload or save a copy.")
            }
            Self::RecoveryConflict => f.write_str("An existing recovery draft changed; recover it or explicitly save a draft before replacing it."),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
