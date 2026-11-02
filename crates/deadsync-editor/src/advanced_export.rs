//! Explicit output preparation. Ordinary lossless Save never uses this module.
//! No destination I/O or live format/history mutation is provided here.
pub mod options;
mod preflight;
pub use options::Options;
pub use preflight::{Issue, IssueKind, Limits, Output, Plan, Severity};
