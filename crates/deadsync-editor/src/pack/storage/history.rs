//! Shared, Send Pack.ini owner. History changes memory; explicit Save alone publishes.
use super::{Data, Error, FileSession};
use std::sync::{Arc, Mutex, MutexGuard};
struct State {
    file: FileSession,
    token: Arc<()>,
    retired: bool,
}
#[derive(Clone)]
pub struct Owner(Arc<Mutex<State>>);
impl std::fmt::Debug for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackOwner").finish_non_exhaustive()
    }
}
impl Owner {
    pub fn new(file: FileSession) -> Self {
        Self(Arc::new(Mutex::new(State {
            file,
            token: Arc::new(()),
            retired: false,
        })))
    }
    fn lock(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Io(std::io::Error::other("Pack owner lock poisoned")))
    }
    /// Storage operations retain the same owner and logical edit token. In
    /// particular Save/retry update their disk receipt, not property history.
    fn with<T>(
        &self,
        operation: impl FnOnce(&mut FileSession) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut state = self.lock()?;
        if state.retired {
            return Err(Error::StaleSnapshot);
        }
        operation(&mut state.file)
    }
    /// A non-history replacement deliberately invalidates earlier logical tokens.
    #[cfg(test)]
    fn replace_untracked(&self, data: Data) -> Result<bool, Error> {
        let mut state = self.lock()?;
        check(&state)?;
        let changed = state.file.replace(data)?;
        if changed {
            state.token = Arc::new(());
        }
        Ok(changed)
    }
    pub fn review(&self) -> Result<Review, crate::Error> {
        let state = self.lock().map_err(core_error)?;
        check_local(&state).map_err(core_error)?;
        Ok(Review {
            owner: self.clone(),
            data: state.file.draft().data().clone(),
            token: state.token.clone(),
        })
    }
    pub fn data(&self) -> Result<Data, Error> {
        self.with(|file| Ok(file.draft().data().clone()))
    }
    pub fn dirty(&self) -> Result<bool, Error> {
        self.with(|file| Ok(!file.is_deleted() && file.draft().has_unsaved_changes()))
    }
    pub fn pending(&self) -> Result<bool, Error> {
        self.with(|file| Ok(file.durability_pending()))
    }
    pub fn save(&self) -> Result<super::Durability, Error> {
        self.with(FileSession::save)
    }
    pub fn retry(&self) -> Result<super::Durability, Error> {
        self.with(FileSession::retry_durability)
    }
    pub fn external(&self) -> Result<bool, Error> {
        self.with(|file| file.has_external_change())
    }
    pub fn prepare_delete(&self) -> Result<super::DeleteSnapshot, Error> {
        self.with(|file| file.prepare_delete())
    }
    pub fn delete(&self, review: super::DeleteSnapshot) -> Result<super::DeleteOutcome, Error> {
        self.with(|file| file.delete(review))
    }
    pub fn deletion_pending(&self) -> Result<bool, Error> {
        self.with(|file| Ok(file.deletion_pending()))
    }
    pub fn retry_delete(&self) -> Result<super::DeleteOutcome, Error> {
        self.with(FileSession::retry_delete)
    }
    pub fn external_snapshot(&self) -> Result<Option<super::ExternalSnapshot>, Error> {
        self.with(|file| Ok((!file.is_deleted()).then(|| file.external_snapshot())))
    }
    pub fn retire(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.retired = true;
        }
    }
}
/// Source cached-field baseline: no mutation or history until deactivation.
#[derive(Clone, Debug)]
pub struct Review {
    owner: Owner,
    data: Data,
    token: Arc<()>,
}
impl Review {
    /// Same local field baseline, including owner and reversible logical token.
    pub fn same_capture(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner.0, &other.owner.0)
            && Arc::ptr_eq(&self.token, &other.token)
            && self.data == other.data
    }
    pub fn data(&self) -> &Data {
        &self.data
    }
}
#[derive(Debug)]
pub(crate) struct Edit {
    owner: Owner,
    before: Data,
    after: Data,
    before_token: Arc<()>,
    after_token: Arc<()>,
}
// Capturing a cached field is local. External disk changes are checked only
// when committing a reviewed edit or publishing Save, never while typing.
fn check_local(state: &State) -> Result<(), Error> {
    if state.retired || state.file.is_deleted() {
        return Err(Error::StaleSnapshot);
    }
    if state.file.durability_pending() {
        return Err(Error::PendingSave);
    }
    Ok(())
}
fn check(state: &State) -> Result<(), Error> {
    check_local(state)?;
    state.file.check_expected()
}
pub(crate) fn core_error(error: Error) -> crate::Error {
    match error {
        Error::ExternalChange => crate::Error::ExternalChange,
        Error::StaleSnapshot => {
            crate::Error::InvalidEdit("Pack property history owner is no longer current".into())
        }
        Error::PendingSave => {
            crate::Error::InvalidEdit("Finish the pending durable pack save first".into())
        }
        Error::Io(error) => crate::Error::Io(error),
        other => crate::Error::InvalidEdit(other.to_string()),
    }
}
impl Edit {
    pub(crate) fn capture_reviewed(
        review: &Review,
        after: Data,
    ) -> Result<Option<Self>, crate::Error> {
        super::super::validate(&after)
            .map_err(Error::from)
            .map_err(core_error)?;
        let state = review.owner.lock().map_err(core_error)?;
        check(&state).map_err(core_error)?;
        if !Arc::ptr_eq(&state.token, &review.token) || state.file.draft().data() != &review.data {
            return Err(crate::Error::ExternalChange);
        }
        if review.data == after {
            return Ok(None);
        }
        Ok(Some(Self {
            owner: review.owner.clone(),
            before: review.data.clone(),
            after,
            before_token: review.token.clone(),
            after_token: Arc::new(()),
        }))
    }
    pub(crate) fn apply(&self, forward: bool) -> Result<(), crate::Error> {
        let mut state = self.owner.lock().map_err(core_error)?;
        check(&state).map_err(core_error)?;
        let (expected, next, token, next_token) = if forward {
            (
                &self.before,
                &self.after,
                &self.before_token,
                &self.after_token,
            )
        } else {
            (
                &self.after,
                &self.before,
                &self.after_token,
                &self.before_token,
            )
        };
        if !Arc::ptr_eq(&state.token, token) || state.file.draft().data() != expected {
            return Err(crate::Error::ExternalChange);
        }
        state.file.replace(next.clone()).map_err(core_error)?;
        state.token = next_token.clone();
        Ok(())
    }
}
#[cfg(test)]
mod tests;
