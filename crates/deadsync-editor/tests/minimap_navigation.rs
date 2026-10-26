//! Isolated source modules until the coordinated UI/preferences integration.
pub use deadsync_editor::Error;
#[path = "../src/scroll_preferences/dragging.rs"]
mod dragging;
#[path = "../src/minimap_navigation.rs"]
mod navigation;
use navigation::{Effect, Gesture, Input, Position};

fn position(time: f64) -> Position {
    Position {
        chart_time: time,
        music_time: time - 0.126,
        row: time * 96.0,
    }
}

#[test]
fn paused_gesture_without_move_resumes_same_exact_fractional_instant() {
    let position = position(-0.0023);
    let mut gesture = Gesture::default();
    assert_eq!(
        gesture.input(Input::Begin {
            position,
            playing: true,
            stop_playback_on_release: false
        }),
        Some(Effect::Pause(position))
    );
    assert!(gesture.active());
    assert_eq!(gesture.input(Input::End), Some(Effect::Resume(position)));
    assert!(!gesture.active());
    assert_eq!(gesture.input(Input::End), None);
}
#[test]
fn capture_policy_handles_playing_stopped_and_both_preferences() {
    for playing in [false, true] {
        for stop in [false, true] {
            let mut gesture = Gesture::default();
            let start = position(0.126);
            let target = position(3.234567890123);
            let begin = Input::Begin {
                position: start,
                playing,
                stop_playback_on_release: stop,
            };
            assert_eq!(
                gesture.input(begin),
                playing.then_some(Effect::Pause(start))
            );
            assert_eq!(gesture.input(begin), None);
            assert_eq!(
                gesture.input(Input::Move(target)),
                Some(Effect::Focus(target))
            );
            assert_eq!(gesture.input(Input::Move(target)), None);
            assert_eq!(
                gesture.input(Input::End),
                (playing && !stop).then_some(Effect::Resume(target))
            );
        }
    }
}
#[test]
fn stale_unstarted_moves_invalid_positions_and_teardown_do_not_resume() {
    let mut gesture = Gesture::default();
    let start = position(0.126);
    assert_eq!(gesture.input(Input::Move(start)), None);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            gesture.input(Input::Begin {
                position: position(value),
                playing: true,
                stop_playback_on_release: false
            }),
            None
        );
        assert!(!gesture.active());
    }
    gesture.input(Input::Begin {
        position: start,
        playing: true,
        stop_playback_on_release: false,
    });
    assert_eq!(gesture.input(Input::Move(position(f64::NAN))), None);
    gesture.clear();
    assert_eq!(gesture.input(Input::End), None);
}
#[test]
fn drag_preference_is_distinct_and_preserves_unrelated_fields() {
    use serde_json::{Map, Value};
    let mut fields = Map::new();
    fields.insert("StopPlaybackWhenScrolling".into(), Value::Bool(true));
    fields.insert("FutureField".into(), Value::String("retain".into()));
    let mut preferences = dragging::Preferences::read(&fields).unwrap();
    assert!(!preferences.stop_playback());
    preferences.set_stop_playback(true);
    preferences.write(&mut fields);
    assert_eq!(dragging::Preferences::read(&fields).unwrap(), preferences);
    assert_eq!(fields["StopPlaybackWhenScrolling"], true);
    assert_eq!(fields["FutureField"], "retain");
    fields.insert("StopPlaybackWhenDraggingScrollBars".into(), Value::from(1));
    assert!(dragging::Preferences::read(&fields).is_err());
}
