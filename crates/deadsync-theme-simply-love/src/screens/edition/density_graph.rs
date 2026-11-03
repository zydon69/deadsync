//! Source density graph runtime. The independent RSSP analysis panel stays available.
use super::{State, view_layout, view_render};
use crate::{act, i18n};
use deadlib_present::{actors::Actor, space::screen_width};
use deadsync_editor::{
    density::{
        StreamSettings,
        task::{Report, Task},
    },
    density_preferences::Preferences,
    generation::{job::Job, metadata::Guid},
    history::file_intent::IntentGuard,
};
use std::{cell::RefCell, sync::Arc};
fn same_preview(a: &Option<Arc<()>>, b: &Option<Arc<()>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}
struct Pending {
    preview: Option<Arc<()>>,
    chart: usize,
    identity: Guid,
    token: IntentGuard,
    stream: StreamSettings,
}
impl Pending {
    fn current(&self, s: &State) -> bool {
        self.chart == s.preview_source_chart().unwrap_or(s.workspace.chart())
            && same_preview(
                &self.preview,
                &s.preview.as_ref().map(|_| s.receptor_views.preview_owner()),
            )
            && s.workspace.file().session.chart_ids().get(self.chart) == Some(&self.identity)
            && self.token.is_current(&s.workspace.file().session)
            && self.stream == s.source_density.stream
    }
}
struct Cached {
    width: i32,
    height: i32,
    orientation: deadsync_editor::density::layout::Orientation,
    reverse: bool,
    mesh: super::density_render::Mesh,
    label: Arc<str>,
}
pub(super) struct Runtime {
    pub preferences: Preferences,
    pub stream: StreamSettings,
    job: Option<Job<Report>>,
    pending: Option<Pending>,
    report: Option<Report>,
    report_preview: Option<Arc<()>>,
    report_stream: StreamSettings,
    mesh: RefCell<Option<Cached>>,
    summary: Arc<str>,
    failed: bool,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            preferences: Default::default(),
            stream: Default::default(),
            job: None,
            pending: None,
            report: None,
            report_preview: None,
            report_stream: Default::default(),
            mesh: RefCell::new(None),
            summary: Arc::from(""),
            failed: false,
        }
    }
}
impl State {
    pub fn density_preferences(&self) -> Preferences {
        self.source_density.preferences
    }
    pub fn configure_density_preferences(
        &mut self,
        p: Preferences,
    ) -> Result<(), deadsync_editor::Error> {
        p.validate()?;
        self.source_density.preferences = p;
        self.source_density.mesh.get_mut().take();
        self.refresh_source_density();
        Ok(())
    }
    pub fn source_density_report(&self) -> Option<&Report> {
        self.source_density.report.as_ref().filter(|r| {
            self.source_density.report_stream == self.source_density.stream
                && self.workspace.has_chart()
                && same_preview(
                    &self.source_density.report_preview,
                    &self
                        .preview
                        .as_ref()
                        .map(|_| self.receptor_views.preview_owner()),
                )
                && r.is_current(
                    &self.workspace.file().session,
                    self.preview_source_chart()
                        .unwrap_or(self.workspace.chart()),
                )
        })
    }
    pub fn is_density_loading(&self) -> bool {
        self.source_density.job.is_some()
    }
    pub(super) fn refresh_source_density(&mut self) {
        self.refresh_source_density_with(|session, chart, stream| {
            Task::capture(session, chart, stream).and_then(|task| task.spawn())
        });
    }
    fn refresh_source_density_with(
        &mut self,
        start: impl FnOnce(
            &deadsync_editor::history::Session,
            usize,
            StreamSettings,
        ) -> Result<Job<Report>, deadsync_editor::Error>,
    ) {
        if !self.workspace.has_chart() || !self.source_density.preferences.visible {
            if let Some(j) = self.source_density.job.take() {
                j.cancel();
            }
            self.source_density.pending = None;
            return;
        }
        if self.source_density_report().is_some()
            && self.source_density.report_stream == self.source_density.stream
        {
            return;
        }
        if self
            .source_density
            .pending
            .as_ref()
            .is_some_and(|p| p.current(self))
        {
            return;
        }
        if let Some(j) = self.source_density.job.take() {
            j.cancel();
        }
        self.source_density.pending = None;
        self.source_density.mesh.get_mut().take();
        let session = &self.workspace.file().session;
        let chart = self
            .preview_source_chart()
            .unwrap_or(self.workspace.chart());
        // An error belongs to this exact snapshot too. Retain its binding so cursor
        // refreshes do not serialize the same unsupported snapshot repeatedly.
        self.source_density.pending = Some(Pending {
            preview: self
                .preview
                .as_ref()
                .map(|_| self.receptor_views.preview_owner()),
            chart,
            identity: session.chart_ids()[chart].clone(),
            token: IntentGuard::capture(session),
            stream: self.source_density.stream,
        });
        let prepared = if let Some(preview) = self.preview.as_ref() {
            Task::capture_preview_grid(session, chart, self.source_density.stream, preview.grid())
                .and_then(|task| task.spawn())
        } else {
            start(session, chart, self.source_density.stream)
        };
        match prepared {
            Ok(job) => {
                self.source_density.job = Some(job);
                self.source_density.failed = false;
            }
            Err(_) => self.source_density.failed = true,
        }
    }
    pub(super) fn poll_source_density(&mut self) {
        // Non-file preference history can invalidate a completed report without
        // triggering navigation or a document refresh. Recover on the next host
        // frame; refresh retains current reports and cached capture failures.
        if self.source_density.job.is_none() {
            self.refresh_source_density();
        }
        let result = self.source_density.job.as_mut().and_then(|j| j.poll());
        let Some(result) = result else {
            return;
        };
        self.source_density.job = None;
        let pending = self.source_density.pending.take();
        let current = pending.as_ref().is_some_and(|p| p.current(self));
        if !current {
            self.refresh_source_density();
            return;
        }
        match result {
            Ok(report)
                if report.is_current(
                    &self.workspace.file().session,
                    self.preview_source_chart()
                        .unwrap_or(self.workspace.chart()),
                ) =>
            {
                self.source_density.report_preview =
                    pending.as_ref().and_then(|p| p.preview.clone());
                self.source_density.report_stream = self.source_density.stream;
                let summary = report
                    .series
                    .breakdown
                    .as_deref()
                    .map(|s| {
                        let mut text = s.chars().take(128).collect::<String>();
                        if s.chars().count() > 128 {
                            text.push_str(" ...");
                        }
                        Arc::<str>::from(text)
                    })
                    .unwrap_or_else(|| i18n::tr("Edition", "DensityNoStreams"));
                self.source_density.summary = summary;
                self.source_density.report = Some(report);
                self.source_density.mesh.get_mut().take();
                self.source_density.failed = false;
            }
            _ => {
                self.source_density.failed = true;
                self.source_density.pending = pending;
            }
        }
    }
    pub(super) fn source_density_bounds(
        &self,
        geometry: view_layout::Geometry,
        viewport: view_render::Viewport,
    ) -> Option<deadsync_editor::density::layout::Bounds> {
        use deadsync_editor::density::layout::{self, Focus, Input, Rect};
        use deadsync_editor::density_preferences::Position;
        let p = self.source_density.preferences;
        let vertical = !matches!(
            p.position,
            Position::TopOfFocusedChart | Position::BottomOfFocusedChart
        );
        let overview = self.overview_vertical_layout();
        // Native labels reserve space above the shared sidebar bodies. Define
        // its ChartArea/header anchors once; then apply the source formula.
        // The default width offset (-10) leaves both native bodies aligned.
        let default_width_offset = Preferences::default().width_offsets[p.position.index()];
        let header = if vertical {
            overview.top.wrapping_add(default_width_offset)
        } else {
            viewport.top as i32
        };
        let height = if vertical {
            overview.bottom.wrapping_sub(default_width_offset)
        } else {
            viewport.bottom as i32
        };
        let focus = self.focused_minimap_mount_layout().map(|layout| Focus {
            lanes_start: geometry.left as i32,
            lanes_end: (geometry.left + geometry.field_width) as i32,
            misc_end: layout.without_scale as i32,
            misc_scaled_end: layout.with_scale as i32,
        });
        layout::bounds(
            p,
            Input {
                chart_area: Rect {
                    x: 0,
                    y: 0,
                    width: screen_width().max(0.0) as i32,
                    height,
                },
                header,
                focus,
                dpi_scale: 1.0,
            },
        )
        .ok()
    }
    pub(super) fn source_density_actors(
        &self,
        actors: &mut Vec<Actor>,
        geometry: view_layout::Geometry,
        viewport: view_render::Viewport,
    ) {
        if !self.workspace.has_chart() || !self.source_density.preferences.visible {
            return;
        }
        use deadsync_editor::density::layout::Orientation;
        let Some(bounds) = self.source_density_bounds(geometry, viewport) else {
            return;
        };
        let r = bounds.rect;
        if r.width < 0 || r.height < 0 {
            return;
        }
        let background = deadsync_editor::density::colors::rgba(
            deadsync_editor::density::colors::quantize(self.source_density.preferences.background),
        );
        actors.push(act!(quad:xy(r.x as f32+r.width as f32*0.5,r.y as f32+r.height as f32*0.5):setsize(r.width as f32,r.height as f32):diffuse(background[0],background[1],background[2],background[3]):z(8004)));
        for (x, y, w, h) in [
            (
                r.x as f32 + 0.5,
                r.y as f32 + r.height as f32 * 0.5,
                1.0,
                r.height as f32,
            ),
            (
                r.x as f32 + r.width as f32 - 0.5,
                r.y as f32 + r.height as f32 * 0.5,
                1.0,
                r.height as f32,
            ),
            (
                r.x as f32 + r.width as f32 * 0.5,
                r.y as f32 + 0.5,
                r.width as f32,
                1.0,
            ),
            (
                r.x as f32 + r.width as f32 * 0.5,
                r.y as f32 + r.height as f32 - 0.5,
                r.width as f32,
                1.0,
            ),
        ] {
            actors.push(act!(quad:xy(x,y):zoomto(w,h):diffuse(1.0,1.0,1.0,1.0):z(8007)));
        }
        // Source still draws background/rim for dimensions0..2, but no measures.
        if r.width <= 2 || r.height <= 2 {
            return;
        }
        let Some(report) = self.source_density_report() else {
            let label = i18n::tr(
                "Edition",
                if self.source_density.failed {
                    "DensityUnavailable"
                } else {
                    "DensityLoading"
                },
            );
            if bounds.orientation == Orientation::Vertical {
                self.overview_label(actors, r.x as f32, r.width as f32, 0, label);
            } else {
                actors.push(act!(text:font("miso"):settext(label):align(0.0,0.0):xy(r.x as f32+3.0,r.y as f32+3.0):zoom(0.42):maxwidth((r.width-6).max(1) as f32):z(8006)));
            }
            return;
        };
        let reverse = self.reverse_display && bounds.orientation == Orientation::Vertical;
        let mut cached = self.source_density.mesh.borrow_mut();
        if !cached.as_ref().is_some_and(|m| {
            m.width == r.width
                && m.height == r.height
                && m.orientation == bounds.orientation
                && m.reverse == reverse
        }) {
            let mesh = super::density_render::Mesh::build(
                if self.source_density.preferences.accumulation
                    == deadsync_editor::density_preferences::Accumulation::Step
                {
                    &report.step_plot
                } else {
                    &report.row_plot
                },
                report.series.final_time,
                self.source_density.preferences,
                r.width,
                r.height,
                bounds.orientation,
                reverse,
            );
            let label = i18n::tr_fmt(
                "Edition",
                if mesh.sampled {
                    "DensityPeakSampled"
                } else {
                    "DensityPeak"
                },
                &[("value", &format!("{:.2}", mesh.peak))],
            );
            *cached = Some(Cached {
                width: r.width,
                height: r.height,
                orientation: bounds.orientation,
                reverse,
                mesh,
                label,
            });
        }
        let mesh = &cached.as_ref().unwrap().mesh;
        actors.push(mesh.actor(r.x as f32, r.y as f32, r.width as f32, r.height as f32));
        let (label_x, label_y, rotation, max_width) = if bounds.orientation == Orientation::Vertical
        {
            (
                r.x as f32 + r.width as f32 - 4.0,
                r.y as f32 + 3.0,
                90.0,
                (r.height - 6).max(1),
            )
        } else {
            (
                r.x as f32 + 3.0,
                r.y as f32 + 3.0,
                0.0,
                (r.width - 6).max(1),
            )
        };
        if bounds.orientation == Orientation::Vertical {
            self.overview_label(
                actors,
                r.x as f32,
                r.width as f32,
                1,
                Arc::clone(&cached.as_ref().unwrap().label),
            );
        } else {
            actors.push(act!(text:font("miso"):settext(Arc::clone(&cached.as_ref().unwrap().label)):align(0.0,0.0):xy(label_x,label_y):zoom(0.35):rotationz(rotation):maxwidth(max_width as f32):z(8006)));
        }
        // Pointer interaction is intentionally represented by existing keyboard navigation.
        // A one-pixel current-time marker uses the source final chart time mapping.
        let time = self
            .preview_position()
            .ok()
            .flatten()
            .map(|p| p.chart_time())
            .or_else(|| self.playback_position().map(|p| p.0))
            .or_else(|| report.chart_time(self.workspace.row()).ok())
            .unwrap_or(0.0);
        let percentage = if report.series.final_time > 0.0 {
            (time / report.series.final_time).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        if bounds.orientation == Orientation::Vertical {
            let t = if reverse {
                1.0 - percentage
            } else {
                percentage
            };
            actors.push(act!(quad:xy(r.x as f32+r.width as f32*0.5,r.y as f32+1.0+t*(r.height-2) as f32):zoomto((r.width-2) as f32,1.0):diffuse(1.0,1.0,1.0,0.8):z(8006)));
        } else {
            actors.push(act!(quad:xy(r.x as f32+1.0+percentage*(r.width-2) as f32,r.y as f32+r.height as f32*0.5):zoomto(1.0,(r.height-2) as f32):diffuse(1.0,1.0,1.0,0.8):z(8006)));
        }
        if self.source_density.preferences.stream {
            let (x, y, rotation, width) = if bounds.orientation == Orientation::Vertical {
                (
                    r.x as f32 + 12.0,
                    r.y as f32 + 3.0,
                    90.0,
                    (r.height - 6).max(1),
                )
            } else {
                (
                    r.x as f32 + 3.0,
                    r.y as f32 + r.height as f32 - 16.0,
                    0.0,
                    (r.width - 6).max(1),
                )
            };
            if bounds.orientation == Orientation::Vertical {
                self.overview_label(
                    actors,
                    r.x as f32,
                    r.width as f32,
                    0,
                    Arc::clone(&self.source_density.summary),
                );
            } else {
                actors.push(act!(text:font("miso"):settext(Arc::clone(&self.source_density.summary)):align(0.0,0.0):xy(x,y):zoom(0.4):rotationz(rotation):maxwidth(width as f32):maxheight(28.0):z(8006)));
            }
        }
    }
}

#[cfg(test)]
mod density_binding_regressions {
    use super::*;
    use std::{
        cell::Cell,
        time::{Duration, Instant},
    };
    fn init(fr: bool) {
        if fr {
            crate::i18n::init(deadsync_assets::language::load_for_tests("fr"));
        } else {
            crate::i18n::init_for_tests();
        }
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
    }
    fn state() -> (tempfile::TempDir, State) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("test.ssc");
        std::fs::write(&p,b"#VERSION:0.83;#TITLE:Binding;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n1000\n1000\n1000,\n1000;").unwrap();
        let s = State::open(&p, 0).unwrap();
        (d, s)
    }
    fn finish(s: &mut State) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while s.is_density_loading() {
            s.update();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn preference_only_history_restarts_stale_density_without_further_input_en_fr() {
        use deadlib_platform::input::KeyCode;
        use deadsync_editor::workspace::Command;
        for fr in [false, true] {
            init(fr);
            let (_dir, mut state) = state();
            finish(&mut state);
            assert!(state.source_density_report().is_some());
            let notes = state.workspace.file().session.serialize().unwrap();
            let row = state.workspace.row();
            state.open_dark_settings();
            state.dark_settings_key(0, KeyCode::ArrowRight);
            state.dark_settings_key(8, KeyCode::Enter);
            assert!(state.panel.is_none(), "real preference Apply must succeed");
            assert!(
                state.source_density_report().is_none(),
                "history invalidates the prior binding"
            );
            // The next host frame must schedule recovery itself. No refresh,
            // navigation, modal reopening or other input is supplied by this test.
            state.update();
            assert!(
                state.is_density_loading() || state.source_density_report().is_some(),
                "a stale density report must have a replacement worker or a current result"
            );
            finish(&mut state);
            assert!(state.source_density_report().is_some());
            for action in [Command::Undo, Command::Redo] {
                state.command(action);
                state.update();
                finish(&mut state);
                assert!(state.source_density_report().is_some());
            }
            assert_eq!(state.workspace.row(), row);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), notes);
        }
    }
    fn fail() -> Result<Job<Report>, deadsync_editor::Error> {
        Err(deadsync_editor::Error::InvalidEdit(
            "injected snapshot failure".into(),
        ))
    }
    #[test]
    fn changed_stream_hides_prior_report_during_loading_and_capture_failure() {
        for fr in [false, true] {
            init(fr);
            let (_d, mut s) = state();
            finish(&mut s);
            assert!(s.source_density_report().is_some());
            s.source_density.stream.threshold = 4;
            assert!(s.source_density_report().is_none());
            s.refresh_source_density();
            assert!(s.is_density_loading());
            assert!(s.source_density_report().is_none());
            finish(&mut s);
            assert!(s.source_density_report().is_some());
            s.source_density.stream.threshold = 8;
            s.refresh_source_density_with(|_, _, _| fail());
            assert!(s.source_density.failed);
            assert!(s.source_density_report().is_none());
            assert!(!s.is_density_loading());
            let mut a = Vec::new();
            s.actors(&mut a);
            let expected = crate::i18n::tr("Edition", "DensityUnavailable");
            assert!(a.iter().any(
                |a| matches!(a,Actor::Text{content,..}if content.as_ref()==expected.as_ref())
            ));
            s.source_density.stream.threshold = 12;
            s.refresh_source_density_with(|session, _, stream| {
                // A real public Task captures this valid source but its worker
                // refuses the last-second hint beyond the source row budget.
                let mut bytes = session.serialize()?;
                bytes.extend_from_slice(b"#LASTSECONDHINT:1000000000;");
                let oversized = deadsync_editor::history::Session::new(
                    deadsync_editor::document::Document::parse(
                        bytes,
                        deadsync_editor::document::Format::Ssc,
                    )?,
                );
                Task::capture(&oversized, 0, stream)?.spawn()
            });
            assert!(s.is_density_loading());
            assert!(s.source_density_report().is_none());
            finish(&mut s);
            assert!(s.source_density.failed);
            assert!(s.source_density.pending.is_some());
            assert!(s.source_density_report().is_none());
        }
    }
    #[test]
    fn failed_snapshot_binding_retries_only_after_snapshot_or_visibility_changes() {
        for fr in [false, true] {
            init(fr);
            let (_d, mut s) = state();
            if let Some(job) = s.source_density.job.take() {
                job.cancel();
            }
            s.source_density.report = None;
            s.source_density.pending = None;
            let calls = Cell::new(0);
            let run = |s: &mut State| {
                s.refresh_source_density_with(|_, _, _| {
                    calls.set(calls.get() + 1);
                    fail()
                })
            };
            run(&mut s);
            run(&mut s);
            assert_eq!(calls.get(), 1);
            s.workspace
                .execute(deadsync_editor::workspace::Command::Move(1))
                .unwrap();
            run(&mut s);
            assert_eq!(
                calls.get(),
                1,
                "cursor-only move retains failed snapshot binding"
            );
            s.source_density.stream.threshold = 4;
            run(&mut s);
            assert_eq!(calls.get(), 2);
            s.workspace
                .execute(deadsync_editor::workspace::Command::Place(1))
                .unwrap();
            run(&mut s);
            assert_eq!(calls.get(), 3, "new content retries");
            s.source_density.preferences.visible = false;
            run(&mut s);
            assert_eq!(calls.get(), 3);
            assert!(s.source_density.pending.is_none());
            s.source_density.preferences.visible = true;
            run(&mut s);
            assert_eq!(calls.get(), 4, "visibility reenable is an explicit retry");
        }
    }
    #[test]
    fn raw_width_offset_changes_actual_projection_and_mesh_while_defaults_stay_aligned() {
        use deadsync_editor::density_preferences::Position;
        init(false);
        let (_dir, mut state) = state();
        finish(&mut state);
        for (width, height) in [(854., 480.), (1100., 760.), (3440., 1440.)] {
            for dpi in [1., 2.] {
                deadlib_present::space::set_current_metrics(
                    deadlib_present::space::Metrics::centered(width, height),
                );
                state.ui_dpi = Some(deadsync_editor::options_preferences::dpi::Capture::new(
                    &state.general_options,
                    dpi,
                ));
                state.ui_dpi_render_scale = dpi as f32;
                state
                    .set_ui_dpi_viewport((width as f64 * dpi) as u32, (height as f64 * dpi) as u32);
                for position in Position::ALL {
                    let mut p = Preferences::default();
                    p.position = position;
                    state.configure_density_preferences(p).unwrap();
                    finish(&mut state);
                    let g = state
                        .visible_view_geometry()
                        .into_iter()
                        .find(|g| g.focused)
                        .unwrap();
                    let viewport = state.chart_viewport();
                    let baseline = state.source_density_bounds(g, viewport).unwrap();
                    let mut actors = Vec::new();
                    state.source_density_actors(&mut actors, g, viewport);
                    let first = state
                        .source_density
                        .mesh
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .mesh
                        .vertices
                        .clone();
                    if !matches!(
                        position,
                        Position::TopOfFocusedChart | Position::BottomOfFocusedChart
                    ) {
                        let map = state.minimap_mount_bounds().unwrap();
                        assert_eq!(baseline.rect.y, map.y);
                        assert_eq!(
                            baseline.rect.y + baseline.rect.height,
                            map.y + map.visible_height as i32
                        );
                    }
                    p.width_offsets[position.index()] += 20;
                    state.configure_density_preferences(p).unwrap();
                    finish(&mut state);
                    let changed = state.source_density_bounds(g, viewport).unwrap();
                    assert_ne!(changed, baseline);
                    let mut changed_actors = Vec::new();
                    state.source_density_actors(&mut changed_actors, g, viewport);
                    {
                        let cached = state.source_density.mesh.borrow();
                        let mesh = &cached.as_ref().unwrap().mesh;
                        assert_ne!(mesh.vertices.as_ref(), first.as_ref());
                        assert!(
                            mesh.vertices
                                .iter()
                                .all(|v| v.pos.into_iter().all(f32::is_finite))
                        );
                    }
                    for raw in [i32::MIN, i32::MAX] {
                        let mut p = p;
                        p.height = raw;
                        p.position_offsets[position.index()] = raw;
                        p.width_offsets[position.index()] = raw;
                        state.configure_density_preferences(p).unwrap();
                        finish(&mut state);
                        let mut extreme = Vec::new();
                        state.source_density_actors(&mut extreme, g, viewport);
                        assert!(extreme.len() < 32);
                    }
                }
            }
        }
    }
    #[test]
    fn source_tiny_raw_dimensions_draw_background_and_rim_without_measure_mesh() {
        init(false);
        let (_dir, mut state) = state();
        finish(&mut state);
        for height in [-1, 0, 1, 2] {
            let mut p = Preferences::default();
            p.height = height;
            state.configure_density_preferences(p).unwrap();
            let g = state
                .visible_view_geometry()
                .into_iter()
                .find(|g| g.focused)
                .unwrap();
            let mut actors = Vec::new();
            state.source_density_actors(&mut actors, g, state.chart_viewport());
            assert_eq!(actors.len(), if height < 0 { 0 } else { 5 });
            assert!(state.source_density.mesh.borrow().is_none());
            assert_eq!(state.density_preferences().height, height);
        }
    }
}
