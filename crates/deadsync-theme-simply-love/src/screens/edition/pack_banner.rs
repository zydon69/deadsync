//! Lazy pack image inference. Filesystem scans never run in input or render.
use super::{
    State,
    image_previews::{PreviewView, Status},
};
use deadsync_editor::{chart_order::SessionBinding, pack::banner};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};
struct Permit(Arc<AtomicBool>);
impl Permit {
    fn acquire(busy: Arc<AtomicBool>) -> Option<Self> {
        busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self(busy))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn scan_permit() -> Option<Permit> {
    static BUSY: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
    Permit::acquire(
        BUSY.get_or_init(|| Arc::new(AtomicBool::new(false)))
            .clone(),
    )
}
struct Flight {
    cancel: Arc<AtomicBool>,
    receive: Receiver<Result<Option<PathBuf>, banner::Error>>,
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
#[derive(Default)]
pub(super) struct Runtime {
    root: Option<PathBuf>,
    binding: Option<SessionBinding>,
    flight: Option<Flight>,
    result: Option<Result<Option<PathBuf>, banner::Error>>,
}
impl Runtime {
    pub(super) fn failed(&self, root: &Path) -> bool {
        self.root.as_deref() == Some(root) && self.result.as_ref().is_some_and(Result::is_err)
    }
    pub(super) fn ready(&self, root: &Path) -> bool {
        self.root.as_deref() == Some(root) && self.result.as_ref().is_some_and(Result::is_ok)
    }
    pub(super) fn inferred_name(&self, root: &Path) -> Option<&str> {
        if !self.ready(root) {
            return None;
        }
        self.result
            .as_ref()?
            .as_ref()
            .ok()?
            .as_ref()?
            .file_name()?
            .to_str()
    }
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
    #[cfg(test)]
    pub(super) fn settled(root: PathBuf, path: Option<PathBuf>) -> Self {
        Self {
            root: Some(root),
            result: Some(Ok(path)),
            ..Self::default()
        }
    }
}
impl State {
    pub(super) fn pack_banner_ready(&self, root: &Path) -> bool {
        self.pack_banner.ready(root)
            && self
                .pack_banner
                .binding
                .as_ref()
                .is_none_or(|binding| binding.belongs_to(&self.workspace.file().session))
    }
    pub(super) fn update_pack_banner(&mut self) {
        let Some((root, _)) = self.pack_banner_context() else {
            self.pack_banner.reset();
            return;
        };
        if self.pack_banner.root.as_ref() != Some(&root)
            || self
                .pack_banner
                .binding
                .as_ref()
                .is_some_and(|binding| !binding.belongs_to(&self.workspace.file().session))
        {
            self.pack_banner.reset();
            self.pack_banner.root = Some(root.clone());
            self.pack_banner.binding =
                Some(SessionBinding::capture(&self.workspace.file().session));
        }
        if self.pack_banner.result.is_some() {
            return;
        }
        if let Some(flight) = &self.pack_banner.flight {
            match flight.receive.try_recv() {
                Ok(result) => {
                    self.pack_banner.result = Some(result);
                    self.pack_banner.flight = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pack_banner.result = Some(Err(banner::Error::Io(
                        "Pack banner worker disconnected".into(),
                    )));
                    self.pack_banner.flight = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            return;
        }
        let Some(permit) = scan_permit() else {
            return;
        };
        let (cancel, receive) = {
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = cancel.clone();
            let (tx, rx) = mpsc::channel();
            match std::thread::Builder::new()
                .name("editor-pack-banner".into())
                .spawn(move || {
                    let _permit = permit;
                    let result = banner::infer(&root, &flag);
                    let _ = tx.send(result);
                }) {
                Ok(_) => (cancel, rx),
                Err(error) => {
                    self.pack_banner.result = Some(Err(banner::Error::Io(error.to_string())));
                    return;
                }
            }
        };
        self.pack_banner.flight = Some(Flight { cancel, receive });
    }
    pub(super) fn pack_banner_path(&self) -> Option<PathBuf> {
        let (root, explicit) = self.pack_banner_context()?;
        if let Some(explicit) = explicit.filter(|value| !value.is_empty()) {
            return Some(root.join(explicit));
        }
        if !self.pack_banner_ready(&root) {
            return None;
        }
        self.pack_banner.result.as_ref()?.as_ref().ok()?.clone()
    }
    pub(super) fn pack_banner_actors(
        &self,
        actors: &mut Vec<deadlib_present::actors::Actor>,
        x: f32,
        y: f32,
        width: f32,
    ) {
        use crate::{act, i18n};
        let expected = self.pack_banner_path();
        let view = self
            .image_preview_views
            .as_ref()
            .map(|views| &views[0])
            .filter(|view| {
                view.document_identity == Some(self.image_preview_identity) && view.path == expected
            });
        let height = 60.;
        actors
            .push(act!(quad: xy(x,y): setsize(width,height): diffuse(0.03,0.04,0.07,1.0): z(8021)));
        if let Some(PreviewView {
            has_resident_image: true,
            source_size: Some([sw, sh]),
            key,
            ..
        }) = view
            && *sw > 0
            && *sh > 0
        {
            let scale = (width / *sw as f32).min(height / *sh as f32);
            actors.push(act!(sprite(key.clone()): xy(x,y): setsize(*sw as f32*scale,*sh as f32*scale): z(8022)));
        }
        let status = if self
            .pack_banner_context()
            .is_some_and(|(_, explicit)| explicit.is_none_or(|v| v.is_empty()))
        {
            match &self.pack_banner.result {
                None => Some("ImagePreviewLoading"),
                Some(Err(_)) => Some("PackBannerScanFailed"),
                Some(Ok(None)) => Some("ImagePreviewEmpty"),
                _ => None,
            }
        } else {
            None
        };
        let key = status
            .or_else(|| {
                if view.is_none() && expected.is_some() {
                    Some("ImagePreviewLoading")
                } else {
                    None
                }
            })
            .or_else(|| {
                view.and_then(|v| match &v.status {
                    Status::Ready => None,
                    Status::Loading => Some("ImagePreviewLoading"),
                    Status::Missing => Some("ImagePreviewMissing"),
                    Status::UnsupportedVideo => Some("ImagePreviewVideo"),
                    Status::Failed(_) => Some("ImagePreviewFailed"),
                    Status::Empty => Some("ImagePreviewEmpty"),
                })
            });
        if let Some(key) = key {
            actors.push(act!(text: font("miso"): settext(i18n::tr("Edition",key)): xy(x,y): zoom(0.5): maxwidth(width-8.): z(8023)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_platform::input::KeyCode;
    use std::fs;
    fn state() -> (tempfile::TempDir, State, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Song");
        fs::create_dir(&song).unwrap();
        let path = song.join("song.ssc");
        fs::write(&path,"#VERSION:0.83;#TITLE:Fixture;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
        let mut state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        state.open_pack_properties();
        let canonical = root.path().canonicalize().unwrap();
        (root, state, canonical)
    }
    #[test]
    fn cancelled_requests_keep_global_worker_slot_until_worker_returns() {
        let busy = Arc::new(AtomicBool::new(false));
        let permit = Permit::acquire(busy.clone()).unwrap();
        let (cancel, receive) = (
            Arc::new(AtomicBool::new(false)),
            mpsc::channel::<Result<Option<PathBuf>, banner::Error>>().1,
        );
        let flight = Flight {
            cancel: cancel.clone(),
            receive,
        };
        drop(flight);
        assert!(cancel.load(Ordering::Acquire));
        assert!(Permit::acquire(busy.clone()).is_none());
        drop(permit);
        assert!(Permit::acquire(busy).is_some());
    }
    #[test]
    fn failed_scan_allows_explicit_create_without_banner_and_keeps_failure() {
        crate::i18n::init_for_tests();
        let (_root, mut state, dir) = state();
        state.pack_banner = Runtime {
            root: Some(dir.clone()),
            result: Some(Err(banner::Error::Budget)),
            ..Runtime::default()
        };
        state.pack_properties_key(KeyCode::Enter);
        assert_eq!(state.pack_banner_context().unwrap().1.as_deref(), Some(""));
        assert!(state.pack_banner.failed(&dir));
        assert!(!dir.join("Pack.ini").exists());
    }
    #[test]
    fn inferred_banner_has_real_preview_and_becomes_creation_default_en_fr() {
        for language in ["en", "fr"] {
            crate::i18n::init(deadsync_assets::language::load_for_tests(language));
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let (_root, mut state, dir) = state();
            let banner = dir.join("é.png");
            state.pack_banner = Runtime::settled(dir.clone(), Some(banner.clone()));
            assert_eq!(
                state.image_preview_request().unwrap().paths[0],
                Some(banner)
            );
            state.pack_properties_key(KeyCode::Enter);
            assert_eq!(
                state.pack_banner_context().unwrap().1.as_deref(),
                Some("é.png")
            );
            assert!(!dir.join("Pack.ini").exists());
        }
    }
    #[test]
    fn explicit_banner_is_never_overwritten_and_clear_restores_inference() {
        crate::i18n::init_for_tests();
        let (_root, mut state, dir) = state();
        state.pack_banner = Runtime::settled(dir.clone(), Some(dir.join("inferred.png")));
        state.pack_properties_key(KeyCode::Enter);
        state.pack_banner_set_explicit_for_test("explicit.png");
        assert_eq!(state.pack_banner_path(), Some(dir.join("explicit.png")));
        assert_eq!(
            state.image_preview_request().unwrap().paths[0],
            Some(dir.join("explicit.png"))
        );
        state.pack_banner_set_explicit_for_test("");
        assert_eq!(state.pack_banner_path(), Some(dir.join("inferred.png")));
    }
    #[test]
    fn delayed_result_is_discarded_after_panel_or_document_changes() {
        crate::i18n::init_for_tests();
        let (_root, mut state, dir) = state();
        let (tx, receive) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        state.pack_banner = Runtime {
            root: Some(dir.clone()),
            binding: Some(SessionBinding::capture(&state.workspace.file().session)),
            flight: Some(Flight {
                cancel: cancel.clone(),
                receive,
            }),
            result: None,
        };
        state.panel = None;
        state.update_pack_banner();
        assert!(cancel.load(Ordering::Acquire));
        assert!(tx.send(Ok(Some(dir.join("old.png")))).is_err());
        assert!(state.pack_banner_path().is_none());
        // A delayed scan from another Session must also fail before a frame update,
        // including a new document in the same pack directory.
        state.open_pack_properties();
        let stale = SessionBinding::capture(&state.workspace.file().session);
        let source = state
            .workspace
            .file()
            .existing_path()
            .unwrap()
            .to_path_buf();
        let mut replacement = State::open_selected(&source, "dance-single", "Hard", "", 9).unwrap();
        replacement.open_pack_properties();
        replacement.pack_banner = Runtime {
            root: Some(dir.clone()),
            binding: Some(stale),
            result: Some(Ok(Some(dir.join("old.png")))),
            flight: None,
        };
        assert!(replacement.pack_banner_path().is_none());
        assert!(!replacement.pack_banner_ready(&dir));
    }
}
