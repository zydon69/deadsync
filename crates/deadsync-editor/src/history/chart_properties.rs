//! Provenance minted only by a successful, validated ChartProperties commit.
use super::timing_live::presentation_candidate as candidate;
use super::{Change, Session};
use crate::Error;
#[derive(Clone, Copy, Debug)]
pub(super) struct Effect {
    pub chart: usize,
    pub clock: bool,
    pub audio: bool,
}
impl Session {
    pub(crate) fn mark_chart_properties_history(&mut self, chart: usize, clock: bool, audio: bool) {
        let entry = self
            .undo
            .last_mut()
            .expect("a committed chart property gesture has history");
        let change = std::mem::replace(&mut entry.change, Change::Group(Vec::new()));
        entry.change = Change::ChartProperties {
            effect: Effect {
                chart,
                clock,
                audio,
            },
            change: Box::new(change),
        };
    }
    pub fn chart_properties_history_chart(&self, redo: bool) -> Option<usize> {
        match &(if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })?
        .change
        {
            Change::ChartProperties { effect, .. } => Some(effect.chart),
            _ => None,
        }
    }
    pub fn chart_properties_history_preserves_audio(&self, redo: bool) -> Option<bool> {
        match &(if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })?
        .change
        {
            Change::ChartProperties { effect, .. } => Some(effect.audio),
            _ => None,
        }
    }
    pub fn apply_chart_properties_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        let effect = match &(if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .ok_or_else(|| Error::InvalidEdit("No chart properties history".into()))?
        .change
        {
            Change::ChartProperties { effect, .. } => *effect,
            _ => return Err(Error::InvalidEdit("No chart properties history".into())),
        };
        super::song_presentation::apply_chart_properties_history(
            self,
            redo,
            effect.clock,
            effect.audio,
        )
    }
}

// Clone only the closed, disk-free ChartProperties grammar. In particular a
// profile/disk receipt or DocumentSwap cannot be replayed during validation.
fn copy_change(change: &Change) -> Result<Change, Error> {
    Ok(match change {
        Change::Value {
            range,
            before,
            after,
        } => Change::Value {
            range: range.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AddedProperty {
            chart,
            name,
            before,
            after,
        } => Change::AddedProperty {
            chart: *chart,
            name: name.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AttackIdentity(change) => Change::AttackIdentity(change.clone()),
        Change::Group(changes) => {
            Change::Group(changes.iter().map(copy_change).collect::<Result<_, _>>()?)
        }
        Change::ChartProperties { effect, change } => Change::ChartProperties {
            effect: *effect,
            change: Box::new(copy_change(change)?),
        },
        _ => return Err(Error::InvalidEdit("No chart properties history".into())),
    })
}
fn position(
    session: &Session,
    focus: &crate::chart_focus::Position,
    target: usize,
    clock: bool,
) -> Result<crate::chart_focus::Position, Error> {
    let chart = focus.chart();
    let forward = crate::chart_focus::Position::at_fractional_row(session, chart, focus.row())?;
    if (!clock && target == chart) || forward.music_time().to_bits() == focus.music_time().to_bits()
    {
        Ok(forward)
    } else {
        crate::chart_focus::Position::at_time(session, chart, focus.music_time())
    }
}
impl Session {
    /// Validate the prospective focused clock before committing the real gesture.
    /// The returned position is stamped for that exact next committed revision.
    pub fn chart_properties_edit_position(
        &self,
        draft: &crate::chart_properties::Draft,
        target: usize,
        focus: &crate::chart_focus::Position,
    ) -> Result<crate::chart_focus::Position, Error> {
        if !focus.is_bound_to(self, focus.chart()) {
            return Err(Error::ExternalChange);
        }
        let mut staged = candidate(self)?;
        draft.apply(&mut staged, target)?;
        position(&staged, focus, target, draft.preserves_presentation_clock())
    }
    pub fn chart_properties_history_position(
        &self,
        redo: bool,
        focus: &crate::chart_focus::Position,
    ) -> Result<crate::chart_focus::Position, Error> {
        if !focus.is_bound_to(self, focus.chart()) {
            return Err(Error::ExternalChange);
        }
        let entry = (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .ok_or_else(|| Error::InvalidEdit("No chart properties history".into()))?;
        let effect = match &entry.change {
            Change::ChartProperties { effect, .. } => *effect,
            _ => return Err(Error::InvalidEdit("No chart properties history".into())),
        };
        let mut change = copy_change(&entry.change)?;
        let mut staged = candidate(self)?;
        staged.apply(&mut change, redo)?;
        staged.revision = if redo {
            entry.after_revision
        } else {
            entry.before_revision
        };
        staged.content_revision = if redo {
            entry.after_content_revision
        } else {
            entry.before_content_revision
        };
        position(&staged, focus, effect.chart, effect.clock)
    }
}
