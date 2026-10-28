use super::*;
use std::time::Instant;

impl State {
    pub fn spacing_zoom(&self) -> f64 {
        self.zoom.spacing_zoom()
    }

    pub(super) fn command_zoom(&mut self, delta: Option<f32>, now: Instant) {
        if let Some(delta) = delta {
            self.zoom
                .process_input(
                    now.saturating_duration_since(self.zoom_epoch).as_secs_f64(),
                    delta,
                    true,
                    false,
                    self.spacing_mode(),
                )
                .expect("finite keyboard zoom input");
        } else {
            self.zoom.set_zoom(1.0).expect("valid default zoom");
        }
        self.status = i18n::tr("Edition", "ZoomHelp");
    }
}
