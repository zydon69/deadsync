//! Exact Pattern menu actions; reseeding commits before asynchronous generation.
use crate::{
    Error,
    event_clipboard::{Event, Selection},
    generation::{
        clear_patterns,
        metadata::{ChartData, Guid},
        pattern_batch,
    },
    history::{Session, file_intent::IntentGuard},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    GenerateFixed,
    GenerateNewSeeds,
    Clear,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    Stale,
    MissingReview,
    Invalid,
}
#[derive(Debug)]
pub struct Failure {
    pub reason: Refusal,
    pub error: Error,
    /// A source-equivalent reseed history action already committed before task setup failed.
    pub seeds_committed: bool,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            reason: Refusal::Invalid,
            error,
            seeds_committed: false,
        }
    }
}
fn refusal(reason: Refusal, message: &str) -> Failure {
    Failure {
        reason,
        error: Error::InvalidEdit(message.into()),
        seeds_committed: false,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    pub rows: Vec<u64>,
    pub action: Action,
    pub new_seeds: BTreeMap<u64, i32>,
}
pub struct Plan {
    guard: IntentGuard,
    chart: usize,
    identity: Guid,
    review: Review,
}
pub enum Worker {
    Generate(pattern_batch::Task),
    Clear(clear_patterns::Task),
}
pub struct Started {
    pub worker: Worker,
    pub seeds_committed: bool,
}
/// Admission can commit a separate seed action even when worker setup fails.
/// The opaque receipt proves that this partial publication changes metadata
/// only; native owners can preserve their exact clock without guessing tokens.
pub struct Admission {
    pub result: Result<Started, Failure>,
    presentation: Option<crate::song_properties::Application>,
}
impl Admission {
    pub fn presentation_receipt(&self) -> Option<&crate::song_properties::PresentationReceipt> {
        self.presentation.as_ref().and_then(|application| application.presentation_receipt())
    }
}
impl Plan {
    pub fn begin_with_presentation_receipt(self, session: &mut Session, chart: usize, reviewed: &Review, directory: Option<&Path>) -> Admission {
        let before = session.content_token();
        let charts = session.chart_ids().to_vec();
        let result = self.begin(session, chart, reviewed, directory);
        let seeded = match &result { Ok(started) => started.seeds_committed, Err(failure) => failure.seeds_committed };
        let presentation = seeded.then(|| crate::history::song_presentation::preserved_authoring(before, charts, session));
        Admission { result, presentation }
    }
    pub fn all(session: &Session, chart: usize, action: Action) -> Result<Self, Failure> {
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| refusal(Refusal::Empty, "No saved pattern markers"))?;
        Self::rows(
            session,
            chart,
            action,
            data.patterns.unwrap_or_default().into_keys(),
            None,
        )
    }
    pub fn checked(
        session: &Session,
        chart: usize,
        selection: &Selection,
        action: Action,
    ) -> Result<Self, Failure> {
        if selection.chart() != chart {
            return Err(refusal(
                Refusal::Stale,
                "Pattern selection belongs to another chart",
            ));
        }
        selection.validate(session).map_err(|_| {
            refusal(
                Refusal::Stale,
                "Pattern selection belongs to another session or revision",
            )
        })?;
        let rows = selection
            .events()
            .iter()
            .enumerate()
            .filter(|(i, _)| selection.is_selected(*i))
            .filter_map(|(_, e)| {
                if let Event::Pattern { row } = e {
                    Some(*row)
                } else {
                    None
                }
            });
        Self::rows(session, chart, action, rows, None)
    }
    /// Deterministic dependency injection for source witnesses; normal menus draw fresh seeds.
    pub fn rows_with_seeds(
        session: &Session,
        chart: usize,
        rows: impl IntoIterator<Item = u64>,
        seeds: Vec<i32>,
    ) -> Result<Self, Failure> {
        Self::rows(session, chart, Action::GenerateNewSeeds, rows, Some(seeds))
    }
    fn rows(
        session: &Session,
        chart: usize,
        action: Action,
        rows: impl IntoIterator<Item = u64>,
        seeds: Option<Vec<i32>>,
    ) -> Result<Self, Failure> {
        let identity = session
            .chart_ids()
            .get(chart)
            .ok_or_else(|| refusal(Refusal::Invalid, "Pattern chart does not exist"))?
            .clone();
        let rows: Vec<_> = rows
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if rows.is_empty() {
            return Err(refusal(Refusal::Empty, "No patterns selected"));
        }
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| refusal(Refusal::Empty, "No saved pattern markers"))?;
        let definitions = data
            .patterns
            .as_ref()
            .ok_or_else(|| refusal(Refusal::Empty, "No saved pattern markers"))?;
        if rows.iter().any(|row| !definitions.contains_key(row)) {
            return Err(refusal(
                Refusal::Stale,
                "A selected pattern marker no longer exists",
            ));
        }
        if action != Action::Clear {
            crate::generation::source::require_reference_source(session.document(), chart)?;
        }
        let mut new_seeds = BTreeMap::new();
        if action == Action::GenerateNewSeeds {
            let seeds = seeds.unwrap_or_else(|| {
                rows.iter()
                    .map(|_| {
                        new_random_seed()
                    })
                    .collect()
            });
            if seeds.len() != rows.len() || seeds.iter().any(|s| *s < 0 || *s == i32::MAX) {
                return Err(refusal(
                    Refusal::Invalid,
                    "Pattern seeds must be source Random.Next nonnegative integers",
                ));
            }
            new_seeds.extend(rows.iter().copied().zip(seeds));
        }
        Ok(Self {
            guard: IntentGuard::capture(session),
            chart,
            identity,
            review: Review {
                rows,
                action,
                new_seeds,
            },
        })
    }
    pub fn review(&self) -> &Review {
        &self.review
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && self.guard.is_current(session)
    }
    /// Reseed and task capture are deliberately sequential, matching the original
    /// separate seed action before asynchronous ActionAutoGeneratePatterns.
    /// Cancelling/dropping the worker does not erase the committed seed action.
    pub fn begin(
        self,
        session: &mut Session,
        chart: usize,
        reviewed: &Review,
        directory: Option<&Path>,
    ) -> Result<Started, Failure> {
        if !self.is_current(session, chart) {
            return Err(refusal(
                Refusal::Stale,
                "Pattern action belongs to another chart or history lifetime",
            ));
        }
        if reviewed != self.review() {
            return Err(refusal(
                Refusal::MissingReview,
                "Review the exact marker set and seeds before starting",
            ));
        }
        let mut seeds_committed = false;
        if self.review.action == Action::GenerateNewSeeds {
            let mut data = ChartData::read(session.document(), chart)?
                .ok_or_else(|| refusal(Refusal::Stale, "Pattern markers disappeared"))?;
            for (row, seed) in &self.review.new_seeds {
                data.patterns
                    .as_mut()
                    .and_then(|p| p.get_mut(row))
                    .ok_or_else(|| refusal(Refusal::Stale, "Pattern marker disappeared"))?
                    .random_seed = *seed;
            }
            seeds_committed =
                data.save_with_offset(session, chart, None, "Reseed selected patterns")?;
        }
        let worker = if self.review.action == Action::Clear {
            clear_patterns::Task::capture(session, chart, self.review.rows, directory)
                .map(Worker::Clear)
        } else {
            pattern_batch::Task::capture(session, chart, self.review.rows, directory)
                .map(Worker::Generate)
        };
        worker
            .map(|worker| Started {
                worker,
                seeds_committed,
            })
            .map_err(|error| Failure {
                reason: Refusal::Invalid,
                error,
                seeds_committed,
            })
    }
}

/// Native entropy source for the source System.Random.Next nonnegative range.
/// Explicit actions may legitimately draw the same value as their predecessor.
pub fn new_random_seed()->i32{
    let bytes=uuid::Uuid::new_v4().into_bytes();
    (u32::from_le_bytes(bytes[..4].try_into().unwrap())%i32::MAX as u32) as i32
}
