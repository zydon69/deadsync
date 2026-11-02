use super::*;
use crate::editor_audio::{Job, LoadError, Loader};
use deadsync_editor::{playback::Clock, waveform::Waveform};
use std::path::PathBuf;

pub(super) struct View {
    render_targets:std::cell::RefCell<Vec<deadlib_present::actors::RenderTarget>>,
    draw_charts: std::collections::BTreeSet<usize>,
    enabled: bool,
    generation_enabled: Option<bool>,
    loader: Loader,
    job: Option<Job>,
    pending: bool,
    path: Option<PathBuf>,
    key: Option<Key>,
    clock: Option<Clock>,
    data: Option<Waveform>,
    prepared_managed: bool,
    prepared_token: Option<(u64, u64)>,
    prepared: Option<Arc<deadlib_audio_decode::prepared::PreparedAudioSource>>,
    job_token: Option<(u64, u64)>,
    data_token: Option<(u64, u64)>,
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
            render_targets:Default::default(),
            draw_charts: Default::default(),
            enabled: false,
            generation_enabled: None,
            loader: Loader::new(256 * 1024 * 1024),
            job: None,
            pending: false,
            path: None,
            key: None,
            clock: None,
            data: None,
            prepared_managed: false,
            prepared_token: None,
            prepared: None,
            job_token: None,
            data_token: None,
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
        self.job_token = None;
        self.data_token = None;
    }
}
impl State {
    /// Direct host hook: only the MAIN immutable source feeds its SoundMipMap.
    /// Snapshot metadata stays Copy; the PCM Arc travels separately.
    pub fn observe_editor_main_source(
        &mut self,
        owner: u64,
        registration: u64,
        token: Option<(u64, u64)>,
        source: Option<Arc<deadlib_audio_decode::prepared::PreparedAudioSource>>,
    ) {
        if owner != self.editor_asset_owner() || registration != self.editor_asset_registration() {
            return;
        }
        if token.is_some_and(|new| {
            self.waveform
                .prepared_token
                .is_some_and(|current| new < current)
        }) {
            return;
        }
        let same_source = match (&self.waveform.prepared, &source) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if self.waveform.prepared_managed && self.waveform.prepared_token == token && same_source {
            return;
        }
        if token.is_some_and(|new| {
            self.waveform
                .prepared_token
                .is_none_or(|old| old.0 != new.0)
        }) {
            // EditorSound captures EnableWaveForm in its load state. LastWrite
            // revisions reuse that state; editing the checkbox affects next load.
            self.waveform.generation_enabled = Some(self.waveform_preferences.enabled());
        }
        self.waveform.prepared_managed = true;
        self.waveform.prepared_token = token;
        self.waveform.prepared = source;
        self.waveform.job = None;
        self.waveform.job_token = None;
        self.waveform.data = None;
        self.waveform.data_token = None;
        self.waveform.key = None;
        self.waveform.pending = self.waveform.enabled && self.waveform.prepared.is_some();
        self.refresh_waveform();
    }
    pub fn waveform_source_revision(&self) -> Option<(u64, u64)> {
        self.waveform.data.as_ref().and(self.waveform.data_token)
    }

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
        self.waveform.loader.set_load_parallelism(self.waveform_preferences.loading_parallelism());
        self.waveform.enabled =
            self.waveform_preferences.enabled() && self.waveform_preferences.show();
        self.refresh_waveform();
    }
    pub(super) fn toggle_waveform(&mut self) {
        // Source OnToggleWaveform changes ShowWaveForm directly, including when
        // EnableWaveForm is false. Publish that same leaf without adding Undo.
        let before=self.waveform_preferences;
        let result=(||->Result<(),deadsync_editor::Error>{
            let current=self.capture_application_preferences_file()?;
            let mut owner=Some(current.clone());
            let mut requested=before;requested.set_show(!before.show());
            let preferences=deadsync_editor::audio_preferences::publication::publish(
                &mut owner,Some(&current),requested,|current|{
                    let mut file=current.clone();let mut preferences=file.waveform();
                    if preferences.show()!=before.show(){return Err(deadsync_editor::Error::ExternalChange);}
                    preferences.set_show(!before.show());file.set_waveform(preferences);
                    Ok((preferences,file))
                },
            )?;
            self.waveform_preferences=preferences;
            if let Some(file)=owner{self.publish_application_preferences_owner(file);}
            self.apply_waveform_preferences();
            if !preferences.show(){self.status=i18n::tr("Edition","WaveformHidden");}
            Ok(())
        })();
        if let Err(error)=result{self.status=i18n::editor_error(&error).into();}
    }
    pub(super) fn refresh_waveform(&mut self) {
        self.prepare_waveform_draw_policy();
        let generate = *self
            .waveform
            .generation_enabled
            .get_or_insert(self.waveform_preferences.enabled());
        if !generate {
            return;
        }
        if !self.workspace.has_chart() {
            // MAIN remains loaded after the final view closes. Keep or rebuild
            // its mipmap even though no chart clock is currently drawable.
            self.waveform.clock = None;
            if self.waveform.prepared_managed && self.waveform.prepared.is_some() {
                self.waveform.path = self.editor_asset_paths()[0].clone();
                if self.waveform.job.is_none() && self.waveform.data.is_none() {
                    self.waveform.pending = true;
                    self.start_waveform();
                }
            }
            return;
        }
        let session = &self.workspace.file().session;
        let document = session.document();
        let chart = self.preview_source_chart().unwrap_or_else(||self.workspace.chart());
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
        let path = if self.waveform.prepared_managed {
            let Some(path) = self.editor_asset_paths()[0].clone() else {
                self.waveform.clear();
                return;
            };
            path
        } else {
            self.workspace
                .file()
                .resource_base()
                .join(music.replace('\\', "/"))
        };
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
                self.waveform.data_token = None;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
                return;
            }
        }
        if self.waveform.path.as_ref() != Some(&path) {
            self.waveform.job = None;
            self.waveform.job_token = None;
            self.waveform.data = None;
            self.waveform.data_token = None;
            self.waveform.path = Some(path);
        }
        if self.waveform.job.is_none() && self.waveform.data.is_none() {
            self.waveform.pending =
                !self.waveform.prepared_managed || self.waveform.prepared.is_some();
            if self.waveform.pending {
                self.start_waveform();
            }
        }
    }
    fn start_waveform(&mut self) {
        let Some(path) = self.waveform.path.clone() else {
            return;
        };
        let job = if self.waveform.prepared_managed {
            let Some(source) = self.waveform.prepared.clone() else {
                self.waveform.pending = false;
                return;
            };
            self.waveform.loader.start_prepared(source)
        } else {
            self.waveform.loader.start(path)
        };
        match job {
            Ok(job) => {
                self.waveform.job = Some(job);
                self.waveform.job_token = self.waveform.prepared_token;
                self.waveform.pending = false;
                self.status = i18n::tr("Edition", "WaveformLoading");
            }
            Err(LoadError::Busy) => {} // The cancelled previous worker still owns its slot.
            Err(error) => {
                self.waveform.pending = false;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &crate::i18n::editor_error(&error))],
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
        let token = self.waveform.job_token.take();
        if self.waveform.prepared_managed && token != self.waveform.prepared_token {
            self.waveform.pending = self.waveform.prepared.is_some();
            return;
        }
        match result {
            Ok(data) => {
                self.waveform.data = Some(data);
                self.waveform.data_token = token;
                self.status = i18n::tr("Edition", "WaveformReady");
            }
            Err(error) => {
                self.waveform.data = None;
                self.status = i18n::tr_fmt(
                    "Edition",
                    "WaveformError",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    pub(super) fn waveform_all_view_actors(
        &self,
        actors: &mut Vec<Actor>,
        top: f32,
        bottom: f32,
        projection: &spacing_view::Projection<'_>,
    ) {
        use deadlib_present::actors::{RenderTarget,SpriteSource};
        use deadlib_render_core::{FxaaPass,FxaaQuality,render_target_texture_handle};
        let geometries=self.visible_view_geometry();
        let mut targets=self.waveform.render_targets.borrow_mut();targets.clear();
        let drawable=self.waveform.enabled&&self.waveform.data.is_some()&&self.waveform.clock.is_some();
        if !drawable || !geometries.iter().any(|geometry|self.waveform_draws_chart(geometry.source,geometry.focused)){return;}
        let aa=self.waveform_preferences.antialias();
        let height=screen_height().ceil().max(1.0)as u32;
        let source=render_target_texture_handle(0x45445700);
        {
            let mut children=Vec::new();self.waveform_actors(&mut children,512.0,1024.0,top,bottom,projection);
            for child in &mut children{if let Actor::Sprite{blend,..}=child{*blend=deadlib_render_core::BlendMode::Replace;}}
            targets.push(RenderTarget{texture_handle:source,size:[1024,height],viewport:[1024,height],logical_size:[1024.0,height as f32],float_color:false,alpha:true,depth:false,preserve:false,postprocess:None,children:children.into()});
        }
        for (index,geometry)in geometries.into_iter().enumerate(){
            if !self.waveform_draws_chart(geometry.source,geometry.focused){continue;}
            let width=self.waveform_preferences.render_width(f64::from(geometry.field_width),self.zoom.size_zoom())as u32;
            if width==0{continue;}
            let x=geometry.left+geometry.field_width*0.5;
            let handle=if aa {
                let handle=render_target_texture_handle(0x45445800+index as u64);
                targets.push(RenderTarget{texture_handle:handle,size:[width,height],viewport:[width,height],logical_size:[width as f32,height as f32],float_color:false,alpha:true,depth:false,preserve:false,postprocess:Some(FxaaPass{source:Some(source),quality:FxaaQuality{subpix:self.waveform_preferences.subpix()as f32,edge_threshold:self.waveform_preferences.edge_threshold()as f32,edge_threshold_min:self.waveform_preferences.edge_threshold_min()as f32}}),children:Arc::from([])});
                handle
            }else{source};
                // Use explicit bounds: zoomto on a solid stores a scale relative
                // to 1x1. Changing its source to a render target would multiply
                // those dimensions by the target's native size a second time.
                let mut sprite=act!(quad:xy(x,height as f32*0.5):setsize(width as f32,height as f32):z(8001));
                if let Actor::Sprite{source,blend,..}=&mut sprite{*source=SpriteSource::RenderTarget{handle,size:[if aa{width as f32}else{1024.0},height as f32]};*blend=deadlib_render_core::BlendMode::PremultipliedAlpha;}actors.push(sprite);
        }
    }
    pub(super) fn begin_waveform_frame(&self){self.waveform.render_targets.borrow_mut().clear();}
    /// Offscreen passes built with the same display projection as this frame's actors.
    /// Draining once prevents stale targets on help/chartless frames and retains no document parser in rendering.
    pub fn take_waveform_render_targets(&self)->Vec<deadlib_present::actors::RenderTarget>{std::mem::take(&mut*self.waveform.render_targets.borrow_mut())}
    pub(super) fn prepare_waveform_draw_policy(&mut self) {
        let session = &self.workspace.file().session;
        let focused = self.preview_source_chart().or_else(||self.workspace.active_chart());
        let reference = focused.and_then(|chart| self.view_inputs.get(session, chart).ok());
        let location = self.waveform_preferences.location();
        self.waveform.draw_charts = self.visible_view_geometry().into_iter().filter_map(|geometry| {
            let draw = if geometry.focused {
                location.draws_chart(true, false, 0., 0.)
            } else {
                match (reference, self.view_inputs.get(session, geometry.source).ok()) {
                    (Some(focused), Some(view)) => location.draws_chart(false,
                        view.waveform_music == focused.waveform_music, view.waveform_offset, focused.waveform_offset),
                    _ => location == deadsync_editor::waveform_preferences::DrawLocation::AllCharts,
                }
            };
            draw.then_some(geometry.source)
        }).collect();
    }
    fn waveform_draws_chart(&self, chart: usize, _focused: bool) -> bool {
        self.waveform.draw_charts.contains(&chart)
    }
    pub(super) fn waveform_actors(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        top: f32,
        bottom: f32,
        projection: &spacing_view::Projection<'_>,
    ) {
        if !self.waveform.enabled {
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
        let scale = self.waveform_preferences.effective_channel_width() as f32;
        let margin = (1.0 - scale) * 0.5;
        let palette = self.waveform_preferences.palette();
        for pixel in 0..rows {
            let y = top + pixel as f32 * step;
            let interval = projection
                .music_interval(f64::from(y), f64::from(y + step), clock)
                .and_then(|(a, b)| {
                    if b <= 0.0 || b < a {
                        return None;
                    }
                    let start = (a.max(0.0) * f64::from(data.sample_rate())).floor() as usize;
                    let end = (b.max(0.0) * f64::from(data.sample_rate())).ceil() as usize;
                    Some((start, end.max(start.saturating_add(1))))
                });
            for channel in 0..data.channels() {
                let left = x - width * 0.5 + channel as f32 * channel_width;
                let right = left + channel_width;
                let envelope = interval.and_then(|(a, b)| data.envelope(channel, a, b));
                let Some(envelope) = envelope else {
                    stripe(actors, left, right, y, step, palette.background().rgba());
                    continue;
                };
                if scale==0.0{
                    // Source multiplies ushort min/max by zero, then BlockCopy
                    // includes the endpoint: sparseEnd + 1 - sparseStart == 1.
                    let column=(left+channel_width*0.5).floor();
                    stripe(actors,left,column,y,step,palette.background().rgba());
                    stripe(actors,column,column+1.0,y,step,palette.resolved_sparse().rgba());
                    stripe(actors,column+1.0,right,y,step,palette.background().rgba());
                    continue;
                }
                let a = left + channel_width * (margin + scale * f32::from(envelope.min) / 511.0);
                let b = left + channel_width * (margin + scale * f32::from(envelope.max) / 511.0);
                let sparse_width = (b - a).max(0.5).min(channel_width);
                let mid =
                    ((a + b) * 0.5).clamp(left + sparse_width * 0.5, right - sparse_width * 0.5);
                let a = mid - sparse_width * 0.5;
                let b = mid + sparse_width * 0.5;
                let dense = if interval.is_some_and(|(start, end)| end - start > 1) {
                    (envelope.mean_distance / 511.0 * f64::from(self.waveform_preferences.dense_scale() as f32))
                        .min(1.0) as f32
                        * ((f32::from(envelope.max) - f32::from(envelope.min)) / 511.0
                            * channel_width
                            * scale)
                } else {
                    0.0
                };
                let dense = dense.min(sparse_width);
                let dl = mid - dense * 0.5;
                let dr = mid + dense * 0.5;
                // The original recolouring shader assigns exactly one colour
                // per pixel. Disjoint spans avoid compositing semi-transparent
                // dense over sparse, or sparse over background, a second time.
                stripe(actors, left, a, y, step, palette.background().rgba());
                if dense > 0.0 {
                    stripe(actors, a, dl, y, step, palette.resolved_sparse().rgba());
                    stripe(actors, dl, dr, y, step, palette.dense().rgba());
                    stripe(actors, dr, b, y, step, palette.resolved_sparse().rgba());
                } else {
                    stripe(actors, a, b, y, step, palette.resolved_sparse().rgba());
                }
                stripe(actors, b, right, y, step, palette.background().rgba());
            }
        }
    }
}
fn stripe(actors: &mut Vec<Actor>, left: f32, right: f32, top: f32, height: f32, color: [f32; 4]) {
    let color=color.map(|v|v.clamp(0.0,1.0));
    if right <= left || color == [0.0;4] {
        return;
    }
    actors.push(act!(quad: xy((left+right)*0.5,top+height*0.5): zoomto(right-left,height): diffuse(color[0],color[1],color[2],color[3]): z(8001)));
}

#[cfg(test)]
mod dark_order_tests {
    use super::*;
    #[test]
    fn dark_overlay_follows_waveform_stripes_and_precedes_chart_guides() {
        use deadsync_editor::dark_preferences::{Order, Preferences};
        i18n::init_for_tests();
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("dark.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let mut builder = deadsync_editor::waveform::Builder::new(1, 48000).unwrap();
        builder.push_i16(&vec![16000; 96000]).unwrap();
        state.waveform.enabled = true;
        state.waveform.data = Some(builder.finish().unwrap());
        state.waveform.clock = Some(Clock::capture(&state.workspace.file().session, 0).unwrap());
        let mut dark = Preferences::default();
        dark.set_show(true);
        dark.set_order(Order::AfterWaveForm);
        let color = [1.0, 0.0, 0.25, 0.5];
        dark.set_color(color).unwrap();
        let rendered_dark = dark.rendered_color();
        state.configure_dark_preferences(dark);
        let palette = state.waveform_preferences.palette();
        let colors = [
            palette.background().rgba(),
            palette.resolved_sparse().rgba(),
            palette.dense().rgba(),
        ];
        let mut actors = Vec::new();
        state.actors(&mut actors);
        let overlay = actors
            .iter()
            .position(|a| matches!(a,Actor::Sprite{z:8001,tint,..} if *tint==rendered_dark))
            .unwrap();
        let targets = state.take_waveform_render_targets();
        let source = targets.iter().find(|target| target.postprocess.is_none())
            .expect("real waveform source pass must be rendered");
        assert!(source.children.iter().any(|actor|
            matches!(actor, Actor::Sprite { tint, .. } if colors.contains(tint))));
        let waveform = actors.iter().position(|actor|
            matches!(actor, Actor::Sprite { source: deadlib_present::actors::SpriteSource::RenderTarget { .. }, .. }))
            .expect("waveform pass must be composited into the chart");
        assert!(waveform < overlay);
        assert!(actors.iter().enumerate().any(|(i, a)| i > overlay
            && matches!(a,Actor::Sprite{z:8001,tint,..} if *tint==[0.12,0.16,0.22,1.0])));
    }
}

#[cfg(test)]
mod prepared_source_tests {
    use super::*;
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    fn wav(path: &std::path::Path, frames: u32, value: i16) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&192_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(frames * 4).to_le_bytes());
        for _ in 0..frames * 2 {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }
    fn load(path: &std::path::Path) -> Arc<deadlib_audio_decode::prepared::PreparedAudioSource> {
        deadlib_audio_decode::prepared::PreparedAudioSource::load(
            path,
            deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
            &AtomicBool::new(false),
        )
        .unwrap()
    }
    fn wait(state: &mut State, token: (u64, u64)) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !state.waveform_ready() || state.waveform_source_revision() != Some(token) {
            state.poll_waveform();
            assert!(
                Instant::now() < deadline,
                "prepared waveform did not publish current source revision"
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn waveform_uses_exact_main_pcm_after_path_changed_then_unlinked_and_rejects_stale_tokens() {
        crate::i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let audio = directory.path().join("music.wav");
        wav(&audio, 4800, -10000);
        let original = load(&audio);
        let song = directory.path().join("song.ssc");
        std::fs::write(&song,"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&song, 0).unwrap();
        state.take_audio_commands();
        state.waveform.enabled = true;
        let owner = state.editor_asset_owner();
        let registration = state.editor_asset_registration();
        wav(&audio, 9600, 10000);
        let replacement = load(&audio);
        wav(&audio, 12000, -20000);
        std::fs::remove_file(&audio).unwrap();
        state.observe_editor_main_source(
            owner,
            registration,
            Some((100, 1)),
            Some(Arc::clone(&original)),
        );
        wait(&mut state, (100, 1));
        assert_eq!(state.waveform_frames(), Some(4800));
        let envelope = state
            .waveform
            .data
            .as_ref()
            .unwrap()
            .envelope(0, 0, 4800)
            .unwrap();
        assert!(
            envelope.max < 256,
            "waveform reopened replaced pathname instead of authoritative PCM"
        );
        state.observe_editor_main_source(owner, registration, Some((100, 2)), None);
        assert!(!state.waveform_ready());
        state.observe_editor_main_source(
            owner,
            registration,
            Some((100, 2)),
            Some(Arc::clone(&replacement)),
        );
        state.observe_editor_main_source(owner, registration, Some((100, 1)), Some(original));
        wait(&mut state, (100, 2));
        assert_eq!(state.waveform_frames(), Some(9600));
        assert!(
            state
                .waveform
                .data
                .as_ref()
                .unwrap()
                .envelope(0, 0, 9600)
                .unwrap()
                .min
                > 255
        );
        state.observe_editor_main_source(owner + 1, registration, Some((100, 3)), None);
        state.observe_editor_main_source(owner, registration + 1, Some((100, 3)), None);
        assert_eq!(state.waveform_source_revision(), Some((100, 2)));
        // A dedicated-preview-only registration changes the outer UI revision,
        // while MAIN's source Arc/token remain unchanged and must not rebuild.
        let mut draft = state.workspace.song_properties().unwrap();
        draft
            .set_text(
                deadsync_editor::song_text::Field::PreviewMusic,
                "dedicated.wav",
            )
            .unwrap();
        state.workspace.apply_song_properties(&draft).unwrap();
        state.sync_editor_asset_registration();
        state.observe_editor_main_source(
            owner,
            state.editor_asset_registration(),
            Some((100, 2)),
            Some(replacement),
        );
        assert_eq!(state.waveform_source_revision(), Some((100, 2)));
        assert!(state.waveform.job.is_none());
    }
    #[test]
    fn newer_reset_cancels_inflight_builder_and_no_source_disables_path_fallback() {
        crate::i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let audio = directory.path().join("music.wav");
        wav(&audio, 96_000, -10000);
        let old = load(&audio);
        wav(&audio, 480, 10000);
        let new = load(&audio);
        let song = directory.path().join("song.ssc");
        std::fs::write(&song,"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&song, 0).unwrap();
        state.take_audio_commands();
        state.waveform.enabled = true;
        let owner = state.editor_asset_owner();
        let registration = state.editor_asset_registration();
        state.observe_editor_main_source(owner, registration, Some((200, 1)), Some(old));
        state.observe_editor_main_source(owner, registration, Some((200, 2)), None);
        assert!(!state.waveform_ready());
        state.observe_editor_main_source(owner, registration, Some((200, 2)), Some(new));
        wait(&mut state, (200, 2));
        assert_eq!(state.waveform_frames(), Some(480));
        state.observe_editor_main_source(owner, registration, None, None);
        assert!(!state.waveform_ready());
        for _ in 0..5 {
            state.poll_waveform();
        }
        assert!(state.waveform.job.is_none());
        assert!(state.waveform.data.is_none());
    }
    #[test]
    fn hidden_main_still_builds_and_enable_is_captured_per_registration_including_chartless_reload()
    {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let audio = root.path().join("music.wav");
        wav(&audio, 4800, 10000);
        let pcm = load(&audio);
        let song = root.path().join("song.ssc");
        std::fs::write(&song,"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&song, 0).unwrap();
        let owner = state.editor_asset_owner();
        let registration = state.editor_asset_registration();
        state.waveform_preferences.set_show(false);
        state.apply_waveform_preferences();
        state.observe_editor_main_source(
            owner,
            registration,
            Some((300, 1)),
            Some(Arc::clone(&pcm)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.waveform_source_revision() != Some((300, 1)) {
            state.poll_waveform();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!state.waveform_ready());
        assert_eq!(state.waveform_frames(), Some(4800));
        state.waveform_preferences.set_show(true);
        state.apply_waveform_preferences();
        assert!(state.waveform_ready());
        assert!(state.waveform.job.is_none());
        state.waveform_preferences.set_enabled(false);
        state.apply_waveform_preferences();
        assert!(!state.waveform_ready());
        assert_eq!(state.waveform_frames(), Some(4800));
        // LastWrite keeps the accepted load-state Enable flag, even if the UI changed.
        state.chart_views_apply(super::super::chart_views::ViewAction::Close);
        let registration = state.editor_asset_registration();
        state.observe_editor_main_source(
            owner,
            registration,
            Some((300, 2)),
            Some(Arc::clone(&pcm)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.waveform_source_revision() != Some((300, 2)) {
            state.poll_waveform();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(state.waveform_frames(), Some(4800));
        // A fresh MAIN registration captures disabled generation; Show cannot override it.
        state.observe_editor_main_source(owner, registration, Some((301, 1)), Some(pcm));
        for _ in 0..5 {
            state.poll_waveform();
        }
        assert!(state.waveform.data.is_none());
        assert!(state.waveform.job.is_none());
    }
}

#[cfg(test)]
mod source_waveform_layout_tests {
    use super::*;
    use deadsync_editor::waveform_preferences::DrawLocation;
    #[test]
    fn source_location_uses_exact_music_name_and_one_ulp_offset_and_native_controls_persist() {
        i18n::init_for_tests();
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(2000.,480.));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:a.wav;#OFFSET:0;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:a.wav;#OFFSET:0;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:a.wav;#OFFSET:0;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:./a.wav;#OFFSET:0;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let session=&state.workspace.file().session;
        for chart in 0..3 {
            state.active_views.as_mut().unwrap().set_dedicated(session,chart,true).unwrap();
        }
        state.refresh();
        assert!(state.waveform_draws_chart(0, true));
        assert!(state.waveform_draws_chart(1, false));
        assert!(!state.waveform_draws_chart(2, false));
        state
            .waveform_preferences
            .set_location(DrawLocation::FocusedChart);
        state.prepare_waveform_draw_policy();
        assert!(!state.waveform_draws_chart(1, false));
        state
            .waveform_preferences
            .set_location(DrawLocation::AllCharts);
        state.prepare_waveform_draw_policy();
        assert!(state.waveform_draws_chart(2, false));
        state
            .waveform_preferences
            .set_location(DrawLocation::Unknown(99));
        state.prepare_waveform_draw_policy();
        assert!(!state.waveform_draws_chart(0, true));
        let preferences = root.path().join("preferences.json");
        std::fs::write(&preferences, b"{\"Version\":1}").unwrap();
        state.configure_audio_preferences(&preferences).unwrap();
        state.open_waveform_settings();
        state.waveform_settings_key(5, KeyCode::Enter);
        state.waveform_settings_key(6, KeyCode::Enter);
        state.waveform_settings_key(7, KeyCode::Home);
        state.waveform_settings_key(7, KeyCode::ArrowLeft);
        state.waveform_settings_key(14, KeyCode::Enter);
        let saved = deadsync_editor::audio_preferences::File::open(&preferences)
            .unwrap()
            .waveform();
        assert!(saved.scale_with_zoom());
        assert!(!saved.size_to_chart());
        assert_eq!(saved.location(), DrawLocation::AllCharts);
        state.open_waveform_settings();
        state.waveform_settings_key(13, KeyCode::Enter);
        state.waveform_settings_key(15, KeyCode::Enter);
        assert_eq!(state.waveform_preferences, saved);
        state.open_waveform_settings();
        state.waveform_settings_key(13, KeyCode::Enter);
        state.waveform_settings_key(14, KeyCode::Enter);
        assert_eq!(
            state.waveform_preferences,
            deadsync_editor::waveform_preferences::Preferences::default()
        );
    }
}

#[cfg(test)]mod fxaa_producer_tests{
 use super::*;
 fn assert_destination_bounds(actors: &[Actor]) {
  for actor in actors {
   let Actor::Sprite { size, scale, .. } = actor else { panic!("waveform destination must be a sprite"); };
   let [deadlib_present::actors::SizeSpec::Px(width), deadlib_present::actors::SizeSpec::Px(height)] = *size else { panic!("explicit waveform destination bounds required"); };
   assert!(width > 0.0 && width <= 854.0);
   assert_eq!(height, 480.0);
   assert_eq!(*scale, [1.0, 1.0], "render target intrinsic size must not be applied twice");
  }
 }
 #[test]fn real_waveform_produces_source_resolution_then_destination_fxaa_and_toggle_restores_direct_stripes(){
  i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0));
  let root=tempfile::tempdir().unwrap();let path=root.path().join("fxaa.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();
  let mut builder=deadsync_editor::waveform::Builder::new(1,48000).unwrap();builder.push_i16(&vec![16384;48000]).unwrap();state.waveform.data=Some(builder.finish().unwrap());state.waveform.clock=Some(Clock::capture(&state.workspace.file().session,0).unwrap());state.waveform_preferences.set_subpix(0.75).unwrap();state.apply_waveform_preferences();state.prepare_waveform_draw_policy();
  let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut actors=Vec::new();state.waveform_all_view_actors(&mut actors,105.0,395.0,&projection);let passes=state.take_waveform_render_targets();assert_eq!(passes.len(),2);assert_eq!(passes[0].size,[1024,480]);assert!(passes[0].postprocess.is_none());assert!(!passes[0].children.is_empty());let post=passes[1].postprocess.unwrap();assert_eq!(post.source,Some(passes[0].texture_handle));assert_eq!(post.quality.subpix,0.75);assert_ne!(passes[1].size[0],1024);assert!(matches!(actors.as_slice(),[Actor::Sprite{source:deadlib_present::actors::SpriteSource::RenderTarget{..},..}]));assert!(state.take_waveform_render_targets().is_empty());
  state.waveform_preferences.set_antialias(false);let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut direct=Vec::new();state.waveform_all_view_actors(&mut direct,105.0,395.0,&projection);let off=state.take_waveform_render_targets();assert_eq!(off.len(),1);assert!(off[0].postprocess.is_none());assert_eq!(off[0].size,[1024,480]);assert_eq!(direct.len(),actors.len());assert!(direct.iter().all(|actor|matches!(actor,Actor::Sprite{source:deadlib_present::actors::SpriteSource::RenderTarget{..},blend:deadlib_render_core::BlendMode::PremultipliedAlpha,..})));
  assert_destination_bounds(&actors);
  assert_destination_bounds(&direct);
  state.waveform_all_view_actors(&mut Vec::new(),105.0,395.0,&projection);assert!(!state.waveform.render_targets.borrow().is_empty());state.help=true;state.actors(&mut Vec::new());assert!(state.take_waveform_render_targets().is_empty());
 }
}

#[cfg(test)]mod source_zero_alpha_waveform_tests{use super::*;#[test]fn source_recolour_keeps_nonzero_rgb_even_at_zero_alpha(){let mut actors=Vec::new();stripe(&mut actors,0.0,1.0,0.0,1.0,[0.5,0.25,0.125,0.0]);assert!(matches!(actors.as_slice(),[Actor::Sprite{tint:[0.5,0.25,0.125,0.0],..}]));stripe(&mut actors,0.0,1.0,0.0,1.0,[0.0;4]);assert_eq!(actors.len(),1);}}

#[cfg(test)]mod raw_source_waveform_projection_tests{
 use super::*;
 fn projected(state:&State)->Vec<([f32;2],[f32;2],[f32;4])>{
  let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut actors=Vec::new();state.waveform_actors(&mut actors,512.0,1024.0,105.0,395.0,&projection);actors.into_iter().map(|actor|match actor{Actor::Sprite{offset,size,scale,tint,..}=>{let sizes=std::array::from_fn(|i|match size[i]{deadlib_present::actors::SizeSpec::Px(v)=>if v==0.0{scale[i]}else{v*scale[i]},_=>panic!("waveform span must be pixel sized")});(offset,sizes,tint)},_=>panic!("unexpected waveform actor")}).collect()
 }
 #[test]fn raw_width_is_preserved_but_real_projection_clamps_and_zero_keeps_one_sparse_pixel(){
  i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0));let root=tempfile::tempdir().unwrap();let path=root.path().join("raw.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();
  let mut builder=deadsync_editor::waveform::Builder::new(2,48000).unwrap();let pcm=(0..96000).map(|i|if(i/2)%2==0{30000}else{-30000}).collect::<Vec<i16>>();builder.push_i16(&pcm).unwrap();state.waveform.data=Some(builder.finish().unwrap());state.waveform.clock=Some(Clock::capture(&state.workspace.file().session,0).unwrap());state.apply_waveform_preferences();
  state.waveform_preferences.set_channel_width(0.0).unwrap();let zero=projected(&state);assert!(!zero.is_empty());assert!(zero.iter().all(|(_,size,tint)|size[0]==1.0&&*tint==state.waveform_preferences.palette().resolved_sparse().unorm_rgba()), "zero-width waveform must retain exactly one sparse pixel per channel");assert!(zero.iter().all(|(offset,_,_)|offset[0]==256.5||offset[0]==768.5));
  state.waveform_preferences.set_channel_width(-2.0).unwrap();assert_eq!(state.waveform_preferences.channel_width(),-2.0);assert_eq!(projected(&state),zero);
  state.waveform_preferences.set_channel_width(1.0).unwrap();let full=projected(&state);state.waveform_preferences.set_channel_width(3.0).unwrap();assert_eq!(state.waveform_preferences.channel_width(),3.0);assert_eq!(projected(&state),full);
  state.waveform_preferences.set_dense_scale(0.0).unwrap();let no_dense=projected(&state);state.waveform_preferences.set_dense_scale(-40.0).unwrap();assert_eq!(projected(&state),no_dense);state.waveform_preferences.set_dense_scale(12.0).unwrap();let dense=projected(&state);assert!(dense.iter().any(|(_,_,tint)|*tint==state.waveform_preferences.palette().dense().rgba()));assert_ne!(dense,no_dense);state.waveform_preferences.set_dense_scale(1e20).unwrap();assert_eq!(projected(&state),dense);
 }
}

#[cfg(test)]mod source_palette_projection_tests{
 use super::*;
 #[test]fn unknown_sparse_uses_custom_unorm_in_real_projection_and_unknown_location_draws_nothing(){
  use deadsync_editor::{waveform_palette::{Color,SparseMode},waveform_preferences::DrawLocation};
  i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0));let root=tempfile::tempdir().unwrap();let path=root.path().join("palette.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();let mut builder=deadsync_editor::waveform::Builder::new(2,48000).unwrap();builder.push_i16(&vec![16384;96000]).unwrap();state.waveform.data=Some(builder.finish().unwrap());state.waveform.clock=Some(Clock::capture(&state.workspace.file().session,0).unwrap());state.waveform_preferences.set_channel_width(0.0).unwrap();let mut p=state.waveform_preferences.palette();p.set_mode(SparseMode::Unknown(99));p.set_sparse(Color::new([0.25,2.0,-1.0,0.5]).unwrap());p.set_darken(-4.0).unwrap();state.waveform_preferences.set_palette(p);state.apply_waveform_preferences();state.prepare_waveform_draw_policy();
  let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut actors=Vec::new();state.waveform_actors(&mut actors,512.0,1024.0,105.0,395.0,&projection);assert!(!actors.is_empty());assert!(actors.iter().all(|actor|matches!(actor,Actor::Sprite{tint:[0.25,1.0,0.0,0.5],..})));
  state.open_waveform_settings();state.waveform_colors_key(0,KeyCode::ArrowRight);assert_eq!(state.waveform_preferences_draft.unwrap().palette().mode(),SparseMode::SameAsDenseColor);assert_eq!(state.waveform_preferences.palette().mode(),SparseMode::Unknown(99));state.waveform_settings_key(15,KeyCode::Enter);
  state.waveform_preferences.set_location(DrawLocation::Unknown(99));state.prepare_waveform_draw_policy();let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut actors=Vec::new();state.waveform_all_view_actors(&mut actors,105.0,395.0,&projection);assert!(actors.is_empty());assert!(state.take_waveform_render_targets().is_empty());
 }
}

#[cfg(test)]mod preference_numeric_waveform_consumer_tests{
 use super::*;
 fn projected(state:&State)->Vec<([f32;2],[f32;2])>{let projection=state.display_projection(105.0,395.0,state.chart_viewport().cursor).unwrap();let mut actors=Vec::new();state.waveform_actors(&mut actors,512.0,1024.0,105.0,395.0,&projection);actors.into_iter().map(|actor|match actor{Actor::Sprite{offset,size,scale,..}=>(offset,std::array::from_fn(|i|match size[i]{deadlib_present::actors::SizeSpec::Px(v)=>if v==0.0{scale[i]}else{v*scale[i]},_=>panic!("pixel span")})),_=>panic!("waveform sprite")}).collect()}
 #[test]fn precise_channel_width_entry_changes_real_pcm_stripes_and_global_undo_restores_them(){
  i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0));let root=tempfile::tempdir().unwrap();let path=root.path().join("numeric.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();
  let mut builder=deadsync_editor::waveform::Builder::new(2,48000).unwrap();builder.push_i16(&(0..96000).map(|i|if (i/2)%2==0{30000}else{-30000}).collect::<Vec<_>>()).unwrap();state.waveform.data=Some(builder.finish().unwrap());state.waveform.clock=Some(Clock::capture(&state.workspace.file().session,0).unwrap());state.apply_waveform_preferences();let before=projected(&state);assert!(!before.is_empty());
  state.open_waveform_settings();state.panel=Some(Panel::WaveformSettings(2));state.panel_key(KeyCode::Enter);assert!(state.text_input("0.123456"));state.panel_key(KeyCode::Enter);state.panel=Some(Panel::WaveformSettings(14));state.panel_key(KeyCode::Enter);let after=projected(&state);assert!(!after.is_empty());assert_ne!(before,after);
  state.command(Command::Undo);assert_eq!(projected(&state),before);state.command(Command::Redo);assert_eq!(projected(&state),after);
 }
}

#[cfg(test)]
mod canonical_toggle_tests {
    use super::*;
    // Source ToggleWaveForm is unbound by default. These imported-profile tests
    // deliberately bind F23; native W is tested separately without a source import.
    fn press(state:&mut State){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code:KeyCode::F23,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0});}}
    #[test]
    fn source_waveform_shortcut_updates_canonical_show_with_or_without_generation_and_no_undo(){
        for language in ["en","fr"] {for disk in [false,true] {for enabled in [false,true] {
            i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();
            if disk {let path=root.path().join("prefs.json");std::fs::write(&path,br#"{"Version":1,"Waveform":{"Future":7},"Vendor":{"keep":true}}"#).unwrap();state.configure_audio_preferences(&path).unwrap();}
            let file=state.capture_application_preferences_file().unwrap();let raw=serde_json::json!({"PreferencesWaveForm":{"EnableWaveForm":enabled,"ShowWaveForm":true},"PreferencesKeyBinds":{"ToggleWaveForm":[["F23"]]},"VendorSource":{"keep":[1,9]}}).to_string();let draft=state.workspace.prepare_source_preferences(&file,raw.as_bytes(),deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();
            let revision=state.workspace.file().session.revision();let song=state.workspace.file().session.serialize().unwrap();assert!(state.waveform_preferences.show());
            press(&mut state);assert!(!state.waveform_preferences.show(),"{}",state.status);assert!(!state.waveform.enabled);assert_eq!(state.workspace.file().session.revision(),revision);
            state.apply_waveform_preferences();assert!(!state.waveform.enabled);state.open_waveform_settings();assert!(!state.waveform_preferences_draft.unwrap().show());state.waveform_settings_key(15,KeyCode::Enter);
            let file=state.capture_application_preferences_file().unwrap();assert!(!file.waveform().show());assert!(file.source_archive().is_some());assert!(!file.has_changes());
            if disk {let mut reopened=State::without_song(root.path()).unwrap();reopened.configure_audio_preferences(file.path()).unwrap();assert!(!reopened.waveform_preferences.show());let data:serde_json::Value=serde_json::from_slice(&std::fs::read(file.path()).unwrap()).unwrap();assert_eq!(data["Waveform"]["Future"],7);assert_eq!(data["Vendor"]["keep"],true);}
            press(&mut state);assert!(state.waveform_preferences.show());assert_eq!(state.waveform.enabled,enabled);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.workspace.file().session.serialize().unwrap(),song);
        }}}
    }
    #[test]
    fn original_toggle_transcripts_match_shortcut_and_actual_disk_reopen(){
        let fixture:serde_json::Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/waveform-toggle-source.json"))).unwrap();let cases=fixture["cases"].as_array().unwrap();assert_eq!(cases.len(),16);
        for disk in [false,true] {for stages in cases.chunks_exact(4) {
            i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();if disk {let path=root.path().join("prefs.json");std::fs::write(&path,br#"{"Version":1}"#).unwrap();state.configure_audio_preferences(&path).unwrap();}
            let file=state.capture_application_preferences_file().unwrap();let mut input:serde_json::Value=serde_json::from_str(stages[0]["raw"].as_str().unwrap()).unwrap();input["PreferencesKeyBinds"]=serde_json::json!({"ToggleWaveForm":[["F23"]]});let input=input.to_string();let draft=state.workspace.prepare_source_preferences(&file,input.as_bytes(),deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();
            let revision=state.workspace.file().session.revision();
            for (stage,case) in stages.iter().enumerate().take(3) {if stage>0 {press(&mut state);}assert_eq!(state.waveform_preferences.show(),case["show"].as_bool().unwrap());assert_eq!(state.waveform_preferences.enabled(),case["enabled"].as_bool().unwrap());assert_eq!(state.workspace.file().session.revision(),revision);}
            let file=state.capture_application_preferences_file().unwrap();assert!(!file.has_changes());assert_eq!(file.waveform().show(),stages[3]["show"].as_bool().unwrap());
            if disk {let reopened=deadsync_editor::audio_preferences::File::open(file.path()).unwrap();assert_eq!(reopened.waveform().show(),stages[3]["show"].as_bool().unwrap());assert_eq!(reopened.waveform().enabled(),stages[3]["enabled"].as_bool().unwrap());}
        }}
    }
    #[test]
    fn competing_show_leaf_refuses_shortcut_before_runtime_or_history_publication(){
        i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("prefs.json");std::fs::write(&path,br#"{"Version":1}"#).unwrap();let mut state=State::without_song(root.path()).unwrap();state.configure_audio_preferences(&path).unwrap();let before=state.waveform_preferences;let revision=state.workspace.file().session.revision();let mut external=deadsync_editor::audio_preferences::File::open(&path).unwrap();let mut p=external.waveform();p.set_show(!before.show());external.set_waveform(p);external.save().unwrap();let bytes=std::fs::read(&path).unwrap();
        state.toggle_waveform();assert_eq!(state.waveform_preferences,before);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(std::fs::read(&path).unwrap(),bytes);
    }

    #[test]
    fn native_waveform_and_zoom_shortcuts_are_global_without_a_song_or_chart(){
        fn raw(state:&mut State,code:KeyCode,pressed:bool){state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0});}
        fn key(state:&mut State,code:KeyCode){raw(state,code,true);raw(state,code,false);}
        for language in ["en","fr"] {for chartless_song in [false,true] {
            i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let mut state=if chartless_song{State::from_workspace(deadsync_editor::workspace::Workspace::new_song(root.path(),0.009).unwrap()).unwrap()}else{State::without_song(root.path()).unwrap()};assert!(!state.workspace.has_chart());
            let before=state.waveform_preferences.show();let revision=state.workspace.file().session.revision();let song=state.workspace.file().session.serialize().unwrap();key(&mut state,KeyCode::KeyW);assert_eq!(state.waveform_preferences.show(),!before,"{}",state.status);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.capture_application_preferences_file().unwrap().waveform().show(),!before);
            raw(&mut state,KeyCode::ShiftLeft,true);key(&mut state,KeyCode::KeyW);raw(&mut state,KeyCode::ShiftLeft,false);assert!(matches!(state.panel,Some(Panel::WaveformSettings(0))),"{}",state.status);key(&mut state,KeyCode::Escape);
            raw(&mut state,KeyCode::ShiftLeft,true);key(&mut state,KeyCode::KeyZ);raw(&mut state,KeyCode::ShiftLeft,false);assert!(matches!(state.panel,Some(Panel::ZoomSettings(0))),"{}",state.status);key(&mut state,KeyCode::Escape);assert_eq!(state.workspace.file().session.serialize().unwrap(),song);assert_eq!(state.workspace.file().session.revision(),revision);
        }}
    }

}
#[cfg(test)]
mod shared_float_main_source_tests {
    use super::*;
    use deadsync_editor::generation::profiles::{Kind, Profile, ProfileFile};
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    fn float_wav(path: &std::path::Path, frames: u32, values: [f32; 2]) {
        let size = frames.checked_mul(8).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float, not PCM16.
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&384_000u32.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        for _ in 0..frames {
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        std::fs::write(path, bytes).unwrap();
    }
    fn load(
        path: &std::path::Path,
        expected: [f32; 2],
    ) -> Arc<deadlib_audio_decode::prepared::PreparedAudioSource> {
        let source = deadlib_audio_decode::prepared::PreparedAudioSource::load_editor(
            path,
            deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(source.channels, 2);
        assert_eq!(source.sample_rate_hz, 48_000);
        let pcm = source
            .editor_samples()
            .expect("real editor preparation must retain float PCM");
        for channel in 0..2 {
            assert_eq!(pcm[channel].to_bits(), expected[channel].to_bits());
            assert_ne!(
                pcm[channel].to_bits(),
                (f32::from(source.samples()[channel]) / 32768.).to_bits(),
                "fixture must distinguish compatibility PCM16"
            );
        }
        source
    }
    fn admit(state: &mut State, token: (u64, u64)) {
        // Verify the real hook accepted owner/registration/token before waiting
        // for its Loader permit. A rejected hook must not look like Busy.
        assert_eq!(
            state.waveform.prepared_token, Some(token),
            "MAIN hook refused owner/registration/token: owner={}, registration={}",
            state.editor_asset_owner(), state.editor_asset_registration(),
        );
        assert!(state.waveform.prepared_managed && state.waveform.prepared.is_some(),
            "accepted MAIN token has no managed prepared source");
        assert!(state.editor_asset_paths()[0].is_some(), "MAIN registration has no configured path");
        assert_eq!(state.waveform.path, state.editor_asset_paths()[0],
            "accepted MAIN token did not produce a coherent waveform path: {}",state.status);
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.waveform.job.is_none() {
            assert!(state.waveform.data.is_none());
            state.refresh_waveform();
            // Same retry branch as poll_waveform. A cache hit in refresh alone
            // deliberately does not retry a previous Busy permit. Do not poll a
            // completion here: the test first authenticates the actual job.
            assert!(state.waveform.pending || state.waveform.job.is_some(),
                "MAIN hook accepted token {token:?}, but Loader admission is not pending: {}",state.status);
            if state.waveform.pending { state.start_waveform(); }
            assert!(
                Instant::now() < deadline,
                "MAIN token {token:?} accepted; real Loader admission remained Busy: {}",state.status
            );
            std::thread::yield_now();
        }
        assert_eq!(state.waveform.job_token, Some(token));
    }
    fn wait(state: &mut State, token: (u64, u64)) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // A queued result from a cancelled older worker is never admitted.
            assert!(
                state
                    .waveform_source_revision()
                    .is_none_or(|published| published == token)
            );
            state.poll_waveform();
            if state.waveform_ready() && state.waveform_source_revision() == Some(token) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "float MAIN waveform did not publish: {}",
                state.status
            );
            std::thread::yield_now();
        }
    }
    fn envelope(state: &State, frames: usize, levels: [u16; 2]) {
        assert_eq!(state.waveform_frames(), Some(frames));
        let waveform = state.waveform.data.as_ref().unwrap();
        for (channel, level) in levels.into_iter().enumerate() {
            let actual = waveform.envelope(channel, 0, frames).unwrap();
            assert_eq!(
                (actual.min, actual.max),
                (level, level),
                "channel {channel} must use shared float, not its i16 compatibility view"
            );
        }
    }
    #[test]
    fn real_float_main_hook_worker_publication_profile_history_and_new_revision_cancellation() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("music.wav");
        // Just above packed-envelope quantization thresholds. Their PCM16
        // compatibility values quantize to 256/383, rather than 257/384.
        let first_values = [f32::from_bits(0x3bc06001), f32::from_bits(0x3f00c061)];
        float_wav(&path, 4_800, first_values);
        let first = load(&path, first_values);
        let next_values = [f32::from_bits(0x3e325931), f32::from_bits(0x3f10c865)];
        float_wav(&path, 9_600, next_values);
        let next = load(&path, next_values);
        let song = root.path().join("song.ssc");
        std::fs::write(&song,"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&song, 0).unwrap();
        state.take_audio_commands();
        state.waveform_preferences.set_enabled(true);
        state.waveform_preferences.set_show(true);
        state.waveform.enabled = true;
        let owner = state.editor_asset_owner();
        let registration = state.editor_asset_registration();
        // Neither the builder nor a profile refresh may reopen this pathname.
        std::fs::remove_file(&path).unwrap();
        state.observe_editor_main_source(
            owner,
            registration,
            Some((900, 1)),
            Some(Arc::clone(&first)),
        );
        admit(&mut state, (900, 1));
        wait(&mut state, (900, 1));
        envelope(&state, 4_800, [257, 384]);
        let mut legacy = deadsync_editor::waveform::Builder::new(2, 48_000).unwrap();
        legacy.push_i16(first.samples()).unwrap();
        let legacy = legacy.finish().unwrap();
        assert_eq!(legacy.envelope(0, 0, 4_800).unwrap().max, 256);
        assert_eq!(legacy.envelope(1, 0, 4_800).unwrap().max, 383);

        // Real profile filesystem/history owners exercise State::refresh and
        // Undo without touching MAIN registration or rebuilding the mipmap.
        let configs = root.path().join("configs");
        std::fs::create_dir(&configs).unwrap();
        let profile = Profile::new_editor_default(Kind::Pattern).unwrap();
        let profile_path = state.workspace.create_profile(&configs, &profile).unwrap();
        let captured = ProfileFile::open(&profile_path).unwrap();
        let mut edited = captured.profile().clone();
        edited.rename("Waveform integration profile").unwrap();
        assert!(state.workspace.edit_profile(&captured, &edited).unwrap());
        state.refresh();
        for command in [None, Some(Command::Undo), Some(Command::Redo)] {
            if let Some(command) = command {
                state.command(command);
            }
            assert_eq!(state.waveform_source_revision(), Some((900, 1)));
            assert!(
                state.waveform.job.is_none(),
                "profile history must not reload PCM"
            );
            assert!(Arc::ptr_eq(
                state.waveform.prepared.as_ref().unwrap(),
                &first
            ));
            envelope(&state, 4_800, [257, 384]);
        }

        // Begin another genuine job, leave it unpublished, then invalidate it.
        // It may be computing or already queued: both outcomes must be cancelled
        // by the actual revision reset/Job drop, never by injecting a result.
        let registration = state.editor_asset_registration();
        state.observe_editor_main_source(
            owner,
            registration,
            Some((900, 2)),
            Some(Arc::clone(&first)),
        );
        admit(&mut state, (900, 2));
        state.observe_editor_main_source(owner, registration, Some((900, 3)), None);
        assert!(state.waveform.job.is_none());
        assert!(!state.waveform_ready());
        assert!(state.waveform.data.is_none());
        state.observe_editor_main_source(
            owner,
            registration,
            Some((900, 3)),
            Some(Arc::clone(&next)),
        );
        state.observe_editor_main_source(owner, registration, Some((900, 2)), Some(first));
        wait(&mut state, (900, 3));
        envelope(&state, 9_600, [300, 400]);
        assert!(Arc::ptr_eq(
            state.waveform.prepared.as_ref().unwrap(),
            &next
        ));
        assert!(state.waveform.job.is_none());
    }
}
