//! GrooveAuthor DrawBackground: image/GIF, HSV clear color, Box fit and source
//! options. Native ChartArea is the reserved partition viewport (y105..h-85).
use super::*;
#[cfg(test)]
use deadsync_editor::options_preferences::BackgroundPolicy;
use deadsync_editor::options_preferences::BackgroundSize;
use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}
fn box_fit(area: Rect, source: [u32; 2]) -> Option<Rect> {
    if source.contains(&0)
        || ![area.x, area.y, area.width, area.height]
            .iter()
            .all(|v| v.is_finite())
        || area.width <= 0.0
        || area.height <= 0.0
    {
        return None;
    }
    let scale = (area.width / source[0] as f32).min(area.height / source[1] as f32);
    let width = source[0] as f32 * scale;
    let height = source[1] as f32 * scale;
    Some(Rect {
        x: area.x + (area.width - width) * 0.5,
        y: area.y + (area.height - height) * 0.5,
        width,
        height,
    })
}
impl State {
    fn song_background_view(&self) -> Option<&image_previews::PreviewView> {
        if !self.has_song() || self.song_background_policy().hide {
            return None;
        }
        let request = self.image_preview_request()?;
        let path = request.paths[2].as_ref()?;
        let view = &self.image_preview_views.as_ref()?[2];
        (view.document_identity == Some(request.document_identity)
            && view.path.as_ref() == Some(path)
            && view.has_resident_image
            && view.status == image_previews::Status::Ready)
            .then_some(view)
    }
    pub(super) fn song_background_actors(&self, actors: &mut Vec<Actor>) {
        let width = screen_width();
        let height = screen_height();
        let view = self.song_background_view();
        let color = view.map_or([0.0, 0.0, 0.0, 1.0], |view| view.average_color);
        let [r, g, b, a] = color;
        actors.push(
            act!(quad:xy(width*0.5,height*0.5):setsize(width,height):diffuse(r,g,b,a):z(8000)),
        );
        let Some(view) = view else {
            return;
        };
        let Some(source) = view.source_size else {
            return;
        };
        let area = match self.song_background_policy().size {
            BackgroundSize::Window => Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            BackgroundSize::Unknown(_) => return,
            BackgroundSize::ChartArea => {
                let top = 105.0_f32.min(height.max(0.0));
                let bottom = (height - 85.0).max(top);
                let chart_width = if self.pattern_set_side_panel() {
                    (width - 390.0).max(0.0)
                } else {
                    width
                };
                Rect {
                    x: 0.0,
                    y: top,
                    width: chart_width,
                    height: bottom - top,
                }
            }
        };
        if let Some(rect) = box_fit(area, source) {
            actors.push(act!(sprite(view.key.clone()):xy(rect.x+rect.width*0.5,rect.y+rect.height*0.5):setsize(rect.width,rect.height):z(8000)));
        }
    }
}
/// TextureUtils.GetTextureColor: circular mean hue, RMS saturation/value;
/// ignores alpha. Computed once by the existing bounded image worker.
pub(super) fn texture_color(image: &RgbaImage) -> [f32; 4] {
    let mut hue_x = 0.0_f64;
    let mut hue_y = 0.0_f64;
    let mut saturation = 0.0;
    let mut value = 0.0;
    let count = f64::from(image.width()) * f64::from(image.height());
    if count == 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    for pixel in image.pixels() {
        let r = f64::from(pixel[0]) / 255.0;
        let g = f64::from(pixel[1]) / 255.0;
        let b = f64::from(pixel[2]) / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let sector = if delta == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        let hue = sector * std::f64::consts::TAU / 6.0;
        let sat = if max == 0.0 { 0.0 } else { delta / max };
        hue_x += hue.cos();
        hue_y += hue.sin();
        saturation += sat * sat;
        value += max * max;
    }
    let hue = hue_y.atan2(hue_x).rem_euclid(std::f64::consts::TAU) * 6.0 / std::f64::consts::TAU;
    let sat = (saturation / count).sqrt();
    let val = (value / count).sqrt();
    let chroma = val * sat;
    let secondary = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let low = val - chroma;
    let (r, g, b) = match hue as u32 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let byte = |v: f64| f32::from((v.clamp(0.0, 1.0) as f32 * 255.0) as u8) / 255.0;
    [byte(r + low), byte(g + low), byte(b + low), 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn box_geometry_preserves_ratio_and_centers_in_chart_area_or_window() {
        assert_eq!(
            box_fit(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0
                },
                [400, 200]
            ),
            Some(Rect {
                x: 0.0,
                y: 100.0,
                width: 800.0,
                height: 400.0
            })
        );
        assert_eq!(
            box_fit(
                Rect {
                    x: 0.0,
                    y: 105.0,
                    width: 800.0,
                    height: 410.0
                },
                [400, 200]
            ),
            Some(Rect {
                x: 0.0,
                y: 110.0,
                width: 800.0,
                height: 400.0
            })
        );
        assert!(
            box_fit(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 10.0
                },
                [1, 1]
            )
            .is_none()
        );
        assert!(
            box_fit(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0
                },
                [0, 1]
            )
            .is_none()
        );
    }
    #[test]
    fn clear_color_matches_source_rms_value_and_ignores_alpha() {
        let image = RgbaImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 0, 0, 255]).unwrap();
        let [r, g, b, a] = texture_color(&image);
        assert!((r - 0.5_f32.sqrt()).abs() < 1.0 / 255.0);
        assert!(g > 0.20 && g < 0.21);
        assert_eq!(g, b);
        assert_eq!(a, 1.0);
    }
}

#[cfg(test)]
mod actor_tests {
    use super::*;
    use deadlib_present::actors::{SizeSpec, SpriteSource};
    fn state(language: &str) -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            800.0, 600.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("chartless.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#TITLE:Background;#BACKGROUND:bg.png;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.configure_song_background_policy(BackgroundPolicy {
            hide: false,
            size: BackgroundSize::ChartArea,
        });
        let request = state.image_preview_request().unwrap();
        state.set_image_preview_views(std::array::from_fn(|index| image_previews::PreviewView {
            status: image_previews::Status::Ready,
            key: Arc::from(match index {
                0 => "__editor_song_preview::banner",
                1 => "__editor_song_preview::cdtitle",
                _ => "__editor_song_preview::background",
            }),
            source_size: Some([400, 200]),
            has_resident_image: true,
            document_identity: Some(request.document_identity),
            path: request.paths[index].clone(),
            average_color: [0.2, 0.3, 0.4, 1.0],
        }));
        (root, state)
    }
    fn image(actors: &[Actor]) -> Option<([f32; 2], [f32; 2], i16)> {
        actors.iter().find_map(|a| match a {
            Actor::Sprite {
                source,
                size: [SizeSpec::Px(w), SizeSpec::Px(h)],
                offset,
                z,
                ..
            } if source.texture_key() == Some("__editor_song_preview::background") => {
                Some((*offset, [*w, *h], *z))
            }
            _ => None,
        })
    }
    #[test]
    fn chartless_image_geometry_and_layer_are_preserved_in_both_languages() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            assert!(!state.workspace.has_chart());
            let mut actors = Vec::new();
            state.song_background_actors(&mut actors);
            assert_eq!(actors.len(), 2);
            assert_eq!(image(&actors), Some(([400.0, 310.0], [800.0, 400.0], 8000)));
            assert!(
                matches!(&actors[0],Actor::Sprite{source:SpriteSource::Solid,tint,z:8000,..} if *tint==[0.2,0.3,0.4,1.0])
            );
            state.configure_song_background_policy(BackgroundPolicy {
                hide: false,
                size: BackgroundSize::Window,
            });
            actors.clear();
            state.song_background_actors(&mut actors);
            assert_eq!(image(&actors), Some(([400.0, 300.0], [800.0, 400.0], 8000)));
            let mut dark = deadsync_editor::dark_preferences::Preferences::default();
            dark.set_show(true);
            dark.set_order(deadsync_editor::dark_preferences::Order::AfterBackground);
            state.configure_dark_preferences(dark);
            state.dark_background_actors(
                &mut actors,
                deadsync_editor::dark_preferences::Order::AfterBackground,
            );
            assert!(matches!(
                &actors[2],
                Actor::Sprite {
                    source: SpriteSource::Solid,
                    z: 8000,
                    ..
                }
            ));
            // Source sprite and clear are inserted before the existing Dark/notes pipeline.
            assert!(
                matches!(&actors[1],Actor::Sprite{source,..} if source.texture_key()==Some("__editor_song_preview::background"))
            );
        }
    }
    #[test]
    fn hidden_loading_missing_and_stale_document_never_draw_old_background_en_fr() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            for status in [
                image_previews::Status::Loading,
                image_previews::Status::Missing,
                image_previews::Status::UnsupportedVideo,
            ] {
                state.image_preview_views.as_mut().unwrap()[2].status = status;
                let mut actors = Vec::new();
                state.song_background_actors(&mut actors);
                assert!(image(&actors).is_none());
                assert_eq!(actors.len(), 1);
                assert!(matches!(&actors[0],Actor::Sprite{tint,..} if *tint==[0.0,0.0,0.0,1.0]));
            }
            state.image_preview_views.as_mut().unwrap()[2].status = image_previews::Status::Ready;
            let current_path = state.image_preview_views.as_ref().unwrap()[2].path.clone();
            state.image_preview_views.as_mut().unwrap()[2].path =
                Some(PathBuf::from("other-background.png"));
            let mut stale_path = Vec::new();
            state.song_background_actors(&mut stale_path);
            assert!(image(&stale_path).is_none());
            state.image_preview_views.as_mut().unwrap()[2].path = current_path;
            state.image_preview_identity = image_previews::next_document_identity();
            let mut actors = Vec::new();
            state.song_background_actors(&mut actors);
            assert!(image(&actors).is_none());
            state.configure_song_background_policy(BackgroundPolicy {
                hide: true,
                size: BackgroundSize::Window,
            });
            assert!(state.image_preview_request().is_none());
            actors.clear();
            state.song_background_actors(&mut actors);
            assert!(image(&actors).is_none());
        }
    }
    #[test]
    fn source_unknown_size_keeps_average_clear_without_image_and_no_song_stays_empty() {
        let (_root, mut state) = state("en");
        state.configure_song_background_policy(BackgroundPolicy {
            hide: false,
            size: BackgroundSize::Unknown(2),
        });
        let mut actors = Vec::new();
        state.song_background_actors(&mut actors);
        assert_eq!(actors.len(), 1);
        assert!(image(&actors).is_none());
        assert!(matches!(&actors[0],Actor::Sprite{tint,..} if *tint==[0.2,0.3,0.4,1.0]));
        state.document_presence = DocumentPresence::NoSong;
        assert!(state.image_preview_request().is_none());
        actors.clear();
        state.song_background_actors(&mut actors);
        assert!(matches!(&actors[0],Actor::Sprite{tint,..} if *tint==[0.0,0.0,0.0,1.0]));
    }
}

#[cfg(test)]
mod source_oracle_tests {
    use super::*;
    #[test]
    fn original_box_geometry_and_hsv_clear_color_match_cpu_source_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../deadsync-editor/tests/fixtures/song-background/expected.json"
        )))
        .unwrap();
        for case in fixture["Geometry"].as_array().unwrap() {
            let input = case["Input"].as_array().unwrap();
            let actual = box_fit(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: input[2].as_f64().unwrap() as f32,
                    height: input[3].as_f64().unwrap() as f32,
                },
                [
                    input[0].as_u64().unwrap() as u32,
                    input[1].as_u64().unwrap() as u32,
                ],
            )
            .unwrap();
            for (actual, expected) in [actual.x, actual.y, actual.width, actual.height]
                .into_iter()
                .zip([
                    case["Offset"][0].as_f64().unwrap(),
                    case["Offset"][1].as_f64().unwrap(),
                    case["Size"][0].as_f64().unwrap(),
                    case["Size"][1].as_f64().unwrap(),
                ])
            {
                assert!((f64::from(actual) - expected).abs() < 0.0001);
            }
        }
        for case in fixture["Colors"].as_array().unwrap() {
            let pixels = case["Pixels"].as_array().unwrap();
            let bytes: Vec<u8> = pixels
                .iter()
                .flat_map(|p| (p.as_u64().unwrap() as u32).to_le_bytes())
                .collect();
            let image = RgbaImage::from_raw(pixels.len() as u32, 1, bytes).unwrap();
            for (actual, expected) in texture_color(&image)
                .into_iter()
                .zip(case["Color"].as_array().unwrap())
            {
                assert!(
                    (f64::from(actual) - expected.as_f64().unwrap()).abs() <= 1.0 / 255.0 + 1e-6
                );
            }
        }
    }
}
