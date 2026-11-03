//! Source OnMusicOffsetChanged keeps the focused *double row*, then seeks the
//! musical clock. Only the real ChartProperties producer/history enters here.
use super::*;
pub(super) struct Presentation {
    exact: playback::SongPropertyPresentation,
    timing: playback::TimingPresentation,
}
impl Presentation {
    pub(super) fn focus(&self) -> &deadsync_editor::chart_focus::Position {
        self.exact.focus()
    }
}
impl State {
    pub(super) fn capture_chart_property_presentation(
        &self,
    ) -> Result<Option<Presentation>, deadsync_editor::Error> {
        if !self.workspace.has_chart() {
            return Ok(None);
        }
        Ok(Some(Presentation {
            exact: self.capture_song_property_presentation()?,
            timing: self.capture_timing_presentation()?,
        }))
    }
    pub(super) fn finish_chart_property_presentation(
        &mut self,
        before: Option<Presentation>,
        position: Option<deadsync_editor::chart_focus::Position>,
        chart: usize,
        audio: bool,
        applied: &deadsync_editor::song_properties::Application,
    ) -> Result<(), deadsync_editor::Error> {
        let Some(before) = before else {
            return Ok(());
        };
        if let Some(receipt) = applied
            .presentation_receipt()
            .filter(|receipt| receipt.preserves_audio())
        {
            return self.rebind_song_property_presentation(before.exact, receipt);
        }
        let focused = self.workspace.chart();
        let position = position.ok_or(deadsync_editor::Error::ExternalChange)?;
        // Source OnMusicChanged stops audition only; offset changes do not. Main's
        // asset replacement and musical seek are owned by the canonical shared path.
        if chart == focused && !audio && self.is_playing() && !self.chart_transport_is_playing() {
            self.stop_audio();
        }
        self.finish_timing_presentation(before.timing, position)
    }
    pub(super) fn apply_chart_properties_history(&mut self, redo: bool) {
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let session = &self.workspace.file().session;
            let chart = session
                .chart_properties_history_chart(redo)
                .ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("No chart properties history".into())
                })?;
            let audio = session
                .chart_properties_history_preserves_audio(redo)
                .unwrap_or(false);
            let before = self.capture_chart_property_presentation()?;
            let position = before
                .as_ref()
                .map(|before| {
                    self.workspace
                        .file()
                        .session
                        .chart_properties_history_position(redo, before.focus())
                })
                .transpose()?;
            let applied = self.workspace.apply_chart_properties_history(redo)?;
            if applied.changed() {
                self.finish_chart_property_presentation(before, position, chart, audio, &applied)?;
            }
            Ok(())
        })();
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "Ready"),
            Err(error) => i18n::editor_error(&error).into(),
        };
        self.refresh();
    }
}
