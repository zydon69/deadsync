//! A proof minted only by media-only relocation, not by arbitrary content edits.
use super::Plan;
use crate::{
    Error,
    history::{ContentToken, Session},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone)]
pub struct RelocationReceipt {
    before: ContentToken,
    after: ContentToken,
    stamps: Arc<BTreeMap<u64, u64>>,
    published:Arc<std::sync::atomic::AtomicBool>,
    policy_transition:Option<(u64,u64)>,
    before_next: u64,
    after_next: u64,
}
impl Plan {
    pub(crate) fn rebind_receipt(&self) -> RelocationReceipt {
        RelocationReceipt {
            before: ContentToken {
                identity: Arc::clone(&self.guard.identity),
                revision: self.guard.content,
            },
            after: self.relocated_content_token(),
            stamps: Arc::new(self.stamps.clone()),
            published:Arc::clone(&self.published),
            policy_transition:None,
            before_next: self.guard.next,
            after_next: self.next,
        }
    }
}
impl RelocationReceipt {
    pub(crate) fn bind_output_policy(&mut self,before:u64,after:u64){self.policy_transition=Some((before,after));}
    pub fn accepts_output_policy_transition(&self,before:u64,after:u64)->bool{self.published.load(std::sync::atomic::Ordering::Acquire)&&self.policy_transition==Some((before,after))}

    pub(crate) fn rebind_intent_token(
        &self,
        token: &ContentToken,
        before_next: u64,
        session: &Session,
    ) -> Result<ContentToken, Error> {
        if before_next != self.before_next || session.next_revision != self.after_next {
            return Err(Error::InvalidEdit("File intent history changed during relocation".into()));
        }
        self.rebind_token(token, session)
    }
    pub fn remap_revision(&self, old: u64) -> Option<u64> {
        self.stamps.get(&old).copied()
    }
    pub(crate) fn accepts_before(&self, token: &ContentToken) -> bool {
        Arc::ptr_eq(&self.before.identity, &token.identity)
            && self.before.revision == token.revision
    }
    pub(crate) fn validate_after(&self, session: &Session) -> Result<(), Error> {
        if self.published.load(std::sync::atomic::Ordering::Acquire)&&session.accepts_content_token(&self.after) {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Relocation output token is stale".into(),
            ))
        }
    }
    pub(crate) fn rebind_token(
        &self,
        token: &ContentToken,
        session: &Session,
    ) -> Result<ContentToken, Error> {
        if !self.published.load(std::sync::atomic::Ordering::Acquire)||!self.accepts_before(token) || !session.accepts_content_token(&self.after) {
            return Err(Error::InvalidEdit(
                "Relocation rebind proof is stale or belongs to another document".into(),
            ));
        }
        Ok(self.after.clone())
    }
}
impl Plan {
    pub(crate) fn include_external_revisions(&mut self, extra: &[u64]) -> Result<(), Error> {
        let mut old: std::collections::BTreeSet<u64> = self.stamps.keys().copied().collect();
        old.extend(extra.iter().copied());
        let maximum = old.iter().copied().max().unwrap_or(0).max(self.guard.next);
        let mut next = maximum
            .checked_add(1)
            .ok_or_else(|| Error::InvalidEdit("Relocation external revision overflow".into()))?;
        let mut stamps = BTreeMap::new();
        for old in old {
            stamps.insert(old, next);
            next = next.checked_add(1).ok_or_else(|| {
                Error::InvalidEdit("Relocation external revision overflow".into())
            })?;
        }
        self.stamps = stamps;
        self.next = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        history::Session,
    };
    use std::path::Path;
    #[test]
    fn recovered_dirty_sentinel_is_preserved_and_external_stale_stamps_are_mapped_totally() {
        let document=Document::parse(b"#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),Format::Ssc).unwrap();
        let mut session = Session::recovered(document);
        assert!(session.is_dirty());
        let mut plan = Plan::prepare(
            &session,
            Path::new("/old/song.ssc"),
            Path::new("/new/song.ssc"),
        )
        .unwrap();
        plan.include_external_revisions(&[10, 5000, 99]).unwrap();
        let receipt = plan.rebind_receipt();
        let mapped: Vec<_> = [0, 10, 5000, 99]
            .into_iter()
            .map(|old| receipt.remap_revision(old).unwrap())
            .collect();
        assert!(mapped.iter().all(|new| *new > 5000));
        assert_eq!(
            mapped
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
        plan.publication(&mut session).unwrap().commit();
        assert!(session.is_dirty());
        assert_eq!(session.saved_revision, u64::MAX);
        assert!(session.next_revision > *mapped.iter().max().unwrap());
    }
    #[test]
    fn external_stamp_overflow_leaves_prepared_plan_and_live_history_unchanged() {
        let mut session =
            Session::new(Document::parse(b"#MUSIC:music.ogg;".as_slice(), Format::Sm).unwrap());
        let mut plan = Plan::prepare(
            &session,
            Path::new("/old/song.sm"),
            Path::new("/new/song.sm"),
        )
        .unwrap();
        let original = plan.stamps.clone();
        assert!(plan.include_external_revisions(&[u64::MAX]).is_err());
        assert_eq!(plan.stamps, original);
        plan.publication(&mut session).unwrap().commit();
        assert_eq!(
            session.document().text(None, "MUSIC").as_deref(),
            Some("../old/music.ogg")
        );
    }
    #[test]
    fn unpublished_receipt_cannot_rebind_even_when_ordinary_edits_collide_with_future_stamps(){
        let mut session=Session::new(Document::parse(b"#VERSION:0.83;#MUSIC:music.ogg;".as_slice(),Format::Ssc).unwrap());
        let before=session.content_token();let intent=crate::history::file_intent::IntentGuard::capture(&session);
        let plan=Plan::prepare(&session,Path::new("/old/song.ssc"),Path::new("/new/song.ssc")).unwrap();let receipt=plan.rebind_receipt();
        assert!(receipt.accepts_before(&before));assert!(receipt.remap_revision(0).is_some());drop(plan);
        session.set_text(None,"TITLE","First").unwrap();session.set_text(None,"TITLE","Second").unwrap();
        assert!(session.accepts_content_token(&receipt.after),"the regression must actually collide with the planned stamp");assert_eq!(session.next_revision,receipt.after_next);
        assert!(receipt.rebind_token(&before,&session).is_err());assert!(receipt.validate_after(&session).is_err());assert!(intent.rebind(&session,&receipt).is_err());
        assert_eq!(session.document().text(None,"MUSIC").as_deref(),Some("music.ogg"));
    }

    #[test]
    fn receipt_clones_activate_only_after_the_prepared_publication_commits(){
        let mut session=Session::new(Document::parse(b"#MUSIC:music.ogg;".as_slice(),Format::Ssc).unwrap());let before=session.content_token();let plan=Plan::prepare(&session,Path::new("/old/song.ssc"),Path::new("/new/song.ssc")).unwrap();let receipt=plan.rebind_receipt();let clone=receipt.clone();
        assert!(!receipt.published.load(std::sync::atomic::Ordering::Acquire));assert!(receipt.accepts_before(&before));plan.publication(&mut session).unwrap().commit();
        assert!(clone.published.load(std::sync::atomic::Ordering::Acquire));assert!(clone.validate_after(&session).is_ok());assert!(clone.rebind_token(&before,&session).is_ok());
    }

}
