//! Main-thread culture presentation preserves selected identities across reorder.
use super::*;
impl State {
    pub fn process_profile_order(
        &mut self,
        culture: &str,
        orderer: &mut impl profile_catalog::Orderer,
    ) -> bool {
        if self.profile_order_culture.as_deref() == Some(culture)
            || self.profile_selection.is_none()
        {
            return false;
        }
        let settings = self.profile_selection.as_ref().unwrap();
        if !self.profile_catalog_current(&settings.catalog) {
            return false;
        }
        let offset = usize::from(matches!(
            self.catalog_target,
            ProfileTarget::Conversion | ProfileTarget::Batch
        ));
        let previous = [settings.pattern, settings.performed, settings.expressed];
        let ids = [
            (ProfileKind::Pattern, settings.pattern),
            (ProfileKind::Performed, settings.performed),
            (ProfileKind::Expressed, settings.expressed),
        ]
        .map(|(kind, index)| {
            index
                .and_then(|i| i.checked_sub(offset))
                .and_then(|i| settings.catalog.entries(kind).get(i))
                .map(|entry| entry.guid.clone())
        });
        let rows = orderer.order(&settings.catalog);
        let settings = self.profile_selection.as_mut().unwrap();
        match rows
            .and_then(|rows| settings.catalog.install_order(rows))
            .and_then(|()| {
                for kind in profile_catalog::KINDS {
                    settings
                        .catalog
                        .apply_source_sort(kind, self.profile_sort.effective_specs(kind))?;
                }
                Ok(())
            }) {
            Ok(()) => {
                let locate = |kind,
                              id: Option<&deadsync_editor::generation::metadata::Guid>,
                              old: Option<usize>| {
                    id.and_then(|id| {
                        settings
                            .catalog
                            .entries(kind)
                            .iter()
                            .position(|entry| &entry.guid == id)
                    })
                    .map(|i| i + offset)
                    .or_else(|| (offset == 1 && old.is_some()).then_some(0))
                };
                let pattern = locate(ProfileKind::Pattern, ids[0].as_ref(), previous[0]);
                let performed = locate(ProfileKind::Performed, ids[1].as_ref(), previous[1]);
                let expressed = locate(ProfileKind::Expressed, ids[2].as_ref(), previous[2]);
                settings.pattern = pattern;
                settings.performed = performed;
                settings.expressed = expressed;
                self.profile_order_culture = Some(culture.to_owned());
                true
            }
            Err(error) => {
                self.profile_selection = None;
                self.panel = Some(self.profile_return_panel());
                let status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
                self.cancel_autogen_expression();
                self.status = status;
                true
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Reverse {
        calls: usize,
        fail: bool,
    }
    impl profile_catalog::Orderer for Reverse {
        fn order(
            &mut self,
            catalog: &profile_catalog::Catalog,
        ) -> Result<
            Vec<(
                ProfileKind,
                Vec<(deadsync_editor::generation::metadata::Guid, String)>,
            )>,
            deadsync_editor::Error,
        > {
            self.calls += 1;
            if self.fail {
                return Err(deadsync_editor::Error::Unsupported(
                    "injected collation unavailable".into(),
                ));
            }
            Ok(profile_catalog::KINDS
                .into_iter()
                .map(|kind| {
                    (
                        kind,
                        catalog
                            .entries(kind)
                            .iter()
                            .rev()
                            .map(|entry| (entry.guid.clone(), format!("full label {}", entry.guid)))
                            .collect(),
                    )
                })
                .collect())
        }
    }
    #[test]
    fn cultural_reorder_preserves_selected_guids_default_rows_and_document_en_fr() {
        for language in ["en", "fr"] {
            crate::i18n::init_for_tests();
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("song.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#TITLE:Catalog;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            let catalog = profile_catalog::Task::capture(&state.workspace.file().session, 0, None)
                .unwrap()
                .run(&std::sync::atomic::AtomicBool::new(false))
                .unwrap();
            let chosen = catalog.entries(ProfileKind::Expressed)[1].guid.clone();
            state.catalog_target = ProfileTarget::Conversion;
            state.profile_selection = Some(ProfileSettings {
                catalog,
                pattern: None,
                performed: Some(0),
                expressed: Some(2),
            });
            let mut orderer = Reverse {
                calls: 0,
                fail: false,
            };
            assert!(state.process_profile_order(language, &mut orderer));
            let settings = state.profile_selection.as_ref().unwrap();
            assert_eq!(settings.pattern, None);
            assert_eq!(settings.performed, Some(0));
            assert_eq!(
                settings.catalog.entries(ProfileKind::Expressed)[settings.expressed.unwrap() - 1]
                    .guid,
                chosen
            );
            assert!(!state.process_profile_order(language, &mut orderer));
            assert_eq!(orderer.calls, 1);
            assert!(state.process_profile_order("another-culture", &mut orderer));
            assert_eq!(orderer.calls, 2);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            let settings = state.profile_selection.as_ref().unwrap();
            let ids = settings
                .catalog
                .entries(ProfileKind::Expressed)
                .iter()
                .map(|entry| entry.guid.clone())
                .collect::<Vec<_>>();
            let invalid = vec![(ProfileKind::Expressed, vec![])];
            assert!(
                state
                    .profile_selection
                    .as_mut()
                    .unwrap()
                    .catalog
                    .install_order(invalid)
                    .is_err()
            );
            assert_eq!(
                state
                    .profile_selection
                    .as_ref()
                    .unwrap()
                    .catalog
                    .entries(ProfileKind::Expressed)
                    .iter()
                    .map(|entry| entry.guid.clone())
                    .collect::<Vec<_>>(),
                ids
            );
            orderer.fail = true;
            assert!(state.process_profile_order("unavailable", &mut orderer));
            assert!(state.profile_selection.is_none());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        }
    }
}
