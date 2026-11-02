//! Batch UI contracts through the production keyboard adapter and actor tree.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::{Document, Format},
    generation::{
        batch_job::Task,
        config::{PerformedConfig, PerformedPreset},
        job::{ChartOptions, ProfileOverrides},
        metadata::{ChartData, Guid},
    },
    history::Session,
};
use deadsync_theme_simply_love::screens::edition::State;
use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

const SONG: &[u8] = b"#TITLE:Batch;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:3;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:pump-single;#DIFFICULTY:Medium;#METER:6;#NOTES:10000\n01000\n00100\n00010;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:0010\n0001\n1000\n0100;";
fn init() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(screen: &mut State, code: KeyCode, pressed: bool) {
    screen.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(screen: &mut State, code: KeyCode) {
    key(screen, code, true);
    key(screen, code, false);
}
fn ctrl(screen: &mut State, code: KeyCode) {
    key(screen, KeyCode::SuperLeft, true);
    press(screen, code);
    key(screen, KeyCode::SuperLeft, false);
}
fn down(screen: &mut State, count: usize) {
    for _ in 0..count {
        press(screen, KeyCode::ArrowDown);
    }
}
fn finish(screen: &mut State) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while screen.is_generating() {
        screen.update();
        assert!(Instant::now() < deadline, "Batch UI worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn texts(screen: &State) -> Vec<String> {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
fn contains(screen: &State, text: &str) -> bool {
    texts(screen).iter().any(|content| content.contains(text))
}
fn start(screen: &mut State) {
    press(screen, KeyCode::F8);
    down(screen, 1);
    press(screen, KeyCode::ArrowRight); // dance-single -> double
    down(screen, 4);
    press(screen, KeyCode::Enter);
}

#[test]
fn f8_options_review_apply_all_undo_focus_save_and_reopen_match_core() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 2).unwrap();
    press(&mut screen, KeyCode::F8);
    assert!(contains(&screen, "Source: dance-single (2 charts)"));
    down(&mut screen, 1);
    press(&mut screen, KeyCode::ArrowRight);
    down(&mut screen, 1);
    press(&mut screen, KeyCode::ArrowRight);
    down(&mut screen, 1);
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Random seed: 1"));
    down(&mut screen, 2);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Digit1); // calculation cannot edit the song
    finish(&mut screen);
    let before = screen.workspace().file().session.serialize().unwrap();
    assert_eq!(before, SONG);
    let mut options = ChartOptions::balanced("dance-double");
    options.seed = 1;
    options.performed = PerformedConfig::preset(PerformedPreset::StaminaSingle);
    let expected = Task::capture(
        &screen.workspace().file().session,
        "dance-single",
        options,
        None,
        ProfileOverrides::default(),
    )
    .unwrap()
    .run(&AtomicBool::new(false), |_| {})
    .unwrap();
    let (preview, index) = screen.batch_preview().unwrap();
    assert_eq!(index, 0);
    assert_eq!(preview.report().generated, 2);
    for (actual, expected) in preview.entries().iter().zip(expected.entries()) {
        assert_eq!(
            actual.preview().unwrap().grid(),
            expected.preview().unwrap().grid()
        );
    }
    assert!(contains(&screen, "2/2 charts ready, 0 failed"));
    assert!(contains(&screen, "Source #1: Easy 3"));
    press(&mut screen, KeyCode::ArrowLeft);
    assert_eq!(screen.batch_preview().unwrap().1, 1);
    assert!(contains(&screen, "Source #3: Hard 9"));
    press(&mut screen, KeyCode::ArrowRight);
    assert_eq!(screen.batch_preview().unwrap().1, 0);
    press(&mut screen, KeyCode::End);
    assert!(screen.workspace().row() > 0);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::Enter);
    assert!(screen.batch_preview().is_none());
    assert_eq!(screen.workspace().chart(), 3);
    assert_eq!(
        screen.workspace().file().session.document().charts().len(),
        5
    );
    let after = screen.workspace().file().session.serialize().unwrap();
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(screen.workspace().chart(), 2);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(screen.workspace().chart(), 3);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        after
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 4).unwrap();
    assert_eq!(
        reopened.workspace().grid(),
        expected.entries()[1].preview().unwrap().grid()
    );
}
#[test]
fn partial_failures_are_reviewable_and_accept_all_successes_even_on_failed_entry() {
    init();
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    let mut data = ChartData::existing_or_initial(source.document(), 0).unwrap();
    data.expressed_chart_config = Guid::default();
    data.save(&mut source, 0).unwrap();
    // Missing profile IDs are repaired during file opening. An unsupported
    // metadata version remains reviewable but cannot be used for generation.
    source.set_text(Some(0), "GAChartDataVersion", "2").unwrap();
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("partial.ssc");
    fs::write(&path, &before).unwrap();
    // Keep the editor focus on a valid chart while the batch independently
    // reviews the unsupported peer and its successful sibling.
    let mut screen = State::open(&path, 2).unwrap();
    start(&mut screen);
    finish(&mut screen);
    assert_eq!(screen.batch_preview().unwrap().0.report().failed, 1);
    assert!(contains(&screen, "Failed:"));
    let error = screen.batch_preview().unwrap().0.entries()[0]
        .error()
        .unwrap();
    assert!(contains(&screen, error));
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert_eq!(
        actors
            .iter()
            .filter(|actor| matches!(actor, Actor::Sprite { z: 8004, .. }))
            .count(),
        0
    );
    assert!(actors.iter().any(|actor|matches!(actor,Actor::Text{content,z:8021,wrap_width_pixels:Some(_),max_height:Some(_),..} if content.contains(error))));
    assert!(contains(&screen, "Add all 1 successful charts"));
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Source #3: Hard 9"));
    assert!(contains(&screen, "Notes: 4"));
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(
        actors
            .iter()
            .any(|actor| matches!(actor, Actor::Sprite { z: 8004, .. })),
        "successful preview actors: {:?}",
        texts(&screen)
    );
    press(&mut screen, KeyCode::ArrowLeft);
    assert!(contains(&screen, "Failed:"));
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.document().charts().len(),
        4
    );
    assert_eq!(
        screen.workspace().file().session.document().charts()[3].difficulty,
        "Hard"
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
}
#[test]
fn all_failed_preview_offers_only_discard_and_keeps_redo() {
    init();
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    for chart in [0, 2] {
        let mut data = ChartData::existing_or_initial(source.document(), chart).unwrap();
        data.expressed_chart_config = Guid::default();
        data.save(&mut source, chart).unwrap();
        source
            .set_text(Some(chart), "GAChartDataVersion", "2")
            .unwrap();
    }
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    let edited = screen.workspace().file().session.serialize().unwrap();
    ctrl(&mut screen, KeyCode::KeyZ);
    start(&mut screen);
    finish(&mut screen);
    assert!(contains(&screen, "0/2 charts ready, 2 failed"));
    assert!(!contains(&screen, "Add all"));
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert_eq!(actors.iter().filter(|actor|matches!(actor,Actor::Text{z:8021,content,..} if content.as_ref()=="Discard preview")).count(),1);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.batch_preview().is_none());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        edited
    );
}
#[test]
fn menu_entry_source_type_selection_discard_cancel_and_unsupported_sources_are_read_only() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("menu.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 1).unwrap();
    press(&mut screen, KeyCode::F1);
    down(&mut screen, 10);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Source: pump-single (1 charts)"));
    press(&mut screen, KeyCode::ArrowLeft);
    assert!(contains(&screen, "Source: dance-single (2 charts)"));
    press(&mut screen, KeyCode::Escape);
    start(&mut screen);
    press(&mut screen, KeyCode::Escape);
    finish(&mut screen);
    assert!(screen.batch_preview().is_none());
    assert_eq!(screen.workspace().file().session.serialize().unwrap(), SONG);
    start(&mut screen);
    finish(&mut screen);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.batch_preview().is_none());
    assert_eq!(screen.workspace().file().session.serialize().unwrap(), SONG);
    let unsupported = dir.path().join("routine.ssc");
    fs::write(
        &unsupported,
        b"#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000;",
    )
    .unwrap();
    let mut screen = State::open(&unsupported, 0).unwrap();
    press(&mut screen, KeyCode::F8);
    assert!(contains(&screen, "No supported source charts"));
    assert!(!screen.is_generating());
}

fn choose_profile(screen: &mut State, name: &str) {
    for _ in 0..20 {
        let mut actors = Vec::new();
        screen.actors(&mut actors);
        if actors
            .iter()
            .any(|actor| matches!(actor,Actor::Text{content,z:8021,..} if content.contains(name)))
        {
            return;
        }
        press(screen, KeyCode::ArrowRight);
    }
    panic!("Profile {name} was not selectable");
}
#[test]
fn batch_profiles_return_to_batch_preserve_source_metadata_and_drive_every_result() {
    use deadsync_editor::generation::{
        config::{BracketDetermination, BracketMethod, ExpressedConfig},
        profiles::{Kind, Profile},
    };
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.ssc");
    let configs = dir.path().join("configs");
    let id = "11111111-2222-3333-4444-555555555555";
    let performed=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":id,"Name":"Batch custom placement","Config":PerformedConfig::preset(PerformedPreset::PatternNoCandles)})).unwrap()).unwrap();
    let expressed=Profile::parse(Kind::Expressed,&serde_json::to_vec(&serde_json::json!({"Guid":id,"Name":"Batch custom interpretation","Config":ExpressedConfig{bracket_parsing_determination:BracketDetermination::UseDefaultMethod,default_bracket_parsing_method:BracketMethod::NoBrackets,..ExpressedConfig::default()}})).unwrap()).unwrap();
    let placement = performed.export_new(&configs).unwrap();
    let interpretation = expressed.export_new(&configs).unwrap();
    let placement_before = fs::read(placement.path()).unwrap();
    let interpretation_before = fs::read(interpretation.path()).unwrap();
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    let mut metadata = ChartData::existing_or_initial(source.document(), 0).unwrap();
    metadata.expressed_chart_config = deadsync_editor::generation::metadata::DYNAMIC
        .parse()
        .unwrap();
    metadata.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    fs::write(&path, &before).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &configs).unwrap();
    press(&mut screen, KeyCode::F8);
    down(&mut screen, 1);
    press(&mut screen, KeyCode::ArrowRight);
    down(&mut screen, 3);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    choose_profile(&mut screen, "Batch custom placement");
    press(&mut screen, KeyCode::Escape);
    assert!(contains(&screen, "Source: dance-single (2 charts)"));
    assert!(!contains(&screen, "Preset: Batch custom placement"));
    press(&mut screen, KeyCode::Enter); // choose profiles again
    press(&mut screen, KeyCode::Escape);
    finish(&mut screen); // cancel loading
    assert!(contains(&screen, "Source: dance-single (2 charts)"));
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    choose_profile(&mut screen, "Batch custom placement");
    down(&mut screen, 1);
    choose_profile(&mut screen, "Batch custom interpretation");
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Source: dance-single (2 charts)"));
    assert!(contains(&screen, "Batch custom placement"));
    assert!(contains(&screen, "Batch custom interpretation"));
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    let preview = screen.batch_preview().unwrap().0;
    assert_eq!(preview.report().generated, 2);
    let expected = Task::capture(
        &source,
        "dance-single",
        ChartOptions::balanced("dance-double"),
        Some(&configs),
        ProfileOverrides {
            performed: Some(performed.guid().clone()),
            expressed: Some(expressed.guid().clone()),
        },
    )
    .unwrap()
    .run(&AtomicBool::new(false), |_| {})
    .unwrap();
    for (actual, expected) in preview.entries().iter().zip(expected.entries()) {
        let actual = actual.preview().unwrap();
        assert_eq!(actual.performed_profile(), Some(performed.guid()));
        assert_eq!(actual.expression_profile(), Some(expressed.guid()));
        assert_eq!(actual.grid(), expected.preview().unwrap().grid());
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        ChartData::read(screen.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config,
        deadsync_editor::generation::metadata::DYNAMIC
            .parse::<Guid>()
            .unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(placement.path()).unwrap(), placement_before);
    assert_eq!(
        fs::read(interpretation.path()).unwrap(),
        interpretation_before
    );
}
