//! Preserve Workspace editing state and preallocate structural-focus migration.
use super::{StructuralFocus, Workspace};
use crate::{
    Error,
    storage::{PendingDurability, RelocationReceipt, SaveAsOutcome, SaveAsReview},
};
use std::{collections::BTreeMap, path::Path};
pub struct WorkspaceSaveAsReview {
    file: SaveAsReview,
    focus: BTreeMap<u64, StructuralFocus>,
}
impl WorkspaceSaveAsReview {
    pub fn is_current(&self,workspace:&Workspace)->bool{self.file.is_current(&workspace.file)}
    pub fn issues(&self)->&[crate::advanced_export::Issue]{self.file.issues()}
    pub fn output_policy(&self)->&crate::output_policy::OutputPolicy{self.file.output_policy()}

    pub fn destination(&self) -> &Path {
        self.file.destination()
    }
    pub fn replaces_existing(&self) -> bool {
        self.file.replaces_existing()
    }
    pub fn staged_bytes(&self) -> &[u8] {
        self.file.staged_bytes()
    }
    pub fn relocation(&self) -> &RelocationReceipt {
        self.file.relocation()
    }
}
impl Workspace {
    pub fn capture_unambiguous_source_output_order(&mut self,culture:std::sync::Arc<dyn crate::source_output_order::CultureRevision>,cancelled:&std::sync::atomic::AtomicBool)->Result<Option<crate::source_output_order::SourceOutputOrder>,Error>{crate::source_output_order::SourceOutputOrder::capture_unambiguous(&mut self.file.session,culture,cancelled)}
    pub fn capture_source_output_order(&mut self,orderer:&mut impl crate::chart_order::Orderer,culture:std::sync::Arc<dyn crate::source_output_order::CultureRevision>,cancelled:&std::sync::atomic::AtomicBool)->Result<crate::source_output_order::SourceOutputOrder,Error>{crate::source_output_order::SourceOutputOrder::capture(&mut self.file.session,orderer,culture,cancelled)}
    pub fn prepare_save_as(&self, destination: &Path) -> Result<WorkspaceSaveAsReview, Error> {
        self.prepare_save_as_with_revisions(destination, &[])
    }
    pub fn prepare_save_as_with_revisions(
        &self,
        destination: &Path,
        extra: &[u64],
    ) -> Result<WorkspaceSaveAsReview, Error> {
        self.prepare_save_as_with_policy_and_revisions(destination,self.file.output_policy().clone(),extra)
    }
    pub fn prepare_save_as_with_policy(&self,destination:&Path,policy:crate::output_policy::OutputPolicy)->Result<WorkspaceSaveAsReview,Error>{self.prepare_save_as_with_policy_and_revisions(destination,policy,&[])}
    pub fn prepare_save_as_with_policy_and_revisions(&self,destination:&Path,policy:crate::output_policy::OutputPolicy,extra:&[u64])->Result<WorkspaceSaveAsReview,Error>{self.prepare_save_as_order(destination,policy,extra,None)}
    pub fn prepare_save_as_with_policy_and_revisions_ordered(&self,destination:&Path,policy:crate::output_policy::OutputPolicy,extra:&[u64],order:&crate::source_output_order::SourceOutputOrder)->Result<WorkspaceSaveAsReview,Error>{self.prepare_save_as_order(destination,policy,extra,Some(order))}
    fn prepare_save_as_order(&self,destination:&Path,policy:crate::output_policy::OutputPolicy,extra:&[u64],order:Option<&crate::source_output_order::SourceOutputOrder>)->Result<WorkspaceSaveAsReview,Error>{
        let mut stamps = extra.to_vec();
        for (stamp, value) in &self.structural_focus {
            stamps.extend([*stamp, value.before_revision]);
        }
        let file=match order{Some(order)=>self.file.prepare_save_as_with_policy_and_revisions_ordered(destination,policy,&stamps,order)?,None=>self.file.prepare_save_as_with_policy_and_revisions(destination,policy,&stamps)?};
        let mut focus = BTreeMap::new();
        for (stamp, value) in &self.structural_focus {
            let after = file.relocation().remap_revision(*stamp).ok_or_else(|| {
                Error::InvalidEdit("Missing structural-focus revision mapping".into())
            })?;
            let before = file
                .relocation()
                .remap_revision(value.before_revision)
                .ok_or_else(|| {
                    Error::InvalidEdit("Missing structural-focus before revision mapping".into())
                })?;
            focus.insert(
                after,
                StructuralFocus {
                    before_revision: before,
                    before_chart: value.before_chart,
                    after_chart: value.after_chart,
                },
            );
        }
        Ok(WorkspaceSaveAsReview { file, focus })
    }
    pub fn save_as(&mut self, review: WorkspaceSaveAsReview) -> Result<SaveAsOutcome, Error> {
        let result = self.file.save_as(review.file)?;
        self.structural_focus = review.focus;
        Ok(result)
    }
    pub fn save_as_reviewed(&mut self,review:WorkspaceSaveAsReview,acknowledged:&std::collections::BTreeSet<usize>)->Result<SaveAsOutcome,Error>{
        let result=self.file.save_as_reviewed(review.file,acknowledged)?;self.structural_focus=review.focus;Ok(result)
    }
    pub fn prepare_save_with_policy_ordered(&mut self,policy:crate::output_policy::OutputPolicy,order:&crate::source_output_order::SourceOutputOrder)->Result<crate::storage::SaveReview,Error>{self.file.prepare_save_with_policy_ordered(policy,order)}
    pub fn prepare_save(&mut self)->Result<crate::storage::SaveReview,Error>{self.file.prepare_save()}
    pub fn prepare_save_with_policy(&mut self,policy:crate::output_policy::OutputPolicy)->Result<crate::storage::SaveReview,Error>{self.file.prepare_save_with_policy(policy)}
    pub fn save_or_review(&mut self)->Result<crate::storage::SaveAttempt,Error>{self.file.save_or_review()}
    pub fn save_reviewed(&mut self,review:crate::storage::SaveReview,acknowledged:&std::collections::BTreeSet<usize>)->Result<crate::storage::SaveAttempt,Error>{self.file.save_reviewed(review,acknowledged)}
    pub fn retry_save_as_durability(&mut self, receipt: &PendingDurability) -> Result<(), Error> {
        receipt.retry(&mut self.file)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chart_focus::Position, chart_views::ActiveChartViews, storage::FileSession,
        workspace::Command,
    };
    fn workspace() -> (tempfile::TempDir, Workspace, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("old")).unwrap();
        std::fs::create_dir(root.path().join("new")).unwrap();
        let source = root.path().join("old/song.ssc");
        std::fs::write(&source,b"#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#WARPS:3=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
        let workspace = Workspace::open(&source, 0).unwrap();
        let destination = root.path().join("new/song.ssc");
        (root, workspace, destination)
    }
    fn receipt(outcome: &SaveAsOutcome) -> &RelocationReceipt {
        match outcome {
            SaveAsOutcome::Durable { relocation, .. }
            | SaveAsOutcome::WrittenPendingDurability { relocation, .. } => relocation,
        }
    }
    #[test]
    fn structural_focus_history_including_detached_stamps_survives_workspace_save_as() {
        let (_root, mut workspace, destination) = workspace();
        workspace.execute(Command::DuplicateChart).unwrap();
        assert_eq!(workspace.chart(), 1);
        workspace.structural_focus.insert(
            5000,
            StructuralFocus {
                before_revision: 4999,
                before_chart: 0,
                after_chart: 0,
            },
        );
        let length = workspace.structural_focus.len();
        let review = workspace.prepare_save_as(&destination).unwrap();
        let remapped = review.relocation().remap_revision(5000).unwrap();
        workspace.save_as(review).unwrap();
        assert_eq!(workspace.structural_focus.len(), length);
        assert!(workspace.structural_focus.contains_key(&remapped));
        assert!(remapped > 5000);
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.chart(), 0);
        workspace.execute(Command::Redo).unwrap();
        assert_eq!(workspace.chart(), 1);
    }
    #[test]
    fn exact_fractional_warp_focus_and_view_selection_rebind_without_time_inversion() {
        let (_root, mut workspace, destination) = workspace();
        let session = &workspace.file.session;
        let focus = Position::at_fractional_row(session, 0, 160.5).unwrap();
        let mut views = ActiveChartViews::empty(session, focus.music_time()).unwrap();
        views.show(session, 0, true).unwrap();
        views.set_selection_anchor(session, Some(48)).unwrap();
        workspace
            .seek_preserving_selection(focus.edit_row())
            .unwrap();
        let review = workspace.prepare_save_as(&destination).unwrap();
        assert!(focus.can_rebind_relocation(review.relocation()));
        assert!(views.can_rebind_relocation(review.relocation()));
        let outcome = workspace.save_as(review).unwrap();
        let session = &workspace.file.session;
        let rebound = focus.rebind_relocation(session, receipt(&outcome)).unwrap();
        assert_eq!(rebound.row().to_bits(), 160.5f64.to_bits());
        assert_eq!(rebound.music_time().to_bits(), focus.music_time().to_bits());
        assert!(rebound.is_current(&workspace));
        let original: Vec<_> = views
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.identity().clone(),
                    entry.position().row().to_bits(),
                    entry.selection_anchor(),
                    entry.dedicated(),
                )
            })
            .collect();
        views.rebind_relocation(session, receipt(&outcome)).unwrap();
        let after: Vec<_> = views
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.identity().clone(),
                    entry.position().row().to_bits(),
                    entry.selection_anchor(),
                    entry.dedicated(),
                )
            })
            .collect();
        assert_eq!(after, original);
        assert!(
            views.rebind_relocation(session, receipt(&outcome)).is_err(),
            "proof cannot bless arbitrary/repeated content"
        );
    }
    #[test]
    fn recovered_dirty_workspace_save_as_durably_saves_without_losing_guid_or_cursor() {
        let (root, original, destination) = workspace();
        let path = original.file.require_existing_path().unwrap().to_path_buf();
        drop(original);
        let mut file = FileSession::open(&path).unwrap();
        file.session = crate::history::Session::recovered(file.session.document().clone());
        let mut workspace = Workspace::from_file(file, 0).unwrap();
        workspace.seek_preserving_selection(17).unwrap();
        let ids = workspace.file.session.chart_ids().to_vec();
        assert!(workspace.file.session.is_dirty());
        let review = workspace.prepare_save_as(&destination).unwrap();
        workspace.save_as(review).unwrap();
        assert!(!workspace.file.session.is_dirty());
        assert_eq!(workspace.row(), 17);
        assert_eq!(workspace.file.session.chart_ids(), ids);
        assert!(root.path().join("old/song.ssc").exists());
    }
    #[test]
    fn empty_views_and_focus_proofs_reject_other_session_or_later_timing_content() {
        let (_root, mut workspace, destination) = workspace();
        let session = &workspace.file.session;
        let focus = Position::at_fractional_row(session, 0, 160.5).unwrap();
        let mut empty = ActiveChartViews::empty(session, focus.music_time()).unwrap();
        let review = workspace.prepare_save_as(&destination).unwrap();
        let outcome = workspace.save_as(review).unwrap();
        let other = FileSession::open(&destination).unwrap();
        assert!(
            focus
                .rebind_relocation(&other.session, receipt(&outcome))
                .is_err()
        );
        workspace
            .file
            .session
            .set_text(None, "BPMS", "0=150")
            .unwrap();
        assert!(
            empty
                .rebind_relocation(&workspace.file.session, receipt(&outcome))
                .is_err()
        );
        assert!(
            focus
                .rebind_relocation(&workspace.file.session, receipt(&outcome))
                .is_err()
        );
    }
}
