//! Use the source stream notation independently of density graph visibility.
use super::*;
use deadsync_editor::{
    density::{
        StreamSettings,
        task::{Report, Task},
    },
    generation::job::Job,
};

pub(super) struct Pending {
    job: Job<Report>,
    stream: StreamSettings,
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::{
        chart_text::{Field, Text},
        document::Format,
    };
    use std::time::{Duration, Instant};

    fn fixture(format: Format, fr: bool) -> (tempfile::TempDir, State) {
        if fr {
            i18n::init(deadsync_assets::language::load_for_tests("fr"));
        } else {
            i18n::init_for_tests();
        }
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        let header = "#VERSION:0.83;#TITLE:stream;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};";
        let chart = if format == Format::Sm {
            "#NOTES:dance-single:manual:Hard:9::1000\n0100\n0010\n0001;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:manual;#NOTES:1000\n0100\n0010\n0001;"
        };
        std::fs::write(&path, format!("{header}{chart}")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let mut density = state.density_preferences();
        density.visible = false;
        state.configure_density_preferences(density).unwrap();
        let mut stream = state.stream_preferences();
        stream.note_type = deadsync_editor::stream_preferences::NoteType::Quarter;
        state.configure_stream_preferences(stream).unwrap();
        let commands = state.take_audio_commands();
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(
                    command,
                    deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { .. }
                ))
                .count(),
            1
        );
        state.update_audio(deadsync_gameplay::GameplayAudioSnapshot::default());
        state.take_audio_commands();
        state.open_chart_properties();
        assert!(matches!(state.panel, Some(Panel::ChartProperties(_))));
        (root, state)
    }
    fn press_f5(state: &mut State) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code: KeyCode::F5,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn finish(state: &mut State) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.stream_description.is_some() {
            assert!(Instant::now() < deadline, "{}", state.status);
            state.update();
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn physical_use_stream_works_with_hidden_graph_and_one_undo_sm_ssc_en_fr() {
        for fr in [false, true] {
            for format in [Format::Sm, Format::Ssc] {
                let (_root, mut state) = fixture(format, fr);
                state.navigate_exact_position(60.5, true);
                let focus = state.focus_position.as_ref().unwrap().clone();
                let before = state.workspace.file().session.serialize().unwrap();
                press_f5(&mut state);
                assert!(state.stream_description.is_some(), "{}", state.status);
                finish(&mut state);
                assert_eq!(
                    state.status,
                    i18n::tr("Edition", "StreamDescriptionApplied")
                );
                let description = Text::read(state.workspace.file().session.document(), 0)
                    .unwrap()
                    .get(Field::Description)
                    .to_owned();
                assert!(!description.is_empty());
                assert_ne!(description, "manual");
                assert!(
                    state.source_density_report().is_none(),
                    "hidden graph must stay hidden"
                );
                let after = state.focus_position.as_ref().unwrap();
                assert!(after.is_current(&state.workspace));
                assert_eq!(after.row(), focus.row());
                assert_eq!(after.music_time(), focus.music_time());
                assert!(state.take_audio_commands().is_empty());
                press_f5(&mut state);
                finish(&mut state);
                assert_eq!(
                    state.status,
                    i18n::tr("Edition", "StreamDescriptionUnchanged")
                );
                state.command(Command::Undo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert!(state.workspace.file().session.undo_label().is_none());
                state.command(Command::Redo);
                assert_eq!(
                    Text::read(state.workspace.file().session.document(), 0)
                        .unwrap()
                        .get(Field::Description),
                    description
                );
            }
        }
    }
    #[test]
    fn unreviewed_drafts_and_stale_reports_never_replace_description() {
        let (_root, mut state) = fixture(Format::Ssc, false);
        let before = state.workspace.file().session.serialize().unwrap();
        state
            .chart_properties
            .as_mut()
            .unwrap()
            .set_text(Field::Name, "pending")
            .unwrap();
        press_f5(&mut state);
        assert!(state.stream_description.is_none());
        assert_eq!(
            state.status,
            i18n::tr("Edition", "StreamDescriptionDraftPending")
        );
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        state.chart_properties = Some(state.workspace.chart_properties().unwrap());
        press_f5(&mut state);
        assert!(state.stream_description.is_some());
        let mut edit = state.workspace.song_properties().unwrap();
        edit.set_text(deadsync_editor::song_text::Field::Title, "new source")
            .unwrap();
        state.workspace.apply_song_properties(&edit).unwrap();
        let edited = state.workspace.file().session.serialize().unwrap();
        finish(&mut state);
        assert_eq!(state.status, i18n::tr("Edition", "StreamDescriptionFailed"));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), edited);
        assert_eq!(
            Text::read(state.workspace.file().session.document(), 0)
                .unwrap()
                .get(Field::Description),
            "manual"
        );
    }
}

impl State {
    pub(super) fn use_stream_description(&mut self) {
        if !self.stream_description_editable() {
            return;
        }
        if self.stream_description.is_some() {
            self.status = i18n::tr("Edition", "StreamDescriptionLoading");
            return;
        }
        if let Some(report) = self.source_density_report() {
            let draft =
                report.description_draft(&self.workspace.file().session, self.workspace.chart());
            self.commit_stream_description(draft);
            return;
        }
        let stream = self.stream_preferences().density_settings();
        match Task::capture(
            &self.workspace.file().session,
            self.workspace.chart(),
            stream,
        )
        .and_then(Task::spawn)
        {
            Ok(job) => {
                self.stream_description = Some(Pending { job, stream });
                self.status = i18n::tr("Edition", "StreamDescriptionLoading");
            }
            Err(_) => self.status = i18n::tr("Edition", "StreamDescriptionFailed"),
        }
    }
    fn stream_description_editable(&mut self) -> bool {
        if !self.workspace.has_chart() || self.file_io_active() {
            self.status = i18n::tr("Edition", "StreamDescriptionUnavailable");
            return false;
        }
        if self
            .chart_properties
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "StreamDescriptionDraftPending");
            return false;
        }
        true
    }
    pub(super) fn poll_stream_description(&mut self) {
        if !matches!(self.panel, Some(Panel::ChartProperties(_))) {
            // Dropping Job requests cancellation; its worker retains its permit
            // until it really exits, using the common generation job contract.
            self.stream_description = None;
            return;
        }
        let Some(pending) = self.stream_description.as_mut() else {
            return;
        };
        let Some(result) = pending.job.poll() else {
            return;
        };
        let pending = self
            .stream_description
            .take()
            .expect("observed pending stream job");
        if pending.stream != self.stream_preferences().density_settings() {
            self.status = i18n::tr("Edition", "StreamDescriptionStale");
            return;
        }
        if !self.stream_description_editable() {
            return;
        }
        let draft = result.and_then(|report| {
            report.description_draft(&self.workspace.file().session, self.workspace.chart())
        });
        self.commit_stream_description(draft);
    }
    fn commit_stream_description(
        &mut self,
        result: Result<deadsync_editor::chart_properties::Draft, deadsync_editor::Error>,
    ) {
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            let draft = result?;
            if !draft.has_changes() {
                return Ok(false);
            }
            let presentation = self.capture_song_property_presentation()?;
            let applied = self
                .workspace
                .apply_chart_properties_with_presentation_receipt(&draft)?;
            if applied.changed() {
                let receipt = applied.presentation_receipt().ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit(
                        "Stream description unexpectedly changed the chart clock".into(),
                    )
                })?;
                self.rebind_song_property_presentation(presentation, receipt)?;
                self.chart_properties = Some(self.workspace.chart_properties()?);
                self.refresh();
            }
            Ok(applied.changed())
        })();
        self.status = i18n::tr(
            "Edition",
            match result {
                Ok(true) => "StreamDescriptionApplied",
                Ok(false) => "StreamDescriptionUnchanged",
                Err(_) => "StreamDescriptionFailed",
            },
        );
    }
}
