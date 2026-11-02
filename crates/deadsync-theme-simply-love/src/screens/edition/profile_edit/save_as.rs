//! Preserve application preference Undo/Redo maps across song stamp relocation.
use super::{PreferenceChange, State};
use deadsync_editor::{Error, storage::RelocationReceipt};
use std::collections::BTreeMap;
pub(crate) struct ProfileHistoryRelocation {
    preferences: BTreeMap<u64, PreferenceChange>,
    creations: BTreeMap<u64, (super::Kind, deadsync_editor::generation::metadata::Guid)>,
}
impl State {
    pub(crate) fn save_as_profile_stamps(&self) -> Vec<u64> {
        self.profile_preference_history
            .keys()
            .chain(self.profile_creations.keys())
            .copied()
            .collect()
    }
}
impl ProfileHistoryRelocation {
    pub(crate) fn capture(state: &State, receipt: &RelocationReceipt) -> Result<Self, Error> {
        let mut preferences = BTreeMap::new();
        let mut creations = BTreeMap::new();
        for (old, change) in &state.profile_preference_history {
            let stamp = receipt.remap_revision(*old).ok_or_else(|| {
                Error::InvalidEdit("Missing profile preference relocation stamp".into())
            })?;
            preferences.insert(
                stamp,
                PreferenceChange {
                    before: change.before.clone(),
                    after: change.after.clone(),
                },
            );
        }
        for (old, value) in &state.profile_creations {
            let stamp = receipt.remap_revision(*old).ok_or_else(|| {
                Error::InvalidEdit("Missing profile creation relocation stamp".into())
            })?;
            creations.insert(stamp, value.clone());
        }
        Ok(Self {
            preferences,
            creations,
        })
    }
    pub(crate) fn publish(self, state: &mut State) {
        state.profile_preference_history = self.preferences;
        state.profile_creations = self.creations;
        // Snapshot revision/serial use their own namespace and stay unchanged.
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(revision: u64) -> super::super::PreferenceSnapshot {
        super::super::PreferenceSnapshot {
            profiles: Default::default(),
            performed_name: None,
            expressed_name: None,
            preset: if revision == 7 { 1 } else { 2 },
            revision,
        }
    }
    #[test]
    fn detached_profile_history_keys_relocate_totally_without_changing_preference_serial_namespace()
    {
        crate::i18n::init(deadsync_assets::language::load_for_tests("en"));
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.ssc");
        let destination = root.path().join("copy.ssc");
        std::fs::write(
            &source,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        )
        .unwrap();
        let mut state = State::open(&source, 0).unwrap();
        state.profile_preference_history.insert(
            6000,
            PreferenceChange {
                before: snapshot(7),
                after: snapshot(11),
            },
        );
        let guid = deadsync_editor::generation::metadata::Guid::random();
        state
            .profile_creations
            .insert(7000, (super::super::Kind::Performed, guid.clone()));
        state.profile_preference_revision = 11;
        state.profile_preference_serial = 20;
        let review = state
            .workspace
            .prepare_save_as_with_revisions(&destination, &state.save_as_profile_stamps())
            .unwrap();
        let preference_key = review.relocation().remap_revision(6000).unwrap();
        let creation_key = review.relocation().remap_revision(7000).unwrap();
        let staged = ProfileHistoryRelocation::capture(&state, review.relocation()).unwrap();
        assert_eq!(state.profile_preference_history.len(), 1);
        state.workspace.save_as(review).unwrap();
        staged.publish(&mut state);
        assert_eq!(state.profile_preference_history.len(), 1);
        assert_eq!(state.profile_creations.len(), 1);
        let change = &state.profile_preference_history[&preference_key];
        assert_eq!(change.before.revision, 7);
        assert_eq!(change.after.revision, 11);
        assert_eq!(change.before.preset, 1);
        assert_eq!(change.after.preset, 2);
        assert_eq!(state.profile_creations[&creation_key].1, guid);
        assert_eq!(state.profile_preference_revision, 11);
        assert_eq!(state.profile_preference_serial, 20);
        assert!(preference_key > 7000 && creation_key > 7000);
    }
}
