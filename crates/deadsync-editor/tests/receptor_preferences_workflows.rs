use deadsync_editor::{receptor_preferences::Preferences, receptors};
#[test]
fn source_defaults_and_all_eight_flags_roundtrip_preserving_unknown_controls() {
    let mut map = serde_json::json!({"PositionX":842,"Vendor":42})
        .as_object()
        .unwrap()
        .clone();
    let defaults = Preferences::read(&map).unwrap();
    assert_eq!(defaults, Preferences::default());
    let off = Preferences {
        hide_arrows: false,
        light_holds: false,
        placement:Default::default(),
        animation: receptors::Preferences {
            autoplay_rim: false,
            autoplay_glow: false,
            autoplay_shrink: false,
            tap_rim: false,
            tap_shrink: false,
            pulse: false,
        },
    };
    off.write(&mut map);
    assert_eq!(Preferences::read(&map).unwrap(), off);
    assert_eq!(map["PositionX"], 842);
    assert_eq!(map["Vendor"], 42);
    map.insert("AutoPlayHideArrows".into(), serde_json::json!("false"));
    assert!(Preferences::read(&map).is_err());
}
#[test]
fn stored_effect_flags_drive_the_existing_animation_model() {
    let map=serde_json::json!({"AutoPlayRimEffect":false,"AutoPlayGlowEffect":false,"AutoPlayShrinkEffect":false,"TapRimEffect":false,"TapShrinkEffect":false,"PulseReceptorsWithTempo":false}).as_object().unwrap().clone();
    let p = Preferences::read(&map).unwrap();
    let mut state = receptors::State::default();
    state.manual(true, 1.0, p.animation).unwrap();
    state
        .autoplay(
            deadsync_editor::autoplay::ActionKind::Down,
            1.0,
            0.0,
            p.animation,
        )
        .unwrap();
    let visual = state.sample(1.0, true, 24.0, p.animation).unwrap();
    assert_eq!(visual.tap_scale, 1.0);
    assert_eq!(visual.rim_alpha, 0.0);
    assert_eq!(visual.glow_alpha, 0.0);
}
#[test]
fn shared_application_file_roundtrips_flags_and_checks_external_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        b"{\"Version\":1,\"Receptors\":{\"Vendor\":42,\"PositionX\":842}}",
    )
    .unwrap();
    let mut file = deadsync_editor::audio_preferences::File::open(&path).unwrap();
    let mut p = file.receptors();
    p.hide_arrows = false;
    p.light_holds = false;
    p.animation.pulse = false;
    file.set_receptors(p);
    assert!(file.save().unwrap());
    assert_eq!(
        deadsync_editor::audio_preferences::File::open(&path)
            .unwrap()
            .receptors(),
        p
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["Receptors"]["Vendor"], 42);
    assert_eq!(value["Receptors"]["PositionX"], 842);
    std::fs::write(&path, b"{\"Version\":1,\"External\":true}").unwrap();
    file.set_receptors(Preferences::default());
    assert!(matches!(
        file.save(),
        Err(deadsync_editor::Error::ExternalChange)
    ));
}

#[test]
fn filtered_active_hold_and_raw_start_fill_have_distinct_source_contracts() {
    use deadsync_editor::{
        autoplay::Timeline,
        document::{Document, Format},
        history::Session,
        notes::Position,
    };
    for fake in [false, true] {
        let source = format!(
            "#VERSION:0.83;#BPMS:0=120;{}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:2000\n0000\n3000\n0000;",
            if fake { "#FAKES:0=4;" } else { "" }
        );
        let session = Session::new(Document::parse(source.into_bytes(), Format::Ssc).unwrap());
        let timeline = Timeline::capture(&session, 0).unwrap();
        for playing in [false, true] {
            let p = timeline
                .presentation(
                    &session,
                    0,
                    Position {
                        row: 0,
                        lane: 0,
                        player: 0,
                    },
                    0.25,
                    playing,
                    true,
                    false,
                )
                .unwrap()
                .unwrap();
            assert_eq!(p.fake, fake);
            assert!(
                !p.active_hold,
                "LightHolds continues filtering existing field"
            );
            assert_eq!(p.raw_active_hold, playing && !fake);
            assert_eq!(p.clip_hold, playing && !fake);
        }
    }
}
