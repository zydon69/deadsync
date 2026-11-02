//! Main and dedicated-preview source caches are independent from active transport.
use crate::editor_asset_watch::{AssetRegistration, EditorAssetKind};
use deadlib_audio_decode::prepared::PreparedAudioSource;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetLoadToken {
    pub registration: AssetRegistration,
    pub revision: u64,
}
/// Owner/load identity and the exact immutable PCM currently published for it.
/// `None` PCM is an accepted reset/loading/failure, never an old cache fallback.
#[derive(Clone)]
pub struct PreparedAssetSnapshot {
    pub token: AssetLoadToken,
    pub source: Option<Arc<PreparedAudioSource>>,
}
#[derive(Default)]
struct Slot {
    registration: Option<AssetRegistration>,
    revision: u64,
    source: Option<Arc<PreparedAudioSource>>,
}
#[derive(Default)]
pub struct EditorAssetSlots {
    main: Slot,
    preview: Slot,
}
impl EditorAssetSlots {
    fn slot(&self, kind: EditorAssetKind) -> &Slot {
        match kind {
            EditorAssetKind::Main => &self.main,
            EditorAssetKind::DedicatedPreview => &self.preview,
        }
    }
    fn slot_mut(&mut self, kind: EditorAssetKind) -> &mut Slot {
        match kind {
            EditorAssetKind::Main => &mut self.main,
            EditorAssetKind::DedicatedPreview => &mut self.preview,
        }
    }
    pub fn register(&mut self, token: AssetRegistration) {
        let slot = self.slot_mut(token.kind);
        if slot.registration != Some(token) {
            slot.registration = Some(token);
            slot.source = None;
            slot.revision = 0;
        }
    }
    /// Reset cache before asynchronous load, matching EditorSound.Reset. Does
    /// not mutate active PCM epochs, musical clocks, output leases or gains.
    pub fn begin_load(&mut self, registration: AssetRegistration) -> Option<AssetLoadToken> {
        let slot = self.slot_mut(registration.kind);
        if slot.registration != Some(registration) {
            return None;
        }
        slot.revision = slot
            .revision
            .checked_add(1)
            .expect("Audio asset load revision exhausted");
        slot.source = None;
        Some(AssetLoadToken {
            registration,
            revision: slot.revision,
        })
    }
    pub fn current_load_token(&self, kind: EditorAssetKind) -> Option<AssetLoadToken> {
        let slot = self.slot(kind);
        if slot.revision == 0 {
            return None;
        }
        slot.registration.map(|registration| AssetLoadToken {
            registration,
            revision: slot.revision,
        })
    }
    pub fn is_current(&self, token: AssetLoadToken) -> bool {
        let slot = self.slot(token.registration.kind);
        slot.registration == Some(token.registration) && slot.revision == token.revision
    }
    pub fn publish(&mut self, token: AssetLoadToken, source: Arc<PreparedAudioSource>) -> bool {
        if !self.is_current(token) {
            return false;
        }
        self.slot_mut(token.registration.kind).source = Some(source);
        true
    }
    pub fn source(&self, kind: EditorAssetKind) -> Option<&Arc<PreparedAudioSource>> {
        self.slot(kind).source.as_ref()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_revision_and_registration_reject_aba_and_cleared_owner() {
        let mut slots = EditorAssetSlots::default();
        let a = AssetRegistration {
            owner: 1,
            kind: EditorAssetKind::Main,
            registration: 1,
        };
        slots.register(a);
        let old = slots.begin_load(a).unwrap();
        let newer = slots.begin_load(a).unwrap();
        assert!(!slots.is_current(old));
        assert!(slots.is_current(newer));
        let b = AssetRegistration {
            registration: 2,
            ..a
        };
        slots.register(b);
        assert!(!slots.is_current(newer));
        slots.register(AssetRegistration {
            owner: 2,
            registration: 3,
            ..a
        });
        assert!(slots.begin_load(a).is_none());
        slots.clear();
        assert!(!slots.is_current(newer));
    }
    #[test]
    fn inactive_slot_load_does_not_change_other_source_revision() {
        let mut slots = EditorAssetSlots::default();
        let a = AssetRegistration {
            owner: 1,
            kind: EditorAssetKind::Main,
            registration: 1,
        };
        let b = AssetRegistration {
            kind: EditorAssetKind::DedicatedPreview,
            registration: 2,
            ..a
        };
        slots.register(a);
        slots.register(b);
        let main = slots.begin_load(a).unwrap();
        let old_preview = slots.begin_load(b).unwrap();
        let preview = slots.begin_load(b).unwrap();
        assert!(slots.is_current(main));
        assert!(!slots.is_current(old_preview));
        assert!(slots.is_current(preview));
    }
}
