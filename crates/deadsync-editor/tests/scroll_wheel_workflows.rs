use deadsync_editor::{
    scroll_preferences::{
        Preferences,
        spacing::Preference,
        wheel::{self, Input, Intent, Interpolation},
    },
    zoom::SpacingMode,
};
fn input() -> Input {
    Input {
        detents: 1.,
        zoom_held: false,
        spacing_held: false,
        playing: false,
        has_chart: true,
        snapped: false,
        spacing_zoom: 1.,
        spacing_mode: Preference::ConstantTime,
    }
}
#[test]
fn source_priority_unknown_modes_and_threshold() {
    let p = Preferences::default();
    let mut i = input();
    i.zoom_held = true;
    i.spacing_held = true;
    assert_eq!(wheel::decide(p, i), Intent::Zoom);
    i.zoom_held = false;
    assert_eq!(
        wheel::decide(p, i),
        Intent::Spacing(Some(SpacingMode::ConstantTime))
    );
    i.spacing_mode = Preference::Unknown(99);
    assert_eq!(wheel::decide(p, i), Intent::Spacing(None));
    i.spacing_held = false;
    assert_eq!(wheel::decide(p, i), Intent::InterpolateRows(-48.));
    i.detents = 0.0000001;
    assert_eq!(wheel::decide(p, i), Intent::None);
    i.detents = f32::NAN;
    assert_eq!(wheel::decide(p, i), Intent::None);
}
#[test]
fn source_playing_always_uses_time_and_snap_ignores_magnitude() {
    let mut p = Preferences::default();
    let mut i = input();
    i.playing = true;
    i.snapped = true;
    i.spacing_mode = Preference::Variable;
    i.detents = 2.;
    i.spacing_zoom = 2.;
    assert_eq!(
        wheel::decide(p, i),
        Intent::PlayingTime {
            delta: -0.25,
            stop: false
        }
    );
    p.set_reverse(true);
    p.set_stop_playback(true);
    assert_eq!(
        wheel::decide(p, i),
        Intent::PlayingTime {
            delta: 0.25,
            stop: true
        }
    );
    i.playing = false;
    assert_eq!(wheel::decide(p, i), Intent::SnapLater);
    i.detents = 0.001;
    assert_eq!(wheel::decide(p, i), Intent::SnapLater);
    i.detents = -0.001;
    assert_eq!(wheel::decide(p, i), Intent::SnapEarlier);
    i.has_chart = false;
    i.snapped = false;
    assert_eq!(wheel::decide(p, i), Intent::None);
}
#[test]
fn continuous_frames_accumulate_desired_and_move_on_each_frame() {
    let mut interpolation = Interpolation::new(10.);
    interpolation.begin(0., 10., 2.);
    let first = interpolation.value(0.025, 0.1);
    assert_eq!(first, 10.5);
    interpolation.begin(0.025, first, 2.);
    assert_eq!(interpolation.desired(), 14.);
    assert_eq!(interpolation.value(0.05, 0.1), 11.375);
    assert_eq!(interpolation.value(1., 0.1), 14.);
    interpolation.reset(-3.);
    interpolation.begin(2., -3., -2.);
    assert_eq!(interpolation.value(2., 0.), -5.);
    // Completed interpolation retains its current value on later updates.
    assert_eq!(interpolation.value(1., 0.), -5.);
    interpolation.reset(-3.);
    interpolation.begin(2., -3., -2.);
    assert_eq!(interpolation.value(1., 0.), -3.);
}
#[test]
fn pinned_original_wheel_routing_384_cases() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scroll-wheel.expected.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 384);
    for case in cases {
        let modifiers = case["modifiers"].as_u64().unwrap();
        let mut preferences = Preferences::default();
        preferences.set_reverse(case["reverse"].as_bool().unwrap());
        let i = Input {
            detents: case["delta"].as_i64().unwrap() as f32 / 120.,
            zoom_held: modifiers & 1 != 0,
            spacing_held: modifiers & 2 != 0,
            playing: case["playing"].as_bool().unwrap(),
            has_chart: true,
            snapped: case["snapped"].as_bool().unwrap(),
            spacing_zoom: 2.,
            spacing_mode: Preference::from_raw(case["mode"].as_i64().unwrap() as i32),
        };
        let intent = wheel::decide(preferences, i);
        let trace = case["trace"].as_array().unwrap();
        let first = trace.first();
        let kind = first.and_then(|entry| entry["kind"].as_str());
        match intent {
            Intent::None | Intent::Spacing(None) => assert!(trace.is_empty(), "{case}"),
            Intent::Zoom => assert_eq!(kind, Some("Zoom"), "{case}"),
            Intent::Spacing(Some(mode)) => assert_eq!(
                kind,
                Some(match mode {
                    SpacingMode::ConstantTime => "TimeSpacing",
                    SpacingMode::ConstantRow => "RowSpacing",
                    SpacingMode::Variable => "VariableSpacing",
                }),
                "{case}"
            ),
            Intent::PlayingTime { delta, .. } => {
                assert_eq!(kind, Some("PlayingTime"), "{case}");
                assert!(
                    (first.unwrap()["time"].as_f64().unwrap() - (2. + delta)).abs() < 1e-12,
                    "{case}"
                );
            }
            Intent::InterpolateTime(delta) | Intent::InterpolateRows(delta) => {
                assert_eq!(
                    kind,
                    Some(if matches!(intent, Intent::InterpolateTime(_)) {
                        "Time"
                    } else {
                        "Rows"
                    }),
                    "{case}"
                );
                assert_eq!(first.unwrap()["time"].as_f64().unwrap(), 9.975);
                assert_eq!(first.unwrap()["delta"].as_f64().unwrap(), delta);
            }
            Intent::SnapEarlier => assert_eq!(kind, Some("Earlier"), "{case}"),
            Intent::SnapLater => assert_eq!(kind, Some("Later"), "{case}"),
        }
    }
}
#[test]
fn native_precise_wheel_is_retina_invariant_and_keeps_fractional_frames() {
    use wheel::FrameInput;
    let mut normal = FrameInput::default();
    let mut retina = FrameInput::default();
    for delta in [0.25, 0.5, -0.125, 1.5] {
        normal.physical_pixels(delta, 1.);
        retina.physical_pixels(delta * 2., 2.);
    }
    assert_eq!(normal.take(), retina.take());
    assert_eq!(normal.take(), 0.);
    normal.lines(0.25);
    normal.physical_pixels(10., 1.);
    assert_eq!(normal.take(), 0.5);
    normal.lines(1.);
    normal.cancel();
    assert_eq!(normal.take(), 0.);
    normal.physical_pixels(40., 0.);
    normal.lines(f64::NAN);
    assert_eq!(normal.take(), 0.);
    let mut p = Preferences::default();
    let mut i = input();
    normal.physical_pixels(1., 2.);
    i.detents = normal.take();
    assert_eq!(
        wheel::decide(p, i),
        Intent::InterpolateTime(-0.25 * f64::from(1.0_f32 / 80.0))
    );
    p.set_reverse(true);
    assert_eq!(
        wheel::decide(p, i),
        Intent::InterpolateTime(0.25 * f64::from(1.0_f32 / 80.0))
    );
}

#[test]
fn completion_keeps_current_and_desired_for_following_wheel() {
    let desired = f64::from_bits(1.0f64.to_bits() + 1);
    let mut interpolation = Interpolation::new(1.0);
    interpolation.begin(0.0, 1.0, desired - 1.0);
    assert!(interpolation.is_complete(1.0));
    assert_eq!(interpolation.value(10.0, 0.1).to_bits(), 1.0f64.to_bits());
    assert_eq!(interpolation.desired().to_bits(), desired.to_bits());
    interpolation.begin(10.0, 1.0, 1.0);
    assert_eq!(interpolation.desired().to_bits(), (desired + 1.0).to_bits());
    assert_eq!(interpolation.value(10.1, 0.1), desired + 1.0);
}

#[test]
fn interpolation_keeps_source_setter_tolerance_and_accumulated_target() {
    let initial = 1.0_f64;
    let target = f64::from_bits(initial.to_bits() + 2);
    let mut interpolation = Interpolation::new(initial);
    interpolation.begin(0., initial, target - initial);
    // The source setter ignores a movement of one ULP, even while the target
    // remains two ULP away and must be retained for the next wheel event.
    assert_eq!(interpolation.value(0.5, 1.), initial);
    assert_eq!(interpolation.desired(), target);
    interpolation.begin(0.5, initial, target - initial);
    assert_eq!(interpolation.desired().to_bits(), initial.to_bits() + 4);
    assert_eq!(
        interpolation.value(1.5, 1.).to_bits(),
        initial.to_bits() + 4
    );
}

#[test]
fn full_original_editor_position_interpolation_oracle() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/editor-position-interpolation.expected.json"
    ))
    .unwrap();
    for case in oracle["cases"].as_array().unwrap() {
        // Axis switching uses the cross-axis chart setter. Its source fixture is
        // retained for host integration, outside this single-axis controller.
        let kind = case["kind"].as_str().unwrap();
        if kind == "axis_switch" {
            continue;
        }
        let mut axis = Interpolation::new(1.0);
        let duration = if kind == "zero_duration" { 0.0 } else { 0.1 };
        axis.begin(
            0.0,
            1.0,
            if kind == "desired_ulp" {
                f64::from_bits(1.0f64.to_bits() + 1) - 1.0
            } else if kind == "negative" {
                -2.0
            } else {
                1.0
            },
        );
        for (index, entry) in case["trace"].as_array().unwrap().iter().enumerate() {
            if index == 2 && kind == "cancel" {
                axis.reset(axis.current());
            }
            if index == 3 {
                axis.begin(0.1, axis.current(), 1.0);
            }
            let got = axis.value(entry["now"].as_f64().unwrap(), duration);
            assert_eq!(
                got.to_bits(),
                entry["current"].as_i64().unwrap() as u64,
                "{kind} {index}"
            );
            assert_eq!(
                axis.desired().to_bits(),
                entry["desired"].as_i64().unwrap() as u64,
                "{kind} desired {index}"
            );
            assert_eq!(
                !axis.is_complete(got),
                entry["active"].as_bool().unwrap(),
                "{kind} active {index}"
            );
        }
    }
}
