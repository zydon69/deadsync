use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    scroll_preferences::Preferences,
    zoom::SpacingMode,
};
#[test]
fn imported_source_speeds_reverse_defaults_match_original_preferences() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scroll-preferences.expected.json")).unwrap();
    for index in [0, 1, 3] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        let input = &reference[index]["Input"];
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"Version":1,"Scroll":input})).unwrap(),
        )
        .unwrap();
        let file = File::open(&path).unwrap();
        let p = file.scroll();
        let expected = &reference[index]["Normalized"];
        for (i, key) in [
            "TimeBasedPixelsPerSecond",
            "RowBasedPixelsPerRow",
            "VariablePixelsPerSecondAtDefaultBPM",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(p.speed(i).unwrap(), expected[*key].as_f64().unwrap());
        }
        assert_eq!(p.reverse(), expected["Reverse"].as_bool().unwrap());
        assert_eq!(
            p.wheel_time(),
            expected["ScrollWheelTime"].as_f64().unwrap()
        );
        assert_eq!(
            p.wheel_rows(),
            expected["ScrollWheelRows"].as_i64().unwrap() as i32
        );
    }
    // The unchanged source has this self-comparison bug; do not confuse it with
    // proof that its Restore action never restores SizeCap.
    assert_eq!(reference[3]["UsingDefaultsBefore"], true);
}
#[test]
fn reverse_history_preserves_external_speed_and_unknown_values_without_dirtying_song() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Scroll":{"Future":"kept"},"Options":{"Future":7}}"#,
    )
    .unwrap();
    let file = File::open(&path).unwrap();
    let before = file.scroll();
    let mut after = before;
    after.set_reverse(true);
    let mut s =
        Session::new(Document::parse(&b"#VERSION:0.83;#TITLE:Policy;"[..], Format::Ssc).unwrap());
    let bytes = s.serialize().unwrap();
    assert!(
        s.set_scroll_preferences(before, after, Some(&file))
            .unwrap()
    );
    assert!(!s.is_dirty());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert!(
        s.take_scroll_preferences_update()
            .unwrap()
            .preferences
            .reverse()
    );
    let mut external = File::open(&path).unwrap();
    let mut p = external.scroll();
    p.set_speed(0, 600.).unwrap();
    external.set_scroll(p);
    external.save().unwrap();
    assert!(s.undo().unwrap());
    let u = s.take_scroll_preferences_update().unwrap();
    assert!(!u.preferences.reverse());
    assert_eq!(u.preferences.speed(0), Some(600.));
    assert!(s.redo().unwrap());
    assert!(
        s.take_scroll_preferences_update()
            .unwrap()
            .preferences
            .reverse()
    );
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(json["Scroll"]["Future"], "kept");
    assert_eq!(json["Options"]["Future"], 7);
}
#[test]
fn source_speed_bounds_feed_actual_zoom_values_in_all_three_modes() {
    let mut preferences = Preferences::default();
    let mut zoom = deadsync_editor::zoom::Manager::default();
    for (index, mode, value) in [
        (0, SpacingMode::ConstantTime, 450.),
        (1, SpacingMode::ConstantRow, 3.),
        (2, SpacingMode::Variable, 600.),
    ] {
        preferences.set_speed(index, value).unwrap();
        zoom.set_spacing(mode, preferences.speed(index).unwrap())
            .unwrap();
        assert_eq!(zoom.spacing(mode), value);
    }
    preferences.set_wheel_time(0.).unwrap();
    preferences.set_wheel_rows(0).unwrap();
    assert_eq!(preferences.wheel_time(), 0.);
    assert_eq!(preferences.wheel_rows(), 0);
}
