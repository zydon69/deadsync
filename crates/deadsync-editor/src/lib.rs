//! Editor domain independent of rendering, audio playback, and gameplay state.
//!
//! Simfiles retain their original bytes. Edits replace explicit value spans,
//! leaving unknown tags, comments, encodings, and unedited charts intact.
pub mod advanced_export;
pub mod analysis;
pub mod assistant;
pub mod attacks;
pub mod audio_preferences;
pub mod autogen_preferences;
pub mod autoplay;
pub mod chart_creation;
pub mod chart_deletion;
pub mod chart_event_copy;
pub mod chart_focus;
pub mod chart_identity;
pub mod chart_order;
pub mod chart_properties;
pub mod chart_text;
pub mod chart_views;
pub mod clipboard;
pub mod display_tempo;
pub mod document;
pub mod event_clipboard;
pub mod event_shift;
pub mod ftue;
pub mod generation;
pub mod history;
pub mod log_preferences;
pub mod minimap_background;
pub mod minimap_content;
pub mod minimap_content_preferences;
pub mod minimap_event_policy;
pub mod minimap_mount;
pub mod minimap_palette;
pub mod minimap_position_preferences;
pub mod minimap_preferences;
pub mod minimap_select;
pub mod minimap_viewport;
pub mod music_file;
pub mod navigation;
pub mod note_attributes;
pub mod note_operation;
pub mod note_swap;
pub mod notes;
mod number;
pub mod open_preferences;
pub mod output_policy;
pub mod pack;
pub mod pad;
pub mod performance;
pub mod playback;
mod preference_enum;
pub mod projection;
pub mod rate_mapping;
pub mod receptors;
pub mod region_index;
pub mod runtime;
pub mod scroll_interpolation;
pub mod scroll_preferences;
pub mod selected_transform;
pub mod song_assets;
pub mod song_properties;
pub mod song_text;
pub mod source_output_order;
pub mod source_visual_clock;
pub mod spacing;
pub mod spacing_geometry;
pub mod spacing_index;
pub mod spacing_search;
pub mod stepf2;
pub mod storage;
pub mod text_buffer;
pub mod timing_edit;
pub mod transform;
pub mod visual_preferences;
pub mod waveform;
pub mod waveform_palette;
pub mod waveform_preferences;
pub mod waveform_spacing;
pub mod workspace;
pub mod zoom;
pub mod zoom_preferences;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidFormat(String),
    Unsupported(String),
    InvalidEdit(String),
    ExternalChange,
    RecoveryConflict,
    OutputReviewRequired,
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
            Self::OutputReviewRequired => f.write_str("The current output requires a new review before saving."),
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

pub mod minimap_navigation;
pub mod minimap_projection;

pub mod lane_entry;

pub mod note_color_preferences;

pub mod minimap_pattern_region;

pub mod note_color_palette;

pub mod snap;

pub mod draft_note_policy;

pub mod dark_preferences;
pub mod receptor_preferences;
pub mod source_color;

pub mod note_color_inspector;

pub mod media_relocation;

pub mod key_binding_preferences;
pub mod key_commands;

pub mod file_origin;
pub mod recent_song_preferences;
pub mod recent_songs;

pub mod quick_add;

pub mod shift_all;

pub mod pattern_actions;

pub mod density;
pub mod density_preferences;

pub mod stream_preferences;

pub mod audio_ticks;

pub mod options_preferences;

pub mod selection_preferences;

pub mod region_selection;

pub mod song_timing_chart;
