//! Clock-only presentation handoff: never replace a decoder or audition lease.
use super::*;
use deadsync_editor::{chart_focus::Position as Focus, Error};

pub(in crate::screens::edition) struct TimingPresentation {
    focus: Focus,
    main: bool,
    paused: bool,
}
impl TimingPresentation {
    pub(in crate::screens::edition) fn anchor(&self) -> deadsync_editor::timing_edit::live::Anchor {
        if self.main { deadsync_editor::timing_edit::live::Anchor::MusicTime(self.music_time()) }
        else { deadsync_editor::timing_edit::live::Anchor::Row(self.row()) }
    }
    pub(in crate::screens::edition) fn row(&self) -> f64 { self.focus.row() }
    pub(in crate::screens::edition) fn music_time(&self) -> f64 { self.focus.music_time() }
}
impl State {
    pub(in crate::screens::edition) fn capture_timing_presentation(&self) -> Result<TimingPresentation, Error> {
        let focus = self.capture_song_property_presentation()?.focus;
        Ok(TimingPresentation { focus, main: self.chart_transport_is_playing(), paused: self.paused_position().is_some() })
    }
    /// `focus` must be validated on the prospective committed session before
    /// publication. A stale token is refused before touching transport state.
    pub(in crate::screens::edition) fn finish_timing_presentation(&mut self, before: TimingPresentation, focus: Focus) -> Result<(), Error> {
        if !focus.is_bound_to(&self.workspace.file().session, self.workspace.chart()) {
            return Err(Error::InvalidEdit("Timing presentation belongs to another chart or revision".into()));
        }
        let prepared = if before.main || before.paused {
            // Unlike a focus change this keeps a logical Main transport alive
            // even when MUSIC is empty; None is not permission to stop it.
            Some(match self.prepare_chart_focus_audio(self.workspace.chart()) {
                Some(prepared)=>prepared,
                None=>(Preview::rebind_chart_asset(&self.workspace.file().session, self.workspace.chart(), self.workspace.file().resource_base())?,false),
            })
        } else { None };
        self.workspace.seek_preserving_selection(focus.edit_row())?;
        if let Some(views) = self.active_views.as_mut() {
            views.synchronize_focused_position(&self.workspace.file().session, &focus)?;
        }
        if let Some(preview) = prepared {
            // Timing has no authority to change the asset, owner or generation.
            self.apply_chart_focus_audio(Some(preview), before.paused, &focus);
            if before.main && before.music_time().to_bits() != focus.music_time().to_bits() {
                let playing = self.playback.as_mut().expect("captured Main transport");
                playing.valid_after = Instant::now();
                playing.mapped = false;
                if let Some(clock) = playing.visual_clock.as_mut() { clock.seek(focus.music_time(), playing.valid_after); }
                self.audio_commands.push(GameplayAudioCommand::SeekMusic(focus.music_time() + self.audio_preferences.effective().offset));
            }
        }
        self.focus_position = Some(focus);
        self.retarget_editor_tick_windows();
        Ok(())
    }
}
impl State {
    pub(in crate::screens::edition) fn apply_live_timing_history(&mut self, redo:bool) {
        let result=(|| {
            if !self.workspace.has_chart() {
                self.workspace.apply_unfocused_timing_history(redo)?;
                if let Some(paused)=self.paused_playback.as_mut().filter(|paused|paused.chart==usize::MAX){
                    paused.source=Arc::clone(self.workspace.file().session.document().source());
                    paused.revision=self.workspace.file().session.content_revision();
                }
                self.retarget_editor_tick_windows();
                return Ok(());
            }
            let before=self.capture_timing_presentation()?;
            let plan=deadsync_editor::timing_edit::live::Plan::prepare_history(&self.workspace.file().session,self.workspace.chart(),redo,before.anchor())?;
            let application=self.workspace.apply_timing_live_plan(plan)?;
            self.finish_timing_presentation(before,application.into_position())?;
            Ok::<_,Error>(())
        })();
        self.status=match result {Err(error)=>i18n::editor_error(&error).into(),Ok(())=>i18n::tr("Edition","Ready")};
        self.refresh();
    }
}
