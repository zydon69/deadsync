//! Event-copy handoff uses the actual focused clock, independently of the source.
use super::*;
use deadsync_editor::{
    Error,
    chart_event_copy::{
        EventType, Report,
        live::{Application, Plan},
    },
};
impl State {
    fn chart_copy_focus(
        &self,
    ) -> Result<
        Option<(
            deadsync_editor::chart_focus::Position,
            deadsync_editor::timing_edit::live::Anchor,
            TimingPresentation,
        )>,
        Error,
    > {
        if !self.workspace.has_chart() {
            return Ok(None);
        }
        let before = self.capture_timing_presentation()?;
        let position = self.capture_song_property_presentation()?.focus().clone();
        Ok(Some((position, before.anchor(), before)))
    }
    fn validate_chart_copy_draft(&self) -> Result<(), Error> {
        if self
            .song_properties
            .as_ref()
            .is_some_and(|draft| !draft.is_current(&self.workspace.file().session))
        {
            return Err(Error::ExternalChange);
        }
        Ok(())
    }
    fn finish_chart_copy(
        &mut self,
        before: Option<TimingPresentation>,
        application: Application,
    ) -> Result<Report, Error> {
        let report = application.report().clone();
        if !report.changed {
            return Ok(report);
        }
        if let Some(draft) = self.song_properties.as_mut() {
            draft.rebind_chart_event_copy(&self.workspace.file().session, application.receipt())?;
        }
        if let (Some(before), Some(position)) = (before, application.into_position()) {
            self.finish_timing_presentation(before, position)?;
        } else if let Some(paused) = self
            .paused_playback
            .as_mut()
            .filter(|paused| paused.chart == usize::MAX)
        {
            // Chartless event copy changes no Main descriptor or musical clock.
            paused.source = Arc::clone(self.workspace.file().session.document().source());
            paused.revision = self.workspace.file().session.content_revision();
        }
        self.retarget_editor_tick_windows();
        Ok(report)
    }
    pub(in crate::screens::edition) fn apply_chart_event_copy_presented(
        &mut self,
        source: usize,
        destinations: impl IntoIterator<Item = usize>,
        types: impl IntoIterator<Item = EventType>,
    ) -> Result<Report, Error> {
        self.validate_chart_copy_draft()?;
        let focus = self.chart_copy_focus()?;
        let (projection, before) = match focus {
            Some((position, anchor, before)) => (Some((position, anchor)), Some(before)),
            None => (None, None),
        };
        let plan =
            self.workspace
                .prepare_chart_event_copy(source, destinations, types, projection)?;
        let application = self.workspace.apply_chart_event_copy_live(plan)?;
        self.finish_chart_copy(before, application)
    }
    pub(in crate::screens::edition) fn apply_chart_event_copy_history(&mut self, redo: bool) {
        let result = (|| {
            self.validate_chart_copy_draft()?;
            let focus = self.chart_copy_focus()?;
            let (projection, before) = match focus {
                Some((position, anchor, before)) => (Some((position, anchor)), Some(before)),
                None => (None, None),
            };
            let plan = Plan::prepare_history(&self.workspace.file().session, projection, redo)?;
            let application = self.workspace.apply_chart_event_copy_live(plan)?;
            self.finish_chart_copy(before, application)?;
            Ok::<_, Error>(())
        })();
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "Ready"),
            Err(error) => i18n::editor_error(&error).into(),
        };
        self.refresh();
    }
}
