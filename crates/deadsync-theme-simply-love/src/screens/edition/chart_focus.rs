use super::State;
use crate::i18n;
use deadsync_editor::chart_focus::Transition;
impl State {
    /// Exposes the exact music-file instant for chart/view presentation checks.
    /// It does not claim that a decoder is playing or an audio mapping exists.
    pub fn chart_focus_time(&self) -> Result<f64, deadsync_editor::Error> {
        self.chart_focus_music_time()
    }
    pub(super) fn focus_chart_at_time(&mut self, chart: usize, music_time: f64) {
        let prepared = Transition::at_time(&self.workspace, chart, music_time);
        match prepared {
            Ok(transition) => {
                // Preserve the existing intentional stop on chart navigation.
                // Capture transport time first, while the source is focused.
                self.stop_audio();
                match transition.apply(&mut self.workspace) {
                    Ok(position) => {
                        self.focus_position = Some(position);
                        self.status = i18n::tr("Edition", "Ready");
                    }
                    Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
                }
            }
            Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
        }
        self.refresh();
    }
}
