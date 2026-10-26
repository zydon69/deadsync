//! Coordinated profile-file deletion and undoable active-song reference repair.
use super::{
    profile_references::Reset,
    profiles::{DeletedProfile, ProfileFile},
};
use crate::{
    Error,
    history::{RevisionToken, Session},
};

pub struct Action {
    file: DeletedProfile,
    token: RevisionToken,
    changed_song: bool,
    applied_token: RevisionToken,
    references: usize,
    charts: usize,
}
impl Action {
    pub fn apply(file: &ProfileFile, session: &mut Session) -> Result<Self, Error> {
        let reset = Reset::capture(
            session,
            file.profile().config().kind(),
            file.profile().guid(),
        )?;
        let references = reset.affected_references();
        let charts = reset.affected_charts();
        let mut deleted = file.delete()?;
        let changed_song = match reset.apply(session) {
            Ok(changed) => changed,
            Err(error) => {
                if let Err(restore) = deleted.restore() {
                    let backup = deleted.preserve_backup()?;
                    return Err(Error::InvalidEdit(format!(
                        "Reference repair failed ({error}); restoration failed ({restore}); recovery profile: {}",
                        backup.display()
                    )));
                }
                return Err(error);
            }
        };
        Ok(Self {
            file: deleted,
            token: session.revision_token(),
            changed_song,
            applied_token: session.revision_token(),
            references,
            charts,
        })
    }
    pub fn affected_references(&self) -> usize {
        self.references
    }
    pub fn affected_charts(&self) -> usize {
        self.charts
    }
    pub fn is_deleted(&self) -> bool {
        self.file.is_deleted()
    }
    fn check(&self, session: &Session) -> Result<(), Error> {
        if session.accepts_token(&self.token) {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Profile deletion belongs to another document or revision".into(),
            ))
        }
    }
    pub fn undo(&mut self, session: &mut Session) -> Result<bool, Error> {
        self.check(session)?;
        if !self.file.is_deleted() {
            return Ok(false);
        }
        self.file.restore()?;
        if self.changed_song {
            if let Err(error) = session.undo() {
                self.file.delete_again().map_err(|rollback| {
                    Error::InvalidEdit(format!(
                        "Reference undo failed ({error}); disk rollback failed ({rollback})"
                    ))
                })?;
                return Err(error);
            }
        }
        self.token = session.revision_token();
        Ok(true)
    }
    pub fn redo(&mut self, session: &mut Session) -> Result<bool, Error> {
        self.check(session)?;
        if self.file.is_deleted() {
            return Ok(false);
        }
        if self.changed_song && !session.redo_reaches(&self.applied_token) {
            return Err(Error::InvalidEdit(
                "Profile deletion redo history has changed".into(),
            ));
        }
        self.file.delete_again()?;
        if self.changed_song {
            if let Err(error) = session.redo() {
                self.file.restore().map_err(|rollback| {
                    Error::InvalidEdit(format!(
                        "Reference redo failed ({error}); disk rollback failed ({rollback})"
                    ))
                })?;
                return Err(error);
            }
        }
        self.token = session.revision_token();
        Ok(true)
    }
}
