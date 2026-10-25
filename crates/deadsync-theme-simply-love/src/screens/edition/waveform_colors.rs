use super::*;
use deadsync_editor::waveform_palette::{Palette, SparseMode};
impl State {
    pub(super) fn waveform_colors_label(&self, index: usize, key: &str) -> Arc<str> {
        let p = self
            .waveform_preferences_draft
            .unwrap_or(self.waveform_preferences)
            .palette();
        let value = if index == 0 {
            i18n::tr("Edition", p.mode().name()).to_string()
        } else if index == 1 {
            format!("{:.1}", p.darken() * 100.0)
        } else {
            let color = if index < 6 {
                p.dense()
            } else if index < 10 {
                p.sparse()
            } else {
                p.background()
            };
            format!("{:.3}", color.rgba()[(index - 2) % 4])
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn waveform_color_swatch(&self, index: usize) -> [f32; 4] {
        let p = self
            .waveform_preferences_draft
            .unwrap_or(self.waveform_preferences)
            .palette();
        if index < 2 {
            p.resolved_sparse().rgba()
        } else if index < 6 {
            p.dense().rgba()
        } else if index < 10 {
            p.sparse().rgba()
        } else {
            p.background().rgba()
        }
    }
    pub(super) fn waveform_colors_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 14 && enter) {
            self.panel = Some(Panel::WaveformSettings(0));
            self.status = i18n::tr("Edition", "WaveformSettingsHelp");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::WaveformColors(
                (index + if back { 14 } else { 1 }) % 15,
            ));
            return;
        }
        if index >= 14
            || !(matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            ) || (index == 0 && enter))
        {
            return;
        }
        let Some(settings) = self.waveform_preferences_draft.as_mut() else {
            return;
        };
        let mut palette = settings.palette();
        let defaults = Palette::default();
        let home = code == KeyCode::Home;
        let zero = matches!(code, KeyCode::Digit0 | KeyCode::Numpad0);
        let delta = if self.shift { 0.001 } else { 0.01 }
            * if code == KeyCode::ArrowLeft {
                -1.0
            } else {
                1.0
            };
        let result = if index == 0 {
            let current = SparseMode::ALL
                .iter()
                .position(|m| *m == palette.mode())
                .unwrap();
            palette.set_mode(if home || zero {
                defaults.mode()
            } else {
                SparseMode::ALL[(current + if code == KeyCode::ArrowLeft { 2 } else { 1 }) % 3]
            });
            Ok(())
        } else if index == 1 {
            palette.set_darken(if home {
                defaults.darken()
            } else if zero {
                0.0
            } else {
                (palette.darken() + delta).clamp(0.0, 1.0)
            })
        } else {
            let (color, default) = if index < 6 {
                (palette.dense(), defaults.dense())
            } else if index < 10 {
                (palette.sparse(), defaults.sparse())
            } else {
                (palette.background(), defaults.background())
            };
            let component = (index - 2) % 4;
            let value = if home {
                default.rgba()[component]
            } else if zero {
                0.0
            } else {
                (color.rgba()[component] + delta).clamp(0.0, 1.0)
            };
            color.with_component(component, value).map(|color| {
                if index < 6 {
                    palette.set_dense(color);
                } else if index < 10 {
                    palette.set_sparse(color);
                } else {
                    palette.set_background(color);
                }
            })
        };
        match result {
            Ok(()) => settings.set_palette(palette),
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
}
