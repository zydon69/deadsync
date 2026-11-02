//! Ephemeral authority for a source-compatible chart order. No locale is persisted.
use crate::{
    Error,
    chart_order::{Key, Orderer, Snapshot},
    generation::metadata::Guid,
    history::{Session, file_intent::IntentGuard},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
/// Host epoch must advance for every locale change, including an ABA return.
/// Native uses i18n::revision directly; no frame callback is needed for checks.
pub trait CultureRevision: Send + Sync {
    fn revision(&self) -> u64;
}
#[derive(Clone)]
pub(crate) struct CultureGuard {
    authority: Arc<dyn CultureRevision>,
    revision: u64,
}
impl CultureGuard {
    pub(crate) fn is_current(&self) -> bool {
        self.authority.revision() == self.revision
    }
}
#[derive(Clone)]
pub struct SourceOutputOrder {
    guard: IntentGuard,
    ids: Vec<Guid>,
    keys: Vec<Key>,
    indices: Vec<usize>,
    culture: CultureGuard,
}
impl SourceOutputOrder {
    /// Zero/one chart has a unique permutation; no textual collation is involved.
    pub fn capture_unambiguous(
        session: &mut Session,
        culture: Arc<dyn CultureRevision>,
        cancelled: &AtomicBool,
    ) -> Result<Option<Self>, Error> {
        if session.chart_ids().len() > 1 {
            return Ok(None);
        }
        struct Unique;
        impl Orderer for Unique {
            fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
                Ok(snapshot.source_indices().to_vec())
            }
        }
        Self::capture(session, &mut Unique, culture, cancelled).map(Some)
    }
    pub fn capture(
        session: &mut Session,
        orderer: &mut impl Orderer,
        culture: Arc<dyn CultureRevision>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        let limits = crate::advanced_export::Limits::default();
        if session.chart_ids().len() > limits.charts
            || session.document().source().len() > limits.bytes
        {
            return Err(invalid(
                "Source-compatible output ordering exceeds configured source limits",
            ));
        }
        let guard = IntentGuard::capture(session);
        let revision = culture.revision();
        let snapshot = Snapshot::loadable_checked(session, cancelled)?.snapshot;
        if snapshot.keys().len() != session.chart_ids().len() {
            return Err(invalid(
                "Source-compatible output ordering requires every chart to be loadable",
            ));
        }
        let indices = orderer.order(&snapshot)?;
        if indices.len() != snapshot.source_indices().len()
            || indices.iter().copied().collect::<BTreeSet<_>>()
                != snapshot.source_indices().iter().copied().collect()
        {
            return Err(invalid("Cultural output order is incomplete or duplicated"));
        }
        let result = Self {
            guard,
            ids: session.chart_ids().to_vec(),
            keys: snapshot.keys().to_vec(),
            indices,
            culture: CultureGuard {
                authority: culture,
                revision,
            },
        };
        if cancelled.load(Ordering::Relaxed) || !result.is_current(session) {
            return Err(invalid("Source-compatible output order became stale"));
        }
        Ok(result)
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.guard.is_current(session)
            && self.ids.as_slice() == session.chart_ids()
            && self.culture.is_current()
    }
    pub(crate) fn indices(&self) -> &[usize] {
        &self.indices
    }
    pub(crate) fn culture(&self) -> CultureGuard {
        self.culture.clone()
    }
    /// Only the storage candidate path can carry an order across media relocation.
    /// Every comparison input must remain identical, not merely the chart count.
    pub(crate) fn for_rebased_candidate(
        &self,
        source: &Session,
        candidate: &mut Session,
    ) -> Result<Self, Error> {
        if !self.is_current(source) || candidate.chart_ids() != self.ids.as_slice() {
            return Err(invalid("Output order does not own the Save As source"));
        }
        let snapshot = Snapshot::loadable(candidate)?.snapshot;
        if snapshot.keys() != self.keys.as_slice() {
            return Err(invalid(
                "Media relocation changed cultural chart ordering inputs",
            ));
        }
        let result = Self {
            guard: IntentGuard::capture(candidate),
            ids: self.ids.clone(),
            keys: self.keys.clone(),
            indices: self.indices.clone(),
            culture: self.culture.clone(),
        };
        if !self.is_current(source) || !result.is_current(candidate) {
            return Err(invalid("Output order changed during media relocation"));
        }
        Ok(result)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
