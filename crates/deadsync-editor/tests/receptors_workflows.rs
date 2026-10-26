use deadsync_editor::{
    autoplay::ActionKind,
    receptors::{Preferences, State, alpha},
};
fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}
#[test]
fn tap_animation_uses_original_down_up_durations_and_stops_at_completion() {
    let p = Preferences::default();
    let mut s = State::default();
    s.manual(true, 0.0, p).unwrap();
    for (time, scale) in [(0.01, 0.95), (0.02, 0.9), (0.06, 0.95), (0.11, 1.0)] {
        close(s.sample(time, false, 0.0, p).unwrap().tap_scale, scale);
    }
}
#[test]
fn manual_and_autoplay_held_states_share_rim_without_releasing_each_other() {
    let p = Preferences::default();
    let mut s = State::default();
    s.manual(true, 0.0, p).unwrap();
    s.autoplay(ActionKind::Down, 0.0, 0.0, p).unwrap();
    s.autoplay(ActionKind::Up, 0.1, 0.0, p).unwrap();
    close(s.sample(0.2, false, 0.0, p).unwrap().rim_alpha, 1.0);
    s.manual(false, 0.2, p).unwrap();
    let visual = s.sample(0.3, false, 0.0, p).unwrap();
    close(visual.rim_alpha, 0.5);
    close(visual.rim_scale, 1.1);
    s.autoplay(ActionKind::Cancel, 0.3, 0.0, p).unwrap();
    close(s.sample(0.3, false, 0.0, p).unwrap().glow_alpha, 0.0);
}
#[test]
fn beat_pulse_uses_fractional_rows_including_negative_lead_in_and_preferences() {
    let p = Preferences::default();
    let mut s = State::default();
    for (row, value) in [
        (-48.0, 1.0),
        (-12.0, 0.8),
        (0.0, 1.0),
        (7.2, 0.9),
        (14.4, 0.8),
        (48.0, 1.0),
    ] {
        close(s.sample(0.0, true, row, p).unwrap().brightness, value);
    }
    close(s.sample(0.0, false, 0.0, p).unwrap().brightness, 0.8);
    close(
        s.sample(0.0, true, 0.0, Preferences { pulse: false, ..p })
            .unwrap()
            .brightness,
        0.8,
    );
}
#[test]
fn precise_hit_delta_offsets_animation_and_size_zoom_fades_without_audio_time_changes() {
    let p = Preferences::default();
    let mut s = State::default();
    s.autoplay(ActionKind::Down, 10.0, 0.05, p).unwrap();
    let v = s.sample(10.0, true, 0.0, p).unwrap();
    close(v.tap_scale, 0.9375);
    close(v.glow_alpha, 1.0 - 0.05 / 0.3);
    close(alpha(0.01).unwrap(), 0.0);
    close(alpha(0.015).unwrap(), 0.5);
    close(alpha(0.02).unwrap(), 1.0);
    assert!(alpha(f64::NAN).is_err());
    assert!(s.sample(f64::NAN, true, 0.0, p).is_err());
}
