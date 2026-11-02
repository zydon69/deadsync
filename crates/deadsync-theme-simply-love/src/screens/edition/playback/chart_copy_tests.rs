use super::super::{Panel, RawKeyboardEvent, chart_views::ViewAction};
use super::*;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    chart_order::{Orderer, Snapshot},
    key_binding_preferences::Preferences,
    song_text::Field,
};
struct Identity;
impl Orderer for Identity {
    fn order(&mut self, s: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
        Ok(s.source_indices().to_vec())
    }
}
struct Reverse;
impl Orderer for Reverse {
    fn order(&mut self, s: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
        Ok(s.source_indices().iter().copied().rev().collect())
    }
}
#[derive(Default)]
struct CountedReverse {
    calls: usize,
}
impl Orderer for CountedReverse {
    fn order(&mut self, s: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
        self.calls += 1;
        Ok(s.source_indices().iter().copied().rev().collect())
    }
}
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
fn state(language: &str, mode: usize) -> (tempfile::TempDir, State) {
    crate::i18n::init(deadsync_assets::language::load_for_tests(language));
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.wav"), b"existing descriptor").unwrap();
    std::fs::write(dir.path().join("source.wav"), b"other existing descriptor").unwrap();
    let file = dir.path().join("copy.ssc");
    std::fs::write(&file,b"#VERSION:0.83;#MUSIC:main.wav;#OFFSET:0.125;#SAMPLESTART:0;#SAMPLELENGTH:2;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Focus A;#BPMS:0=120;#SCROLLS:0=1;#LABELS:1=old;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Timing B;#MUSIC:source.wav;#OFFSET:0.75;#BPMS:0=240;#SCROLLS:0=2;#LABELS:2=source;#NOTES:0100;").unwrap();
    let mut s = State::open(&file, 0).unwrap();
    s.configure_key_binding_preferences(Arc::new(Preferences::default()));
    let owner = s
        .take_audio_commands()
        .into_iter()
        .find_map(|c| {
            if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                Some(owner)
            } else {
                None
            }
        })
        .unwrap();
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.editor.owner = owner;
    snapshot.editor.output.available = true;
    snapshot.editor.output.leased = true;
    snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
    snapshot.editor.output.requested_period = Some(512);
    snapshot.editor.output.negotiated_period = Some(512);
    s.update_audio(snapshot);
    s.take_audio_commands();
    s.toggle_audio();
    s.bind_decoder_generation(Some(71));
    s.bind_decoder_asset_epoch(Some(owner), Some(83));
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.editor.owner = owner;
    snapshot.editor.output = s.audio_output_snapshot();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 1_250_000_000;
    snapshot.stream_clock.music_seconds_per_second = 1.;
    snapshot.stream_clock.stream_seconds = 12.;
    snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
        generation: 71,
        asset_epoch: 83,
        music_nanos: 1_250_000_000,
    });
    s.update_audio(snapshot);
    match mode {
        1 => {
            s.toggle_audio();
            assert!(s.paused_position().is_some());
        }
        2 | 3 => {
            s.chart_views_apply(ViewAction::Close);
            assert!(!s.workspace.has_chart());
            if mode == 3 {
                s.toggle_audio();
                assert!(s.paused_position().is_some());
            }
        }
        4 => {
            s.stop_audio();
            let owner = s.editor_asset_owner();
            s.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness {
                owner,
                registration: s.editor_asset_registration(),
                main: true,
                dedicated_preview: true,
            });
            s.command_toggle_sample_audio();
            assert!(s.is_playing());
            assert!(!s.chart_transport_is_playing());
        }
        _ => {}
    }
    s.take_audio_commands();
    (dir, s)
}
#[derive(Debug, PartialEq)]
struct Stamp {
    intent: (u64, Option<Instant>, bool, bool),
    generation: Option<u64>,
    epoch: Option<u64>,
    lease: bool,
    sample: Option<GameplayAudioCommand>,
}
fn stamp(s: &State) -> Stamp {
    if s.paused_playback.is_some() {
        assert!(s.paused_position().is_some());
    }
    Stamp {
        intent: s.save_as_transport_stamp(),
        generation: s.chart_audio_generation(),
        epoch: s.playback_asset_epoch(),
        lease: s.audio_output_snapshot().leased,
        sample: s
            .playback
            .as_ref()
            .and_then(|p| p.sample_descriptor.clone()),
    }
}
fn no_handoff(s: &mut State) {
    let commands = s.take_audio_commands();
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            GameplayAudioCommand::StopMusic
                | GameplayAudioCommand::SeekMusic(_)
                | GameplayAudioCommand::PlayMusic { .. }
                | GameplayAudioCommand::EditorPlayMusic { .. }
                | GameplayAudioCommand::EditorPlayPreview { .. }
                | GameplayAudioCommand::EditorResumePreview { .. }
                | GameplayAudioCommand::EditorClearMusicAsset { .. }
                | GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                | GameplayAudioCommand::EditorSeekMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )),
        "{commands:?}"
    );
}
fn open_quick(s: &mut State) {
    open_quick_ordered(s, &mut Identity, KeyCode::ArrowRight);
}
fn open_quick_ordered(s: &mut State, orderer: &mut impl Orderer, cycle: KeyCode) {
    press(s, KeyCode::F11);
    assert!(
        matches!(s.panel, Some(Panel::SongProperties(0))),
        "{}",
        s.status
    );
    s.song_properties
        .as_mut()
        .unwrap()
        .set_text(Field::Title, "staged only")
        .unwrap();
    s.song_properties_key(6, cycle);
    assert!(s.process_chart_order(orderer));
    assert_eq!(
        s.song_properties
            .as_ref()
            .unwrap()
            .timing_chart(&s.workspace.file().session)
            .unwrap(),
        Some(1)
    );
    s.panel = Some(Panel::SongProperties(6));
    press(s, KeyCode::Enter);
    assert!(matches!(s.panel, Some(Panel::ChartEventCopy)));
}
#[test]
fn physical_song_timing_apply_from_b_keeps_focus_a_main_pause_and_chartless_en_fr() {
    for language in ["en", "fr"] {
        for mode in 0..5 {
            let (_dir, mut s) = state(language, mode);
            let original = s.workspace.file().session.serialize().unwrap();
            let original_choice = s.workspace.file().session.song_timing_chart().unwrap();
            let before = stamp(&s);
            let old_position = s.workspace.has_chart().then(|| {
                s.capture_song_property_presentation()
                    .unwrap()
                    .focus()
                    .clone()
            });
            open_quick(&mut s);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
            assert_eq!(stamp(&s), before);
            no_handoff(&mut s);
            press(&mut s, KeyCode::Enter);
            assert!(
                matches!(s.panel, Some(Panel::SongProperties(6))),
                "{}",
                s.status
            );
            assert_eq!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(Some(0), "BPMS")
                    .as_deref(),
                Some("0=240")
            );
            assert_eq!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(Some(0), "SCROLLS")
                    .as_deref(),
                Some("0=1")
            );
            assert_eq!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(Some(0), "LABELS")
                    .as_deref(),
                Some("1=old")
            );
            assert_eq!(
                s.workspace.file().session.document().text(Some(0), "MUSIC"),
                None
            );
            assert_eq!(
                s.workspace.file().session.song_timing_chart().unwrap(),
                original_choice
            );
            assert_eq!(
                s.song_properties.as_ref().unwrap().text(Field::Title),
                "staged only"
            );
            assert_eq!(stamp(&s), before);
            no_handoff(&mut s);
            if let Some(old) = &old_position {
                let new = s.capture_song_property_presentation().unwrap();
                assert_eq!(s.workspace.chart(), 0);
                if mode == 0 {
                    assert_eq!(
                        new.focus().music_time().to_bits(),
                        old.music_time().to_bits()
                    );
                    assert_ne!(new.focus().row().to_bits(), old.row().to_bits());
                } else {
                    assert_eq!(new.focus().row().to_bits(), old.row().to_bits());
                }
            }
            let changed = s.workspace.file().session.serialize().unwrap();
            assert_ne!(changed, original);
            // Keep the F11 staged draft throughout history; its token is rebased only by
            // the closed content receipt and never publishes the staged title/owner.
            for (redo, expected) in [(false, &original), (true, &changed)] {
                s.command(if redo { Command::Redo } else { Command::Undo });
                assert_eq!(
                    s.workspace.file().session.serialize().unwrap(),
                    *expected,
                    "{}",
                    s.status
                );
                assert_eq!(stamp(&s), before);
                no_handoff(&mut s);
                assert_eq!(
                    s.song_properties.as_ref().unwrap().text(Field::Title),
                    "staged only"
                );
            }
            press(&mut s, KeyCode::Escape);
            assert!(s.song_properties.is_none());
            assert_eq!(s.workspace.file().session.serialize().unwrap(), changed);
        }
    }
}
#[test]
fn physical_advanced_source_picker_is_independent_culturally_ordered_and_cancel_stale_are_non_mutating_en_fr()
 {
    for language in ["en", "fr"] {
        let (_dir, mut s) = state(language, 0);
        let original = s.workspace.file().session.serialize().unwrap();
        let before = stamp(&s);
        // A culture/content signature identifies one host ordering. Configure
        // that same collator from the first request, rather than swapping a
        // second fake collator underneath a correctly cached result.
        let mut orderer = CountedReverse::default();
        open_quick_ordered(&mut s, &mut orderer, KeyCode::ArrowLeft);
        assert_eq!(
            orderer.calls, 1,
            "the first request must call the host collator"
        );
        for _ in 0..3 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        assert_eq!(s.chart_event_copy_source().unwrap(), 0); // Advanced defaults to focused A, not TimingChart B.
        press(&mut s, KeyCode::Enter);
        assert!(s.chart_event_copy_request.is_some());
        press(&mut s, KeyCode::Enter);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
        assert!(s.process_chart_order(&mut orderer));
        assert_eq!(
            orderer.calls, 1,
            "unchanged culture/content must reuse the same reverse ordering"
        );
        press(&mut s, KeyCode::Home);
        press(&mut s, KeyCode::Enter);
        assert_eq!(s.chart_event_copy_source().unwrap(), 1);
        assert_eq!(s.workspace.chart(), 0);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
        assert_eq!(stamp(&s), before);
        no_handoff(&mut s);
        press(&mut s, KeyCode::Escape);
        assert!(matches!(s.panel, Some(Panel::SongProperties(6))));
        press(&mut s, KeyCode::Escape);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
        assert_eq!(stamp(&s), before);
        s.open_chart_event_copy();
        press(&mut s, KeyCode::End);
        press(&mut s, KeyCode::ArrowUp);
        s.workspace.execute(Command::Place(1)).unwrap();
        let competing = s.workspace.file().session.serialize().unwrap();
        press(&mut s, KeyCode::Enter);
        assert_eq!(s.workspace.file().session.serialize().unwrap(), competing);
        assert!(s.chart_event_copy.is_none());
        assert_eq!(stamp(&s), before);
        no_handoff(&mut s);
    }
}
#[test]
fn chartless_advanced_defaults_to_first_host_sorted_chart_and_empty_timing_choice_is_disabled() {
    let (_dir, mut s) = state("en", 2);
    let original = s.workspace.file().session.serialize().unwrap();
    let before = stamp(&s);
    s.open_chart_event_copy();
    assert!(s.chart_event_copy_request.is_some());
    assert!(s.process_chart_order(&mut Reverse));
    assert_eq!(s.chart_event_copy_source().unwrap(), 1);
    assert!(!s.workspace.has_chart());
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F11);
    s.song_properties
        .as_mut()
        .unwrap()
        .set_timing_chart(&s.workspace.file().session, None)
        .unwrap();
    s.panel = Some(Panel::SongProperties(6));
    press(&mut s, KeyCode::Enter);
    assert!(matches!(s.panel, Some(Panel::SongProperties(6))));
    assert!(s.chart_event_copy.is_none());
    assert_eq!(s.workspace.file().session.serialize().unwrap(), original);
    assert_eq!(stamp(&s), before);
    no_handoff(&mut s);
}
#[test]
fn three_song_presets_publish_exact_selected_timing_scroll_and_label_sets_en_fr() {
    for language in ["en", "fr"] {
        for preset in 0..3 {
            let (_dir, mut s) = state(language, 0);
            let before = stamp(&s);
            open_quick(&mut s);
            for _ in 0..preset {
                press(&mut s, KeyCode::ArrowDown);
            }
            press(&mut s, KeyCode::Enter);
            let doc = s.workspace.file().session.document();
            assert_eq!(doc.text(Some(0), "BPMS").as_deref(), Some("0=240"));
            assert_eq!(
                doc.text(Some(0), "SCROLLS").as_deref(),
                Some(if preset >= 1 { "0=2" } else { "0=1" })
            );
            assert_eq!(
                doc.text(Some(0), "LABELS").as_deref(),
                Some(if preset == 2 { "2=source" } else { "1=old" })
            );
            assert!(
                s.workspace
                    .grid()
                    .get(deadsync_editor::notes::Position {
                        player: 0,
                        lane: 0,
                        row: 0
                    })
                    .is_some()
            );
            assert!(
                s.workspace
                    .grid()
                    .get(deadsync_editor::notes::Position {
                        player: 0,
                        lane: 1,
                        row: 0
                    })
                    .is_none()
            );
            assert_eq!(stamp(&s), before);
            no_handoff(&mut s);
        }
    }
}
