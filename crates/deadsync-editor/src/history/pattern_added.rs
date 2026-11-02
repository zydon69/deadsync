//! Source OnEvent(s)Added callback, separate from note selection and song edits.
use super::*;
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub chart: crate::generation::metadata::Guid,
    pub row: u64,
}
#[derive(Debug)]
pub(crate) struct History {
    pub before: Vec<Target>,
    pub after: Vec<Target>,
}
#[derive(Clone, Debug)]
pub struct PatternAddedEffect {
    token: ContentToken,
    targets: Vec<Target>,
}
impl PatternAddedEffect {
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_content_token(&self.token)
    }
    pub fn additions(&self) -> impl Iterator<Item = (&crate::generation::metadata::Guid, u64)> {
        self.targets
            .iter()
            .map(|target| (&target.chart, target.row))
    }
}
impl Session {
    /// Source ForceAdd creates fresh event objects even for equal definitions.
    /// This explicit file action changes history identity, never document bytes.
    pub(crate) fn commit_pattern_readd(&mut self, chart: usize) -> Result<bool, Error> {
        if self.chart_ids.get(chart).is_none() {
            return Err(Error::InvalidEdit(
                "Pattern readd chart is unavailable".into(),
            ));
        }
        self.execute(
            "Readd pasted pattern events".into(),
            Change::PatternReadd { chart },
        )?;
        Ok(true)
    }

    pub fn take_pattern_added_update(&mut self) -> Option<PatternAddedEffect> {
        if self.pattern_added_update.is_empty() {
            return None;
        }
        Some(PatternAddedEffect {
            token: self.content_token(),
            targets: std::mem::take(&mut self.pattern_added_update),
        })
    }
    pub(crate) fn record_pattern_additions(
        &mut self,
        chart: usize,
        before: Vec<u64>,
        after: Vec<u64>,
        changed: bool,
    ) {
        let identity = self.chart_ids[chart].clone();
        let target = |row| Target {
            chart: identity.clone(),
            row,
        };
        self.record_pattern_history(
            History {
                before: before.into_iter().map(target).collect(),
                after: after.into_iter().map(target).collect(),
            },
            changed,
        );
    }
    pub(crate) fn record_pattern_replacements(
        &mut self,
        chart: usize,
        before: Vec<u64>,
        after: Vec<u64>,
        changed: bool,
    ) {
        // Metadata save just recorded additions inferred from new heads. ForceAdd's
        // explicit ordered readd list replaces that receipt, including equal rows.
        if changed {
            if let Some(previous) = self
                .undo
                .last_mut()
                .and_then(|entry| entry.pattern_added.take())
            {
                self.pattern_added_update.truncate(
                    self.pattern_added_update
                        .len()
                        .saturating_sub(previous.after.len()),
                );
            }
        }
        self.record_pattern_additions(chart, before, after, changed);
    }
    pub(super) fn record_pattern_history(&mut self, receipt: History, changed: bool) {
        if receipt.before.is_empty() && receipt.after.is_empty() {
            return;
        }
        self.pattern_added_update
            .extend(receipt.after.iter().cloned());
        if changed {
            let entry = self
                .undo
                .last_mut()
                .expect("committed Pattern callback action");
            assert_eq!(entry.after_revision, self.revision);
            entry.pattern_added = Some(receipt);
        }
    }
}
