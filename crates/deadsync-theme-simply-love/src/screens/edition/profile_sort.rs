//! Keyboard adaptation of source multi-column profile table sorting.
use super::*;
use deadsync_editor::generation::profile_catalog::source_sort::{self, Column, Spec};
#[derive(Default)]
pub(super) struct Runtime {
    pub(super) specs: std::collections::BTreeMap<ProfileKind, Vec<Spec>>,
}
// Fresh native cimgui frames at GA's exact ImGui.NET gitlink select column0
// ascending for both sortable tables; Expressed is not sortable.
fn initial_specs(kind: ProfileKind) -> &'static [Spec] {
    const PATTERN: [Spec; 1] = [Spec {
        column: Column::NoteType,
        descending: false,
    }];
    const PERFORMED: [Spec; 1] = [Spec {
        column: Column::StepSpeedMin,
        descending: false,
    }];
    match kind {
        ProfileKind::Pattern => &PATTERN,
        ProfileKind::Performed => &PERFORMED,
        ProfileKind::Expressed => &[],
    }
}
impl Runtime {
    pub(super) fn effective_specs(&self, kind: ProfileKind) -> &[Spec] {
        self.specs
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or_else(|| initial_specs(kind))
    }
}

impl State {
    pub(super) fn profile_sort_key(&mut self, panel: Panel, code: KeyCode) -> bool {
        if code != KeyCode::F9 {
            return false;
        }
        let kind = match panel {
            Panel::Profiles(0) => ProfileKind::Pattern,
            Panel::Profiles(1) | Panel::ConversionProfiles(0) => ProfileKind::Performed,
            _ => return false,
        };
        let Some(settings) = self.profile_selection.as_ref() else {
            return false;
        };
        if !self.profile_catalog_current(&settings.catalog) {
            return false;
        }
        let columns = source_sort::columns(kind);
        let specs = self
            .profile_sort
            .specs
            .entry(kind)
            .or_insert_with(|| initial_specs(kind).to_vec());
        if self.control && self.shift {
            specs.pop();
        } else if self.control {
            if let Some(column) = columns
                .iter()
                .find(|column| !specs.iter().any(|spec| spec.column == **column))
            {
                specs.push(Spec {
                    column: *column,
                    descending: false,
                });
            }
        } else if self.shift {
            if let Some(spec) = specs.last_mut() {
                spec.descending = !spec.descending;
            }
        } else if specs.is_empty() {
            specs.push(Spec {
                column: columns[0],
                descending: false,
            });
        } else {
            let last = specs.len() - 1;
            let current = columns
                .iter()
                .position(|column| *column == specs[last].column)
                .unwrap_or(0);
            if let Some(column) = (1..=columns.len())
                .map(|step| columns[(current + step) % columns.len()])
                .find(|column| !specs[..last].iter().any(|spec| spec.column == *column))
            {
                specs[last].column = column;
            }
        }
        let order = if specs.is_empty() {
            i18n::tr("Edition", "ProfileSortInitial").to_string()
        } else {
            specs
                .iter()
                .map(|spec| {
                    format!(
                        "{} {}",
                        i18n::tr("Edition", column_key(spec.column)),
                        if spec.descending { "↓" } else { "↑" }
                    )
                })
                .collect::<Vec<_>>()
                .join(" > ")
        };
        // The host rebuilds culture-sensitive labels/order, then reapplies these typed
        // source specs. Existing process_profile_order preserves all selected GUIDs.
        self.profile_order_culture = None;
        self.status = i18n::tr_fmt("Edition", "ProfileSortStatus", &[("order", &order)]);
        true
    }
}
fn column_key(column: Column) -> &'static str {
    match column {
        Column::Name => "ProfileSortName",
        Column::Abbreviation => "ProfileSortAbbreviation",
        Column::NoteType => "ProfileSortNoteType",
        Column::RepetitionLimit => "ProfileSortRepetitionLimit",
        Column::StepType => "ProfileSortStepType",
        Column::StepTypeCheckPeriod => "ProfileSortStepTypeCheckPeriod",
        Column::StartingFoot => "ProfileSortStartingFoot",
        Column::StartingFooting => "ProfileSortStartingFooting",
        Column::EndingFooting => "ProfileSortEndingFooting",
        Column::StepSpeedMin => "ProfileSortStepSpeedMin",
        Column::StepDistanceMin => "ProfileSortStepDistanceMin",
        Column::StepStretchMin => "ProfileSortStepStretchMin",
        Column::LateralSpeed => "ProfileSortLateralSpeed",
        Column::LateralRelativeNps => "ProfileSortLateralRelativeNps",
        Column::LateralAbsoluteNps => "ProfileSortLateralAbsoluteNps",
        Column::TransitionMin => "ProfileSortTransitionMin",
        Column::TransitionMax => "ProfileSortTransitionMax",
        Column::FacingInwardLimit => "ProfileSortFacingInwardLimit",
        Column::FacingOutwardLimit => "ProfileSortFacingOutwardLimit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host;
    impl profile_catalog::Orderer for Host {
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
            profile_catalog::KINDS
                .into_iter()
                .map(|kind| {
                    let mut rows = catalog
                        .entries(kind)
                        .iter()
                        .map(|entry| {
                            Ok((
                                entry.guid.clone(),
                                entry.source_text(&mut |number| Ok(format!("{number:.2}")))?,
                            ))
                        })
                        .collect::<Result<Vec<_>, deadsync_editor::Error>>()?;
                    rows.sort_by(|a, b| b.0.cmp(&a.0));
                    Ok((kind, rows))
                })
                .collect()
        }
    }
    fn selected(state: &State) -> deadsync_editor::generation::metadata::Guid {
        let s = state.profile_selection.as_ref().unwrap();
        s.catalog.entries(ProfileKind::Performed)[s.performed.unwrap() - 1]
            .guid
            .clone()
    }
    #[test]
    fn real_selector_keys_sort_multiple_columns_keep_selected_guid_and_reset_to_host_order_without_history()
     {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let catalog = profile_catalog::Task::capture(&state.workspace.file().session, 0, None)
            .unwrap()
            .run(&std::sync::atomic::AtomicBool::new(false))
            .unwrap();
        state.catalog_target = ProfileTarget::Conversion;
        state.profile_selection = Some(ProfileSettings {
            catalog,
            pattern: None,
            performed: Some(2),
            expressed: Some(0),
        });
        state.panel = Some(Panel::ConversionProfiles(0));
        let bytes = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        let id = selected(&state);
        let mut host = Host;
        assert!(state.process_profile_order("en", &mut host));
        let initial = state
            .profile_selection
            .as_ref()
            .unwrap()
            .catalog
            .entries(ProfileKind::Performed)
            .iter()
            .map(|e| e.guid.clone())
            .collect::<Vec<_>>();
        let mut host_initial = initial.clone();
        host_initial.sort_by(|a, b| b.cmp(a));
        assert_eq!(selected(&state), id);
        state.panel_key(KeyCode::F9);
        assert_eq!(state.profile_sort.specs[&ProfileKind::Performed].len(), 1);
        assert!(state.process_profile_order("en", &mut host));
        assert_eq!(selected(&state), id);
        state.control = true;
        state.panel_key(KeyCode::F9);
        state.control = false;
        assert_eq!(state.profile_sort.specs[&ProfileKind::Performed].len(), 2);
        state.shift = true;
        state.panel_key(KeyCode::F9);
        state.shift = false;
        assert!(state.profile_sort.specs[&ProfileKind::Performed][1].descending);
        assert!(state.process_profile_order("fr", &mut host));
        assert_eq!(selected(&state), id);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..source_sort::PERFORMED.len() {
            state.panel_key(KeyCode::F9);
            seen.insert(state.profile_sort.specs[&ProfileKind::Performed][1].column as u8);
        }
        assert_eq!(
            seen.len(),
            source_sort::PERFORMED.len() - 1,
            "all secondary columns remain reachable without duplicate primary"
        );
        state.control = true;
        state.shift = true;
        state.panel_key(KeyCode::F9);
        state.panel_key(KeyCode::F9);
        state.control = false;
        state.shift = false;
        assert!(state.profile_sort.specs[&ProfileKind::Performed].is_empty());
        assert!(state.process_profile_order("fr", &mut host));
        assert_eq!(
            state
                .profile_selection
                .as_ref()
                .unwrap()
                .catalog
                .entries(ProfileKind::Performed)
                .iter()
                .map(|e| e.guid.clone())
                .collect::<Vec<_>>(),
            host_initial
        );
        assert_eq!(selected(&state), id);
        assert_eq!(state.profile_selection.as_ref().unwrap().expressed, Some(0));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert!(state.workspace.file().session.undo_label().is_none());
        assert!(!state.profile_sort_key(Panel::ProfileEdit(0), KeyCode::F9));
        state.workspace.execute(Command::Place(1)).unwrap();
        let changed = state.workspace.file().session.serialize().unwrap();
        assert!(!state.profile_sort_key(Panel::ConversionProfiles(0), KeyCode::F9));
        assert!(state.profile_sort.specs[&ProfileKind::Performed].is_empty());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), changed);
    }
}

#[cfg(test)]
mod default_order_tests;
