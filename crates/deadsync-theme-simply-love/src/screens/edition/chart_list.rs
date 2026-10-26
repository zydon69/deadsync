//! Shared sorted chart rows for the chart picker and next/previous navigation.
//! Source indices select documents; row positions are presentation only.
use deadsync_editor::{
    Error,
    chart_order::{Key, Orderer},
    generation::metadata::Guid,
    workspace::Workspace,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub source_index: usize,
    pub key: Key,
}

#[derive(Default)]
pub(super) struct Cache {
    signature: Option<Signature>,
    rows: Vec<Row>,
}
#[derive(PartialEq, Eq)]
struct Signature {
    revision: u64,
    identities: Vec<Guid>,
    language_revision: u64,
}
impl Signature {
    fn capture(workspace: &Workspace, language_revision: u64) -> Self {
        let session = &workspace.file().session;
        Self {
            revision: session.content_revision(),
            identities: session.chart_ids().to_vec(),
            language_revision,
        }
    }
}
impl Cache {
    pub fn clear(&mut self) {
        self.signature = None;
        self.rows.clear();
    }
    pub fn is_current(&self, workspace: &Workspace, language_revision: u64) -> bool {
        self.signature.as_ref() == Some(&Signature::capture(workspace, language_revision))
    }
    /// An unchanged content/culture lookup does not build a Snapshot, inspect
    /// any grid, reconstruct clocks, or invoke the native collator. Resident
    /// IDs distinguish sessions and structural undo branches; active chart is
    /// intentionally absent so repeated navigation can reuse these rows.
    pub fn refresh(
        &mut self,
        workspace: &mut Workspace,
        language_revision: u64,
        orderer: &mut impl Orderer,
    ) -> Result<&[Row], Error> {
        if crate::i18n::revision() != language_revision {
            self.signature = None;
            self.rows.clear();
            return Err(Error::InvalidEdit("Chart list culture changed".into()));
        }
        let signature = Signature::capture(workspace, language_revision);
        if self.signature.as_ref() == Some(&signature) {
            return Ok(&self.rows);
        }
        // Failed refreshes must not leave an old ordering available as current.
        self.signature = None;
        self.rows.clear();
        let snapshot = workspace.chart_order_snapshot()?;
        let indices = if snapshot.source_indices().len() < 2 {
            snapshot.source_indices().to_vec()
        } else {
            orderer.order(&snapshot)?
        };
        let mut expected = snapshot.source_indices().to_vec();
        let mut received = indices.clone();
        expected.sort_unstable();
        received.sort_unstable();
        if received != expected {
            return Err(Error::InvalidEdit(
                "Chart list is not an exhaustive source-index permutation".into(),
            ));
        }
        // The shell boundary can reenter and change localization. Reject such
        // a result before caching it, just as deferred navigation rejects it.
        if crate::i18n::revision() != language_revision {
            return Err(Error::InvalidEdit("Chart list culture changed".into()));
        }
        let keys: std::collections::HashMap<_, _> = snapshot
            .source_indices()
            .iter()
            .copied()
            .zip(snapshot.keys().iter())
            .collect();
        self.rows = indices
            .into_iter()
            .map(|source_index| Row {
                source_index,
                key: (*keys[&source_index]).clone(),
            })
            .collect();
        self.signature = Some(signature);
        Ok(&self.rows)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Returns a source chart index. This opens/selects a chart, and does not
    /// model focus traversal or rearrangement among simultaneously open tabs.
    pub fn adjacent(&self, active_source: usize, backwards: bool) -> Option<usize> {
        let position = self
            .rows
            .iter()
            .position(|row| row.source_index == active_source)?;
        let delta = if backwards { self.rows.len() - 1 } else { 1 };
        Some(self.rows[(position + delta) % self.rows.len()].source_index)
    }
}

/// Keyboard highlight stays separate from committed Workspace selection.
/// Refresh preserves the highlighted resident identity through reordered rows
/// and source-index shifts. Cancel simply discards this value.
#[derive(Default)]
pub(super) struct Selection {
    identity: Option<Guid>,
}
impl Selection {
    pub fn reconcile(&mut self, rows: &[Row], active_source: usize) {
        if self
            .identity
            .as_ref()
            .is_some_and(|id| rows.iter().any(|row| &row.key.guid == id))
        {
            return;
        }
        self.identity = rows
            .iter()
            .find(|row| row.source_index == active_source)
            .or_else(|| rows.first())
            .map(|row| row.key.guid.clone());
    }
    pub fn position(&self, rows: &[Row]) -> Option<usize> {
        self.identity
            .as_ref()
            .and_then(|id| rows.iter().position(|row| &row.key.guid == id))
    }
    pub fn move_by(&mut self, rows: &[Row], delta: isize) {
        if rows.is_empty() {
            self.identity = None;
            return;
        }
        let current = self.position(rows).unwrap_or(0);
        let next = (current as i128 + delta as i128).rem_euclid(rows.len() as i128) as usize;
        self.identity = Some(rows[next].key.guid.clone());
    }
    pub fn selected_source(&self, rows: &[Row]) -> Option<usize> {
        self.position(rows)
            .map(|position| rows[position].source_index)
    }
}
