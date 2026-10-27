//! Prospective note fake flags from the same editor timing dependency pass.
//! Cache misses walk timing entries plus at most MAX_DRAFTS probe heads, never
//! the committed note grid. Repeated frames reuse the result unchanged.
use crate::{
    Error,
    chart_order::Binding,
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    history::Session,
    lane_entry::Draft,
    notes::{Grid, Note, Position},
};
use std::collections::BTreeMap;
pub const MAX_DRAFTS: usize = 64;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub builds: u64,
    pub timing_entries: usize,
    pub probe_heads: usize,
}
pub struct Cache {
    binding: Binding,
    chart: usize,
    columns: u16,
    timing: GenerationTiming,
    keys: Vec<Draft>,
    flags: BTreeMap<Position, bool>,
    work: Work,
}
impl Cache {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let binding = Binding::capture(session, chart)?;
        let descriptor = session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Invalid draft policy chart".into()))?;
        let columns = crate::notes::layout_columns(&descriptor.steps_type)
            .ok_or_else(|| Error::Unsupported("Unsupported draft policy layout".into()))?;
        let timing = GenerationTiming::from_document(session.document(), chart)?;
        Ok(Self {
            binding,
            chart,
            columns,
            timing,
            keys: Vec::new(),
            flags: BTreeMap::new(),
            work: Work::default(),
        })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.binding.is_current(session, chart)
    }
    pub fn work(&self) -> Work {
        self.work
    }
    /// Canonical, bounded draft set. Error leaves the last valid result intact.
    pub fn prepare(
        &mut self,
        session: &Session,
        drafts: impl IntoIterator<Item = Draft>,
    ) -> Result<(), Error> {
        if !self.is_current(session, self.chart) {
            return Err(Error::InvalidEdit("Stale draft note policy".into()));
        }
        let mut keys = Vec::new();
        for draft in drafts {
            if keys.len() == MAX_DRAFTS {
                return Err(Error::InvalidEdit("Too many draft note probes".into()));
            }
            keys.push(draft);
        }
        keys.sort_by_key(|draft| draft.position);
        if keys
            .windows(2)
            .any(|pair| pair[0].position == pair[1].position)
        {
            return Err(Error::InvalidEdit("Duplicate draft note probe".into()));
        }
        // Tail dragging cannot change head fake classification. Entry order
        // and fake propagation use only head row/kind; preserve the source
        // head phase while avoiding replay on every extension of a sustain.
        if keys.len() == self.keys.len()
            && keys
                .iter()
                .zip(&self.keys)
                .all(|(a, b)| a.position == b.position && a.kind == b.kind)
        {
            return Ok(());
        }
        if keys.is_empty() {
            self.keys.clear();
            self.flags.clear();
            return Ok(());
        }
        let players = keys
            .iter()
            .map(|d| d.position.player)
            .max()
            .unwrap()
            .checked_add(1)
            .ok_or_else(|| Error::InvalidEdit("Draft participant overflow".into()))?;
        let mut probes = Grid::new(self.columns, players)?;
        for draft in &keys {
            probes.set(
                draft.position,
                Some(Note {
                    kind: draft.kind,
                    end: draft.end,
                    attributes: Default::default(),
                    tail_attributes: Default::default(),
                }),
            )?;
        }
        let timeline = EditorTimeline::build_clock(&self.timing, &probes, &[])?;
        let flags = timeline
            .heads()
            .iter()
            .map(|head| (head.position, head.fake))
            .collect();
        self.keys = keys;
        self.flags = flags;
        self.work = Work {
            builds: self.work.builds.saturating_add(1),
            timing_entries: self.timing.editor_events().len(),
            probe_heads: self.keys.len(),
        };
        Ok(())
    }
    /// No fallback classification: absence or stale data requires preparation.
    pub fn fake(&self, session: &Session, draft: Draft) -> Result<bool, Error> {
        if !self.is_current(session, self.chart)
            || !self
                .keys
                .iter()
                .any(|key| key.position == draft.position && key.kind == draft.kind)
        {
            return Err(Error::InvalidEdit(
                "Unprepared draft fake classification".into(),
            ));
        }
        self.flags
            .get(&draft.position)
            .copied()
            .ok_or_else(|| Error::InvalidEdit("Missing draft fake classification".into()))
    }
}
