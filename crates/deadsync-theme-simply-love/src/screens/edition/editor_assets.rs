//! Register source music roles independently, including when transport is idle
//! or no output device exists. Only the published host executes these commands.
use super::*;
use deadsync_gameplay::{
    EditorAssetKind as Kind, EditorAssetReadiness, GameplayAudioCommand as AudioCommand,
};
#[derive(Clone,PartialEq)]
struct SourceMusicConfiguration {
    music:String,preview:String,
    chart:Option<(deadsync_editor::generation::metadata::Guid,Option<String>)>,
}
#[derive(Default)]
pub(super) struct Runtime {
    source_configuration:Option<SourceMusicConfiguration>,
    pub(super) force_register: bool,
    song_music: Option<String>,
    resource_base: Option<PathBuf>,
    retained_chart: Option<usize>,
    // Explicit MusicChanged with no focused chart loads null in GrooveAuthor.
    chartless_main_cleared: Option<deadsync_editor::chart_order::SessionBinding>,
    raw_paths: Option<[Option<PathBuf>; 2]>,
    registered: Option<[Option<PathBuf>; 2]>,
    revision: u64,
    ready: EditorAssetReadiness,
}
impl State {
    pub fn editor_asset_owner(&self) -> u64 {
        self.audio_session.scope.owner
    }
    pub(super) fn source_editor_asset_paths(&self) -> [Option<PathBuf>; 2] {
        if !self.has_song() {
            return [None, None];
        }
        let file = self.workspace.file();
        let document = file.session.document();
        let directory = file.resource_base();
        let song = document.text(None, "MUSIC").unwrap_or_default();
        let local = self
            .workspace
            .active_chart()
            .and_then(|chart| {
                deadsync_editor::chart_text::local_value(
                    document,
                    chart,
                    deadsync_editor::chart_text::Field::Music,
                )
                .ok()
                .flatten()
            })
            .filter(|name| !name.is_empty());
        let dedicated = deadsync_editor::song_text::resident::effective(document).unwrap_or_default();
        let main = if self.workspace.has_chart() {
            let name = local.unwrap_or(song);
            (!name.is_empty()).then(|| directory.join(name.replace('\\', "/")))
        } else if self.audio_session.assets.chartless_main_cleared.as_ref().is_some_and(|binding|binding.belongs_to(&file.session)) {
            None
        } else if self
            .audio_session
            .assets
            .resource_base
            .as_deref()
            .is_some_and(|old| old != directory)
        {
            // Save As rewrites media references in the same document. The last
            // focused chart still owns MAIN even though its view was closed.
            let name = self
                .audio_session
                .assets
                .retained_chart
                .and_then(|chart| {
                    deadsync_editor::chart_text::local_value(
                        document,
                        chart,
                        deadsync_editor::chart_text::Field::Music,
                    )
                    .ok()
                    .flatten()
                })
                .filter(|name| !name.is_empty())
                .unwrap_or(song);
            (!name.is_empty()).then(|| directory.join(name.replace('\\', "/")))
        } else if self.audio_session.assets.song_music.as_deref() == Some(song.as_str())
            && self.audio_session.assets.registered.is_some()
        {
            self.audio_session.assets.registered.as_ref().unwrap()[0].clone()
        } else {
            (!song.is_empty()).then(|| directory.join(song.replace('\\', "/")))
        };
        [
            main,
            (!dedicated.is_empty()).then(|| directory.join(dedicated.replace('\\', "/"))),
        ]
    }
    pub fn editor_asset_paths(&self) -> [Option<PathBuf>; 2] {
        self.audio_session
            .assets
            .registered
            .clone()
            .unwrap_or([None, None])
    }
    pub(super) fn song_main_music_notification(&mut self) {
        if !self.workspace.has_chart(){
            self.audio_session.assets.chartless_main_cleared=Some(deadsync_editor::chart_order::SessionBinding::capture(&self.workspace.file().session));
        }
    }
    pub(super) fn sync_editor_asset_registration(&mut self) {
        let relocated = self
            .audio_session
            .assets
            .resource_base
            .as_deref()
            .is_some_and(|old| old != self.workspace.file().resource_base());
        if self.workspace.has_chart(){self.audio_session.assets.chartless_main_cleared=None;}
        else if !relocated && self.audio_session.assets.source_configuration.as_ref().is_some_and(|old|old.music!=self.workspace.file().session.document().text(None,"MUSIC").unwrap_or_default()) {
            // A reviewed relocation rebases media paths without representing an
            // explicit Music edit. Explicit typed commits notify before
            // this synchronization, so this guard never suppresses that event.
            self.song_main_music_notification();
        }
        let raw = self.source_editor_asset_paths();
        let document=self.workspace.file().session.document();
        let configuration=SourceMusicConfiguration{
            music:document.text(None,"MUSIC").unwrap_or_default(),
            preview:deadsync_editor::song_text::resident::effective(document).unwrap_or_default(),
            chart:self.workspace.active_chart().map(|chart|(
                self.workspace.file().session.chart_ids()[chart].clone(),
                deadsync_editor::chart_text::local_value(document,chart,deadsync_editor::chart_text::Field::Music).ok().flatten(),
            )),
        };
        let configured_changed=self.audio_session.assets.source_configuration.as_ref().is_some_and(|old|old!=&configuration);
        self.audio_session.assets.source_configuration=Some(configuration);
        if configured_changed && self.is_playing() && !self.chart_transport_is_playing(){
            // Source OnMusicChanged/OnMusicPreviewChanged always stop preview,
            // including a Main edit hidden by a focused chart override. The
            // configuration excludes file bytes and source relocation alone.
            self.stop_audio();
        }
        self.audio_session.assets.resource_base =
            Some(self.workspace.file().resource_base().to_owned());
        if let Some(chart) = self.workspace.active_chart() {
            self.audio_session.assets.retained_chart = Some(chart);
        }
        self.audio_session.assets.song_music = Some(
            self.workspace
                .file()
                .session
                .document()
                .text(None, "MUSIC")
                .unwrap_or_default(),
        );
        if !self.audio_session.assets.force_register
            && self.audio_session.assets.raw_paths.as_ref() == Some(&raw)
        {
            return;
        }
        if !self.workspace.has_chart()
            && self
                .audio_session
                .assets
                .registered
                .as_ref()
                .is_none_or(|old| old[0] != raw[0])
        {
            if !relocated {
                self.audio_session.assets.retained_chart = None;
            }
            self.unfocused_music = raw[0].clone();
            self.clear_unfocused_music_projection();
        }
        self.audio_session.assets.raw_paths = Some(raw.clone());
        // Keep the configured filename: watcher identity must survive symlink retargeting.
        let paths = raw;
        let owner = self.editor_asset_owner();
        if self.audio_session.assets.registered.as_ref() != Some(&paths) {
            self.audio_session.assets.revision = self.audio_session.assets.revision.wrapping_add(1);
        }
        for (index, kind) in [Kind::Main, Kind::DedicatedPreview].into_iter().enumerate() {
            if self.audio_session.assets.force_register
                || self
                    .audio_session
                    .assets
                    .registered
                    .as_ref()
                    .is_none_or(|old| old[index] != paths[index])
            {
                match kind {
                    Kind::Main => self.audio_session.assets.ready.main = false,
                    Kind::DedicatedPreview => {
                        self.audio_session.assets.ready.dedicated_preview = false
                    }
                }
                self.audio_commands.push(AudioCommand::EditorRegisterAsset {
                    owner,
                    kind,
                    path: paths[index].clone(),
                });
            }
        }
        self.audio_session.assets.registered = Some(paths);
        self.audio_session.assets.force_register = false;
    }
    pub fn editor_asset_registration(&self) -> u64 {
        self.audio_session.assets.revision
    }
    pub(super) fn observe_editor_assets(&mut self, assets: EditorAssetReadiness) {
        if assets.owner == self.editor_asset_owner()
            && assets.registration == self.editor_asset_registration()
        {
            self.audio_session.assets.ready = assets;
        }
    }
    pub(super) fn editor_preview_ready(&self, kind: Kind) -> bool {
        let ready = self.audio_session.assets.ready;
        ready.owner == self.editor_asset_owner()
            && ready.registration == self.editor_asset_registration()
            && match kind {
                Kind::Main => ready.main,
                Kind::DedicatedPreview => ready.dedicated_preview,
            }
    }
    pub(super) fn sample_path_is_registered(&self, kind: Kind, path: &std::path::Path) -> bool {
        let index = match kind {
            Kind::Main => 0,
            Kind::DedicatedPreview => 1,
        };
        self.editor_asset_paths()[index]
            .as_deref()
            .is_some_and(|configured| {
                configured == path || configured.canonicalize().ok().as_deref() == Some(path)
            })
    }
    pub(super) fn configured_preview_path(&self, kind: Kind) -> Option<PathBuf> {
        self.editor_asset_paths()[match kind {
            Kind::Main => 0,
            Kind::DedicatedPreview => 1,
        }]
        .clone()
    }
    pub(super) fn sample_asset_kind(draft: &deadsync_editor::song_properties::Draft) -> Kind {
        if draft
            .text(deadsync_editor::song_text::Field::PreviewMusic)
            .is_empty()
        {
            Kind::Main
        } else {
            Kind::DedicatedPreview
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn state() -> (tempfile::TempDir, State) {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.wav"), b"resource").unwrap();
        std::fs::write(root.path().join("preview.wav"), b"resource").unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:main.wav;#PREVIEW:preview.wav;#SAMPLESTART:0;#SAMPLELENGTH:1;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        // This command-level fixture models an acknowledged output lease, not a real device.
        let owner = state.editor_asset_owner();
        let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
        snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);
        state.update_audio(snapshot);
        (root, state)
    }
    #[test]
    fn inactive_registration_is_role_distinct_and_readiness_requires_owner_and_revision() {
        i18n::init_for_tests();
        let (_root, mut s) = state();
        let commands = s.take_audio_commands();
        let owner = s.editor_asset_owner();
        let registration = s.editor_asset_registration();
        let directory = s.workspace.file().resource_base().to_path_buf();
        for (kind, name) in [
            (Kind::Main, "main.wav"),
            (Kind::DedicatedPreview, "preview.wav"),
        ] {
            assert!(commands.iter().any(|c|matches!(c,AudioCommand::EditorRegisterAsset{owner:o,kind:k,path:Some(p)}if *o==owner&&*k==kind&&*p==directory.join(name))));
        }
        assert!(!s.is_playing());
        assert!(!s.editor_preview_ready(Kind::Main));
        s.observe_editor_assets(EditorAssetReadiness {
            owner: owner.wrapping_add(1),
            registration,
            main: true,
            dedicated_preview: true,
        });
        assert!(!s.editor_preview_ready(Kind::Main));
        s.observe_editor_assets(EditorAssetReadiness {
            owner,
            registration: registration.wrapping_sub(1),
            main: true,
            dedicated_preview: true,
        });
        assert!(!s.editor_preview_ready(Kind::Main));
        s.observe_editor_assets(EditorAssetReadiness {
            owner,
            registration,
            main: true,
            dedicated_preview: false,
        });
        assert!(s.editor_preview_ready(Kind::Main));
        assert!(!s.editor_preview_ready(Kind::DedicatedPreview));
        s.command_toggle_sample_audio();
        assert!(!s.is_playing());
        assert!(
            !s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, AudioCommand::EditorPlayPreview { .. }))
        );
        s.observe_editor_assets(EditorAssetReadiness {
            owner,
            registration,
            main: true,
            dedicated_preview: true,
        });
        s.command_toggle_sample_audio();
        assert!(s.is_playing());
        assert!(s.take_audio_commands().iter().any(|c|matches!(c,AudioCommand::EditorPlayPreview{kind:Kind::DedicatedPreview,path,..}if *path==directory.join("preview.wav"))));
    }
    #[cfg(unix)]
    #[test]
    fn configured_symlink_filename_is_retained_for_watching_and_playback() {
        i18n::init_for_tests();
        let (root, mut s) = state();
        s.take_audio_commands();
        std::fs::write(root.path().join("other.wav"), b"other").unwrap();
        std::fs::remove_file(root.path().join("preview.wav")).unwrap();
        std::os::unix::fs::symlink("other.wav", root.path().join("preview.wav")).unwrap();
        let configured = s.workspace.file().resource_base().join("preview.wav");
        assert_eq!(s.editor_asset_paths()[1].as_ref(), Some(&configured));
        assert!(s.sample_path_is_registered(
            Kind::DedicatedPreview,
            &configured.canonicalize().unwrap()
        ));
        let owner = s.editor_asset_owner();
        let registration = s.editor_asset_registration();
        s.observe_editor_assets(EditorAssetReadiness {
            owner,
            registration,
            main: true,
            dedicated_preview: true,
        });
        s.command_toggle_sample_audio();
        assert!(
            s.take_audio_commands()
                .iter()
                .any(|c| matches!(c,AudioCommand::EditorPlayPreview{path,..}if path==&configured))
        );
    }
    #[test]
    fn changed_media_draft_cannot_reuse_committed_ready_cache() {
        i18n::init_for_tests();
        let (root, mut s) = state();
        s.take_audio_commands();
        let owner = s.editor_asset_owner();
        let registration = s.editor_asset_registration();
        s.observe_editor_assets(EditorAssetReadiness {
            owner,
            registration,
            main: true,
            dedicated_preview: true,
        });
        let before = s.workspace.file().session.serialize().unwrap();
        s.toggle_sample_audio_role_result(
            Kind::DedicatedPreview,
            Ok(deadsync_editor::song_properties::SampleAudio {
                path: root.path().join("main.wav").canonicalize().unwrap(),
                start: 0.,
                length: Some(1.),
            }),
        );
        assert!(!s.is_playing());
        assert!(
            !s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, AudioCommand::EditorPlayPreview { .. }))
        );
        assert_eq!(before, s.workspace.file().session.serialize().unwrap());
    }
}

#[cfg(test)]
mod final_view_music_tests {
    use super::*;
    #[test]
    fn final_view_close_retains_chart_override_and_explicit_song_music_edit_clears_registration() {
        crate::i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:a.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:b.wav;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        let owner = state.editor_asset_owner();
        let held = state.editor_asset_paths()[0].clone().unwrap();
        assert!(held.ends_with("b.wav"));
        let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        state.take_audio_commands();
        state.toggle_audio();
        state.take_audio_commands();
        state.bind_decoder_generation(Some(17));
        state.bind_decoder_asset_epoch(Some(owner), Some(29));
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 17,
            asset_epoch: 29,
            music_nanos: 1_250_000_000,
        });
        snapshot.stream_clock.valid_at = std::time::Instant::now();
        state.update_audio(snapshot);
        state.take_audio_commands();
        let time = state.transport_music_time();
        state.chart_views_apply(super::super::chart_views::ViewAction::Close);
        let commands = state.take_audio_commands();
        assert!(!state.workspace.has_chart());
        assert_eq!(state.editor_asset_paths()[0].as_ref(), Some(&held));
        assert_eq!(state.chart_audio_generation(), Some(17));
        assert_eq!(state.playback_asset_epoch(), Some(29));
        assert_eq!(state.transport_music_time(), time);
        assert!(commands.iter().all(|command| !matches!(
            command,
            AudioCommand::EditorRegisterAsset {
                kind: Kind::Main,
                ..
            } | AudioCommand::EditorSwapMusicAsset { .. }
                | AudioCommand::EditorClearMusicAsset { .. }
                | AudioCommand::EditorStartMusic { .. }
                | AudioCommand::EditorStopMusic { .. }
                | AudioCommand::EditorPauseMusic { .. }
        )));
        let mut draft = state.workspace.song_properties().unwrap();
        draft
            .set_text(deadsync_editor::song_text::Field::Music, "c.wav")
            .unwrap();
        state.workspace.apply_song_properties(&draft).unwrap();
        state.sync_editor_asset_registration();
        let commands = state.take_audio_commands();
        // EditorSong.MusicPath notifies even without a focused chart;
        // GetFullPathToFocusedChartMusicFile then returns null, not song MUSIC.
        assert!(state.editor_asset_paths()[0].is_none());
        assert!(state.unfocused_music.is_none());
        assert!(commands.iter().any(|command|matches!(command,AudioCommand::EditorRegisterAsset{owner:actual,kind:Kind::Main,path:None}if *actual==owner)));
        assert_eq!(state.transport_music_time(), time);
        assert_eq!(state.chart_audio_generation(), Some(17));
    }
    #[test]
    fn configured_main_change_stops_dedicated_preview_but_inactive_file_reload_does_not(){
        i18n::init_for_tests();let(root,mut state)=super::tests::state();state.take_audio_commands();let owner=state.editor_asset_owner();let registration=state.editor_asset_registration();state.observe_editor_assets(EditorAssetReadiness{owner,registration,main:true,dedicated_preview:true});state.command_toggle_sample_audio();assert!(state.is_playing());state.take_audio_commands();
        // An inactive cache/file update is not EditorSong.MusicChanged.
        std::fs::write(root.path().join("main.wav"),b"reloaded inactive main").unwrap();state.sync_editor_asset_registration();assert!(state.is_playing());assert_eq!(state.editor_asset_registration(),registration);assert!(!state.take_audio_commands().iter().any(|c|matches!(c,AudioCommand::EditorStopMusic{..}|AudioCommand::EditorPauseMusic{..})));
        let mut draft=state.workspace.song_properties().unwrap();draft.set_text(deadsync_editor::song_text::Field::Music,"new.wav").unwrap();state.workspace.apply_song_properties(&draft).unwrap();state.sync_editor_asset_registration();assert!(!state.is_playing(),"source OnMusicChanged stops preview even if DedicatedPreview supplied its PCM");assert!(state.take_audio_commands().iter().any(|c|matches!(c,AudioCommand::EditorStopMusic{owner:o}if *o==owner)));
    }
}
