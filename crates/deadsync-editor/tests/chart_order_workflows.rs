use deadsync_editor::{
    chart_order::{Key, Snapshot, TYPE_ORDER, compare_with},
    document::{Document, Format},
    generation::metadata::Guid,
    history::Session,
    notes::{Note, Position},
};
use std::{cmp::Ordering, sync::Arc};
fn guid(index: u32) -> Guid {
    format!("{index:08x}-0000-0000-0000-000000000000")
        .parse()
        .unwrap()
}
fn session(format: Format) -> Session {
    let mut source = String::from("#VERSION:0.83;#TITLE:Ordering;#BPMS:0=120;#VENDOR:keep;\n");
    for (kind, difficulty, meter, description, columns) in [
        ("dance-double", "Easy", 1, "A", 8),
        ("dance-single", "Hard", 9, "A", 4),
        ("dance-single", "Easy", 2, "A", 4),
        ("dance-couple", "Beginner", 1, "A", 8),
    ] {
        let notes = format!(
            "1{}\n{}\n{}\n{}",
            "0".repeat(columns - 1),
            "0".repeat(columns),
            "0".repeat(columns),
            "0".repeat(columns)
        );
        match format {
            Format::Sm => {
                source += &format!(
                    "#NOTES:{kind}:{description}:{difficulty}:{meter}:0,0,0,0,0:\n{notes};\n"
                )
            }
            Format::Ssc => {
                source += &format!(
                    "#NOTEDATA:;#STEPSTYPE:{kind};#DESCRIPTION:{description};#DIFFICULTY:{difficulty};#METER:{meter};#NOTES:\n{notes};\n"
                )
            }
        }
    }
    Session::new(Document::parse(Arc::<[u8]>::from(source.into_bytes()), format).unwrap())
}
#[test]
fn presentation_tracks_live_properties_undo_redo_and_preserves_original_sections() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = session(format);
        let identities: Vec<_> = (1..=4).map(guid).collect();
        let original = session.serialize().unwrap();
        let snapshot = Snapshot::read(&mut session, &identities).unwrap();
        assert_eq!(snapshot.indices_with(str::cmp).unwrap(), [2, 1, 0, 3]);
        assert_eq!(session.serialize().unwrap(), original);
        assert!(!session.is_dirty());
        let mut draft = deadsync_editor::chart_properties::Draft::capture(&session, 1).unwrap();
        draft.set_difficulty(deadsync_editor::chart_identity::Difficulty::Beginner);
        draft.apply(&mut session, 1).unwrap();
        assert_eq!(
            Snapshot::read(&mut session, &identities)
                .unwrap()
                .indices_with(str::cmp)
                .unwrap(),
            [1, 2, 0, 3]
        );
        let changed = session.serialize().unwrap();
        // The source document still starts with dance-double; only presentation moves.
        assert_eq!(session.document().charts()[0].steps_type, "dance-double");
        assert!(String::from_utf8_lossy(&changed).contains("#VENDOR:keep;"));
        session.undo().unwrap();
        assert_eq!(
            Snapshot::read(&mut session, &identities)
                .unwrap()
                .indices_with(str::cmp)
                .unwrap(),
            [2, 1, 0, 3]
        );
        assert_eq!(session.serialize().unwrap(), original);
        session.redo().unwrap();
        assert_eq!(
            Snapshot::read(&mut session, &identities)
                .unwrap()
                .indices_with(str::cmp)
                .unwrap(),
            [1, 2, 0, 3]
        );
        assert_eq!(session.serialize().unwrap(), changed);
    }
}
#[test]
fn live_note_counts_break_ties_before_resident_guids_without_serialization() {
    let mut source = String::from("#BPMS:0=120;");
    for _ in 0..2 {
        source += "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0000\n0000\n0000;";
    }
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.into_bytes()), Format::Ssc).unwrap());
    let identities = [guid(2), guid(1)];
    let original = session.serialize().unwrap();
    assert_eq!(
        Snapshot::read(&mut session, &identities)
            .unwrap()
            .indices_with(str::cmp)
            .unwrap(),
        [1, 0]
    );
    session
        .edit_notes(
            1,
            "More steps",
            [(
                Position {
                    player: 0,
                    lane: 1,
                    row: 48,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let snapshot = Snapshot::read(&mut session, &identities).unwrap();
    assert_eq!(
        snapshot
            .keys()
            .iter()
            .map(|key| key.step_count)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(snapshot.indices_with(str::cmp).unwrap(), [0, 1]);
    session.undo().unwrap();
    assert_eq!(
        Snapshot::read(&mut session, &identities)
            .unwrap()
            .indices_with(str::cmp)
            .unwrap(),
        [1, 0]
    );
    assert_eq!(session.serialize().unwrap(), original);
}
#[test]
fn fake_and_warped_heads_are_excluded_and_mismatched_identity_maps_rejected() {
    let source=b"#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#FAKES:1=1;#WARPS:2=1;#NOTES:1000\n0100\n0010\n0001;";
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    assert!(Snapshot::read(&mut session, &[]).is_err());
    let snapshot = Snapshot::read(&mut session, &[guid(1)]).unwrap();
    assert_eq!(snapshot.keys()[0].step_count, 2);
    assert!(!session.is_dirty());
    assert_eq!(session.serialize().unwrap(), source);
}
#[test]
fn current_culture_reference_comparator_matches_all_pairs_and_tie_breaks() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/chart-order.reference.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["provenance"]["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let keys: Vec<Option<Key>> = fixture["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            if entry.is_null() {
                return None;
            }
            Some(Key {
                steps_type: entry["steps_type"].as_str().unwrap().into(),
                difficulty: entry["difficulty"].as_i64().unwrap() as i32,
                rating: entry["rating"].as_i64().unwrap() as i32,
                name: entry["name"].as_str().unwrap().into(),
                description: entry["description"].as_str().unwrap().into(),
                step_count: entry["step_count"].as_u64().unwrap() as usize,
                guid: entry["guid"].as_str().unwrap().parse().unwrap(),
            })
        })
        .collect();
    let strings: Vec<_> = fixture["strings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert!(keys.len() >= 50);
    assert_eq!(fixture["cultures"].as_array().unwrap().len(), 3);
    for culture in fixture["cultures"].as_array().unwrap() {
        let collate = |a: &str, b: &str| {
            let a = strings.iter().position(|value| *value == a).unwrap();
            let b = strings.iter().position(|value| *value == b).unwrap();
            culture["text_comparisons"][a][b].as_i64().unwrap().cmp(&0)
        };
        for (a, left) in keys.iter().enumerate() {
            for (b, right) in keys.iter().enumerate() {
                let expected = culture["comparisons"][a][b].as_i64().unwrap().cmp(&0);
                assert_eq!(
                    compare_with(left.as_ref(), right.as_ref(), collate),
                    expected,
                    "culture {} pair {a}/{b}",
                    culture["culture"]
                );
            }
        }
    }
    assert_eq!(TYPE_ORDER.len(), 16);
    assert_eq!(
        compare_with(None, None, |_, _| panic!("null charts do not collate")),
        Ordering::Equal
    );
}

#[test]
fn overflowing_imported_ordering_keys_are_rejected_before_sorting() {
    // MIN/0 compare Less in both directions in the original unchecked
    // subtraction. MIN/0/MAX additionally violates transitivity. Keep the exact
    // low-level comparison for differential proof; never feed it to Rust sort.
    for (field, values) in [
        ("METER", vec![i32::MIN, 0]),
        ("METER", vec![i32::MIN, 0, i32::MAX]),
        ("DIFFICULTY", vec![i32::MIN, 0, i32::MAX]),
    ] {
        let mut source = String::from("#BPMS:0=120;");
        for value in &values {
            let difficulty = if field == "DIFFICULTY" {
                value.to_string()
            } else {
                "Hard".into()
            };
            let rating = if field == "METER" {
                value.to_string()
            } else {
                "9".into()
            };
            source += &format!(
                "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:{difficulty};#METER:{rating};#NOTES:1000;"
            );
        }
        let mut session = Session::new(
            Document::parse(Arc::<[u8]>::from(source.into_bytes()), Format::Ssc).unwrap(),
        );
        let identities: Vec<_> = (1..=values.len() as u32).map(guid).collect();
        let before = session.serialize().unwrap();
        let snapshot = Snapshot::read(&mut session, &identities).unwrap();
        assert_eq!(
            compare_with(
                Some(&snapshot.keys()[0]),
                Some(&snapshot.keys()[1]),
                str::cmp
            ),
            Ordering::Less
        );
        assert_eq!(
            compare_with(
                Some(&snapshot.keys()[1]),
                Some(&snapshot.keys()[0]),
                str::cmp
            ),
            Ordering::Less
        );
        assert!(snapshot.indices_with(str::cmp).is_err());
        assert_eq!(session.serialize().unwrap(), before);
        assert!(!session.is_dirty());
    }
}

#[test]
fn workspace_orders_live_properties_and_preserves_resident_ids_across_structural_history() {
    use deadsync_editor::workspace::{Command, Workspace};
    for format in [Format::Sm, Format::Ssc] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("song.{}", format.extension()));
        let initial = session(format).serialize().unwrap();
        std::fs::write(&path, &initial).unwrap();
        let mut workspace = Workspace::open(&path, 1).unwrap();
        let original_ids = workspace.file().session.chart_ids().to_vec();
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), [2, 1, 0, 3]);
        assert_eq!(workspace.chart(), 1);
        assert!(!workspace.file().session.is_dirty());
        let mut draft = workspace.chart_properties().unwrap();
        draft.set_difficulty(deadsync_editor::chart_identity::Difficulty::Beginner);
        workspace.apply_chart_properties(&draft).unwrap();
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), [1, 2, 0, 3]);
        assert_eq!(workspace.file().session.chart_ids(), original_ids);
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), [2, 1, 0, 3]);
        workspace.execute(Command::Redo).unwrap();
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), [1, 2, 0, 3]);
        let duplicated = workspace.duplicate_chart().unwrap();
        assert_eq!(duplicated, 4);
        let with_duplicate = workspace.file().session.chart_ids().to_vec();
        assert_eq!(&with_duplicate[..4], original_ids);
        assert!(!original_ids.contains(&with_duplicate[4]));
        let duplicate_order = workspace.ordered_charts(str::cmp).unwrap();
        let expected_first = if with_duplicate[1] < with_duplicate[4] {
            vec![1, 4]
        } else {
            vec![4, 1]
        };
        assert_eq!(&duplicate_order[..2], expected_first);
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), original_ids);
        workspace.execute(Command::Redo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), with_duplicate);
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), duplicate_order);
        // Delete the middle source chart and keep identities attached to charts,
        // not numeric positions. Undo/redo restore the duplicate's exact GUID.
        workspace.execute(Command::SelectChart(1)).unwrap();
        let mut deletion = workspace.deletion_draft().unwrap();
        let before_delete = workspace.file().session.serialize().unwrap();
        let before_revision = workspace.file().session.revision();
        // The timing-chart successor depends on the current cultural order.
        // Refusal without that order must be atomic and preserve identities.
        assert!(workspace.delete_chart(&deletion).is_err());
        assert_eq!(workspace.file().session.serialize().unwrap(), before_delete);
        assert_eq!(workspace.file().session.revision(), before_revision);
        assert_eq!(workspace.file().session.chart_ids(), with_duplicate);
        assert_eq!(workspace.chart(), 1);
        deletion
            .prepare_song_timing(
                &workspace.file().session,
                &Default::default(),
                &duplicate_order,
            )
            .unwrap();
        workspace.delete_chart(&deletion).unwrap();
        let mut after_delete = with_duplicate.clone();
        after_delete.remove(1);
        assert_eq!(workspace.file().session.chart_ids(), after_delete);
        let deletion_order = workspace.ordered_charts(str::cmp).unwrap();
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), with_duplicate);
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), duplicate_order);
        workspace.execute(Command::Redo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), after_delete);
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), deletion_order);
        let create = workspace.new_chart_draft().unwrap();
        let created = workspace.create_chart(&create, "pump-single").unwrap();
        assert_eq!(created, 4);
        let after_create = workspace.file().session.chart_ids().to_vec();
        assert_eq!(&after_create[..4], after_delete);
        assert!(!with_duplicate.contains(&after_create[4]));
        let created_order = workspace.ordered_charts(str::cmp).unwrap();
        assert_eq!(created_order.last(), Some(&created));
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), after_delete);
        workspace.execute(Command::Redo).unwrap();
        assert_eq!(workspace.file().session.chart_ids(), after_create);
        assert_eq!(workspace.ordered_charts(str::cmp).unwrap(), created_order);
        // Ordering remains in-memory; the original file has not been saved.
        assert_eq!(std::fs::read(&path).unwrap(), initial);
        let bytes = workspace.file().session.serialize().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains(&after_create[4].to_string()));
        workspace.execute(Command::Save).unwrap();
        let reopened = Workspace::open(&path, 0).unwrap();
        assert_eq!(reopened.file().session.serialize().unwrap(), bytes);
        assert_ne!(reopened.file().session.chart_ids(), after_create);
    }
}

#[test]
fn session_generated_batch_ids_and_failed_or_abandoned_history_keep_identity_invariants() {
    use deadsync_editor::history::ChartAddition;
    let mut session = session(Format::Ssc);
    let original = session.chart_ids().to_vec();
    let source_grid = session.grid(1).unwrap().clone();
    let before = session.serialize().unwrap();
    let wrong_revision = session.revision() + 1;
    assert!(
        session
            .append_charts(
                &[ChartAddition {
                    source_chart: 1,
                    layout: "dance-single",
                    grid: &source_grid
                }],
                wrong_revision,
                "Wrong"
            )
            .is_err()
    );
    assert_eq!(session.chart_ids(), original);
    assert_eq!(session.serialize().unwrap(), before);
    let revision = session.revision();
    let added = session
        .append_charts(
            &[
                ChartAddition {
                    source_chart: 1,
                    layout: "dance-single",
                    grid: &source_grid,
                },
                ChartAddition {
                    source_chart: 1,
                    layout: "dance-single",
                    grid: &source_grid,
                },
            ],
            revision,
            "Generated pair",
        )
        .unwrap();
    assert_eq!(added, [4, 5]);
    let generated = session.chart_ids().to_vec();
    assert_eq!(&generated[..4], original);
    assert_ne!(generated[4], generated[5]);
    assert!(!original.contains(&generated[4]));
    assert!(!original.contains(&generated[5]));
    let generated_order = Snapshot::resident(&mut session)
        .unwrap()
        .indices_with(str::cmp)
        .unwrap();
    session.undo().unwrap();
    assert_eq!(session.chart_ids(), original);
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.chart_ids(), generated);
    assert_eq!(
        Snapshot::resident(&mut session)
            .unwrap()
            .indices_with(str::cmp)
            .unwrap(),
        generated_order
    );
    session.undo().unwrap();
    let revision = session.revision();
    session.duplicate_chart(1, revision).unwrap();
    let replacement = session.chart_ids().to_vec();
    assert_eq!(&replacement[..4], original);
    assert!(!generated.contains(&replacement[4]));
    assert!(!session.redo().unwrap());
    assert_eq!(session.chart_ids(), replacement);
}

#[test]
fn stepf2_pump_double_uses_fumen_effective_routine_type_without_rewriting_source() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = String::from("#VERSION:0.83;#BPMS:0=120;");
        for (kind, difficulty, notes) in [
            ("pump-double", "Beginner", "X000000000"),
            ("pump-double", "Hard", "1000000000"),
            ("pump-routine", "Medium", "0000000001"),
        ] {
            source += &match format {
                Format::Sm => format!("#NOTES:{kind}::{difficulty}:9::\n{notes};"),
                Format::Ssc => format!(
                    "#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:9;#NOTES:{notes};"
                ),
            };
        }
        let mut session = Session::new(
            Document::parse(Arc::<[u8]>::from(source.clone().into_bytes()), format).unwrap(),
        );
        let snapshot = Snapshot::resident(&mut session).unwrap();
        assert_eq!(snapshot.keys()[0].steps_type, "pump-routine");
        assert_eq!(snapshot.keys()[1].steps_type, "pump-double");
        assert_eq!(snapshot.indices_with(str::cmp).unwrap(), [1, 0, 2]);
        assert_eq!(session.document().charts()[0].steps_type, "pump-double");
        assert_eq!(session.serialize().unwrap(), source.as_bytes());
        assert!(!session.is_dirty());
    }
}

#[test]
fn unsupported_charts_are_retained_while_editable_keys_keep_original_source_indices() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = String::from("#VERSION:0.83;#TITLE:Mixed;#BPMS:0=120;");
        for (kind, difficulty, notes) in [
            ("kb7-single", "Easy", "1000000"),
            ("dance-double", "Hard", "10000000"),
            ("lights-cabinet", "Easy", "01000000"),
            ("dance-single", "Easy", "0100"),
        ] {
            source += &match format {
                Format::Sm => format!("#NOTES:{kind}::{difficulty}:9::\n{notes};"),
                Format::Ssc => format!(
                    "#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:9;#VENDOR:{kind};#NOTES:{notes};"
                ),
            };
        }
        let mut session = Session::new(Document::parse(source.as_bytes(), format).unwrap());
        let identities = session.chart_ids().to_vec();
        let snapshot = Snapshot::resident(&mut session).unwrap();
        assert_eq!(snapshot.source_indices(), [1, 3]);
        assert_eq!(snapshot.keys().len(), 2);
        assert_eq!(snapshot.keys()[0].guid, identities[1]);
        assert_eq!(snapshot.keys()[1].guid, identities[3]);
        assert_eq!(snapshot.indices_with(str::cmp).unwrap(), [3, 1]);
        assert!(session.loaded_grid(0).is_none());
        assert!(session.loaded_grid(2).is_none());
        assert_eq!(session.serialize().unwrap(), source.as_bytes());
        assert_eq!(session.chart_ids(), identities);
        assert!(!session.is_dirty());
    }
    let mut session = Session::new(
        Document::parse(
            b"#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let snapshot = Snapshot::resident(&mut session).unwrap();
    assert!(snapshot.source_indices().is_empty());
    assert!(snapshot.indices_with(str::cmp).unwrap().is_empty());
    assert!(!session.is_dirty());
}
