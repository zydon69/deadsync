//! Sealed Song Properties publication: validate the new row clock before mutation.
use super::{ApplicationReviewToken, Change, RevisionToken, Session};
use crate::{Error, chart_focus::Position, song_properties::Draft};
enum Operation {
    Edit { label: String, change: Change },
    History { redo: bool },
    None,
}
pub struct Plan {
    review: ApplicationReviewToken,
    revision: RevisionToken,
    focus: Option<(usize, f64)>,
    position: Option<Position>,
    music_changed: bool,
    operation: Operation,
}
pub struct Application {
    changed: bool,
    position: Option<Position>,
    music_changed: bool,
}
impl Application {
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn music_changed(&self) -> bool {
        self.music_changed
    }
    pub fn into_position(self) -> Option<Position> {
        self.position
    }
}
fn invalid() -> Error {
    Error::InvalidEdit("History is not a typed Song Properties edit".into())
}
// Explicit property grammar, never replay filesystem/profile/preferences actions.
fn copy(change: &Change) -> Result<Change, Error> {
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
        Change::SongTimingChoice { before, after } => Change::SongTimingChoice {
            before: before.clone(),
            after: after.clone(),
        },
        Change::SongTimingProperties(change) => {
            Change::SongTimingProperties(Box::new(copy(change)?))
        }
        Change::SongPropertiesLive(change) => Change::SongPropertiesLive(Box::new(copy(change)?)),
        Change::Group(changes) => {
            Change::Group(changes.iter().map(copy).collect::<Result<_, _>>()?)
        }
        _ => return Err(invalid()),
    })
}
fn position(session: &Session, focus: Option<(usize, f64)>) -> Result<Option<Position>, Error> {
    focus
        .map(|(chart, row)| Position::at_fractional_row(session, chart, row))
        .transpose()
}
impl Plan {
    pub fn prepare(
        draft: &Draft,
        session: &Session,
        focus: Option<(usize, f64)>,
    ) -> Result<Self, Error> {
        if !draft.requires_live_handoff() {
            return Err(invalid());
        }
        let mut staged = super::timing_live::presentation_candidate(session)?;
        let changed = draft.apply(&mut staged)?;
        let operation = if changed {
            if staged.undo.len() != 1 {
                return Err(invalid());
            }
            let entry = staged.undo.pop().ok_or_else(invalid)?;
            if !matches!(entry.change, Change::SongPropertiesLive(_)) {
                return Err(invalid());
            }
            copy(&entry.change)?;
            Operation::Edit {
                label: entry.label,
                change: entry.change,
            }
        } else {
            Operation::None
        };
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            focus,
            position: position(&staged, focus)?,
            music_changed: session.document.text(None, "MUSIC")
                != staged.document.text(None, "MUSIC"),
            operation,
        })
    }
    pub fn prepare_history(
        session: &Session,
        focus: Option<(usize, f64)>,
        redo: bool,
    ) -> Result<Self, Error> {
        let entry = (if redo {
            session.redo.last()
        } else {
            session.undo.last()
        })
        .filter(|entry| matches!(entry.change, Change::SongPropertiesLive(_)))
        .ok_or_else(invalid)?;
        let mut staged = super::timing_live::presentation_candidate(session)?;
        staged.apply(&mut copy(&entry.change)?, redo)?;
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
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            focus,
            position: position(&staged, focus)?,
            music_changed: session.document.text(None, "MUSIC")
                != staged.document.text(None, "MUSIC"),
            operation: Operation::History { redo },
        })
    }
    pub fn chart(&self) -> Option<usize> {
        self.focus.map(|(chart, _)| chart)
    }
    pub fn apply(self, session: &mut Session) -> Result<Application, Error> {
        if !session.accepts_application_review(&self.review)
            || !session.accepts_token(&self.revision)
        {
            return Err(Error::ExternalChange);
        }
        let changed = match self.operation {
            Operation::Edit { label, change } => {
                session.execute(label, change)?;
                true
            }
            Operation::History { redo } => {
                if redo {
                    session.redo()?
                } else {
                    session.undo()?
                }
            }
            Operation::None => false,
        };
        debug_assert!(
            self.position
                .as_ref()
                .is_none_or(|position| position.is_bound_to(session, self.focus.unwrap().0))
        );
        Ok(Application {
            changed,
            position: self.position,
            music_changed: self.music_changed,
        })
    }
}
impl Session {
    pub(crate) fn wrap_song_properties_live(&mut self) {
        let entry = self
            .undo
            .last_mut()
            .expect("successful Song Properties commit");
        let change = std::mem::replace(&mut entry.change, Change::Group(Vec::new()));
        entry.change = Change::SongPropertiesLive(Box::new(change));
    }
    pub fn history_changes_song_properties_live(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::SongPropertiesLive(_)))
    }
}
