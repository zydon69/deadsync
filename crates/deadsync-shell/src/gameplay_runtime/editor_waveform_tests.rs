//! Real asset service → shell bridge → native waveform, without an output device.
use super::*;
use deadsync_audio_stream::{AudioControl, MusicClock, editor_asset_watch::EditorAssetKind};
use deadsync_theme_simply_love::screens::edition::State;
use std::time::Duration;

fn wav(path: &Path, frames: u32) {
    let bytes = frames * 2;
    let mut data = b"RIFF".to_vec();
    data.extend_from_slice(&(36 + bytes).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&48_000u32.to_le_bytes());
    data.extend_from_slice(&96_000u32.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&bytes.to_le_bytes());
    for _ in 0..frames {
        data.extend_from_slice(&8192i16.to_le_bytes());
    }
    std::fs::write(path, data).unwrap();
}

fn drive(editor: &mut State, audio: &mut AudioControl, clock: &mut MusicClock) {
    audio.poll_editor_assets(clock);
    sync_editor_main_waveform(editor, audio);
    editor.update();
    execute_audio_commands(
        editor.take_audio_commands(),
        audio,
        &crate::gameplay_prewarm::GameplaySfx::default(),
        clock,
    );
}

fn wait_frames(
    editor: &mut State,
    audio: &mut AudioControl,
    clock: &mut MusicClock,
    frames: usize,
) -> (u64, u64) {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        drive(editor, audio, clock);
        if editor.waveform_ready() && editor.waveform_frames() == Some(frames) {
            let [path, _] = editor.editor_asset_paths();
            let prepared = audio
                .editor_prepared_asset_for(
                    editor.editor_asset_owner(),
                    EditorAssetKind::Main,
                    path.as_deref().unwrap(),
                )
                .unwrap();
            let token = (
                prepared.token.registration.registration,
                prepared.token.revision,
            );
            if editor.waveform_source_revision() == Some(token) {
                return token;
            }
        }
        assert!(
            Instant::now() < deadline,
            "current MAIN PCM did not reach the native waveform"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn escaped_relative_music_reaches_managed_waveform_with_multiple_charts() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let media_directory = root.path().join("média été");
    let song_directory = root.path().join("Songs");
    std::fs::create_dir(&media_directory).unwrap();
    std::fs::create_dir(&song_directory).unwrap();
    let music = media_directory.join("main.wav");
    wav(&music, 4800);
    let song = song_directory.join("song.ssc");
    std::fs::write(
        &song,
        "#VERSION:0.83;#MUSIC:..\\/média été\\/main.wav;#BPMS:0=120;\
         #NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:A;#NOTES:1000;\
         #NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:B;#OFFSET:0.75;#BPMS:0=60;#NOTES:0100;",
    )
    .unwrap();
    let original = std::fs::read(&song).unwrap();
    for chart in [0, 1] {
        let mut editor = State::open(&song, chart).unwrap();
        editor
            .configure_audio_preferences(&root.path().join("preferences.json"))
            .unwrap();
        let [main, _] = editor.editor_asset_paths();
        assert_eq!(main.unwrap().canonicalize().unwrap(), music.canonicalize().unwrap());
        let mut audio = AudioControl::without_audio();
        let mut clock = MusicClock::without_audio();
        execute_audio_commands(
            editor.take_audio_commands(),
            &mut audio,
            &crate::gameplay_prewarm::GameplaySfx::default(),
            &mut clock,
        );
        sync_editor_main_waveform(&mut editor, &audio);
        wait_frames(&mut editor, &mut audio, &mut clock, 4800);
        assert_eq!(std::fs::read(&song).unwrap(), original);
    }
}

#[test]
fn real_main_reload_and_reset_reach_waveform_through_shell_without_claiming_device() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let music = root.path().join("main.wav");
    wav(&music, 4800);
    let song = root.path().join("song.ssc");
    std::fs::write(
        &song,
        "#VERSION:0.83;#MUSIC:main.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut editor = State::open(&song, 0).unwrap();
    editor
        .configure_audio_preferences(&root.path().join("preferences.json"))
        .unwrap();
    let mut audio = AudioControl::without_audio();
    let mut clock = MusicClock::without_audio();
    execute_audio_commands(
        editor.take_audio_commands(),
        &mut audio,
        &crate::gameplay_prewarm::GameplaySfx::default(),
        &mut clock,
    );
    // Install managed mode before the first UI update; a missing service result
    // must not cause a separate pathname decoder to become authoritative.
    sync_editor_main_waveform(&mut editor, &audio);
    let first = wait_frames(&mut editor, &mut audio, &mut clock, 4800);
    assert_eq!(audio.editor_audio_status().owner, 0);
    assert!(audio.editor_logical_phase().is_none());

    // The bridge hands over retained PCM, even if the pathname no longer exists.
    std::fs::remove_file(&music).unwrap();
    sync_editor_main_waveform(&mut editor, &audio);
    editor.update();
    assert_eq!(editor.waveform_frames(), Some(4800));
    assert_eq!(editor.waveform_source_revision(), Some(first));

    wav(&music, 9600);
    let second = wait_frames(&mut editor, &mut audio, &mut clock, 9600);
    assert!(
        second > first,
        "LastWrite must publish a new accepted source token"
    );
    assert_eq!(audio.editor_audio_status().owner, 0);

    // Reset for an invalid replacement clears the old waveform through the same bridge.
    std::fs::write(&music, b"invalid audio replacement").unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        drive(&mut editor, &mut audio, &mut clock);
        let [main, _] = editor.editor_asset_paths();
        if let Some(prepared) = audio.editor_prepared_asset_for(
            editor.editor_asset_owner(),
            EditorAssetKind::Main,
            main.as_deref().unwrap(),
        ) {
            let token = (
                prepared.token.registration.registration,
                prepared.token.revision,
            );
            if token > second
                && prepared.source.is_none()
                && !editor.waveform_ready()
                && editor.waveform_frames().is_none()
            {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "MAIN reload reset retained an obsolete waveform"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    for _ in 0..5 {
        drive(&mut editor, &mut audio, &mut clock);
        assert!(editor.waveform_frames().is_none());
    }
    editor.release_host_audio();
    execute_audio_commands(
        editor.take_audio_commands(),
        &mut audio,
        &crate::gameplay_prewarm::GameplaySfx::default(),
        &mut clock,
    );
    assert!(!audio.editor_asset_ready(editor.editor_asset_owner(), EditorAssetKind::Main));
}
