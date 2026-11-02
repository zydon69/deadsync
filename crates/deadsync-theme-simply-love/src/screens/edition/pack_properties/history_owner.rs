//! Native view cache of one shared Pack owner. This cache owns no mutations or
//! history: every accepted field action and Undo/Redo synchronizes it explicitly.
use super::{Data, DeleteOutcome, DeleteSnapshot, Durability, Error, FileSession, PackFile};
struct SharedFile {
    owner: deadsync_editor::pack::storage::history::Owner,
    data: Data,
}
impl PackFile for SharedFile {
    fn history_owner(&self) -> Option<deadsync_editor::pack::storage::history::Owner> {
        Some(self.owner.clone())
    }
    fn synchronize(&mut self) {
        if let Ok(data) = self.owner.data() {
            self.data = data;
        }
    }
    fn retire(&self) {
        self.owner.retire();
    }
    fn data(&self) -> &Data {
        &self.data
    }
    fn dirty(&self) -> bool {
        self.owner.dirty().unwrap_or(true)
    }
    fn pending(&self) -> bool {
        self.owner.pending().unwrap_or(true)
    }
    fn save(&mut self) -> Result<Durability, Error> {
        self.owner.save()
    }
    fn retry(&mut self) -> Result<Durability, Error> {
        self.owner.retry()
    }
    fn external(&self) -> Result<bool, Error> {
        self.owner.external()
    }
    fn prepare_delete(&self) -> Result<DeleteSnapshot, Error> {
        self.owner.prepare_delete()
    }
    fn delete(&mut self, review: DeleteSnapshot) -> Result<DeleteOutcome, Error> {
        self.owner.delete(review)
    }
    fn deletion_pending(&self) -> bool {
        self.owner.deletion_pending().unwrap_or(true)
    }
    fn retry_delete(&mut self) -> Result<DeleteOutcome, Error> {
        self.owner.retry_delete()
    }
    fn external_snapshot(&self) -> Option<deadsync_editor::pack::storage::ExternalSnapshot> {
        self.owner.external_snapshot().ok().flatten()
    }
}
pub(super) fn boxed(file: FileSession) -> Box<dyn PackFile> {
    let data = file.draft().data().clone();
    Box::new(SharedFile {
        owner: deadsync_editor::pack::storage::history::Owner::new(file),
        data,
    })
}
