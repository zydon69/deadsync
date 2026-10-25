use super::*;

impl State {
    pub fn spacing_zoom(&self) -> f64 {
        self.zoom.spacing_zoom()
    }

    pub(super) fn zoom_key(&mut self, event: &RawKeyboardEvent) -> bool {
        if !self.control {
            return false;
        }
        match event.code {
            KeyCode::ArrowUp | KeyCode::ArrowDown => {
                let delta = if event.code == KeyCode::ArrowUp {
                    1.0
                } else {
                    -1.0
                };
                self.zoom
                    .process_input(
                        event
                            .timestamp
                            .saturating_duration_since(self.zoom_epoch)
                            .as_secs_f64(),
                        delta,
                        true,
                        false,
                        deadsync_editor::zoom::SpacingMode::ConstantRow,
                    )
                    .expect("finite keyboard zoom input");
            }
            KeyCode::Digit0 | KeyCode::Numpad0 => {
                self.zoom.set_zoom(1.0).expect("valid default zoom");
            }
            _ => return false,
        }
        self.status = i18n::tr("Edition", "ZoomHelp");
        true
    }
}
