//! Saved-pattern clear action: sorted sequential deletion and hold shortening.
//! The lane-edit primitive follows ActionDeletePatternNotes (GrooveAuthor MIT).
use super::{
    job::{Job, Phase, Progress},
    metadata::{ChartData, Registry},
    pattern_job::Span,
    pattern_region::{Deletions, NoteEdit, deletion_edits},
    profiles::Loaded,
};
use crate::{
    Error,
    document::{Document, Format},
    history::{RevisionToken, Session},
    notes::Grid,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Task {
    bytes: Arc<[u8]>,
    format: Format,
    token: RevisionToken,
    chart: usize,
    rows: Vec<u64>,
    directory: Option<PathBuf>,
}
pub struct Preview {
    token: RevisionToken,
    chart: usize,
    grid: Grid,
    edits: Vec<NoteEdit>,
    layout: String,
    report: Deletions,
    issues: Vec<super::profiles::Issue>,
}
impl Task {
    pub fn capture(
        session: &Session,
        chart: usize,
        rows: impl IntoIterator<Item = u64>,
        directory: Option<&Path>,
    ) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return Err(invalid("Pattern chart does not exist"));
        }
        let mut rows: Vec<_> = rows.into_iter().collect();
        rows.sort_unstable();
        rows.dedup();
        if rows.is_empty() {
            return Err(invalid("No patterns selected for clearing"));
        }
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            token: session.revision_token(),
            chart,
            rows,
            directory,
        })
    }
    pub fn spawn(self) -> Result<Job<Preview>, Error> {
        Job::spawn_with("editor-clear-pattern-notes", move |cancelled, progress| {
            let preview = self.run(cancelled, |completed, total| {
                progress(Progress {
                    phase: Phase::Converting,
                    completed,
                    total,
                })
            })?;
            progress(Progress {
                phase: Phase::Ready,
                completed: 1,
                total: 1,
            });
            Ok(preview)
        })
    }
    pub fn run(
        self,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<Preview, Error> {
        check(cancelled)?;
        let document = Document::parse(self.bytes, self.format)?;
        let layout = document.charts()[self.chart].steps_type.clone();
        let data = ChartData::read(&document, self.chart)?
            .ok_or_else(|| invalid("No saved pattern markers in this chart"))?;
        let (registry, issues) = if let Some(directory) = self.directory {
            let loaded = Loaded::directory_checked(&directory, cancelled)?;
            (loaded.registry, loaded.issues)
        } else {
            (Registry::builtin(), Vec::new())
        };
        // Validate all selected markers before preparing any partial result.
        let mut regions = Vec::new();
        for row in &self.rows {
            check(cancelled)?;
            let marker = data
                .patterns
                .as_ref()
                .and_then(|patterns| patterns.get(row))
                .ok_or_else(|| invalid("Selected pattern marker is missing"))?;
            let config = registry
                .patterns
                .get(&marker.pattern_config_guid)
                .ok_or_else(|| {
                    Error::Unsupported(format!(
                        "Missing pattern generation profile {}",
                        marker.pattern_config_guid
                    ))
                })?;
            config.validate()?;
            let spacing = 48 / u64::from(config.beat_sub_division);
            let span = Span {
                start: *row,
                end: *row + marker.length,
                include_start: marker.start_position_inclusive,
                include_end: marker.end_position_inclusive,
            };
            regions.push((span.optional_steps(spacing)?, spacing));
        }
        let mut snapshot = Session::new(document);
        let mut grid = snapshot.grid(self.chart)?.clone();
        let mut edits = BTreeMap::new();
        let mut report = Deletions::default();
        for (index, (range, spacing)) in regions.into_iter().enumerate() {
            check(cancelled)?;
            if let Some((start, end, _)) = range {
                for player in 0..grid.players() {
                    let (changes, counts) =
                        deletion_edits(&grid, player, start, end, spacing, cancelled)?;
                    report.removed += counts.removed;
                    report.shortened_holds += counts.shortened_holds;
                    for (position, note) in changes {
                        check(cancelled)?;
                        grid.set(position, note.clone())?;
                        edits.insert(position, note);
                    }
                }
            }
            progress(index + 1, self.rows.len());
        }
        check(cancelled)?;
        Ok(Preview {
            token: self.token,
            chart: self.chart,
            grid,
            edits: edits.into_iter().collect(),
            layout,
            report,
            issues,
        })
    }
}
impl Preview {
    pub fn grid(&self) -> &Grid {
        &self.grid
    }
    pub fn layout(&self) -> &str {
        &self.layout
    }
    pub fn report(&self) -> Deletions {
        self.report
    }
    pub fn profile_issues(&self) -> &[super::profiles::Issue] {
        &self.issues
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid(
                "Clear preview belongs to another chart or revision",
            ));
        }
        session.edit_notes(chart, "Clear pattern notes", self.edits.clone())
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Pattern clearing cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
