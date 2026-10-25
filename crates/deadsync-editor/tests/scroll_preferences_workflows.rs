use deadsync_editor::{
    audio_preferences::File,
    scroll_preferences::{Preferences, snap_destination},
};
use std::fs;
#[test]
fn original_truncate_then_snap_formulas_cover_fractional_signed_and_exact_positions() {
    for (p, earlier, later) in [
        (0.0, -48, 48),
        (12.096, 0, 48),
        (48.0, 0, 96),
        (48.75, 0, 96),
        (49.25, 48, 96),
        (-12.5, 0, 48),
        (-48.5, -96, 0),
    ] {
        assert_eq!(snap_destination(p, 48, false).unwrap(), earlier);
        assert_eq!(snap_destination(p, 48, true).unwrap(), later);
    }
    for rows in [1, 3, 4, 6, 8, 12, 16, 24, 48] {
        for n in 0..100 {
            let p = (n * rows) as f64;
            assert_eq!(
                snap_destination(p, rows, true).unwrap(),
                ((n + 1) * rows) as i64
            );
            assert_eq!(
                snap_destination(p, rows, false).unwrap(),
                (n as i64 - 1) * rows as i64
            );
        }
    }
    for p in [
        f64::NAN,
        f64::INFINITY,
        i32::MAX as f64 + 1.0,
        i32::MIN as f64 - 1.0,
    ] {
        assert!(snap_destination(p, 48, true).is_err());
    }
    assert!(snap_destination(0.0, 0, true).is_err());
    assert!(snap_destination(i32::MAX as f64, 1, true).is_err());
}
#[test]
fn scroll_policy_defaults_noop_roundtrip_and_unknown_groups_are_preserved() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    let source=b"{\"Version\":1,\"Future\":7,\"Scroll\":{\"FutureNested\":{\"a\":1}},\"Audio\":{\"PreviewFadeOutTime\":0.5}}";
    fs::write(&p, source).unwrap();
    let mut f = File::open(&p).unwrap();
    assert!(!f.scroll().stop_playback());
    assert!(!f.save().unwrap());
    assert_eq!(fs::read(&p).unwrap(), source);
    let mut scroll = Preferences::default();
    scroll.set_stop_playback(true);
    f.set_scroll(scroll);
    assert!(f.save().unwrap());
    let r = File::open(&p).unwrap();
    assert!(r.scroll().stop_playback());
    assert_eq!(r.audio().fade_out(), 0.5);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    assert_eq!(value["Future"], 7);
    assert_eq!(value["Scroll"]["FutureNested"]["a"], 1);
    fs::write(&p, b"{\"Version\":1}").unwrap();
    f.set_scroll(Preferences::default());
    assert!(f.save().is_err());
}
#[test]
fn malformed_scroll_settings_are_rejected_without_rewriting() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    for scroll in [
        "true",
        "null",
        "[]",
        "{\"StopPlaybackWhenScrolling\":0}",
        "{\"StopPlaybackWhenScrolling\":\"false\"}",
    ] {
        let source = format!("{{\"Version\":1,\"Scroll\":{scroll}}}");
        fs::write(&p, &source).unwrap();
        assert!(File::open(&p).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), source);
    }
}
