//! CPU-only asynchronous song banner/CD-title previews owned by one durable shell
//! service. No filesystem access in view/render calls. Two role keys are reused
//! across editor visits; the shell retires payloads while preserving their handles.
//! A single worker is retained until service shutdown, where it is joined.
use image::{AnimationDecoder, ImageDecoder, RgbaImage};
use std::{
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, SystemTime},
};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAMES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewRequest {
    pub document_identity: u64,
    pub paths: [Option<PathBuf>; 2],
}
#[derive(Clone, Debug)]
pub struct PreviewView {
    pub status: Status,
    pub key: Arc<str>,
    pub source_size: Option<[u32; 2]>,
    pub has_resident_image: bool,
}
pub(super) fn next_document_identity() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
impl super::State {
    pub fn image_preview_request(&self) -> Option<PreviewRequest> {
        use deadsync_editor::song_text::{Field, Text};
        if !matches!(self.panel, Some(super::Panel::SongText(_))) {
            return None;
        }
        let fields = [Field::Banner, Field::CdTitle];
        let committed = Text::read(self.workspace.file().session.document());
        let directory = self.workspace.file().path().parent()?;
        let paths = fields.map(|field| {
            let value = self
                .song_properties
                .as_ref()
                .map_or_else(|| committed.get(field), |draft| draft.text(field));
            if value.is_empty() {
                None
            } else {
                Some(directory.join(value))
            }
        });
        Some(PreviewRequest {
            document_identity: self.image_preview_identity,
            paths,
        })
    }
    pub fn set_image_preview_views(&mut self, views: [PreviewView; 2]) {
        self.image_preview_views = Some(views);
    }
    pub(super) fn image_preview_actors(
        &self,
        actors: &mut Vec<deadlib_present::actors::Actor>,
        x: f32,
        y: f32,
        width: f32,
    ) {
        use crate::{act, i18n};
        let boxes = [
            (x - width * 0.17, width * 0.59),
            (x + width * 0.33, width * 0.29),
        ];
        for (index, (x, w)) in boxes.into_iter().enumerate() {
            let h = 66.0;
            actors.push(act!(quad: xy(x,y): setsize(w,h): diffuse(0.03,0.04,0.07,1.0): z(8021)));
            if let Some(view) = self.image_preview_views.as_ref().map(|views| &views[index])
                && view.has_resident_image
                && let Some([sw, sh]) = view.source_size
                && sw > 0
                && sh > 0
            {
                let scale = (w / sw as f32).min(h / sh as f32);
                actors.push(act!(sprite(view.key.clone()): xy(x,y): setsize(sw as f32*scale,sh as f32*scale): z(8022)));
            }
            let status = self
                .image_preview_views
                .as_ref()
                .map_or(&Status::Empty, |views| &views[index].status);
            let status_key = match status {
                Status::Empty => Some("ImagePreviewEmpty"),
                Status::Loading => Some("ImagePreviewLoading"),
                Status::Ready => None,
                Status::Missing => Some("ImagePreviewMissing"),
                Status::UnsupportedVideo => Some("ImagePreviewVideo"),
                Status::Failed(_) => Some("ImagePreviewFailed"),
            };
            if let Some(key) = status_key {
                let status = i18n::tr("Edition", key);
                actors.push(
                    act!(quad: xy(x,y): setsize(w,24.0): diffuse(0.03,0.04,0.07,0.9): z(8023)),
                );
                actors.push(act!(text: font("miso"): settext(status): xy(x,y): zoom(0.5): maxwidth(w-8.0): z(8024)));
            }
            let title = i18n::tr_fmt(
                "Edition",
                if index == 0 {
                    "SongBanner"
                } else {
                    "SongCdTitle"
                },
                &[("value", "")],
            );
            actors.push(
                act!(text: font("miso"): settext(title): xy(x,y+h*0.5+10.0): zoom(0.55): z(8022)),
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Banner,
    CdTitle,
}
impl Role {
    pub const ALL: [Self; 2] = [Self::Banner, Self::CdTitle];
    fn index(self) -> usize {
        match self {
            Self::Banner => 0,
            Self::CdTitle => 1,
        }
    }
    fn bounds(self) -> (u32, u32) {
        match self {
            Self::Banner => (418, 164),
            Self::CdTitle => (164, 164),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Empty,
    Loading,
    Ready,
    Missing,
    UnsupportedVideo,
    Failed(String),
}
pub struct View<'a> {
    pub status: &'a Status,
    pub key: &'a str,
    pub source_size: Option<[u32; 2]>,
    pub has_resident_image: bool,
}
#[derive(Clone)]
pub struct Upload {
    pub key: Arc<str>,
    pub image: Arc<RgbaImage>,
    pub source_size: [u32; 2],
}
#[derive(Clone)]
struct Frame {
    image: Arc<RgbaImage>,
    duration: f64,
}
struct Animation {
    frames: Vec<Frame>,
    source_size: [u32; 2],
    duration: f64,
}
impl Animation {
    fn frame(&self, time: Duration) -> usize {
        if self.duration <= 0.0 {
            return 0;
        }
        let mut relative = time.as_secs_f64() % self.duration;
        for (index, frame) in self.frames.iter().enumerate() {
            if relative < frame.duration {
                return index;
            }
            relative -= frame.duration;
        }
        self.frames.len().saturating_sub(1)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    generation: u64,
    path: Option<PathBuf>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}
fn stamp(path: &Path) -> std::io::Result<Stamp> {
    let meta = fs::metadata(path)?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(Stamp {
        length: meta.len(),
        modified: meta.modified().ok(),
        #[cfg(unix)]
        device: meta.dev(),
        #[cfg(unix)]
        inode: meta.ino(),
    })
}
struct ResultItem {
    request: Request,
    animation: Result<Animation, Status>,
}
struct Shared {
    requests: [Request; 2],
    results: [Option<ResultItem>; 2],
    stopped: bool,
}
struct Worker {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    fn new() -> std::io::Result<Self> {
        let shared = Arc::new((
            Mutex::new(Shared {
                requests: std::array::from_fn(|_| Request {
                    generation: 0,
                    path: None,
                }),
                results: [None, None],
                stopped: false,
            }),
            Condvar::new(),
        ));
        let owner = shared.clone();
        let thread = std::thread::Builder::new()
            .name("editor-image-previews".into())
            .spawn(move || worker(owner))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    fn request(&self, index: usize, request: Request) {
        let (lock, wake) = &*self.shared;
        let mut shared = lock.lock().unwrap_or_else(|e| e.into_inner());
        shared.requests[index] = request;
        shared.results[index] = None;
        wake.notify_one();
    }
    fn take(&self, index: usize) -> Option<ResultItem> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .results[index]
            .take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap_or_else(|e| e.into_inner()).stopped = true;
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct Slot {
    key: Arc<str>,
    request: Request,
    status: Status,
    animation: Option<Animation>,
    frame: Option<usize>,
    upload: bool,
}
pub struct Cache {
    worker: Worker,
    slots: [Slot; 2],
    retired: Vec<Arc<str>>,
}
impl Cache {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            worker: Worker::new()?,
            slots: std::array::from_fn(|index| Slot {
                key: Arc::from(match index {
                    0 => "__editor_song_preview::banner",
                    _ => "__editor_song_preview::cdtitle",
                }),
                request: Request {
                    generation: 0,
                    path: None,
                },
                status: Status::Empty,
                animation: None,
                frame: None,
                upload: false,
            }),
            retired: Vec::new(),
        })
    }
    /// Paths must already be resolved against the current simfile directory.
    /// Compare every update: draft cancel/apply, undo/redo and Save As then work
    /// without manual invalidation hooks. None means unload this role.
    pub fn set_paths(&mut self, paths: [Option<PathBuf>; 2]) {
        for (index, path) in paths.into_iter().enumerate() {
            let slot = &mut self.slots[index];
            if slot.request.path == path {
                continue;
            }
            slot.request = Request {
                generation: slot.request.generation.wrapping_add(1),
                path,
            };
            slot.status = if slot.request.path.is_some() {
                Status::Loading
            } else {
                Status::Empty
            };
            if slot.request.path.is_none() {
                slot.animation = None;
                slot.frame = None;
                slot.upload = false;
                if !self.retired.contains(&slot.key) {
                    self.retired.push(slot.key.clone());
                }
            }
            self.worker.request(index, slot.request.clone());
        }
    }
    /// Poll completed CPU work and advance GIF timing using application time.
    /// The original editor animates against application time rather than restart
    /// time, so switching back to the same image preserves its global phase.
    pub fn update(&mut self, time: Duration) {
        for index in 0..2 {
            let slot = &mut self.slots[index];
            if let Some(result) = self.worker.take(index) {
                if result.request == slot.request {
                    match result.animation {
                        Ok(animation) => {
                            slot.animation = Some(animation);
                            slot.status = Status::Ready;
                            slot.frame = None;
                        }
                        Err(status) => {
                            if let Status::Failed(detail) = &status {
                                log::warn!(
                                    "Editor image preview failed for {:?}: {detail}",
                                    slot.request.path
                                );
                            }
                            slot.animation = None;
                            slot.status = status;
                            slot.frame = None;
                            slot.upload = false;
                            if !self.retired.contains(&slot.key) {
                                self.retired.push(slot.key.clone());
                            }
                        }
                    }
                }
            }
            if let Some(animation) = &slot.animation {
                let frame = animation.frame(time);
                if slot.frame != Some(frame) {
                    slot.frame = Some(frame);
                    slot.upload = true;
                }
            }
        }
    }
    pub fn view(&self, role: Role) -> View<'_> {
        let slot = &self.slots[role.index()];
        View {
            status: &slot.status,
            key: &slot.key,
            has_resident_image: slot.animation.is_some(),
            source_size: slot
                .animation
                .as_ref()
                .map(|animation| animation.source_size),
        }
    }
    /// Requeue current frames after the renderer asset store was rebuilt.
    pub fn refresh_uploads(&mut self) {
        for slot in &mut self.slots {
            slot.upload = slot.animation.is_some() && slot.frame.is_some();
        }
    }
    /// Drain retirements FIRST, then uploads, on the renderer owner thread.
    pub fn take_retired_keys(&mut self) -> Vec<Arc<str>> {
        std::mem::take(&mut self.retired)
    }
    pub fn take_uploads(&mut self) -> Vec<Upload> {
        self.slots
            .iter_mut()
            .filter_map(|slot| {
                if !std::mem::take(&mut slot.upload) {
                    return None;
                }
                let animation = slot.animation.as_ref()?;
                Some(Upload {
                    key: slot.key.clone(),
                    image: animation.frames.get(slot.frame?)?.image.clone(),
                    source_size: animation.source_size,
                })
            })
            .collect()
    }
    /// Retire these keys on close even if no further update/drain is performed.
    pub fn texture_keys(&self) -> [Arc<str>; 2] {
        self.slots.each_ref().map(|slot| slot.key.clone())
    }
}

fn current(shared: &Arc<(Mutex<Shared>, Condvar)>, index: usize, request: &Request) -> bool {
    let shared = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    !shared.stopped && shared.requests[index] == *request
}
fn worker(shared: Arc<(Mutex<Shared>, Condvar)>) {
    let mut seen: [Option<(Request, Result<Stamp, std::io::ErrorKind>)>; 2] = [None, None];
    loop {
        let requests = {
            let state = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            if state.stopped {
                return;
            }
            state.requests.clone()
        };
        for (index, request) in requests.into_iter().enumerate() {
            let Some(path) = &request.path else {
                seen[index] = None;
                continue;
            };
            let before = stamp(path).map_err(|error| error.kind());
            if seen[index].as_ref() == Some(&(request.clone(), before.clone())) {
                continue;
            }
            if !current(&shared, index, &request) {
                continue;
            }
            let animation = match &before {
                Err(std::io::ErrorKind::NotFound) => Err(Status::Missing),
                Err(kind) => Err(Status::Failed(format!("Cannot inspect image: {kind:?}"))),
                Ok(stamp) if stamp.length > MAX_SOURCE_BYTES => Err(Status::Failed(
                    "Image file exceeds preview input budget".into(),
                )),
                Ok(_) => decode(path, Role::ALL[index], || {
                    !current(&shared, index, &request)
                }),
            };
            if !current(&shared, index, &request) {
                continue;
            }
            // A file modified during decoding is retried, never published stale.
            if stamp(path).map_err(|error| error.kind()) != before {
                continue;
            }
            seen[index] = Some((request.clone(), before));
            let mut state = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            if !state.stopped && state.requests[index] == request {
                state.results[index] = Some(ResultItem { request, animation });
            }
        }
        let (lock, wake) = &*shared;
        let state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopped {
            return;
        }
        // A path changed while decoding: process it before any timed wait.
        let changed = state
            .requests
            .iter()
            .enumerate()
            .any(|(i, r)| r.path.is_some() && seen[i].as_ref().is_none_or(|(old, _)| old != r));
        if !changed {
            let _guard = wake
                .wait_timeout(state, POLL_INTERVAL)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
}
fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_SOURCE_BYTES);
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits
}
fn check_source(size: (u32, u32)) -> Result<(), Status> {
    if u64::from(size.0) * u64::from(size.1) * 4 > MAX_SOURCE_BYTES {
        return Err(Status::Failed(
            "Decoded image exceeds preview memory budget".into(),
        ));
    }
    Ok(())
}
fn resize(image: RgbaImage, role: Role) -> Arc<RgbaImage> {
    let bounds = role.bounds();
    let image = image::DynamicImage::ImageRgba8(image)
        .thumbnail(bounds.0, bounds.1)
        .into_rgba8();
    Arc::new(image)
}
fn decode(path: &Path, role: Role, cancelled: impl Fn() -> bool) -> Result<Animation, Status> {
    let fail = |error: image::ImageError| Status::Failed(error.to_string());
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if deadsync_editor::music_file::VIDEO_EXTENSIONS.contains(&extension) {
        return Err(Status::UnsupportedVideo);
    }
    let open = || {
        fs::File::open(path)
            .map(BufReader::new)
            .map_err(|error| Status::Failed(error.to_string()))
    };
    if extension == "gif" {
        let mut decoder = image::codecs::gif::GifDecoder::new(open()?).map_err(fail)?;
        decoder.set_limits(limits()).map_err(fail)?;
        let size = decoder.dimensions();
        check_source(size)?;
        let mut frames = Vec::new();
        let mut bytes = 0;
        let mut duration = 0.0;
        for frame in decoder.into_frames() {
            if cancelled() {
                return Err(Status::Failed("Preview load superseded".into()));
            }
            let frame = frame.map_err(fail)?;
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let delay = f64::from(numerator) / f64::from(denominator) / 1000.0;
            let image = resize(frame.into_buffer(), role);
            bytes += image.as_raw().len();
            if bytes > MAX_FRAME_BYTES || frames.len() == MAX_FRAMES {
                return Err(Status::Failed(
                    "Animated image exceeds preview memory budget".into(),
                ));
            }
            duration += delay;
            frames.push(Frame {
                image,
                duration: delay,
            });
        }
        if frames.is_empty() {
            return Err(Status::Failed("Animated image has no frames".into()));
        }
        Ok(Animation {
            frames,
            source_size: [size.0, size.1],
            duration,
        })
    } else {
        let mut reader = image::ImageReader::new(open()?)
            .with_guessed_format()
            .map_err(|error| Status::Failed(error.to_string()))?;
        reader.limits(limits());
        let mut decoder = reader.into_decoder().map_err(fail)?;
        decoder.set_limits(limits()).map_err(fail)?;
        let size = decoder.dimensions();
        check_source(size)?;
        let image = image::DynamicImage::from_decoder(decoder)
            .map_err(fail)?
            .into_rgba8();
        Ok(Animation {
            frames: vec![Frame {
                image: resize(image, role),
                duration: 0.0,
            }],
            source_size: [size.0, size.1],
            duration: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn fixture_animation(color: u8) -> Animation {
        Animation {
            frames: vec![Frame {
                image: Arc::new(RgbaImage::from_pixel(2, 1, image::Rgba([color, 0, 0, 255]))),
                duration: 0.0,
            }],
            source_size: [2, 1],
            duration: 0.0,
        }
    }

    #[test]
    fn replacement_preserves_old_visual_until_atomic_swap_and_clear_cancels_results() {
        let mut cache = Cache::new().unwrap();
        // Synthetic completion avoids racing a worker: no path means no I/O job.
        cache.slots[0].animation = Some(fixture_animation(1));
        cache.slots[0].status = Status::Ready;
        cache.update(Duration::ZERO);
        assert_eq!(cache.take_uploads()[0].image.get_pixel(0, 0).0[0], 1);
        cache.set_paths([Some(PathBuf::from("replacement.png")), None]);
        // Disable real work before injecting the completion deterministically.
        cache.worker.request(
            0,
            Request {
                generation: u64::MAX,
                path: None,
            },
        );
        assert_eq!(cache.view(Role::Banner).status, &Status::Loading);
        assert!(cache.view(Role::Banner).has_resident_image);
        assert!(cache.take_retired_keys().is_empty());
        let request = cache.slots[0].request.clone();
        cache.worker.shared.0.lock().unwrap().results[0] = Some(ResultItem {
            request,
            animation: Ok(fixture_animation(2)),
        });
        cache.update(Duration::ZERO);
        assert_eq!(cache.take_uploads()[0].image.get_pixel(0, 0).0[0], 2);
        assert_eq!(cache.view(Role::Banner).status, &Status::Ready);
        let old = cache.slots[0].request.clone();
        cache.set_paths([None, None]);
        cache.worker.shared.0.lock().unwrap().results[0] = Some(ResultItem {
            request: old,
            animation: Ok(fixture_animation(3)),
        });
        cache.update(Duration::ZERO);
        assert!(!cache.view(Role::Banner).has_resident_image);
        assert_eq!(cache.take_retired_keys().len(), 1);
        assert!(cache.take_uploads().is_empty());
    }

    #[test]
    fn retained_frames_can_be_reuploaded_after_renderer_reset() {
        let mut cache = Cache::new().unwrap();
        cache.slots[0].animation = Some(fixture_animation(1));
        cache.update(Duration::ZERO);
        let first = cache.take_uploads().pop().unwrap();
        assert!(cache.take_uploads().is_empty());
        cache.refresh_uploads();
        let next = cache.take_uploads().pop().unwrap();
        assert!(Arc::ptr_eq(&first.image, &next.image));
        assert_eq!(first.key, next.key);
    }
    fn wait(cache: &mut Cache, role: Role, predicate: impl Fn(&Status) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cache.update(Duration::ZERO);
            if predicate(cache.view(role).status) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out: {:?}",
                cache.view(role).status
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn static_preview_is_bounded_preserves_source_size_and_unloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("art.png");
        image::RgbImage::new(836, 328).save(&path).unwrap();
        let mut cache = Cache::new().unwrap();
        cache.set_paths([Some(path), None]);
        assert_eq!(cache.view(Role::Banner).status, &Status::Loading);
        wait(&mut cache, Role::Banner, |s| *s == Status::Ready);
        assert_eq!(cache.view(Role::Banner).source_size, Some([836, 328]));
        let upload = cache.take_uploads().pop().unwrap();
        assert_eq!(upload.image.dimensions(), (418, 164));
        assert!(cache.take_uploads().is_empty());
        cache.update(Duration::from_secs(20));
        assert!(cache.take_uploads().is_empty());
        cache.set_paths([None, None]);
        assert_eq!(cache.view(Role::Banner).status, &Status::Empty);
        assert!(cache.take_retired_keys().contains(&upload.key));
        assert!(cache.take_uploads().is_empty());
    }
    #[test]
    fn missing_created_modified_and_replaced_paths_are_observed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("art.png");
        let mut cache = Cache::new().unwrap();
        cache.set_paths([Some(path.clone()), None]);
        wait(&mut cache, Role::Banner, |s| *s == Status::Missing);
        image::RgbImage::new(2, 3).save(&path).unwrap();
        wait(&mut cache, Role::Banner, |s| *s == Status::Ready);
        cache.take_uploads();
        image::RgbImage::new(4, 5).save(&path).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache.view(Role::Banner).source_size != Some([4, 5]) {
            cache.update(Duration::ZERO);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(cache.take_uploads().len(), 1);
        fs::remove_file(&path).unwrap();
        wait(&mut cache, Role::Banner, |s| *s == Status::Missing);
        let next = dir.path().join("next.png");
        image::RgbImage::new(7, 8).save(&next).unwrap();
        cache.set_paths([Some(path), None]);
        cache.set_paths([Some(next), None]);
        wait(&mut cache, Role::Banner, |s| *s == Status::Ready);
        assert_eq!(cache.view(Role::Banner).source_size, Some([7, 8]));
    }
    #[test]
    fn gif_frames_follow_reference_global_phase_and_zero_durations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("animated.gif");
        let mut encoder = image::codecs::gif::GifEncoder::new(fs::File::create(&path).unwrap());
        for (color, delay) in [
            ([255, 0, 0, 255], 0),
            ([0, 255, 0, 255], 100),
            ([0, 0, 255, 255], 200),
        ] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    RgbaImage::from_pixel(2, 2, image::Rgba(color)),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(delay, 1),
                ))
                .unwrap();
        }
        drop(encoder);
        let animation = decode(&path, Role::CdTitle, || false).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(animation.frames.len(), 3);
        assert_eq!(animation.frame(Duration::ZERO), 1);
        assert_eq!(animation.frame(Duration::from_millis(100)), 2);
        assert_eq!(animation.frame(Duration::from_millis(350)), 1);
        let zero = Animation {
            frames: vec![Frame {
                image: Arc::new(RgbaImage::new(1, 1)),
                duration: 0.0,
            }],
            source_size: [1, 1],
            duration: 0.0,
        };
        assert_eq!(zero.frame(Duration::from_secs(100)), 0);
    }
    #[test]
    fn invalid_video_and_oversized_inputs_fail_without_fallback_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.png");
        fs::write(&bad, b"invalid").unwrap();
        assert!(matches!(
            decode(&bad, Role::Banner, || false),
            Err(Status::Failed(_))
        ));
        assert!(matches!(
            decode(&dir.path().join("clip.mp4"), Role::Banner, || false),
            Err(Status::UnsupportedVideo)
        ));
        assert!(check_source((10_000, 10_000)).is_err());
    }
    #[test]
    fn stable_role_keys_and_path_switches_discard_stale_decode_results() {
        let mut first = Cache::new().unwrap();
        let second = Cache::new().unwrap();
        assert_eq!(first.texture_keys(), second.texture_keys());
        let old = Request {
            generation: 0,
            path: Some(PathBuf::from("old.png")),
        };
        let newer = Request {
            generation: 1,
            path: Some(PathBuf::from("new.png")),
        };
        first.slots[0].request = newer;
        first.worker.shared.0.lock().unwrap().results[0] = Some(ResultItem {
            request: old,
            animation: Ok(Animation {
                frames: vec![Frame {
                    image: Arc::new(RgbaImage::new(1, 1)),
                    duration: 0.0,
                }],
                source_size: [1, 1],
                duration: 0.0,
            }),
        });
        first.update(Duration::ZERO);
        assert!(first.take_uploads().is_empty());
        assert!(first.view(Role::Banner).source_size.is_none());
    }
}
