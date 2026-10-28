//! Explicit output preparation. Ordinary lossless Save never uses this module.
//! No destination I/O or live format/history mutation is provided here.
pub mod options;
mod preflight;
pub use options::Options;
pub use preflight::{Output, Issue, IssueKind, Limits, Plan, Severity};
