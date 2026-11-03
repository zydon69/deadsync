//! Source passive notifications. They never authorize writes or decoder success.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
static UPDATE_SHOWN: AtomicBool = AtomicBool::new(false);
#[derive(Default)]
pub(super) struct Notifications {
    update: Option<(String, String)>,
    pub(super) about_update_checked: bool,
    pub(super) about_update: Option<(String, String)>,
    pub(super) open_url: Option<String>,
    codec_generation: Option<u64>,
}
fn opus_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|v| v.to_str().is_some_and(|v| v.eq_ignore_ascii_case("opus")))
}
fn source_update_eligible(
    ready: bool,
    suppressed: bool,
    shown: bool,
    newer: bool,
    url: &str,
) -> bool {
    ready && !suppressed && !shown && newer && !url.is_empty()
}
impl State {
    pub fn offer_available_update(&mut self, tag: &str, url: &str) {
        if !source_update_eligible(
            self.ftue_complete(),
            self.general_options.suppress_update,
            UPDATE_SHOWN.load(Ordering::Acquire),
            true,
            url,
        ) || self.panel.is_some()
            || self.text_buffer.is_some()
            || self.is_generating()
            || self.file_modal()
            || self.audio_output_busy()
            || tag.is_empty()
            || !url.starts_with("https://")
            || url.contains(['\r', '\n', '\0'])
        {
            return;
        }
        if UPDATE_SHOWN
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        self.notifications.update = Some((tag.into(), url.into()));
        self.lane_entry_release_input();
        self.panel = Some(Panel::UpdateNotice(0));
    }
    pub fn take_notification_url(&mut self) -> Option<String> {
        self.notifications.open_url.take()
    }
    pub fn observe_decoded_audio(
        &mut self,
        owner: Option<u64>,
        generation: u64,
        opus: bool,
        path: &Path,
    ) {
        if !self.owns_audio_notification(owner)
            || !opus
            || self.notifications.codec_generation == Some(generation)
        {
            return;
        }
        self.notifications.codec_generation = Some(generation);
        if !self.general_options.suppress_opus {
            self.warn_opus(path);
        }
    }
    fn warn_opus(&mut self, path: &Path) {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        let text = i18n::tr_fmt("Edition", "OpusCodecWarning", &[("file", &name)]);
        log::warn!("{text}");
        self.status = text;
    }
    pub(super) fn warn_save_opus(&mut self) {
        if self.general_options.suppress_opus {
            return;
        }
        let document = self.workspace.file().session.document();
        let mut paths = Vec::new();
        if let Some(value) = document.text(None, "MUSIC") {
            paths.push(value);
        }
        if let Ok(preview) = deadsync_editor::song_text::resident::effective(document) {
            if !preview.is_empty() {
                paths.push(preview);
            }
        }
        for index in 0..document.charts().len() {
            if let Some(value) = document.text(Some(index), "MUSIC") {
                paths.push(value);
            }
        }
        // Source checks extension only at save, and does not reject publication.
        for value in paths {
            let path = Path::new(&value);
            if opus_extension(path) {
                self.warn_opus(path);
            }
        }
    }
    pub(super) fn update_notice_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape {
            self.notifications.update = None;
            self.panel = None;
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::UpdateNotice((index + if back { 2 } else { 1 }) % 3));
            return;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            return;
        }
        match index {
            0 => {
                self.notifications.update = None;
                self.panel = None;
            }
            1 => {
                if let Some((_, url)) = &self.notifications.update {
                    self.notifications.open_url = Some(url.clone());
                }
            }
            2 => {
                let mut p = self.general_options.clone();
                p.suppress_update = !p.suppress_update;
                if let Err(error) = self.commit_general_options(p) {
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "GeneralOptionsSaveFailedReason",
                        &[("error", &i18n::editor_error(&error))],
                    );
                }
            }
            _ => {}
        }
    }
    pub(super) fn update_notice_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some((tag, url)) = &self.notifications.update else {
            return;
        };
        let x = screen_width() * 0.5;
        let y = screen_height() * 0.5;
        actors.push(act!(quad:xy(x,y):zoomto((screen_width()-20.).max(1.),(screen_height()-20.).clamp(1.,260.)):diffuse(0.04,0.06,0.1,0.96):z(8080)));
        actors.push(act!(text:font("miso"):settext(i18n::tr_fmt("Edition","EditorUpdateAvailable",&[("version",tag)])):xy(x,y-90.):zoom(0.65):maxwidth(screen_width()-40.):z(8081)));
        // Source's default15px UI font uses the same retained Miso mapping as
        // FTUE (LineSpacing24). The former0.4 yielded only6px of glyph ink.
        // maxwidth is an X-only squeeze in the composer, not a text viewport:
        // retain the original glyph proportions and clip an overlong URL.
        let mut link = act!(text:font("miso"):settext(url.clone()):align(0.,0.5):xy(20.,y-55.):zoom(15./24.):z(8081));
        if let Actor::Text { clip, .. } = &mut link {
            *clip = Some([20., y - 68., (screen_width() - 40.).max(1.), 26.]);
        }
        actors.push(link);
        for (i, key) in [
            "EditorNoticeOkay",
            "EditorNoticeOpenRelease",
            "EditorNoticeSuppressUpdate",
        ]
        .iter()
        .enumerate()
        {
            let start = actors.len();
            let label = if i == 2 {
                format!(
                    "{}: {}",
                    i18n::tr("Edition", key),
                    i18n::tr(
                        "Edition",
                        if self.general_options.suppress_update {
                            "WaveformYes"
                        } else {
                            "WaveformNo"
                        }
                    )
                )
            } else {
                i18n::tr("Edition", key).to_string()
            };
            let color = if i == index {
                [1., 1., 0.4, 1.]
            } else {
                [1.; 4]
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,y+i as f32*32.):zoom(0.55):maxwidth(screen_width()-40.):diffuse(color[0],color[1],color[2],color[3]):z(8081)));
            self.ui_row(actors, start, i == index, 32.);
        }
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    #[test]
    fn update_admission_matches_all_executed_original_source_cases() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/notifications.expected.json"
        )))
        .unwrap();
        for case in cases["Updates"].as_array().unwrap() {
            let ready = case["Ftue"].as_bool().unwrap();
            let suppressed = case["Suppress"].as_bool().unwrap();
            let url = case["Url"].as_str().unwrap();
            let newer = case["Latest"].as_str().unwrap().parse::<f64>().unwrap() > 1.;
            let first = source_update_eligible(ready, suppressed, false, newer, url);
            assert_eq!(usize::from(first), case["First"].as_u64().unwrap() as usize);
            let second = source_update_eligible(ready, suppressed, first, true, url);
            assert_eq!(
                usize::from(first) + usize::from(second),
                case["Second"].as_u64().unwrap() as usize
            );
        }
    }
    #[test]
    fn save_extension_matches_real_fumen_source_cases() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/notifications.expected.json"
        )))
        .unwrap();
        for case in cases["Extensions"].as_array().unwrap() {
            assert_eq!(
                opus_extension(Path::new(case["Path"].as_str().unwrap_or(""))),
                case["Opus"].as_bool().unwrap()
            );
        }
    }
    fn state() -> (tempfile::TempDir, State) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        (root, State::open(&path, 0).unwrap())
    }
    #[test]
    fn codec_warning_requires_published_scope_and_is_once_per_actual_opus_generation() {
        crate::i18n::init_for_tests();
        let (_root, mut state) = state();
        let path = Path::new("actual.ogg");
        state.status = "sentinel".into();
        state.observe_decoded_audio(None, 1, true, path);
        assert_eq!(state.status.as_ref(), "sentinel");
        let owner = state
            .take_audio_commands()
            .into_iter()
            .find_map(|c| {
                if let deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                    Some(owner)
                } else {
                    None
                }
            })
            .unwrap();
        state.observe_decoded_audio(Some(owner + 1), 1, true, path);
        assert_eq!(state.status.as_ref(), "sentinel");
        state.observe_decoded_audio(Some(owner), 1, false, path);
        assert_eq!(state.status.as_ref(), "sentinel");
        state.observe_decoded_audio(Some(owner), 1, true, path);
        assert!(state.status.contains("actual.ogg"));
        let expected = i18n::tr_fmt("Edition", "OpusCodecWarning", &[("file", "actual.ogg")]);
        assert_eq!(state.status, expected);
        state.status = "acknowledged".into();
        state.observe_decoded_audio(Some(owner), 1, true, path);
        assert_eq!(state.status.as_ref(), "acknowledged");
        let mut p = state.general_options.clone();
        p.suppress_opus = true;
        state.configure_general_options(p).unwrap();
        state.observe_decoded_audio(Some(owner), 2, true, path);
        assert_eq!(state.status.as_ref(), "acknowledged");
        let mut p = state.general_options.clone();
        p.suppress_opus = false;
        state.configure_general_options(p).unwrap();
        state.observe_decoded_audio(Some(owner), 2, true, path);
        assert_eq!(state.status.as_ref(), "acknowledged");
        state.observe_decoded_audio(Some(owner), 3, true, path);
        assert_eq!(state.status, expected);
    }
    #[test]
    fn save_extension_warning_does_not_need_playback_and_does_not_modify_document() {
        crate::i18n::init_for_tests();
        let (root, _state) = state();
        let path = root.path().join("opus.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#MUSIC:never-decoded.OPUS;#PREVIEW:preview.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.opus;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        state.warn_save_opus();
        assert!(state.status.contains("chart.opus"));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(!state.workspace.file().session.is_dirty());
        let mut p = state.general_options.clone();
        p.suppress_opus = true;
        state.configure_general_options(p).unwrap();
        state.status = "suppressed".into();
        state.warn_save_opus();
        assert_eq!(state.status.as_ref(), "suppressed");
    }
}

#[cfg(test)]
mod resident_preview_notification_tests {
    use super::*;
    #[test]
    fn save_opus_warning_uses_effective_authored_preview_and_ignores_opaque_sm_raw_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("raw.sm");
            std::fs::write(&path,"#MUSIC:main.wav;#PREVIEW:opaque.opus;#BPMS:0=120;#NOTES:dance-single::Edit:9:0,0,0,0,0:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.general_options.suppress_opus = false;
            let sentinel: Arc<str> = "unchanged".into();
            state.status = Arc::clone(&sentinel);
            state.warn_save_opus();
            assert_eq!(
                state.status, sentinel,
                "ignored SM raw path cannot warn as a decoded/configured asset"
            );
            let mut draft = state.workspace.song_properties().unwrap();
            draft
                .set_text(
                    deadsync_editor::song_text::Field::PreviewMusic,
                    "authored.opus",
                )
                .unwrap();
            state.workspace.apply_song_properties(&draft).unwrap();
            state.status = Arc::clone(&sentinel);
            state.warn_save_opus();
            assert_eq!(
                state.status,
                i18n::tr_fmt("Edition", "OpusCodecWarning", &[("file", "authored.opus")])
            );
            let mut draft = state.workspace.song_properties().unwrap();
            draft
                .set_text(deadsync_editor::song_text::Field::PreviewMusic, "")
                .unwrap();
            state.workspace.apply_song_properties(&draft).unwrap();
            state.status = Arc::clone(&sentinel);
            state.warn_save_opus();
            assert_eq!(
                state.status, sentinel,
                "explicit clear never revives opaque SM PREVIEW"
            );
        }
    }
}

#[cfg(test)]
mod update_url_geometry_tests;
