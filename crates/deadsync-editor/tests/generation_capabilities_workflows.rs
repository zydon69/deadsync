use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::{ExpressedConfig, PatternConfig, PerformedConfig},
        job::{ChartOptions, ChartTask, ProfileOverrides, TARGET_LAYOUTS},
        metadata::ChartData,
        pattern::{self, PatternRequest},
        pattern_job::{Options, Span, Task as PatternTask},
        search::RootContext,
        source,
        timing::{Event, EventKind, GenerationTiming},
    },
    history::Session,
    notes::{Grid, layout_columns},
};
use serde_json::Value;
use std::sync::{Arc, atomic::AtomicBool};

fn song(format: Format, layout: &str, notes: &str) -> String {
    match format {
        Format::Sm => format!(
            "#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTES:{layout}::Hard:9:0,0,0,0,0:{notes};"
        ),
        Format::Ssc => format!(
            "#VERSION:0.83;#TITLE:Keep;#VENDOR:opaque;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Hard;#METER:9;#NOTES:{notes};"
        ),
    }
}
fn options() -> Options {
    Options {
        span: Span {
            start: 0,
            end: 96,
            include_start: true,
            include_end: true,
        },
        seed: 42,
        expressed: ExpressedConfig::default(),
        performed: PerformedConfig::balanced(),
        pattern: PatternConfig::sixteenths(),
        ignore_preceding_distribution: false,
    }
}

#[test]
fn graph_generation_inventory_matches_pinned_original_capabilities_without_editor_aliases() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/generation-capabilities.reference.json"
    ))
    .unwrap();
    let reference: Vec<_> = fixture["single_player_graphs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(reference, TARGET_LAYOUTS);
    assert_eq!(reference.len(), 11);
    for case in fixture["cases"].as_array().unwrap() {
        let layout = case["layout"].as_str().unwrap();
        let graph = case["graph_registered"].as_bool().unwrap();
        assert_eq!(TARGET_LAYOUTS.contains(&layout), graph);
        if case["default_players"].as_u64().unwrap() > 1 {
            assert!(!graph);
            assert_eq!(case["autogen_feature"], false);
        }
    }
    // Geometry used by editing remains available; it does not grant a source
    // or target generation capability in the original editor.
    for (layout, canonical) in [
        ("dance-routine", "dance-double"),
        ("pump-routine", "pump-double"),
        ("smx-team", "smx-full"),
    ] {
        assert_eq!(
            deadsync_editor::pad::Pad::builtin(layout)
                .unwrap()
                .arrows
                .len(),
            deadsync_editor::pad::Pad::builtin(canonical)
                .unwrap()
                .arrows
                .len()
        );
        assert!(!TARGET_LAYOUTS.contains(&layout));
    }
}

#[test]
fn one_owner_multiplayer_families_and_legacy_stepf2_p1_sources_are_refused_atomically() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in [
            "dance-routine",
            "pump-routine",
            "smx-team",
            "dance-couple",
            "pump-couple",
            "pump-double",
        ] {
            let columns = layout_columns(layout).unwrap();
            let notes = format!(
                "{}{}",
                if layout == "pump-double" { 'X' } else { '1' },
                "0".repeat(columns as usize - 1)
            );
            let original = song(format, layout, &notes);
            let mut session = Session::new(Document::parse(original.as_bytes(), format).unwrap());
            if layout.ends_with("routine") || layout == "smx-team" {
                ChartData::default().save(&mut session, 0).unwrap();
                assert_eq!(session.grid(0).unwrap().players(), 1);
            }
            session.set_text(None, "TITLE", "Temporary").unwrap();
            session.undo().unwrap();
            let before = session.serialize().unwrap();
            let revision = session.revision();
            let redo = session.redo_label().map(str::to_owned);
            assert!(
                source::read(before.clone().into(), format, 0, &AtomicBool::new(false)).is_err(),
                "{layout}"
            );
            assert!(
                ChartTask::capture(&session, 0, ChartOptions::balanced("dance-single")).is_err(),
                "{layout}"
            );
            assert!(
                PatternTask::capture(&session, 0, options()).is_err(),
                "{layout}"
            );
            assert!(
                deadsync_editor::generation::pattern_batch::Task::capture(&session, 0, [0], None)
                    .is_err(),
                "{layout}"
            );
            assert_eq!(session.serialize().unwrap(), before);
            assert_eq!(session.revision(), revision);
            assert_eq!(session.redo_label(), redo.as_deref());
        }
    }
}

#[test]
fn ordinary_single_owner_sources_keep_all_eleven_original_layouts_including_double() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in TARGET_LAYOUTS {
            let notes = format!(
                "1{}",
                "0".repeat(layout_columns(layout).unwrap() as usize - 1)
            );
            let original = song(format, layout, &notes);
            let session = Session::new(Document::parse(original.as_bytes(), format).unwrap());
            assert!(
                source::read(
                    Arc::from(original.as_bytes()),
                    format,
                    0,
                    &AtomicBool::new(false)
                )
                .is_ok(),
                "{layout}"
            );
            assert!(
                ChartTask::capture(&session, 0, ChartOptions::balanced("dance-single")).is_ok(),
                "{layout}"
            );
            assert_eq!(session.serialize().unwrap(), original.as_bytes());
        }
    }
}

#[test]
fn public_prepared_source_cannot_bypass_layout_capability_or_stepf2_owner_checks() {
    let original = song(Format::Ssc, "pump-double", "1000000000");
    let mut prepared = source::read(
        Arc::from(original.as_bytes()),
        Format::Ssc,
        0,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        prepared
            .prepare_grid(&prepared.grid, &AtomicBool::new(false))
            .is_ok()
    );
    prepared.layout = "pump-routine".into();
    assert!(
        prepared
            .prepare_grid(&prepared.grid, &AtomicBool::new(false))
            .is_err()
    );
    assert!(prepared.prepare_editor(&AtomicBool::new(false)).is_err());
    prepared.layout = "pump-double".into();
    let coop = Grid::parse_layout(b"X000000000", "pump-double").unwrap();
    assert_eq!(coop.players(), 1);
    assert!(coop.uses_stepf2());
    assert!(
        prepared
            .prepare_grid(&coop, &AtomicBool::new(false))
            .is_err()
    );
    prepared.grid = coop;
    assert!(prepared.prepare_editor(&AtomicBool::new(false)).is_err());
}

#[test]
fn direct_pattern_generation_refuses_multiplayer_aliases_before_graph_search() {
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(120.0),
    }])
    .unwrap();
    for layout in [
        "dance-routine",
        "pump-routine",
        "smx-team",
        "dance-couple",
        "pump-couple",
    ] {
        let request = PatternRequest {
            layout: layout.into(),
            start: 0,
            end: 96,
            seed: 42,
            previous_foot: -1,
            previous_footing: [-1; 2],
            following_footing: [-1; 2],
            context: RootContext {
                lane_counts: vec![0; layout_columns(layout).unwrap() as usize],
                previous_times: [0.0; 2],
                previous_same_counts: [0; 2],
                total_steps: 0,
                last_transition: None,
            },
            config: PerformedConfig::balanced(),
            pattern: PatternConfig::sixteenths(),
            average_nps: 0.0,
        };
        let mut searches = 0;
        assert!(
            pattern::generate(&request, &timing, &AtomicBool::new(false), |_, _| {
                searches += 1
            })
            .is_err()
        );
        assert_eq!(searches, 0);
    }
}

#[test]
fn batch_double_source_selection_uses_fumen_effective_layout_without_reclassifying_the_file() {
    use deadsync_editor::generation::batch_job::Task;
    let coop = song(Format::Ssc, "pump-double", "X000000000");
    let mut session = Session::new(Document::parse(coop.as_bytes(), Format::Ssc).unwrap());
    assert!(
        Task::capture(
            &session,
            "pump-double",
            ChartOptions::balanced("dance-single"),
            None,
            ProfileOverrides::default()
        )
        .is_err()
    );
    let mixed = format!(
        "{coop}#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Hard;#METER:9;#NOTES:1000000000;"
    );
    session = Session::new(Document::parse(mixed.as_bytes(), Format::Ssc).unwrap());
    assert!(
        Task::capture(
            &session,
            "pump-double",
            ChartOptions::balanced("dance-single"),
            None,
            ProfileOverrides::default()
        )
        .is_ok()
    );
    assert_eq!(session.document().charts()[0].steps_type, "pump-double");
    assert_eq!(session.serialize().unwrap(), mixed.as_bytes());
}
