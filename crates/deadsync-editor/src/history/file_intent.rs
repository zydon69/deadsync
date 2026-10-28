//! Deferred native File actions bind the exact source history lifetime. This
//! guard never authorizes a discard/close decision on its own.
use super::{ContentToken, Session};
use crate::{Error, storage::RelocationReceipt};
#[derive(Clone)]
pub struct IntentGuard {
    token: ContentToken,
    next: u64,
}
impl IntentGuard {
    pub fn capture(session: &Session) -> Self {
        Self {
            token: session.content_token(),
            next: session.next_revision,
        }
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.next == session.next_revision && session.accepts_content_token(&self.token)
    }
    /// Only the exact completed media publication may carry a queued intent
    /// forward. Ordinary content edits, including edit→Undo, invalidate it.
    pub fn rebind(&self, session: &Session, receipt: &RelocationReceipt) -> Result<Self, Error> {
        let token = receipt.rebind_intent_token(&self.token, self.next, session)?;
        Ok(Self {
            token,
            next: session.next_revision,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        history::media_relocation::Plan,
    };
    fn session() -> Session {
        Session::new(
            Document::parse(
                b"#VERSION:0.83;#TITLE:Initial;#MUSIC:audio.ogg;".as_slice(),
                Format::Ssc,
            )
            .unwrap(),
        )
    }
    #[test]
    fn another_session_or_edit_then_undo_cannot_reuse_a_file_intent() {
        let mut source = session();
        let intent = IntentGuard::capture(&source);
        assert!(intent.is_current(&source));
        assert!(!intent.is_current(&session()));
        source.set_text(None, "TITLE", "intervening edit").unwrap();
        source.undo().unwrap();
        assert!(!intent.is_current(&source));
    }
    #[test]
    fn only_exact_relocation_proof_carries_the_monotone_history_guard() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.ssc");
        let new = dir.path().join("new.ssc");
        let mut source = session();
        let intent = IntentGuard::capture(&source);
        let plan = Plan::prepare(&source, &old, &new).unwrap();
        let receipt = plan.rebind_receipt();
        plan.publication(&mut source).unwrap().commit();
        assert!(!intent.is_current(&source));
        let mapped = intent.rebind(&source, &receipt).unwrap();
        assert!(mapped.is_current(&source));
        source.set_text(None, "TITLE", "later edit").unwrap();
        source.undo().unwrap();
        assert!(!mapped.is_current(&source));
        assert!(intent.rebind(&source, &receipt).is_err());
    }
}
