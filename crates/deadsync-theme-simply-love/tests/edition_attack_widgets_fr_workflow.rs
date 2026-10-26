include!("support/chart_order_transport.rs");
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{attacks::Draft, timing_edit::Scope};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn ctrl(s: &mut State, code: KeyCode) {
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(s, code);
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    if code == KeyCode::Tab { complete_chart_navigation(s); }
}
use deadlib_present::actors::{Actor, SizeSpec};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn ctrl_shift(s: &mut State, code: KeyCode) {
    key(s, KeyCode::ShiftLeft, true);
    ctrl(s, code);
    key(s, KeyCode::ShiftLeft, false);
}
fn actors(s: &State) -> Vec<Actor> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a
}
fn captions(s: &State) -> Vec<(String, f32)> {
    actors(s)
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Text {
                content,
                offset,
                z: 8005,
                ..
            } if content.contains("Attaque :") || content.contains("Suite de l’attaque :") => {
                Some((content.to_string(), offset[1]))
            }
            _ => None,
        })
        .collect()
}
fn bodies(s: &State) -> Vec<(f32, f32)> {
    actors(s)
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Sprite {
                tint,
                offset,
                size: [_, SizeSpec::Px(h)],
                scale,
                ..
            } if tint == [1.0, 0.55, 0.15, 0.45] => {
                Some((offset[1], if h == 0.0 { scale[1] } else { h * scale[1] }))
            }
            _ => None,
        })
        .collect()
}
fn row(s: &mut State, index: usize) {
    press(s, KeyCode::PageUp);
    press(s, KeyCode::PageUp);
    for _ in 0..index {
        press(s, KeyCode::ArrowDown);
    }
}
#[test]
fn widget_navigation_selects_the_actual_source_and_edits_a_local_override_with_undo_and_save() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0.5:LEN=2:MODS=50% Drunk:TIME=1.25:LEN=3:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 48);
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 120);
    ctrl_shift(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 48);
    ctrl(&mut s, KeyCode::KeyJ);
    ctrl(&mut s, KeyCode::Enter);
    row(&mut s, 4);
    assert!(s.wants_text_input());
    assert!(
        actors(&s)
            .iter()
            .any(|actor| matches!(actor,Actor::Text{content,..} if content.contains("Dizzy")))
    );
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input("Reverse"));
    row(&mut s, 12);
    press(&mut s, KeyCode::Enter);
    let local = Draft::capture(&s.workspace().file().session, 0, Scope::Chart).unwrap();
    assert_eq!(local.attacks().events()[1].modifiers[0].name, "Reverse");
    let shared = Draft::capture(&s.workspace().file().session, 0, Scope::Song).unwrap();
    assert_eq!(shared.attacks().events()[1].modifiers[0].name, "Dizzy");
    assert!(
        captions(&s)
            .iter()
            .any(|(text, _)| text.contains("Reverse"))
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert!(captions(&s).iter().any(|(text, _)| text.contains("Dizzy")));
    ctrl(&mut s, KeyCode::KeyY);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert!(
        captions(&reopened)
            .iter()
            .any(|(text, _)| text.contains("Reverse"))
    );
}
#[test]
fn attack_heads_and_duration_bodies_follow_all_spacing_modes_reverse_and_viewport_clipping() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0.5:LEN=8:MODS=*2 -25% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    for mode in 0..3 {
        if mode == 1 {
            ctrl(&mut s, KeyCode::KeyT);
        } else if mode == 2 {
            ctrl_shift(&mut s, KeyCode::KeyT);
        }
        for _ in 0..2 {
            let labels = captions(&s);
            assert!(!labels.is_empty());
            assert!(labels.iter().any(|(text, _)| text.contains("Drunk")
                && text.contains("-25%")
                && text.contains("8s")));
            for (y, h) in bodies(&s) {
                assert!(y - h * 0.5 >= 104.99 && y + h * 0.5 <= 395.01);
                assert!(h.is_finite() && h > 0.0);
            }
            assert!(!bodies(&s).is_empty());
            key(&mut s, KeyCode::ShiftLeft, true);
            press(&mut s, KeyCode::KeyV);
            key(&mut s, KeyCode::ShiftLeft, false);
        }
    }
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    for _ in 0..8 {
        press(&mut s, KeyCode::PageDown);
    }
    assert!(captions(&s).is_empty());
    assert!(bodies(&s).is_empty());
}
#[test]
fn dense_frozen_scroll_widgets_are_bounded_grouped_and_keep_every_attack_in_the_document() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dense.ssc");
    let attacks = (0..4000)
        .map(|index| format!("TIME={}:LEN=1:MODS=100% Dizzy", index as f64 / 100.0))
        .collect::<Vec<_>>()
        .join(":");
    let source = format!(
        "#VERSION:0.83;#BPMS:0=120;#SCROLLS:0=0;#ATTACKS:{attacks};#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
    );
    fs::write(&path, &source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    ctrl_shift(&mut s, KeyCode::KeyT);
    let all = actors(&s);
    assert!(all.len() < 1800, "{}", all.len());
    assert!(!captions(&s).is_empty());
    assert!(captions(&s).len() <= 64);
    assert!(bodies(&s).len() <= 1024);
    assert!(!all.iter().any(|actor|matches!(actor,Actor::Text{content,..} if content.contains("{value}") || content.contains("{duration}") || content.contains("{count}"))));
    assert_eq!(
        Draft::capture(&s.workspace().file().session, 0, Scope::Song)
            .unwrap()
            .attacks()
            .events()
            .len(),
        4000
    );
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
}
#[test]
fn malformed_attack_data_has_an_explicit_annotation_without_blocking_notes_or_rewriting_source() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=NaN:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert!(actors(&s).iter().any(|actor|matches!(actor,Actor::Text{content,..} if content.as_ref()=="Positions des attaques indisponibles.")), "{:?}", actors(&s).iter().filter_map(|actor| match actor { Actor::Text{content,..}=>Some(content.to_string()), _=>None }).collect::<Vec<_>>());
    ctrl(&mut s, KeyCode::Enter);
    assert!(actors(&s).iter().any(|actor| matches!(actor, Actor::Text { content, z:8006, .. } if content.as_ref() == "Positions des attaques indisponibles.")));
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 0);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::Digit2);
    assert!(s.workspace().file().session.is_dirty());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn switching_charts_rebuilds_widget_scope_and_opens_the_current_charts_attack() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("charts.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0.5:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=240;#OFFSET:0.25;#ATTACKS:TIME=0.5:LEN=1:MODS=50% Mini;#NOTES:0100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    assert!(captions(&s).iter().any(|(text, _)| text.contains("Dizzy")));
    ctrl(&mut s, KeyCode::Tab);
    assert_eq!(s.workspace().chart(), 1);
    assert!(captions(&s).iter().any(|(text, _)| text.contains("Mini")));
    assert!(!captions(&s).iter().any(|(text, _)| text.contains("Dizzy")));
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 144);
    ctrl(&mut s, KeyCode::Enter);
    row(&mut s, 4);
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input("Reverse"));
    row(&mut s, 12);
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        Draft::capture(&s.workspace().file().session, 1, Scope::Chart)
            .unwrap()
            .attacks()
            .events()[0]
            .modifiers[0]
            .name,
        "Reverse"
    );
    assert_eq!(
        Draft::capture(&s.workspace().file().session, 0, Scope::Chart)
            .unwrap()
            .attacks()
            .events()[0]
            .modifiers[0]
            .name,
        "Dizzy"
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::Tab);
    assert_eq!(s.workspace().chart(), 0);
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 48);
    assert!(captions(&s).iter().any(|(text, _)| text.contains("Dizzy")));
}

#[test]
fn hidden_attacks_remain_navigable_and_editable_after_keyboard_warp_apply_save_and_reopen() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy:TIME=1.5:LEN=3:MODS=50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::F10);
    for _ in 0..3 {
        press(&mut s, KeyCode::Tab);
    }
    for _ in 0..2 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Delete);
    press(&mut s, KeyCode::Digit4);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    for _ in 0..5 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    let hidden = s.workspace().file().session.serialize().unwrap();
    assert_ne!(hidden, source);
    let timeline =
        deadsync_editor::attacks::placement::Timeline::capture(&s.workspace().file().session, 0)
            .unwrap();
    assert!(timeline.has_resident_rows());
    assert_eq!(
        timeline.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [96, 144]
    );
    assert!(!captions(&s).is_empty());
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 96);
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 144);
    ctrl(&mut s, KeyCode::Enter);
    assert!(
        actors(&s)
            .iter()
            .any(|actor| matches!(actor,Actor::Text{content,..} if content.contains("Drunk")))
    );
    row(&mut s, 4);
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input("Reverse"));
    row(&mut s, 12);
    press(&mut s, KeyCode::Enter);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    let timeline = deadsync_editor::attacks::placement::Timeline::capture(
        &reopened.workspace().file().session,
        0,
    )
    .unwrap();
    assert_eq!(timeline.markers()[0].row, 96);
    assert_eq!(timeline.markers()[1].row, 144);
    assert_eq!(timeline.markers()[1].modifiers[0].name, "Reverse");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), hidden);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
