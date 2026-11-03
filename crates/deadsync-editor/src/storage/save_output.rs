//! Every ordinary publication uses a fresh policy preflight. The legacy Save
//! wrapper remains available; typed callers retain loss reviews and late-sync
//! receipts instead of mistaking namespace publication for durable completion.
use super::*;
use crate::{
    advanced_export::{Issue, Limits},
    output_policy::{OutputPolicy, PreparedOutput},
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
pub enum SaveAttempt {
    Saved,
    NeedsReview(SaveReview),
    WrittenPendingDurability {
        receipt: PendingDurability,
        error: Error,
    },
}
pub struct SaveReview {
    source: crate::file_origin::Origin,
    expected: Arc<[u8]>,
    previous_policy: OutputPolicy,
    policy_revision: u64,
    publication_revision:u64,
    output: PreparedOutput,
    maximum_bytes: usize,
}
impl SaveReview {
    pub fn issues(&self) -> &[Issue] {
        self.output.issues()
    }
    pub fn policy(&self) -> &OutputPolicy {
        self.output.policy()
    }
    pub fn is_current(&self, file: &FileSession) -> bool {
        self.source == file.origin
            && file.expected.as_ref() == Some(&self.expected)
            && self.previous_policy == *file.output_policy()
            && self.policy_revision == file.policy_revision
            && self.publication_revision == file.publication_revision
            && self.output.is_current(&file.session)
    }
}
pub(super) fn limits_for(policy: &OutputPolicy) -> Limits {
    if matches!(policy, OutputPolicy::NativeLossless) {
        Limits {
            bytes: usize::MAX,
            charts: usize::MAX,
            notes: usize::MAX,
            issues: usize::MAX,
        }
    } else {
        Limits::default()
    }
}
pub(super) fn policy_format(policy: &OutputPolicy, source: Format) -> Format {
    match policy {
        OutputPolicy::NativeLossless => source,
        OutputPolicy::Advanced { format, .. }|OutputPolicy::AdvancedChartless{format,..}|OutputPolicy::AdvancedSongTiming{format,..} => *format,
    }
}
pub(super) fn validate_extension(path: &Path, format: Format) -> Result<(), Error> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()))
    {
        return Err(Error::InvalidEdit("The output policy does not match the destination extension; use Save As to change formats".into()));
    }
    Ok(())
}
impl FileSession {
    pub fn output_policy_revision(&self) -> u64 {
        self.policy_revision
    }
    pub fn output_policy(&self) -> &OutputPolicy {
        self.session.pending_authoring_policy().unwrap_or(&self.output_policy)
    }
    pub fn prepare_save(&mut self) -> Result<SaveReview, Error> {
        self.prepare_save_with_policy(self.output_policy().clone())
    }
    pub fn prepare_save_with_policy(&mut self, policy: OutputPolicy) -> Result<SaveReview, Error> {self.prepare_save_order(policy,None)}
    pub fn prepare_save_with_policy_ordered(&mut self,policy:OutputPolicy,order:&crate::source_output_order::SourceOutputOrder)->Result<SaveReview,Error>{self.prepare_save_order(policy,Some(order))}
    fn prepare_save_order(&mut self,policy:OutputPolicy,order:Option<&crate::source_output_order::SourceOutputOrder>)->Result<SaveReview,Error>{
        self.retry_recovery_durability()?;
        self.sync_authoring_policy()?;
        validate_extension(
            self.require_existing_path()?,
            policy_format(&policy, self.session.document().format),
        )?;
        let expected = self
            .expected
            .clone()
            .ok_or_else(|| Error::InvalidEdit("A saved baseline is required".into()))?;
        let limits = limits_for(&policy);
        let output = match order{Some(order)=>policy.prepare_ordered_output(&mut self.session,limits,&AtomicBool::new(false),order)?,None=>policy.prepare_output(&mut self.session,limits,&AtomicBool::new(false))?};
        Ok(SaveReview {
            source: self.origin.clone(),
            expected,
            previous_policy: self.output_policy.clone(),
            policy_revision: self.policy_revision,
            publication_revision:self.publication_revision,
            output,
            maximum_bytes: limits.bytes,
        })
    }
    pub fn save_or_review(&mut self) -> Result<SaveAttempt, Error> {
        let review = self.prepare_save()?;
        if !review.issues().is_empty() {
            return Ok(SaveAttempt::NeedsReview(review));
        }
        self.save_reviewed(review, &BTreeSet::new())
    }
    pub fn save_reviewed(
        &mut self,
        review: SaveReview,
        acknowledged: &BTreeSet<usize>,
    ) -> Result<SaveAttempt, Error> {
        self.save_reviewed_with_sync(review, acknowledged, save_as::sync_output_directory)
    }
    fn save_reviewed_with_sync(
        &mut self,
        review: SaveReview,
        acknowledged: &BTreeSet<usize>,
        sync: impl FnOnce(&Path) -> Result<(), Error>,
    ) -> Result<SaveAttempt, Error> {
        self.retry_recovery_durability()?;
        if !review.is_current(self) {
            return Err(Error::InvalidEdit("Save review became stale".into()));
        }
        let output = review.output.generate(
            &self.session,
            acknowledged,
            review.maximum_bytes,
            &AtomicBool::new(false),
        )?;
        let path = self.require_existing_path()?.to_owned();
        validate_extension(&path, output.format())?;
        let policy = output.policy().clone();
        let next_policy = self.next_policy_revision(&policy)?;
        let next_publication=self.next_publication_revision()?;
        let expected: Arc<[u8]> = output.bytes().to_vec().into();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.sidecar_path(".deadsync-save-lock")?)?;
        lock.try_lock().map_err(std::io::Error::other)?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let parent = path
            .parent()
            .ok_or_else(|| Error::InvalidEdit("Simfile has no parent directory".into()))?;
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending
            .as_file()
            .set_permissions(fs::metadata(&path)?.permissions())?;
        pending.write_all(&expected)?;
        pending.as_file().sync_all()?;
        let receipt = PendingDurability::for_output(
            &path,
            parent,
            Arc::clone(&expected),
            pending.as_file(),
            self.session.content_token(),
            policy.clone(),
            next_policy,
            next_publication,
        )?;
        let witness=receipt.prepare_open_witness(self,output.format());
        if !review.is_current(self){return Err(Error::InvalidEdit("Save review became stale".into()));}
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        save_as::persist_output(pending, &path)?;
        self.expected = Some(expected);
        self.session.rebase_authoring_policy_history(&policy, output.format());
        self.output_policy = policy;
        self.policy_revision = next_policy;
        self.publication_revision=next_publication;
        self.publication_epoch
            .store(next_publication, std::sync::atomic::Ordering::Release);
        witness.activate(&self.session);
        self.publication_witness=Some(witness);
        match sync(parent) {
            Ok(()) => {
                self.session.mark_saved();
                Ok(SaveAttempt::Saved)
            }
            Err(error) => {
                self.session.invalidate_saved_position();
                Ok(SaveAttempt::WrittenPendingDurability { receipt, error })
            }
        }
    }
    pub(super) fn next_publication_revision(&self)->Result<u64,Error>{self.publication_revision.checked_add(1).ok_or_else(||Error::InvalidEdit("Publication revision overflow".into()))}
    pub(super) fn next_policy_revision(&self, policy: &OutputPolicy) -> Result<u64, Error> {
        if &self.output_policy == policy {
            Ok(self.policy_revision)
        } else {
            self.policy_revision
                .checked_add(1)
                .ok_or_else(|| Error::InvalidEdit("Output policy revision overflow".into()))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        fs::write(&path,"#VERSION:0.83;#TITLE:Policy;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let file = FileSession::open(&path).unwrap();
        (root, file)
    }
    fn advanced(file: &FileSession) -> OutputPolicy {
        OutputPolicy::Advanced {
            format: Format::Ssc,
            options: Default::default(),
            timing_chart: file.session.chart_ids()[0].clone(),
        }
    }
    fn ack(review: &SaveReview) -> BTreeSet<usize> {
        assert!(
            !review
                .issues()
                .iter()
                .any(|issue| issue.severity == crate::advanced_export::Severity::Error)
        );
        review
            .issues()
            .iter()
            .enumerate()
            .filter(|(_, issue)| issue.severity == crate::advanced_export::Severity::Warning)
            .map(|(index, _)| index)
            .collect()
    }
    #[test]
    fn late_directory_failure_keeps_published_policy_and_exact_retry_without_rewriting() {
        let (_root, mut file) = file();
        let selected = advanced(&file);
        let review = file.prepare_save_with_policy(selected.clone()).unwrap();
        let acknowledged = ack(&review);
        let receipt = match file
            .save_reviewed_with_sync(review, &acknowledged, |_| {
                Err(Error::Io(std::io::Error::other("injected sync failure")))
            })
            .unwrap()
        {
            SaveAttempt::WrittenPendingDurability { receipt, .. } => receipt,
            _ => panic!("expected pending output"),
        };
        assert_eq!(file.output_policy(), &selected);
        assert!(file.session.is_dirty());
        let bytes = fs::read(file.require_existing_path().unwrap()).unwrap();
        receipt.retry(&mut file).unwrap();
        assert!(!file.session.is_dirty());
        assert_eq!(
            fs::read(file.require_existing_path().unwrap()).unwrap(),
            bytes
        );
    }
    #[test]
    fn edits_after_pending_output_stay_dirty_and_cannot_reuse_receipt() {
        let (_root, mut file) = file();
        let review = file.prepare_save().unwrap();
        let receipt = match file
            .save_reviewed_with_sync(review, &BTreeSet::new(), |_| {
                Err(Error::Io(std::io::Error::other("injected sync failure")))
            })
            .unwrap()
        {
            SaveAttempt::WrittenPendingDurability { receipt, .. } => receipt,
            _ => panic!("expected pending output"),
        };
        file.session.set_text(None, "TITLE", "Later edit").unwrap();
        assert!(receipt.retry(&mut file).is_err());
        assert!(file.session.is_dirty());
    }
    #[test]
    fn policy_revision_overflow_is_detected_before_creating_any_output_or_lock() {
        let (root, mut file) = file();
        let before = fs::read(file.require_existing_path().unwrap()).unwrap();
        file.policy_revision = u64::MAX;
        let selected = advanced(&file);
        let review = file.prepare_save_with_policy(selected).unwrap();
        let acknowledged = ack(&review);
        assert!(file.save_reviewed(review, &acknowledged).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(file.require_existing_path().unwrap()).unwrap(),
            before
        );
        assert!(matches!(file.output_policy(), OutputPolicy::NativeLossless));
    }
    #[test]
    fn policy_epoch_prevents_old_recovery_after_byte_identical_policy_change() {
        let (_root, mut file) = file();
        let selected = advanced(&file);
        let review = file.prepare_save_with_policy(selected).unwrap();
        let acknowledged = ack(&review);
        assert!(matches!(
            file.save_reviewed(review, &acknowledged).unwrap(),
            SaveAttempt::Saved
        ));
        let snapshot = file.recovery_snapshot().unwrap().unwrap();
        let before = fs::read(file.require_existing_path().unwrap()).unwrap();
        let mut selected = advanced(&file);
        if let OutputPolicy::Advanced { options, .. } = &mut selected {
            options.stepf2_pump_multiplayer = true;
        }
        let review = file.prepare_save_with_policy(selected).unwrap();
        let acknowledged = ack(&review);
        assert!(matches!(
            file.save_reviewed(review, &acknowledged).unwrap(),
            SaveAttempt::Saved
        ));
        assert_eq!(
            fs::read(file.require_existing_path().unwrap()).unwrap(),
            before
        );
        assert!(matches!(snapshot.write(true), Err(Error::ExternalChange)));
        assert!(!file.recovery_path().unwrap().exists());
    }
    #[test]
    fn byte_identical_publication_with_same_policy_revokes_older_native_recovery() {
        let (_root,mut file)=file();
        file.session.set_text(None,"CREDIT","First private author").unwrap();
        let mut policy=advanced(&file);if let OutputPolicy::Advanced{options,..}=&mut policy{options.anonymize=true;}
        let review=file.prepare_save_with_policy(policy).unwrap();let acknowledged=ack(&review);file.save_reviewed(review,&acknowledged).unwrap();
        let snapshot=file.recovery_snapshot().unwrap().unwrap();let baseline=fs::read(file.require_existing_path().unwrap()).unwrap();let policy_revision=file.output_policy_revision();
        file.session.set_text(None,"CREDIT","Second private author").unwrap();let review=file.prepare_save().unwrap();let acknowledged=ack(&review);file.save_reviewed(review,&acknowledged).unwrap();
        assert_eq!(file.output_policy_revision(),policy_revision);assert_eq!(fs::read(file.require_existing_path().unwrap()).unwrap(),baseline);
        assert!(matches!(snapshot.write(true),Err(Error::ExternalChange)));assert!(!file.recovery_path().unwrap().exists());
    }
    #[test]
    fn same_policy_save_as_revokes_snapshot_of_unchanged_old_source() {
        let(root,mut file)=file();let snapshot=file.recovery_snapshot().unwrap().unwrap();let old=file.require_existing_path().unwrap().to_owned();let baseline=fs::read(&old).unwrap();let revision=file.output_policy_revision();
        let destination=root.path().join("relocated.ssc");let review=file.prepare_save_as(&destination).unwrap();file.save_as(review).unwrap();assert_eq!(file.output_policy_revision(),revision);assert_eq!(fs::read(old).unwrap(),baseline);
        assert!(matches!(snapshot.write(true),Err(Error::ExternalChange)));
    }
    #[test]
    fn recover_revokes_pre_recovery_snapshot_even_under_same_policy_and_baseline() {
        let(_root,mut file)=file();file.session.set_text(None,"TITLE","Recovered native draft").unwrap();file.save_recovery().unwrap();file.session.undo().unwrap();assert!(!file.session.is_dirty());
        let old=file.recovery_snapshot().unwrap().unwrap();let revision=file.output_policy_revision();let recovery=fs::read(file.recovery_path().unwrap()).unwrap();file.recover().unwrap();assert_eq!(file.output_policy_revision(),revision);
        assert!(matches!(old.write(true),Err(Error::ExternalChange)));assert_eq!(fs::read(file.recovery_path().unwrap()).unwrap(),recovery);assert_eq!(file.session.document().text(None,"TITLE").as_deref(),Some("Recovered native draft"));
    }

    #[test]
    fn pending_publication_revokes_old_snapshot_and_retry_preserves_its_epoch() {
        let(_root,mut file)=file();let snapshot=file.recovery_snapshot().unwrap().unwrap();let before=file.publication_revision;let review=file.prepare_save().unwrap();
        let receipt=match file.save_reviewed_with_sync(review,&BTreeSet::new(),|_|Err(Error::Io(std::io::Error::other("injected sync failure")))).unwrap(){SaveAttempt::WrittenPendingDurability{receipt,..}=>receipt,_=>panic!("expected pending output")};
        assert_eq!(file.publication_revision,before+1);assert!(matches!(snapshot.write(true),Err(Error::ExternalChange)));let published=file.publication_revision;receipt.retry(&mut file).unwrap();assert_eq!(file.publication_revision,published);assert!(!file.session.is_dirty());
    }
    #[test]
    fn publication_epoch_overflow_refuses_save_and_save_as_before_output_or_lock() {
        let(root,mut file)=file();let before=fs::read(file.require_existing_path().unwrap()).unwrap();file.publication_revision=u64::MAX;let review=file.prepare_save().unwrap();assert!(file.save_reviewed(review,&BTreeSet::new()).is_err());let destination=root.path().join("copy.ssc");let review=file.prepare_save_as(&destination).unwrap();assert!(file.save_as(review).is_err());assert!(!destination.exists());assert_eq!(fs::read_dir(root.path()).unwrap().count(),1);assert_eq!(fs::read(file.require_existing_path().unwrap()).unwrap(),before);assert_eq!(file.publication_revision,u64::MAX);
    }

    #[test]
    #[cfg(unix)]
    fn published_open_witness_waits_for_real_pending_output_durability_retry(){
        let root=tempfile::tempdir().unwrap();let path=root.path().join("pending.ssc");std::fs::write(&path,b"#VERSION:0.83;#TITLE:Before;").unwrap();let mut file=FileSession::open(&path).unwrap();file.session.set_text(None,"TITLE","Written pending").unwrap();let review=file.prepare_save().unwrap();
        let attempt=file.save_reviewed_with_sync(review,&BTreeSet::new(),|_|Err(std::io::Error::other("injected late directory sync failure").into())).unwrap();
        let SaveAttempt::WrittenPendingDurability{receipt,..}=attempt else{panic!("real namespace output must remain pending")};assert!(file.published_open_witness().is_err());assert!(std::fs::read_to_string(&path).unwrap().contains("Written pending"));
        receipt.retry(&mut file).unwrap();let witness=file.published_open_witness().unwrap();let opened=witness.open_bounded(1024*1024,&AtomicBool::new(false)).unwrap();assert_eq!(opened.session.document().text(None,"TITLE").as_deref(),Some("Written pending"));
    }

}
