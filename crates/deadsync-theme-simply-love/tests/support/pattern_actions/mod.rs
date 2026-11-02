use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn init() {
    if super::FRENCH {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn open(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyE);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
fn source() -> deadsync_editor::history::Session {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
        history::Session,
        notes::{Grid, Note, Position},
    };
    let mut grid = Grid::new(4, 1).unwrap();
    for row in [0, 192, 384] {
        grid.set(
            Position {
                row,
                lane: 0,
                player: 0,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    let mut bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:".to_vec();
    bytes.extend(grid.serialize().unwrap());
    bytes.push(b';');
    let mut s =
        Session::new(Document::parse(std::sync::Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    ChartData {
        patterns: Some(
            [(0, 10), (192, 20), (384, 30)]
                .into_iter()
                .map(|(row, seed)| {
                    (
                        row,
                        Definition {
                            length: 96,
                            random_seed: seed,
                            start_position_inclusive: true,
                            pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                            performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                            ..Definition::default()
                        },
                    )
                })
                .collect(),
        ),
        ..ChartData::default()
    }
    .save(&mut s, 0)
    .unwrap();
    s
}
fn text(s: &State, key: &str) -> bool {
    let mut a = Vec::new();
    s.actors(&mut a);
    let value = i18n::tr("Edition", key);
    a.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==value.as_ref()))
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn all(s: &mut State) {
    press(s, KeyCode::F1);
    down(s, 7);
    press(s, KeyCode::Enter);
    press(s, KeyCode::F8);
}
fn prepare(s: &mut State) {
    down(s, 2);
    press(s, KeyCode::Enter);
}
fn start(s: &mut State) {
    down(s, 2);
    press(s, KeyCode::Enter);
}
fn finish(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while s.is_generating() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn seeds(s: &State) -> Vec<i32> {
    deadsync_editor::generation::metadata::ChartData::read(
        s.workspace().file().session.document(),
        0,
    )
    .unwrap()
    .unwrap()
    .patterns
    .unwrap()
    .into_values()
    .map(|p| p.random_seed)
    .collect()
}
#[test]
fn all_new_seeds_commit_before_worker_and_cancellation_keeps_undoable_seed_action() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = source().serialize().unwrap();
    fs::write(&path, &before).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    all(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    prepare(&mut s);
    assert!(text(&s, "PatternActionsPrepared"));
    assert_eq!(serialized(&s), before);
    start(&mut s);
    assert!(s.is_generating());
    let newseeds = seeds(&s);
    assert_eq!(newseeds.len(), 3);
    assert!(newseeds.iter().all(|v| *v >= 0));
    let reseeded = serialized(&s);
    assert_ne!(reseeded, before);
    press(&mut s, KeyCode::Escape);
    finish(&mut s);
    assert!(text(&s, "PatternActionsSeedsKept"));
    assert_eq!(serialized(&s), reseeded);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), before);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(serialized(&s), reseeded);
}
#[test]
fn checked_clear_ignores_unchecked_pattern_and_preserves_marker_definitions() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = source().serialize().unwrap();
    fs::write(&path, &before).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F3);
    for _ in 0..10 {
        press(&mut s, KeyCode::ArrowRight);
    }
    down(&mut s, 3);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::F6);
    press(&mut s, KeyCode::ArrowLeft);
    prepare(&mut s);
    assert!(text(&s, "PatternActionsPrepared"));
    start(&mut s);
    finish(&mut s);
    press(&mut s, KeyCode::Enter);
    let grid = s.workspace().grid();
    for row in [0, 192] {
        assert!(
            grid.get(deadsync_editor::notes::Position {
                row,
                lane: 0,
                player: 0
            })
            .is_none()
        );
    }
    assert!(
        grid.get(deadsync_editor::notes::Position {
            row: 384,
            lane: 0,
            player: 0
        })
        .is_some()
    );
    assert_eq!(seeds(&s), vec![10, 20, 30]);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), before);
}
#[test]
fn preparing_then_cancelling_does_not_create_seed_history() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = source().serialize().unwrap();
    fs::write(&path, &before).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    all(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    prepare(&mut s);
    assert!(text(&s, "PatternActionsPrepared"));
    press(&mut s, KeyCode::Escape);
    assert_eq!(serialized(&s), before);
    assert_eq!(seeds(&s), vec![10, 20, 30]);
}
