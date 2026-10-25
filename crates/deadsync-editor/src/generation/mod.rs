//! GrooveAuthor graph, interpretation, pattern and chart-performance engine.
//! Native generation jobs, preview/application and panels are separate stages.
pub mod config;
pub mod expressed;
pub mod fallbacks;
pub mod geometry;
pub mod graph;
pub mod interpretation_cost;
pub mod interpretation_path;
mod jsonc;
pub mod mines;
pub mod output;
pub mod pattern;
pub mod performed;
pub mod random;
pub mod reference_sort;
pub mod search;
pub mod source;
pub mod step_data;
pub mod timing;
