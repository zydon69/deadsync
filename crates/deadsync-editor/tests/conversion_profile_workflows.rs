#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::{
            BracketDetermination, BracketMethod, ExpressedConfig, PerformedConfig, PerformedPreset,
        },
        job::{ChartOptions, ChartTask, Phase, ProfileOverrides},
        metadata::{ChartData, DYNAMIC, Guid},
        profile_catalog::Task,
        profiles::{Kind, Loaded, Profile},
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn source(format: Format) -> Session {
    let header = match format {
        Format::Sm => {
            "#TITLE:Café;#VENDOR:keep;#BPMS:0=120;#NOTES:dance-single:source:Hard:9:0,0,0,0,0:"
        }
        Format::Ssc => {
            "#VERSION:0.83;#TITLE:Café;#VENDOR:keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:"
        }
    };
    let mut bytes = header.as_bytes().to_vec();
    bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn custom(folder: &std::path::Path) -> (Profile, Profile) {
    let id = "11111111-2222-3333-4444-555555555555";
    let performed=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":id,"Name":"Custom placement","Config":PerformedConfig::preset(PerformedPreset::PatternNoCandles)})).unwrap()).unwrap();
    performed.export_new(folder).unwrap();
    let expression = ExpressedConfig {
        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
        default_bracket_parsing_method: BracketMethod::NoBrackets,
        ..ExpressedConfig::default()
    };
    let expressed = Profile::parse(
        Kind::Expressed,
        &serde_json::to_vec(
            &serde_json::json!({"Guid":id,"Name":"Custom interpretation","Config":expression}),
        )
        .unwrap(),
    )
    .unwrap();
    expressed.export_new(folder).unwrap();
    (performed, expressed)
}
#[test]
fn conversion_overrides_match_explicit_configs_and_save_new_charts_without_changing_source_profiles()
 {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = source(format);
        let before = source.serialize().unwrap();
        let source_grid = source.grid(0).unwrap().clone();
        let dir = tempfile::tempdir().unwrap();
        let (performed, expressed) = custom(dir.path());
        fs::write(dir.path().join("pcc-broken.json"), b"invalid").unwrap();
        let registry = Loaded::directory(dir.path()).unwrap().registry;
        let mut options = ChartOptions::balanced("dance-double");
        options.seed = -7;
        let mut explicit = options.clone();
        explicit.performed = registry.performed[performed.guid()].clone();
        explicit.expressed = registry.expressed[expressed.guid()].clone();
        let expected = ChartTask::capture(&source, 0, explicit)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        let overrides = ProfileOverrides {
            performed: Some(performed.guid().clone()),
            expressed: Some(expressed.guid().clone()),
        };
        let catalog = Task::capture(&source, 0, Some(dir.path()))
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        let overrides = catalog.conversion(&source, 0, overrides).unwrap();
        let preview = ChartTask::capture(&source, 0, options.clone())
            .unwrap()
            .with_profile_overrides(Some(dir.path()), overrides.clone())
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        assert_eq!(preview.grid(), expected.grid());
        assert_eq!(preview.performed_profile(), Some(performed.guid()));
        assert_eq!(preview.expression_profile(), Some(expressed.guid()));
        assert_eq!(preview.profile_issues().len(), 1);
        assert_eq!(source.serialize().unwrap(), before);
        let index = preview.apply(&mut source).unwrap();
        assert_eq!(index, 1);
        assert_eq!(source.grid(0).unwrap(), &source_grid);
        assert!(ChartData::read(source.document(), 0).unwrap().is_none());
        let after = source.serialize().unwrap();
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        fs::write(&path, &before).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let preview = ChartTask::capture(&file.session, 0, options)
            .unwrap()
            .with_profile_overrides(Some(dir.path()), overrides)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        preview.apply(&mut file.session).unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        assert_eq!(
            file.session.serialize().unwrap(),
            after,
            "Save must not rewrite resident Undo bytes"
        );
        let published = fs::read(&path).unwrap();
        source_output_context::assert_context_only(&after, &published, format);
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), before);
        file.session.redo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), published);
        assert_eq!(
            reopened.session.document().charts().len(),
            source.document().charts().len()
        );
        for index in 0..source.document().charts().len() {
            assert_eq!(
                reopened.session.grid(index).unwrap(),
                source.grid(index).unwrap()
            );
            assert_eq!(
                serde_json::to_value(ChartData::read(reopened.session.document(), index).unwrap())
                    .unwrap(),
                serde_json::to_value(ChartData::read(source.document(), index).unwrap()).unwrap(),
            );
        }
        assert_eq!(
            fs::read(dir.path().join(performed.filename())).unwrap(),
            performed.serialize().unwrap()
        );
        assert_eq!(
            fs::read(dir.path().join(expressed.filename())).unwrap(),
            expressed.serialize().unwrap()
        );
    }
}
#[test]
fn default_and_partial_overrides_keep_preset_and_chart_expression_independent() {
    let mut source = source(Format::Ssc);
    let mut data = ChartData::default();
    data.expressed_chart_config = "da3f6e12-49d1-416b-8db6-0ab413f740b6".parse().unwrap();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (performed, expressed) = custom(dir.path());
    let registry = Loaded::directory(dir.path()).unwrap().registry;
    for selection in [
        ProfileOverrides::default(),
        ProfileOverrides {
            performed: Some(performed.guid().clone()),
            expressed: None,
        },
        ProfileOverrides {
            performed: None,
            expressed: Some(expressed.guid().clone()),
        },
    ] {
        let mut options = ChartOptions::balanced("dance-double");
        options.performed = PerformedConfig::preset(PerformedPreset::StaminaDouble);
        let mut explicit = options.clone();
        explicit.expressed = registry.expressed[selection
            .expressed
            .as_ref()
            .unwrap_or(&data.expressed_chart_config)]
        .clone();
        if let Some(id) = &selection.performed {
            explicit.performed = registry.performed[id].clone();
        }
        let expected = ChartTask::capture(&source, 0, explicit)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        let preview = ChartTask::capture(&source, 0, options)
            .unwrap()
            .with_profile_overrides(Some(dir.path()), selection.clone())
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        assert_eq!(preview.grid(), expected.grid());
        assert_eq!(preview.performed_profile(), selection.performed.as_ref());
        assert_eq!(
            preview.expression_profile(),
            Some(
                selection
                    .expressed
                    .as_ref()
                    .unwrap_or(&data.expressed_chart_config)
            )
        );
        assert_eq!(source.serialize().unwrap(), before);
    }
}
#[test]
fn explicit_expression_can_override_unresolved_source_identity_without_rewriting_it() {
    let mut source = source(Format::Ssc);
    let mut data = ChartData::default();
    data.expressed_chart_config = Guid::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    assert!(
        ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .with_chart_expression(None)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
    let overrides = ProfileOverrides {
        performed: None,
        expressed: Some(DYNAMIC.parse().unwrap()),
    };
    let preview = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .with_profile_overrides(None, overrides)
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(
        preview.expression_profile(),
        Some(&DYNAMIC.parse().unwrap())
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(
        ChartData::read(source.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config,
        Guid::default()
    );
}
#[test]
fn catalogs_missing_profiles_cancellation_and_stale_previews_are_read_only() {
    let mut source = source(Format::Ssc);
    let before = source.serialize().unwrap();
    let catalog = Task::capture(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(
        catalog
            .conversion(&source, 1, ProfileOverrides::default())
            .is_err()
    );
    assert!(
        catalog
            .conversion(
                &source,
                0,
                ProfileOverrides {
                    performed: Some(DYNAMIC.parse().unwrap()),
                    expressed: None
                }
            )
            .is_err()
    );
    assert!(
        catalog
            .conversion(
                &source,
                0,
                ProfileOverrides {
                    performed: None,
                    expressed: Some(Guid::default())
                }
            )
            .is_err()
    );
    let foreign =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    assert!(
        catalog
            .conversion(&foreign, 0, ProfileOverrides::default())
            .is_err()
    );
    let task = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .with_profile_overrides(None, ProfileOverrides::default())
        .unwrap();
    source
        .edit_notes(
            0,
            "Other",
            [(
                Position {
                    player: 0,
                    row: 12,
                    lane: 3,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    assert!(
        catalog
            .conversion(&source, 0, ProfileOverrides::default())
            .is_err()
    );
    let preview = task.run(&AtomicBool::new(false), |_| {}).unwrap();
    assert!(preview.apply(&mut source).is_err());
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    let missing = ProfileOverrides {
        performed: Some(Guid::default()),
        expressed: None,
    };
    assert!(
        ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .with_profile_overrides(None, missing)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
    let cancelled = AtomicBool::new(false);
    assert!(
        ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .with_profile_overrides(None, ProfileOverrides::default())
            .unwrap()
            .run(&cancelled, |progress| {
                if progress.phase == Phase::Interpreting {
                    cancelled.store(true, Ordering::Relaxed)
                }
            })
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
}
