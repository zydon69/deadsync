//! GrooveAuthor generation domain. Graph data and step semantics are ported
//! with pattern search and performed-path scoring. Expressed-chart interpretation,
//! whole-chart generation and native generation panels remain separate work.
pub mod config;
pub mod expressed;
pub mod geometry;
pub mod graph;
pub mod interpretation_path;
pub mod interpretation_cost;
pub mod pattern;
pub mod random;
pub mod search;
pub mod step_data;
pub mod timing;
