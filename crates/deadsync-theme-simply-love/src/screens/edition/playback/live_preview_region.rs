//! Publish staged or committed sample bounds without replacing preview playback.
//! Only the main-music preview uses SAMPLESTART/LENGTH. Dedicated preview keeps
//! its complete-source descriptor and actual callback cursor unchanged.
use super::*;
use deadsync_gameplay::EditorAssetKind;

impl State {
    pub(in crate::screens::edition) fn refresh_live_preview_region(&mut self) {
        let owner = self.editor_asset_owner();
        let active_main=self.playback.as_ref().and_then(|playing|playing.sample_descriptor.as_ref())
            .is_some_and(|command|matches!(command,GameplayAudioCommand::EditorPlayPreview{owner:actual,kind:EditorAssetKind::Main,..}if *actual==owner));
        if !active_main {
            return;
        }
        let captured;
        let draft = if let Some(draft) = self.song_properties.as_ref() {
            draft
        } else {
            captured =
                deadsync_editor::song_properties::Draft::capture(&self.workspace.file().session);
            let Ok(draft) = &captured else {
                return;
            };
            draft
        };
        let start = draft.sample_start();
        let raw_length = draft.sample_length();
        if !start.is_finite() || !raw_length.is_finite() {
            return;
        }
        // Invalid imported negative metadata remains untouched. The callback's
        // safe zero-length adaptation is silence, never the obsolete audible loop.
        let length = raw_length.max(0.0);
        let playing = self.playback.as_mut().expect("validated main preview");
        let Some(GameplayAudioCommand::EditorPlayPreview { cut, .. }) =
            playing.sample_descriptor.as_mut()
        else {
            return;
        };
        if cut.start_sec == start && cut.length_sec == length {
            return;
        }
        cut.start_sec = start;
        cut.length_sec = length;
        // A device outage may retain an unsubmitted initial request. Both this
        // request and the frozen-cursor resume must use the latest audition cut.
        if let Some(
            GameplayAudioCommand::EditorPlayPreview {
                owner: actual,
                kind: EditorAssetKind::Main,
                cut,
                ..
            }
            | GameplayAudioCommand::EditorResumePreview {
                owner: actual,
                kind: EditorAssetKind::Main,
                cut,
                ..
            },
        ) = playing.pending_sample.as_mut()
        {
            if *actual == owner {
                cut.start_sec = start;
                cut.length_sec = length;
            }
        }
        self.update_queued_preview_region(owner, start, length);
        self.audio_commands
            .push(GameplayAudioCommand::EditorSetPreviewRegion {
                owner,
                start,
                length,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_platform::input::KeyCode;
    fn wave(path: &std::path::Path) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&38u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48000u32.to_le_bytes());
        bytes.extend_from_slice(&96000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        std::fs::write(path, bytes).unwrap();
    }
    fn fixture(kind: EditorAssetKind) -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        wave(&root.path().join("main.wav"));
        wave(&root.path().join("preview.wav"));
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#MUSIC:main.wav;#PREVIEW:preview.wav;#SAMPLESTART:2;#SAMPLELENGTH:3;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        let owner = state.editor_asset_owner();
        state.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness {
            owner,
            registration: state.editor_asset_registration(),
            main: true,
            dedicated_preview: true,
        });
        let file = if kind == EditorAssetKind::Main {
            "main.wav"
        } else {
            "preview.wav"
        };
        state.toggle_sample_audio_role_result(
            kind,
            Ok(deadsync_editor::song_properties::SampleAudio {
                path: root.path().join(file).canonicalize().unwrap(),
                start: if kind == EditorAssetKind::Main {
                    2.
                } else {
                    0.
                },
                length: if kind == EditorAssetKind::Main {
                    Some(3.)
                } else {
                    None
                },
            }),
        );
        assert!(state.is_playing());
        state.audio_commands.clear();
        (root, state)
    }
    fn cut(state: &State) -> GameplayMusicCut {
        match state
            .playback
            .as_ref()
            .unwrap()
            .sample_descriptor
            .as_ref()
            .unwrap()
        {
            GameplayAudioCommand::EditorPlayPreview { cut, .. } => *cut,
            _ => panic!("preview descriptor"),
        }
    }
    fn region(state: &State, start: f64, length: f64) {
        assert!(state.audio_commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSetPreviewRegion{owner,start:actual,length:len}if *owner==state.editor_asset_owner()&&*actual==start&&*len==length)));
    }
    fn no_restart(state: &State) {
        assert!(state.is_playing());
        assert!(!state.audio_commands.iter().any(|c| matches!(
            c,
            GameplayAudioCommand::StopMusic
                | GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorSeekMusic { .. }
                | GameplayAudioCommand::EditorPlayPreview { .. }
                | GameplayAudioCommand::EditorResumePreview { .. }
        )));
    }
    #[test]
    fn real_sample_keys_audition_draft_back_keeps_it_and_cancel_rolls_back_without_history_or_phase_reset()
     {
        let (_root, mut state) = fixture(EditorAssetKind::Main);
        let original = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        let registration = state.playback.as_ref().unwrap().sample_registration;
        let owner = state.editor_asset_owner();
        let cursor = deadsync_gameplay::EditorPreviewCursor {
            suspended: true,
            rendered: true,
            owner,
            kind: EditorAssetKind::Main,
            registration: 11,
            revision: 12,
            generation: 13,
            asset_epoch: 14,
            next_music_seconds: 2.25,
            rendered_frame: 200,
            source_frame: None,
            sample_rate_hz: 48000,
        };
        state.observe_rendered_preview_cursor(Some(cursor));
        assert_eq!(state.playback.as_ref().unwrap().sample_cursor, Some(cursor));
        state.open_song_properties();
        state.song_sample_key(0, KeyCode::ArrowRight);
        let start = state.song_properties.as_ref().unwrap().sample_start();
        region(&state, start, 3.);
        assert_eq!(cut(&state).start_sec, start);
        no_restart(&state);
        assert_eq!(state.playback.as_ref().unwrap().sample_cursor, Some(cursor));
        assert_eq!(
            state.playback.as_ref().unwrap().sample_registration,
            registration
        );
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            original
        );
        assert_eq!(state.workspace.file().session.revision(), revision);
        state.audio_commands.clear();
        state.song_sample_key(0, KeyCode::Escape);
        assert_eq!(cut(&state).start_sec, start);
        assert!(state.audio_commands.is_empty());
        state.song_properties_key(2, KeyCode::Enter);
        region(&state, 2., 3.);
        assert_eq!(cut(&state).start_sec, 2.);
        assert_eq!(state.playback.as_ref().unwrap().sample_cursor, Some(cursor));
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            original
        );
        no_restart(&state);
        state.audio_commands.clear();
        state.refresh_sample_region();
        assert!(
            state.audio_commands.is_empty(),
            "unchanged frames must not republish"
        );
    }
    #[test]
    fn apply_undo_redo_republish_committed_bounds_and_outage_descriptor_uses_latest_cut() {
        let (_root, mut state) = fixture(EditorAssetKind::Main);
        let original = state.workspace.file().session.serialize().unwrap();
        state.observe_audio_backend_available(false);
        state.audio_commands.clear();
        state.open_song_properties();
        state.song_sample_key(1, KeyCode::ArrowRight);
        let length = state.song_properties.as_ref().unwrap().sample_length();
        region(&state, 2., length);
        state.song_properties_key(1, KeyCode::Enter);
        assert!(state.song_properties.is_none());
        let committed = state.workspace.file().session.serialize().unwrap();
        assert_ne!(committed, original);
        no_restart(&state);
        for (command, expected, bytes) in [
            (Command::Undo, 3., &original),
            (Command::Redo, length, &committed),
        ] {
            state.audio_commands.clear();
            state.command(command);
            region(&state, 2., expected);
            assert_eq!(cut(&state).length_sec, expected);
            assert!(
                matches!(state.playback.as_ref().unwrap().pending_sample.as_ref(),Some(GameplayAudioCommand::EditorPlayPreview{cut,..})if cut.length_sec==expected)
            );
            assert_eq!(&state.workspace.file().session.serialize().unwrap(), bytes);
            no_restart(&state);
        }
    }
    #[test]
    fn dedicated_preview_never_changes_its_whole_source_cut_for_metadata_or_draft_updates() {
        let (_root, mut state) = fixture(EditorAssetKind::DedicatedPreview);
        let original = cut(&state);
        state.open_song_properties();
        state.song_sample_key(0, KeyCode::ArrowRight);
        state.song_sample_key(1, KeyCode::ArrowLeft);
        state.song_properties_key(1, KeyCode::Enter);
        state.command(Command::Undo);
        state.command(Command::Redo);
        assert_eq!(cut(&state), original);
        assert!(
            !state
                .audio_commands
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorSetPreviewRegion { .. }))
        );
        no_restart(&state);
    }
    #[test]
    fn fractional_use_and_go_keep_sample_playing_and_negative_length_silences_only_effective_audio()
    {
        let (_root, mut state) = fixture(EditorAssetKind::Main);
        state.navigate_exact_position(48.75, true);
        state.audio_commands.clear();
        state.open_song_properties();
        let expected = state
            .song_properties
            .as_ref()
            .unwrap()
            .clock(&state.workspace.file().session, 0)
            .unwrap()
            .fractional_music_time(48.75)
            .unwrap();
        state.song_sample_key(0, KeyCode::F2);
        assert_eq!(
            state.song_properties.as_ref().unwrap().sample_start(),
            expected
        );
        region(&state, expected, 3.);
        no_restart(&state);
        state
            .song_properties
            .as_mut()
            .unwrap()
            .set_sample_start(-0.25)
            .unwrap();
        state.audio_commands.clear();
        state.song_sample_key(0, KeyCode::F3);
        let row = state
            .song_properties
            .as_ref()
            .unwrap()
            .clock(&state.workspace.file().session, 0)
            .unwrap()
            .position(-0.25)
            .unwrap();
        assert_eq!(state.playback_position().unwrap().1, row);
        assert!(row < 0.);
        region(&state, -0.25, 3.);
        no_restart(&state);
        state
            .song_properties
            .as_mut()
            .unwrap()
            .set_sample_length(-2.)
            .unwrap();
        state.refresh_sample_region();
        assert_eq!(state.song_properties.as_ref().unwrap().sample_length(), -2.);
        assert_eq!(cut(&state).length_sec, 0.);
        region(&state, -0.25, 0.);
        no_restart(&state);
    }
}
