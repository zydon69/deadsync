//! GrooveAuthor graph, interpretation, pattern and chart-performance engine.
//! Snapshot-bound jobs expose previews to the native generation panel.
pub mod config;
pub mod context;
pub mod editor_timing;
pub mod expressed;
pub mod fallbacks;
pub mod geometry;
pub mod graph;
pub mod interpretation_cost;
pub mod interpretation_path;
pub mod job;
mod jsonc;
pub mod mines;
pub mod metadata;
pub mod output;
pub mod pattern;
pub mod pattern_job;
mod pattern_persistence;
pub mod pattern_region;
pub mod performed;
pub mod random;
pub mod profiles;
pub mod reference_sort;
pub mod search;
pub mod source;
pub mod step_data;
pub mod timing;
