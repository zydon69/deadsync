use super::*;
use deadlib_present::{
    actors::SizeSpec,
    space::{self, Metrics},
};
use deadsync_editor::options_preferences::General;
fn px(value: SizeSpec) -> f32 {
    match value {
        SizeSpec::Px(v) => v,
        SizeSpec::Fill => panic!("fixed form"),
    }
}
#[test]
fn source_chart_details_title_and_all_lanes_fit_en_fr_sizes_and_dpi_one_two() {
    for language in ["en", "fr"] {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.focus_position = Some(
            deadsync_editor::chart_focus::Position::at_fractional_row(
                &state.workspace.file().session,
                0,
                -12.75,
            )
            .unwrap(),
        );
        for (width, height) in [(854., 480.), (1100., 760.), (3200., 1860.), (3440., 1440.)] {
            for dpi in [1., 2.] {
                let metrics = Metrics::centered(width / height * 480., 480.);
                space::with_metrics(metrics, || {
                    state.set_ui_dpi_viewport(width as u32, height as u32);
                    state.ui_dpi = Some(Capture::new(&General::default(), dpi));
                    state.ui_dpi_render_scale = dpi as f32;
                    state.ui_dpi_supported = true;
                    state.open_chart_details(None);
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    while state.is_chart_details_loading_for_test() {
                        state.poll_chart_details();
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::yield_now();
                    }

                    for selected in 0..9 {
                        state.chart_details_key(KeyCode::Home);
                        for _ in 0..selected {
                            state.chart_details_key(KeyCode::ArrowDown);
                        }
                        let panel = Panel::Analysis;
                        state.panel = Some(panel);
                        let mut actors = Vec::new();
                        state.panel_actors(panel, &mut actors);
                        let (transform, origin, children) = if let Actor::SharedTransform {
                            transform,
                            children,
                            ..
                        } = &actors[0]
                        {
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
                            offset: modal,
                            size,
                            scale: background_scale,
                            ..
                        } = &children[0]
                        else {
                            panic!("background")
                        };
                        let map = |point: [f32; 2]| {
                            let p = transform.transform_point3(glam::Vec3::new(
                                metrics.left + origin[0] + point[0],
                                metrics.top - origin[1] - point[1],
                                0.,
                            ));
                            [
                                (p.x - metrics.left) / space::screen_width() * width,
                                (metrics.top - p.y) / space::screen_height() * height,
                            ]
                        };
                        let center = map(*modal);
                        let composed_size =
                            super::readout_geometry::composed_solid_size(&children[0], &metrics);
                        let physical = [
                            composed_size[0] * transform.x_axis.x * width / space::screen_width(),
                            composed_size[1] * transform.y_axis.y * height / space::screen_height(),
                        ];
                        assert!((center[0] - width * 0.5).abs() < 0.04);
                        assert!((center[1] - height * 0.5).abs() < 0.04);
                        assert!(
                            center[0] - physical[0] * 0.5 >= -0.04
                                && center[0] + physical[0] * 0.5 <= width + 0.04
                        );
                        assert!(
                            center[1] - physical[1] * 0.5 >= -0.04
                                && center[1] + physical[1] * 0.5 <= height + 0.04
                        );
                        let mut title = false;
                        for actor in &children[1..] {
                            if let Actor::Text {
                                content,
                                offset,
                                align,
                                scale,
                                max_width,
                                ..
                            } = actor
                            {
                                let location =
                                    map([offset[0], offset[1] + (0.5 - align[1]) * 24. * scale[1]]);
                                let half_line = 12. * scale[1] * transform.y_axis.y * height
                                    / space::screen_height();
                                assert!(
                                    location[1] - half_line >= center[1] - physical[1] * 0.5 - 0.05
                                        && location[1] + half_line
                                            <= center[1] + physical[1] * 0.5 + 0.05,
                                    "{language} {width}x{height} dpi{dpi} row{selected} text={:?} location={location:?} half_line={half_line:?} card_center={center:?} card_physical={physical:?} text_offset={offset:?} text_align={align:?} text_scale={scale:?} card_size={size:?} composed_size={composed_size:?} card_scale={background_scale:?} transform={transform:?} frame_origin={origin:?} metrics={metrics:?} canvas={}x{}",
                                    content.as_str(),
                                    space::screen_width(),
                                    space::screen_height()
                                );
                                if content.as_str()
                                    == i18n::tr("Edition", "ChartDetailsTitle").as_ref()
                                {
                                    title = true;
                                    assert!((location[0] - width * 0.5).abs() < 0.05);
                                    assert!(max_width.is_some_and(|w| w <= composed_size[0]));
                                }
                            }
                        }
                        assert!(title);
                    }
                });
            }
        }
    }
}
