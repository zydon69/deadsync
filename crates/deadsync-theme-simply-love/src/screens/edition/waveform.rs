use super::*;
use crate::editor_audio::{Job, LoadError, Loader};
use deadsync_editor::{playback::Clock, waveform::Waveform};
use std::path::PathBuf;

pub(super) struct View {
    enabled: bool,
    loader: Loader,
    job: Option<Job>,
    pending: bool,
    path: Option<PathBuf>,
    key: Option<Key>,
    clock: Option<Clock>,
    data: Option<Waveform>,
}
struct Key {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    offsets: [u64; 2],
    path: PathBuf,
}
impl Default for View {
    fn default() -> Self {
        Self {
            enabled: false,
            loader: Loader::new(256 * 1024 * 1024),
            job: None,
            pending: false,
            path: None,
            key: None,
            clock: None,
            data: None,
        }
    }
}
impl View {
    fn clear(&mut self) {
        self.job = None;
        self.pending = false;
        self.path = None;
        self.key = None;
        self.clock = None;
        self.data = None;
    }
}
impl State {
    pub fn waveform_ready(&self) -> bool {
        self.waveform.enabled && self.waveform.data.is_some()
    }
    pub fn waveform_frames(&self) -> Option<usize> {
        self.waveform.data.as_ref().map(Waveform::frames)
    }
    pub fn waveform_preferences(&self) -> deadsync_editor::waveform_preferences::Preferences {
        self.waveform_preferences
    }
    pub(super) fn apply_waveform_preferences(&mut self) {
        self.waveform.enabled =
            self.waveform_preferences.enabled() && self.waveform_preferences.show();
        if self.waveform.enabled {
            self.refresh_waveform();
        } else {
            self.waveform.clear();
        }
    }
    pub(super) fn toggle_waveform(&mut self) {
        if !self.waveform_preferences.enabled() {
            self.status = i18n::tr("Edition", "WaveformDisabled");
            return;
        }
        self.waveform.enabled = !self.waveform.enabled;
        if self.waveform.enabled {
            self.refresh_waveform();
        } else {
            self.waveform.clear();
            self.status = i18n::tr("Edition", "WaveformHidden");
        }
    }
    pub(super) fn refresh_waveform(&mut self) {
        if !self.waveform.enabled {
            return;
        }
        if !self.workspace.has_chart() {
            self.waveform.clear();
            return;
        }
        let session = &self.workspace.file().session;
        let document = session.document();
        let chart = self.workspace.chart();
        let offsets = self
            .song_properties
            .as_ref()
            .map(|draft| {
                [
                    draft.music_offset().to_bits(),
                    draft.sync_offset().to_bits(),
                ]
            })
            .unwrap_or([0; 2]);
        let music = document
            .text(Some(chart), "MUSIC")
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                self.song_properties.as_ref().map(|draft| {
                    draft
                        .text(deadsync_editor::song_text::Field::Music)
                        .to_owned()
                })
            })
            .or_else(|| document.text(None, "MUSIC"))
            .unwrap_or_default();
        let path = self
            .workspace
            .file()
            .path()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(music.replace('\\', "/"));
        if self.waveform.key.as_ref().is_some_and(|key| {
            Arc::ptr_eq(&key.source, document.source())
                && key.revision == session.revision()
                && key.chart == chart
                && key.offsets == offsets
                && key.path == path
        }) {
            return;
        }
        self.waveform.key = Some(Key {
            source: Arc::clone(document.source()),
            revision: session.revision(),
            chart,
            offsets,
            path: path.clone(),
        });
        let clock = match self.song_properties.as_ref() {
            Some(draft) => draft.clock(session, chart),
            None => Clock::capture(session, chart),
        };
        match clock {
            Ok(clock) => self.waveform.clock = Some(clock),
            Err(error) => {
                self.waveform.clock = None;
                self.waveform.job = None;
                self.waveform.pending = false;
                self.waveform.path = None;
                self.waveform.data = None;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &error.to_string())],
                );
                return;
            }
        }
        if self.waveform.path.as_ref() != Some(&path) {
            self.waveform.job = None;
            self.waveform.data = None;
            self.waveform.path = Some(path);
            self.waveform.pending = true;
            self.start_waveform();
        }
    }
    fn start_waveform(&mut self) {
        let Some(path) = self.waveform.path.clone() else {
            return;
        };
        match self.waveform.loader.start(path) {
            Ok(job) => {
                self.waveform.job = Some(job);
                self.waveform.pending = false;
                self.status = i18n::tr("Edition", "WaveformLoading");
            }
            Err(LoadError::Busy) => {} // The cancelled previous worker still owns its slot.
            Err(error) => {
                self.waveform.pending = false;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &error.to_string())],
                );
            }
        }
    }
    pub(super) fn poll_waveform(&mut self) {
        self.refresh_waveform();
        if self.waveform.pending {
            self.start_waveform();
        }
        let Some(result) = self.waveform.job.as_mut().and_then(Job::poll) else {
            return;
        };
        self.waveform.job = None;
        match result {
            Ok(data) => {
                self.waveform.data = Some(data);
                self.status = i18n::tr("Edition", "WaveformReady");
            }
            Err(error) => {
                self.waveform.data = None;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &error.to_string())],
                );
            }
        }
    }
    pub(super) fn waveform_actors(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        top: f32,
        bottom: f32,
        cursor_y: f32,
    ) {
        if !self.waveform.enabled || self.waveform_preferences.channel_width() == 0.0 {
            return;
        }
        let (Some(data), Some(clock)) = (&self.waveform.data, &self.waveform.clock) else {
            return;
        };
        let rows = ((bottom - top).ceil() as usize).min(4096);
        if rows == 0 {
            return;
        }
        let step = (bottom - top) / rows as f32;
        let channel_width = width / data.channels() as f32;
        let scale = self.waveform_preferences.channel_width() as f32;
        let margin = (1.0 - scale) * 0.5;
        for pixel in 0..rows {
            let y = top + pixel as f32 * step;
            let first = self.workspace.row() as f64 + f64::from(y - cursor_y);
            let last = first + f64::from(step);
            if last <= 0.0 {
                continue;
            }
            let (Ok(a), Ok(b)) = (
                clock.music_time(first.max(0.0).floor() as u64),
                clock.music_time(last.ceil() as u64),
            ) else {
                continue;
            };
            if b <= 0.0 || b < a {
                continue;
            }
            let start = (a.max(0.0) * f64::from(data.sample_rate())).floor() as usize;
            let end = (b.max(0.0) * f64::from(data.sample_rate())).ceil() as usize;
            for channel in 0..data.channels() {
                let Some(envelope) =
                    data.envelope(channel, start, end.max(start.saturating_add(1)))
                else {
                    continue;
                };
                let left = x - width * 0.5 + channel as f32 * channel_width;
                let a = left + channel_width * (margin + scale * f32::from(envelope.min) / 511.0);
                let b = left + channel_width * (margin + scale * f32::from(envelope.max) / 511.0);
                let mid = (a + b) * 0.5;
                actors.push(act!(quad: xy(mid,y+step*0.5): zoomto((b-a).max(0.5),step): diffuse(0.0,0.35,0.164,0.35): z(8001)));
                if end.saturating_sub(start) > 1 {
                    let dense = (envelope.mean_distance / 511.0
                        * self.waveform_preferences.dense_scale())
                    .min(1.0) as f32
                        * (b - a);
                    if dense > 0.0 {
                        actors.push(act!(quad: xy(mid,y+step*0.5): zoomto(dense,step): diffuse(0.0,0.389,0.183,0.5): z(8001)));
                    }
                }
            }
        }
    }
}
