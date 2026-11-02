//! Text labels use GrooveAuthor sanitization and retain decoded legacy text.
use super::{Scope, Target, tempo::beat_row};
use crate::{Error, document::Format, history::Session};
use std::collections::BTreeMap;
pub fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if matches!(c, '\r' | '\n' | '\t' | '#' | ';' | ':' | '\\' | '/' | ',') {
                '_'
            } else {
                c
            }
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    values: BTreeMap<u64, String>,
}
impl Labels {
    pub fn from_document(
        document: &crate::document::Document,
        chart: usize,
    ) -> Result<Self, Error> {
        if chart >= document.charts().len() {
            return Err(error("Label chart does not exist"));
        }
        let own=if document.format==Format::Sm {super::uses_chart_timing(document,chart)} else {super::reference_uses_chart_timing(document,chart)};
        Self::parse(&super::resident::text(document,own.then_some(chart),"LABELS")?)
    }
    /// Input is the decoded MSD parameter. Do not strip comments after unescaping:
    /// escaped slashes inside an existing label are literal text.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut entries = Vec::new();
        for entry in text.trim().split(',') {
            let fields: Vec<_> = entry.split('=').collect();
            if fields.len() != 2 {
                continue;
            }
            let Ok(beat) = fields[0].trim().parse::<f64>() else {
                continue;
            };
            if !beat.is_finite() {
                return Err(error("Nonfinite label beat"));
            }
            let text = fields[1].split(';').next().unwrap_or_default().to_owned();
            entries.push((beat, text));
        }
        entries.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("Finite label beats"));
        let mut values = BTreeMap::new();
        for (beat, text) in entries {
            let row = beat_row(beat)?;
            if row >= 0 {
                values.insert(row as u64, text);
            }
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &BTreeMap<u64, String> {
        &self.values
    }
    /// Strictly preceding label, wrapping to the last label like GrooveAuthor.
    pub fn previous_row(&self, row: u64) -> Option<u64> {
        self.values
            .range(..row)
            .next_back()
            .or_else(|| self.values.last_key_value())
            .map(|(&row, _)| row)
    }
    /// Strictly following label, wrapping to the first label like GrooveAuthor.
    pub fn next_row(&self, row: u64) -> Option<u64> {
        use std::ops::Bound::{Excluded, Unbounded};
        self.values
            .range((Excluded(row), Unbounded))
            .next()
            .or_else(|| self.values.first_key_value())
            .map(|(&row, _)| row)
    }
    pub fn set(&mut self, row: u64, text: &str) -> Result<bool, Error> {
        valid_row(row)?;
        let text = sanitize(text);
        // The original widget accepts '=', but its writer/CSV reader loses that
        // label on reopen. Refuse the unrepresentable value instead of losing data.
        if text.contains('=') {
            return Err(error(
                "Labels cannot contain '=' because the SSC reader treats it as a field delimiter",
            ));
        }
        if self.values.get(&row) == Some(&text) {
            return Ok(false);
        }
        self.values.insert(row, text);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        Ok(self.values.remove(&row).is_some())
    }
    pub(super) fn clone_at(&mut self, row: u64, value: &str) -> Result<(), Error> {
        valid_row(row)?;
        // Cloning an existing label does not invoke the text-entry sanitizer.
        self.values.insert(row, value.to_owned());
        Ok(())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        let text = self
            .values
            .get(&from)
            .ok_or_else(|| error("Label does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(error("Another label occupies the destination"));
        }
        let text = text.clone();
        self.values.remove(&from);
        self.values.insert(to, text);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        let mut value = self
            .values
            .iter()
            .map(|(row, text)| format!("{}={text}", *row as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",");
        // A final empty CSV item prevents MSD/global trim from losing meaningful
        // whitespace at the end of the last label. Fumen ignores the empty item.
        if self
            .values
            .last_key_value()
            .is_some_and(|(_, text)| text.chars().last().is_some_and(char::is_whitespace))
        {
            value.push(',');
        }
        value
    }
}
pub struct Draft {
    target: Target,
    original: Labels,
    current: Labels,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let target = Target::capture(session, chart, scope)?;

        let current = Labels::parse(
            &super::resident::text(session.document(),target.origin(),"LABELS")?,
        )?;
        Ok(Self {
            target,
            original: current.clone(),
            current,
        })
    }
    pub fn scope(&self) -> Scope {
        self.target.scope
    }
    pub fn inherited(&self) -> bool {
        self.target.inherited
    }
    pub fn affected_charts(&self) -> &[usize] {
        &self.target.affected
    }
    pub fn labels(&self) -> &Labels {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn set(&mut self, row: u64, text: &str) -> Result<bool, Error> {
        self.current.set(row, text)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        self.current.remove(row)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        self.current.move_node(from, to)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        let changes = if self.has_changes() {
            vec![("LABELS", self.current.serialize())]
        } else {
            vec![]
        };
        self.target
            .apply(session, chart, &changes, "Edit chart labels")
    }
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row <= i32::MAX as u64 {
        Ok(())
    } else {
        Err(error("Label row exceeds the reference limit"))
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
