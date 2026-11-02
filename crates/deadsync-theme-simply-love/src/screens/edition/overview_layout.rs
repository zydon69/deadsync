//! Shared native sidebar allocation: labels above both overviews, aligned bodies.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Vertical {
    pub label_top: f32,
    pub line_height: f32,
    pub top: i32,
    pub bottom: i32,
    pub font_scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_present::actors::SizeSpec;
    #[test]
    fn both_sidebar_bodies_align_and_labels_stay_above_them_at_all_sizes() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("overview.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
        for (width, height) in [(854., 480.), (1100., 760.), (3440., 1440.)] {
            for dpi in [1., 2.] {
                deadlib_present::space::set_current_metrics(
                    deadlib_present::space::Metrics::centered(width, height),
                );
                let mut state = State::open(&path, 0).unwrap();
                state.ui_dpi = Some(deadsync_editor::options_preferences::dpi::Capture::new(
                    &state.general_options,
                    dpi,
                ));
                state.ui_dpi_render_scale = dpi as f32;
                state
                    .set_ui_dpi_viewport((width as f64 * dpi) as u32, (height as f64 * dpi) as u32);
                let layout = state.overview_vertical_layout();
                let map = state.minimap_mount_bounds().unwrap();
                assert_eq!(map.y, layout.top);
                assert_eq!(map.y + map.visible_height as i32, layout.bottom);
                let geometry = state
                    .visible_view_geometry()
                    .into_iter()
                    .find(|g| g.focused)
                    .unwrap();
                let mut density = Vec::new();
                state.source_density_actors(&mut density, geometry, state.chart_viewport());
                let Actor::Sprite { offset, size, scale, .. } = &density[0] else {
                    panic!("density background");
                };
                let SizeSpec::Px(body_height) = size[1] else {
                    panic!("pixel height");
                };
                let body_height = body_height * scale[1];
                assert_eq!(offset[1] - body_height * 0.5, map.y as f32);
                assert_eq!(
                    offset[1] + body_height * 0.5,
                    (map.y + map.visible_height as i32) as f32
                );
                let mut labels = Vec::new();
                for row in [0, 1] {
                    state.overview_label(
                        &mut labels,
                        map.x as f32,
                        map.width as f32,
                        row,
                        Arc::from("White label"),
                    );
                }
                for label in labels {
                    let Actor::Text {
                        clip: Some(clip), ..
                    } = label
                    else {
                        panic!("clipped label");
                    };
                    assert!(clip[1] >= state.chart_viewport().top);
                    assert!(clip[1] + clip[3] <= map.y as f32);
                }
            }
        }
    }
}
impl State {
    pub(super) fn overview_vertical_layout(&self) -> Vertical {
        let viewport = self.chart_viewport();
        let font_scale = self
            .misc_widget_dpi_metrics()
            .map_or(1.0, |m| m.font_scale as f32);
        let line_height = 16.0 * font_scale;
        let bottom = viewport.bottom.floor() as i32;
        Vertical {
            label_top: viewport.top.ceil(),
            line_height,
            font_scale,
            top: ((viewport.top.ceil() + 2.0 * line_height + 4.0).ceil() as i32).min(bottom - 1),
            bottom,
        }
    }
    pub(super) fn overview_label(
        &self,
        actors: &mut Vec<Actor>,
        x: f32,
        width: f32,
        line: u8,
        text: Arc<str>,
    ) {
        let layout = self.overview_vertical_layout();
        let y = layout.label_top + f32::from(line) * layout.line_height;
        // Miso page quads extend beyond logical glyph advances. Keep a
        // DPI-scaled inset on both sides so uniform fitting also contains
        // those quads inside the column safety clip.
        let inset = (4.0 * layout.font_scale).min(width.max(0.0) * 0.5);
        let text_x = x + inset;
        let mut actor = act!(text:font("miso"):settext(text):align(0.0,0.0):xy(text_x,y):
            zoom((13.0/24.0)*layout.font_scale):diffuse(1.0,1.0,1.0,1.0):z(8006));
        if let Actor::Text { clip, fit_width, fit_height, .. } = &mut actor {
            // Fit both axes uniformly before zoom. A width-only clip silently
            // cuts localized captions at narrow overview widths; max_width
            // would compress glyphs horizontally in the text compositor.
            let zoom = (13.0 / 24.0) * layout.font_scale;
            *fit_width = Some((width - 2.0 * inset).max(0.0) / zoom);
            // Miso has a 15px logical cap height (Baseline19 - Top4),
            // while glyph cells are 24px. Cap the fit at the logical
            // height to prevent widening columns from enlarging the text.
            *fit_height = Some(15.0);
            *clip = Some([x, y, width.max(0.0), layout.line_height]);
        }
        actors.push(actor);
    }
}

#[cfg(test)]
#[path = "overview_label_geometry_tests.rs"]
mod label_geometry_tests;
