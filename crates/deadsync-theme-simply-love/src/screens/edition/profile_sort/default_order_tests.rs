use super::*;
use deadsync_editor::generation::profiles::Config;
use std::time::{Duration, Instant};
struct AlphabeticHost;
impl profile_catalog::Orderer for AlphabeticHost {
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
                            entry.builtin.is_none(),
                            entry.guid.clone(),
                            entry.source_text(&mut |value| Ok(format!("{value:.2}")))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, deadsync_editor::Error>>()?;
                rows.sort_by(|a, b| (&a.0, &a.2, &a.1).cmp(&(&b.0, &b.2, &b.1)));
                Ok((
                    kind,
                    rows.into_iter().map(|(_, id, text)| (id, text)).collect(),
                ))
            })
            .collect()
    }
}
fn id(kind: ProfileKind, rank: u8) -> String {
    format!(
        "{}1111111-2222-4333-8444-00000000000{rank}",
        match kind {
            ProfileKind::Pattern => 1,
            ProfileKind::Performed => 2,
            ProfileKind::Expressed => 3,
        }
    )
}
fn names_ids(state: &State, kind: ProfileKind) -> Vec<String> {
    state
        .profile_selection
        .as_ref()
        .unwrap()
        .catalog
        .entries(kind)
        .iter()
        .filter(|e| e.name.is_some())
        .map(|e| e.guid.to_string())
        .collect()
}
fn selected(state: &State, kind: ProfileKind) -> String {
    let settings = state.profile_selection.as_ref().unwrap();
    let index = match kind {
        ProfileKind::Pattern => settings.pattern,
        ProfileKind::Performed => settings.performed,
        ProfileKind::Expressed => settings.expressed,
    }
    .unwrap();
    settings.catalog.entries(kind)[index].guid.to_string()
}
fn poll(state: &mut State) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.catalog_job.is_some() {
        state.poll_profile_catalog();
        assert!(Instant::now() < deadline, "{}", state.status);
        std::thread::yield_now();
    }
    assert!(state.profile_selection.is_some(), "{}", state.status);
}
fn select_second(state: &mut State) {
    let settings = state.profile_selection.as_mut().unwrap();
    for kind in profile_catalog::KINDS {
        let index = settings
            .catalog
            .entries(kind)
            .iter()
            .position(|e| e.guid.to_string() == id(kind, 2))
            .unwrap();
        match kind {
            ProfileKind::Pattern => settings.pattern = Some(index),
            ProfileKind::Performed => settings.performed = Some(index),
            ProfileKind::Expressed => settings.expressed = Some(index),
        }
    }
}
#[test]
fn original_fresh_table_specs_are_column_zero_ascending_and_expressed_is_not_sortable() {
    let observed: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../deadsync-editor/tests/fixtures/profile-comparers/first-table-sort.json"
    )))
    .unwrap();
    for (kind, name, column) in [
        (ProfileKind::Pattern, "Pattern", Column::NoteType),
        (ProfileKind::Performed, "Performed", Column::StepSpeedMin),
    ] {
        let table = observed["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["kind"] == name)
            .unwrap();
        for frame in table["frames"].as_array().unwrap() {
            let specs = frame["specs"].as_array().unwrap();
            assert_eq!(specs.len(), 1);
            assert_eq!(specs[0]["column_user_id"], 0);
            assert_eq!(specs[0]["sort_direction"], 1);
        }
        assert_eq!(
            Runtime::default().effective_specs(kind),
            &[Spec {
                column,
                descending: false
            }]
        );
    }
    assert!(
        Runtime::default()
            .effective_specs(ProfileKind::Expressed)
            .is_empty()
    );
}
#[test]
fn actual_catalog_initial_source_order_overrides_names_keeps_guid_ties_and_explicit_specs_across_culture_reopen_en_fr()
 {
    for language in ["en", "fr"] {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("configs");
        std::fs::create_dir(&directory).unwrap();
        for kind in profile_catalog::KINDS {
            for (rank, name) in [(1, "Z low"), (2, "B low"), (3, "A high")] {
                let mut config = Config::editor_defaults(kind);
                match &mut config {
                    Config::Pattern(c) => c.beat_sub_division = if rank == 3 { 4 } else { 2 },
                    Config::Performed(c) => {
                        c.step_tightening.speed_tightening_enabled = Some(rank == 3)
                    }
                    Config::Expressed(_) => {}
                }
                let raw = serde_json::json!({"Guid":id(kind,rank),"Name":name,"TravelSpeedMinBPM":125,"TravelSpeedMaxBPM":250,"Config":config.json().unwrap()});
                std::fs::write(
                    directory.join(format!("{}{}.json", kind.prefix(), id(kind, rank))),
                    serde_json::to_vec(&raw).unwrap(),
                )
                .unwrap();
            }
        }
        let mut state = State::without_song(root.path()).unwrap();
        state.profile_directory = Some(directory.clone());
        let source = state.workspace.file().session.serialize().unwrap();
        state.open_profile_manager();
        poll(&mut state);
        select_second(&mut state);
        let mut host = AlphabeticHost;
        assert!(state.process_profile_order(language, &mut host));
        for kind in [ProfileKind::Pattern, ProfileKind::Performed] {
            assert_eq!(
                names_ids(&state, kind),
                vec![id(kind, 1), id(kind, 2), id(kind, 3)],
                "initial source numeric/enable column then GUID, not name"
            );
            assert_eq!(selected(&state, kind), id(kind, 2));
        }
        assert_eq!(
            names_ids(&state, ProfileKind::Expressed),
            vec![
                id(ProfileKind::Expressed, 3),
                id(ProfileKind::Expressed, 2),
                id(ProfileKind::Expressed, 1)
            ],
            "nonsortable expressed retains host cultural order"
        );
        // Actual F9 panel consumer starts from source's implicit first spec, rather
        // than requiring a first no-op key just to install the already-visible sort.
        state.panel = Some(Panel::Profiles(0));
        state.panel_key(KeyCode::F9);
        assert_eq!(
            state.profile_sort.specs[&ProfileKind::Pattern][0].column,
            Column::RepetitionLimit
        );
        state.shift = true;
        state.panel_key(KeyCode::F9);
        state.shift = false;
        assert!(state.profile_sort.specs[&ProfileKind::Pattern][0].descending);
        let explicit = state.profile_sort.specs[&ProfileKind::Pattern].clone();
        assert!(state.process_profile_order("fr-CA", &mut host));
        assert_eq!(state.profile_sort.specs[&ProfileKind::Pattern], explicit);
        assert_eq!(
            selected(&state, ProfileKind::Pattern),
            id(ProfileKind::Pattern, 2)
        );
        state.open_profile_manager();
        poll(&mut state);
        select_second(&mut state);
        assert!(state.process_profile_order(language, &mut host));
        assert_eq!(state.profile_sort.specs[&ProfileKind::Pattern], explicit);
        assert_eq!(
            selected(&state, ProfileKind::Pattern),
            id(ProfileKind::Pattern, 2)
        );
        assert_eq!(
            names_ids(&state, ProfileKind::Performed),
            vec![
                id(ProfileKind::Performed, 1),
                id(ProfileKind::Performed, 2),
                id(ProfileKind::Performed, 3)
            ],
            "unmodified family keeps its own implicit default after reopen"
        );
        // Existing explicit host-reset remains an opt-in keyboard extension, not
        // the initial source table policy. It also survives culture changes.
        state
            .profile_sort
            .specs
            .insert(ProfileKind::Pattern, vec![]);
        state.profile_order_culture = None;
        assert!(state.process_profile_order("en-CA", &mut host));
        assert_eq!(
            names_ids(&state, ProfileKind::Pattern),
            vec![
                id(ProfileKind::Pattern, 3),
                id(ProfileKind::Pattern, 2),
                id(ProfileKind::Pattern, 1)
            ]
        );
        assert_eq!(state.workspace.file().session.serialize().unwrap(), source);
        assert!(state.workspace.file().session.undo_label().is_none());
        assert!(!state.has_song());
    }
}
