//! Application-lifetime UI DPI and explicit keyboard row pages.
use super::*;
use deadsync_editor::options_preferences::dpi::{Capture, Page};
use std::sync::OnceLock;
static APPLICATION_DPI: OnceLock<Capture> = OnceLock::new();
#[derive(Default)]
pub(super) struct Rows {
    items: Vec<Row>,
    collecting: bool,
    horizontal: i32,
    vertical: Option<i32>,
    panel: Option<std::mem::Discriminant<Panel>>,
}
struct Row {
    start: usize,
    end: usize,
    x: f32,
    y: f32,
    align: f32,
    width: f32,
    pitch: f32,
    selected: bool,
}
impl State {
    /// Called only by the published host, never detached file candidates.
    pub fn bind_ui_dpi(&mut self, monitor: f64) -> Result<(), deadsync_editor::Error> {
        if !monitor.is_finite() || monitor <= 0. {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Invalid host monitor DPI scale".into(),
            ));
        }
        if self.ui_dpi.is_some() {
            return if self.ui_dpi_supported {
                Ok(())
            } else {
                Err(deadsync_editor::Error::InvalidEdit(
                    "DPI scale cannot be represented by the native renderer".into(),
                ))
            };
        }
        let capture = *APPLICATION_DPI.get_or_init(|| Capture::new(&self.general_options, monitor));
        let scale = capture.scale() as f32;
        self.ui_dpi = Some(capture);
        if capture.font_pixels().is_ok_and(|px| {
            f64::from(px) / 15. * 24. * 0.8 > f64::from((self.ui_dpi_window[1] - 32.).max(1.))
        }) || capture.native_widget_metrics(80., 20.).is_err()
            || !scale.is_finite()
            || scale <= 0.
            || ![
                screen_width() / scale,
                screen_height() / scale,
                10000. * scale,
            ]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
        {
            self.ui_dpi_supported = false;
            self.status = i18n::tr("Edition", "UiDpiUnsupported");
            return Err(deadsync_editor::Error::InvalidEdit(
                "DPI scale cannot be represented by the native renderer".into(),
            ));
        }
        self.ui_dpi_render_scale = scale;
        Ok(())
    }
    fn render_dpi_capture(&self) -> Capture {
        self.ui_dpi
            .filter(|_| self.ui_dpi_supported)
            .unwrap_or_default()
    }
    /// Physical viewport is supplied by the real published host; candidates never bind.
    pub fn set_ui_dpi_viewport(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.ui_dpi_window = [width as f32, height as f32];
        }
    }
    fn ui_canvas_pixels(&self) -> [f32; 2] {
        [
            self.ui_dpi_window[0] / screen_width(),
            self.ui_dpi_window[1] / screen_height(),
        ]
    }
    fn ui_render_scale(&self) -> [f32; 2] {
        let pixels = self.ui_canvas_pixels();
        [
            self.ui_dpi_render_scale / pixels[0],
            self.ui_dpi_render_scale / pixels[1],
        ]
    }
    pub(super) fn misc_widget_dpi_metrics(
        &self,
    ) -> Result<deadsync_editor::options_preferences::dpi::WidgetMetrics, deadsync_editor::Error>
    {
        let mut m = self.render_dpi_capture().native_widget_metrics(80., 20.)?;
        let pixels = f64::from(self.ui_canvas_pixels()[1]);
        m.width /= pixels;
        m.height /= pixels;
        m.padding /= pixels;
        m.font_scale /= pixels;
        m.inset_x /= pixels;
        m.inset_y /= pixels;
        Ok(m)
    }
    pub fn ui_dpi_is_bound(&self) -> bool {
        self.ui_dpi.is_some()
    }
    fn ui_dpi_has_transform(&self) -> bool {
        self.ui_render_scale() != [1., 1.]
            || self.render_dpi_capture().font_pixels().unwrap_or(15) as f64
                / 15.
                / self.active_ui_dpi()
                != 1.
    }
    pub fn active_ui_dpi(&self) -> f64 {
        f64::from(self.ui_dpi_render_scale)
    }
    pub fn ui_dpi_pending_restart(&self) -> bool {
        self.ui_dpi
            .is_some_and(|c| c.pending_restart(&self.general_options))
    }
    pub(super) fn ui_row(&self, actors: &[Actor], start: usize, selected: bool, pitch: f32) {
        if !self.ui_rows.borrow().collecting {
            return;
        }
        let ratio = (f64::from(self.render_dpi_capture().font_pixels().unwrap_or(15))
            / 15.
            / self.active_ui_dpi()) as f32;
        let Some((x, y, align, width)) = actors.get(start..).and_then(|rows| {
            rows.iter().find_map(|a| match a {
                Actor::Text {
                    offset,
                    align,
                    max_width,
                    scale,
                    ..
                } => Some((
                    offset[0],
                    offset[1] + (0.5 - align[1]) * 24. * scale[1] * ratio,
                    align[0],
                    max_width.unwrap_or(0.),
                )),
                _ => None,
            })
        }) else {
            return;
        };
        self.ui_rows.borrow_mut().items.push(Row {
            start,
            end: actors.len(),
            x,
            y,
            align,
            width,
            pitch,
            selected,
        });
    }
    pub(super) fn ui_dpi_key(&mut self, code: KeyCode) -> bool {
        // A held query belongs to the active editing gesture, including Alt.
        // It must not turn its movement arrows into virtual-panel scrolling.
        use deadsync_editor::key_binding_preferences::scroll_modifiers::Kind as Held;
        if self.receptor_move.is_some()
            || self.region_selection.is_some()
                && [
                    Held::SelectionMisc,
                    Held::SelectionToggle,
                    Held::SelectionExtend,
                ]
                .into_iter()
                .any(|kind| self.owned_gesture_query(kind))
        {
            return false;
        }
        if self
            .scroll_modifier_settings
            .as_ref()
            .is_some_and(|settings| settings.is_capturing())
            || self.text_buffer.is_some() && !self.control
            || self
                .key_binding_draft
                .as_ref()
                .is_some_and(|s| s.is_capturing())
            || self.panel.is_none()
            || !self.command_bindings.alt_held()
            || !self.ui_dpi_has_transform()
        {
            return false;
        }
        let mut rows = self.ui_rows.borrow_mut();
        match code {
            KeyCode::ArrowLeft => rows.horizontal = rows.horizontal.saturating_sub(1).max(-1024),
            KeyCode::ArrowRight => rows.horizontal = rows.horizontal.saturating_add(1).min(1024),
            KeyCode::ArrowUp => {
                rows.vertical = Some(rows.vertical.unwrap_or(0).saturating_sub(1).max(-1024))
            }
            KeyCode::ArrowDown => {
                rows.vertical = Some(rows.vertical.unwrap_or(0).saturating_add(1).min(1024))
            }
            KeyCode::Home => {
                rows.horizontal = 0;
                rows.vertical = None;
            }
            _ => return false,
        };
        true
    }
    pub(super) fn reset_ui_manual_vertical(&mut self) {
        self.ui_rows.borrow_mut().vertical = None;
    }
    pub(super) fn panel_actors(&self, panel: Panel, actors: &mut Vec<Actor>) {
        if matches!(panel, Panel::Hotbar(_)) {
            self.capture_hotbar_frame();
        }
        let scale = self.ui_render_scale();
        if !self.ui_dpi_has_transform() {
            self.panel_actors_unscaled(panel, actors);
            return;
        }
        let physical = [screen_width(), screen_height()];
        let virtual_size = [
            self.ui_dpi_window[0] / self.ui_dpi_render_scale,
            self.ui_dpi_window[1] / self.ui_dpi_render_scale,
        ];
        if virtual_size.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return;
        }
        // These pages own their row/text pagination, including editable File forms.
        // UpdateNotice also lays out against the real viewport: a minimum-size
        // canvas centers its fixed left-aligned URL outside small DPI windows.
        // Give them the actual virtual viewport instead of a minimum-size form;
        // selecting the first title as an implicit row would scroll the modal.
        let owns_pagination = self.chart_details.active && matches!(panel, Panel::Analysis)
            || matches!(
                panel,
                Panel::Hotbar(_)
                    | Panel::UpdateNotice(_)
                    | Panel::EditorAbout(super::editor_about::AboutView::License { .. })
            )
            || matches!(
                panel,
                Panel::FileMenu(_)
                    | Panel::FileRecent(_)
                    | Panel::FileOpen(_)
                    | Panel::FileUnsaved(_)
                    | Panel::FileDiscard
                    | Panel::FileLoading
            );
        let content_size = if owns_pagination {
            virtual_size
        } else {
            [virtual_size[0].max(854.), virtual_size[1].max(480.)]
        };
        {
            let mut rows = self.ui_rows.borrow_mut();
            let key = std::mem::discriminant(&panel);
            if rows.panel != Some(key) {
                rows.horizontal = 0;
                rows.vertical = None;
                rows.panel = Some(key);
            }
            rows.items.clear();
        }
        let mut children = Vec::new();
        // Row registration belongs to this rendering pass. Recomputing the
        // transform predicate inside virtual metrics can return false (e.g.
        // a 2x monitor maps 3200x1860 to 1600x930), losing every control row.
        // The title fallback then scrolls the entire modal toward the bottom.
        {
            struct CaptureRows<'a>(&'a std::cell::RefCell<Rows>);
            impl Drop for CaptureRows<'_> {
                fn drop(&mut self) {
                    self.0.borrow_mut().collecting = false;
                }
            }
            self.ui_rows.borrow_mut().collecting = true;
            let _capture = CaptureRows(&self.ui_rows);
            deadlib_present::space::with_metrics(
                deadlib_present::space::Metrics::centered(content_size[0], content_size[1]),
                || self.panel_actors_unscaled(panel, &mut children),
            );
        }
        let ratio = (f64::from(self.render_dpi_capture().font_pixels().unwrap_or(15))
            / 15.
            / self.active_ui_dpi()) as f32;
        font_ratio(&mut children, ratio);
        let rows = self.ui_rows.borrow();
        let center_shift = [
            (virtual_size[0] - content_size[0]) * 0.5,
            (virtual_size[1] - content_size[1]) * 0.5,
        ];
        let centers: Vec<_> = rows.items.iter().map(|r| r.y + center_shift[1]).collect();
        let selected = rows.items.iter().position(|r| r.selected).unwrap_or(0);
        let pitch = rows.items.iter().map(|r| r.pitch).fold(1_f32, f32::max);
        let page = Page::rows(&centers, selected, pitch, virtual_size[1]);
        let mut hidden = vec![false; children.len()];
        if let Some(page) = page.filter(|_| rows.vertical.is_none()) {
            for (i, row) in rows.items.iter().enumerate() {
                if i < page.first || i >= page.end {
                    hidden[row.start..row.end].fill(true);
                }
            }
        }
        children = children
            .into_iter()
            .enumerate()
            .filter_map(|(i, a)| (!hidden[i]).then_some(a))
            .collect();
        let fallback = children.iter().find_map(|a| {
            if let Actor::Text {
                offset,
                align,
                max_width,
                scale,
                ..
            } = a
            {
                Some((
                    offset[0],
                    offset[1] + (0.5 - align[1]) * 24. * scale[1],
                    align[0],
                    max_width.unwrap_or(0.),
                ))
            } else {
                None
            }
        });
        // An empty event picker has no selected row to reveal. Treating its
        // title as that row moves the whole card and its footer off screen.
        let fallback = if matches!(panel, Panel::EventSelection) && rows.items.is_empty() {
            None
        } else {
            fallback
        };
        let focus = rows
            .items
            .get(selected)
            .map(|r| (r.x, r.y, r.align, r.width))
            .or(fallback);
        let vertical_auto = center_shift[1]
            + if owns_pagination {
                0.
            } else {
                page.map_or_else(
                    || {
                        focus.map_or(0., |(_, y, _, _)| {
                            virtual_size[1] * 0.5 - y - center_shift[1]
                        })
                    },
                    |p| p.translation,
                )
            };
        let vertical = vertical_auto - rows.vertical.unwrap_or(0) as f32 * virtual_size[1] * 0.8;
        let horizontal_auto = center_shift[0]
            + if owns_pagination {
                0.
            } else {
                focus.map_or(0., |(x, _, align, width)| {
                    let x = x + center_shift[0];
                    let pad = 16_f32.min(virtual_size[0] * 0.25);
                    let available = virtual_size[0] - 2. * pad;
                    let (low, high) = if width > available {
                        let target = pad + available * align;
                        (target, target)
                    } else {
                        (
                            pad + width * align,
                            virtual_size[0] - pad - width * (1. - align),
                        )
                    };
                    x.clamp(low, high) - x
                })
            };
        let horizontal = horizontal_auto - (rows.horizontal as f32) * virtual_size[0] * 0.8;
        // Centered virtual Frame keeps the chart/world origin unchanged. The model
        // transform scales only this UI subtree, including fonts, images and clips.
        let frame = Actor::Frame {
            align: [0.5, 0.5],
            offset: [
                physical[0] * 0.5 + (content_size[0] - virtual_size[0]) * 0.5 + horizontal,
                physical[1] * 0.5 + (content_size[1] - virtual_size[1]) * 0.5 + vertical,
            ],
            size: [
                deadlib_present::actors::SizeSpec::Px(content_size[0]),
                deadlib_present::actors::SizeSpec::Px(content_size[1]),
            ],
            children,
            background: None,
            z: 0,
        };
        actors.push(Actor::SharedTransform {
            transform: glam::Mat4::from_scale(glam::Vec3::new(scale[0], scale[1], 1.)),
            source_view_proj: glam::Mat4::IDENTITY,
            children: Arc::from([frame]),
            isolate_order: false,
            z: 0,
            tint: [1.; 4],
            blend: None,
        });
        let page_number = page.map_or(1, |p| p.number);
        let page_count = page.map_or(1, |p| p.count);
        let text = i18n::tr_fmt(
            "Edition",
            "UiDpiPage",
            &[
                ("page", &page_number.to_string()),
                ("count", &page_count.to_string()),
                ("horizontal", &rows.horizontal.to_string()),
            ],
        );
        actors.push(act!(text:font("miso"):settext(text):xy(physical[0]*0.5,physical[1]-10.):zoom(0.45):maxwidth(physical[0]-20.):z(8090)));
    }
}
/// The source font atlas uses (int)(15*dpi), independently of f32 style scaling.
/// Keep that normalized font step while retaining the native bitmap family.
fn font_ratio(actors: &mut [Actor], ratio: f32) -> bool {
    if ratio == 1. {
        return false;
    }
    let mut changed = false;
    for actor in actors {
        match actor {
            Actor::Text { scale, .. } => {
                scale[0] *= ratio;
                scale[1] *= ratio;
                changed = true;
            }
            Actor::Frame { children, .. } | Actor::Camera { children, .. } => {
                changed |= font_ratio(children, ratio)
            }
            Actor::SharedFrame { children, .. }
            | Actor::SharedTransform { children, .. }
            | Actor::SharedCamera { children, .. } => {
                changed |= font_ratio(Arc::make_mut(children), ratio)
            }
            Actor::RetainedFrame { frame, .. } => {
                let mut children = frame.children().to_vec();
                if font_ratio(&mut children, ratio) {
                    *frame = Arc::new(deadlib_present::actors::RetainedActorFrame::new(children));
                    changed = true;
                }
            }
            Actor::Shadow { child, .. } => {
                changed |= font_ratio(std::slice::from_mut(child.as_mut()), ratio)
            }
            _ => {}
        }
    }
    changed
}

#[cfg(test)]
mod ftue_layout_tests {
    use super::*;
    use deadlib_present::{
        actors::SizeSpec,
        space::{self, Metrics},
    };
    use deadsync_editor::{
        ftue::Preferences,
        options_preferences::{General, dpi::Capture},
    };
    fn px(size: SizeSpec) -> f32 {
        match size {
            SizeSpec::Px(n) => n,
            SizeSpec::Fill => panic!("fixed modal expected"),
        }
    }
    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.02, "{a} != {b}");
    }
    #[test]
    fn empty_event_picker_keeps_its_card_centered_without_title_autoscroll_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("empty.ssc");
            std::fs::write(
                &path,
                "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.navigate_exact_position(48., true);
            state.open_event_selection();
            for (width, height, dpi) in [(1280., 720., 1.), (2560., 1440., 2.)] {
                space::with_metrics(Metrics::centered(width / height * 480., 480.), || {
                    state.set_ui_dpi_viewport(width as u32, height as u32);
                    state.ui_dpi = Some(Capture::new(&General::default(), dpi));
                    state.ui_dpi_render_scale = dpi as f32;
                    let mut actors = Vec::new();
                    state.panel_actors(Panel::EventSelection, &mut actors);
                    assert!(state.ui_rows.borrow().items.is_empty());
                    let Actor::SharedTransform { children, .. } = &actors[0] else {
                        panic!("DPI frame expected")
                    };
                    let Actor::Frame { offset, .. } = &children[0] else {
                        panic!("panel frame expected")
                    };
                    close(offset[0], screen_width() * 0.5);
                    close(offset[1], screen_height() * 0.5);
                });
            }
        }
    }
    #[test]
    fn ftue_rows_are_registered_under_virtual_metrics_and_modal_remains_centered_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::without_song(root.path()).unwrap();
            for (width, height, dpi) in [
                (854., 480., 1.),
                (1100., 760., 1.25),
                (3200., 1860., 2.),
                (3440., 1440., 2.),
            ] {
                let metrics = Metrics::centered(width / height * 480., 480.);
                space::with_metrics(metrics, || {
                    state.set_ui_dpi_viewport(width as u32, height as u32);
                    state.ui_dpi = Some(Capture::new(&General::default(), dpi));
                    state.ui_dpi_render_scale = dpi as f32;
                    for page in 0..6 {
                        let mut owner = deadsync_editor::audio_preferences::File::memory().unwrap();
                        owner.set_ftue(Preferences {
                            completed: None,
                            index: page,
                        });
                        state.publish_application_preferences_owner(owner);
                        state.panel = None;
                        state.update_host_ftue();
                        let count = if page == 0 {
                            1
                        } else if page == 5 {
                            4
                        } else {
                            3
                        };
                        for selected in 0..count {
                            let panel = Panel::Ftue(selected);
                            state.panel = Some(panel);
                            let mut actors = Vec::new();
                            state.panel_actors(panel, &mut actors);
                            let (transform, origin, children) = if let Actor::SharedTransform {
                                transform,
                                children,
                                ..
                            } = &actors[0]
                            {
                                assert_eq!(
                                    state.ui_rows.borrow().items.len(),
                                    count,
                                    "dpi={dpi}, page={page}: controls must not disappear under virtual metrics"
                                );
                                assert!(!state.ui_rows.borrow().collecting);
                                let Actor::Frame {
                                    align,
                                    offset,
                                    size,
                                    children,
                                    ..
                                } = &children[0]
                                else {
                                    panic!("UI frame")
                                };
                                (
                                    *transform,
                                    [
                                        offset[0] - align[0] * px(size[0]),
                                        offset[1] - align[1] * px(size[1]),
                                    ],
                                    children.as_slice(),
                                )
                            } else {
                                (glam::Mat4::IDENTITY, [0., 0.], actors.as_slice())
                            };
                            let Actor::Sprite {
                                offset: modal_offset,
                                size,
                                ..
                            } = &children[0]
                            else {
                                panic!("modal background")
                            };
                            let center = transform.transform_point3(glam::Vec3::new(
                                metrics.left + origin[0] + modal_offset[0],
                                metrics.top - origin[1] - modal_offset[1],
                                0.,
                            ));
                            let center_px = [
                                (center.x - metrics.left) / space::screen_width() * width,
                                (metrics.top - center.y) / space::screen_height() * height,
                            ];
                            close(center_px[0], width * 0.5);
                            close(center_px[1], height * 0.5);
                            let physical_size = [
                                px(size[0]) * transform.x_axis.x * width / space::screen_width(),
                                px(size[1]) * transform.y_axis.y * height / space::screen_height(),
                            ];
                            assert!(center_px[0] - physical_size[0] * 0.5 >= -0.02);
                            assert!(center_px[1] - physical_size[1] * 0.5 >= -0.02);
                            assert!(center_px[1] + physical_size[1] * 0.5 <= height + 0.02);
                            for actor in &children[1..] {
                                if let Actor::Text { scale, offset, .. } = actor {
                                    // Miso LineSpacing24 * local font scale * DPI
                                    // must equal source floor(15*DPI), never the tiny
                                    // old body12px or an arbitrary global enlargement.
                                    close(
                                        24. * scale[1] * transform.y_axis.y * height
                                            / space::screen_height(),
                                        (15. * dpi).trunc() as f32,
                                    );
                                    assert!(offset[1] >= modal_offset[1] - px(size[1]) * 0.5);
                                    assert!(offset[1] <= modal_offset[1] + px(size[1]) * 0.5);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod hotbar_geometry_tests;

#[cfg(test)]
mod chart_details_geometry_tests;

#[cfg(test)]
mod readout_geometry;

#[cfg(test)]
mod file_menu_geometry_tests;
