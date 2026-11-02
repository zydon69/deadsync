//! Source speed controls are direct (not Undo); Reverse is an Undoable flag.
use super::*;
use deadsync_editor::{
    scroll_preferences::Preferences, zoom::SpacingMode,
};
impl State {
    pub(super) fn source_scroll_speed_enabled(&self,index:usize)->bool {
        use deadsync_editor::scroll_preferences::spacing::Preference;
        matches!((self.spacing_preference,index),(Preference::ConstantTime,0)|(Preference::ConstantRow,1)|(Preference::Variable,2))
    }
    pub(super) fn install_scroll_preferences(
        &mut self,
        settings: Preferences,
    ) -> Result<(), deadsync_editor::Error> {
        let mut zoom = self.zoom.clone();
        for (i, mode) in [
            SpacingMode::ConstantTime,
            SpacingMode::ConstantRow,
            SpacingMode::Variable,
        ]
        .into_iter()
        .enumerate()
        {
            let value=settings.speed(i).unwrap();
            if zoom.spacing(mode)!=value {zoom.set_spacing(mode,value)?;}
        }
        self.zoom = zoom;
        self.reverse_display = settings.reverse();
        self.scroll_preferences = settings;
        Ok(())
    }
    pub(super) fn current_spacing_speeds(&self) -> deadsync_editor::spacing::Speeds {
        deadsync_editor::spacing::Speeds {
            time: self.zoom.spacing(SpacingMode::ConstantTime),
            row: self.zoom.spacing(SpacingMode::ConstantRow),
            variable: self.zoom.spacing(SpacingMode::Variable),
        }
    }
    pub(super) fn sync_scroll_preferences_history(&mut self)->Result<(),deadsync_editor::Error> {
        if let Some(update) = self.workspace.take_scroll_preferences_update() {
            self.install_scroll_preferences(update.preferences)?;
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
        Ok(())
    }
    pub(super) fn commit_scroll_preferences(
        &mut self,
        settings: Preferences,
    ) -> Result<bool, deadsync_editor::Error> {
        self.flush_wheel_settings()?;
        let file = self.capture_application_preferences_file()?;
        let changed = self.workspace.set_scroll_preferences(
            self.scroll_preferences,
            settings,
            Some(&file),
        )?;
        self.sync_scroll_preferences_history()?;
        Ok(changed)
    }
    pub(super) fn set_source_scroll_speed(
        &mut self,
        index: usize,
        value: f64,
    ) -> Result<(), deadsync_editor::Error> {
        self.publish_source_scroll_speed(index,value,false)
    }
    pub(super) fn set_source_scroll_speed_input(&mut self,index:usize,value:f64)->Result<(),deadsync_editor::Error>{
        self.publish_source_scroll_speed(index,value,true)
    }
    fn publish_source_scroll_speed(&mut self,index:usize,value:f64,typed_input:bool)->Result<(),deadsync_editor::Error>{
        let before = self.scroll_preferences;
        let mut settings = before;
        if typed_input{settings.set_speed_input(index,value as f32)?;}else{settings.set_speed(index,value)?;}
        if settings == before {
            return Ok(());
        }
        let current = self.capture_application_preferences_file()?;
        let mut owner = Some(current.clone());
        settings = deadsync_editor::audio_preferences::publication::publish(
            &mut owner, Some(&current), settings, |current| {
                let mut file = current.clone();
                let mut merged = file.scroll();
                if merged.speed(index) != before.speed(index) {
                    return Err(deadsync_editor::Error::ExternalChange);
                }
                if typed_input{merged.set_speed_input(index,settings.speed(index).unwrap() as f32)?;}else{merged.set_speed(index, settings.speed(index).unwrap())?;}
                file.set_scroll(merged);
                Ok((merged, file))
            },
        )?;
        if let Some(file) = owner {self.publish_application_preferences_owner(file);}
        self.install_scroll_preferences(settings)
    }
    pub(super) fn open_scroll_values(&mut self) {
        self.panel = Some(Panel::ScrollValues(0));
        self.status = i18n::tr("Edition", "ScrollValuesHelp");
    }
    pub(super) fn scroll_values_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape {
            self.panel = Some(Panel::ScrollSettings(0));
            return;
        }
        if matches!(code, KeyCode::Tab | KeyCode::ArrowUp | KeyCode::ArrowDown) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ScrollValues((index + if back { 3 } else { 1 }) % 4));
            return;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::Enter
                | KeyCode::NumpadEnter
        ) {
            return;
        }
        if index<3 && !self.source_scroll_speed_enabled(index){return;}
        let result = if index == 3 {
            let mut settings = self.scroll_preferences;
            settings.set_reverse(if code == KeyCode::Home {
                false
            } else {
                !settings.reverse()
            });
            self.commit_scroll_preferences(settings).map(|_| ())
        } else {
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                return;
            }
            let current = self.scroll_preferences.speed(index).unwrap();
            let step = if index == 1 {
                if self.shift { 0.001 } else { 0.1 }
            } else if self.shift {
                0.1
            } else {
                10.
            };
            let default = Preferences::default().speed(index).unwrap();
            let value = if code == KeyCode::Home {
                default
            } else {
                current
                    + if code == KeyCode::ArrowLeft {
                        -step
                    } else {
                        step
                    }
            };
            let (min, max) = if index == 1 {
                (0.1, 10000.)
            } else {
                (10., 100000.)
            };
            self.set_source_scroll_speed(index, value.clamp(min, max))
        };
        self.status = i18n::tr(
            "Edition",
            if result.is_ok() {
                "ScrollValuesSaved"
            } else {
                "ScrollValuesSaveFailed"
            },
        );
    }
    pub(super) fn scroll_values_label(&self, index: usize) -> Arc<str> {
        let key = [
            "ScrollTimeSpeed",
            "ScrollRowSpeed",
            "ScrollVariableSpeed",
            "ScrollReverse",
        ][index];
        let value = if index == 3 {
            i18n::tr(
                "Edition",
                if self.scroll_preferences.reverse() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string()
        } else {
            format!("{:.3}", self.scroll_preferences.speed(index).unwrap())
        };
        let label=i18n::tr_fmt("Edition", key, &[("value", &value)]);
        if index<3 && !self.source_scroll_speed_enabled(index){i18n::tr_fmt("Edition","PreferenceReadOnlyMode",&[("value",&label)])}else{label}
    }
}

#[cfg(test)]
mod source_projection_tests {
    use super::*;
    #[test]
    fn all_three_partition_axes_consume_installed_manager_speeds_and_reverse() {
        i18n::init_for_tests();
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let mut settings = Preferences::default();
        settings.set_speed(0, 450.).unwrap();
        settings.set_speed(1, 3.).unwrap();
        settings.set_speed(2, 600.).unwrap();
        s.install_scroll_preferences(settings).unwrap();
        for (mode, expected) in [
            (SpacingMode::ConstantTime, 225.),
            (SpacingMode::ConstantRow, 144.),
            // 600 px/s at the source reference tempo 300 BPM, here 120 BPM.
            (SpacingMode::Variable, 120.),
        ] {
            s.configure_spacing_preference(deadsync_editor::scroll_preferences::spacing::Preference::from_effective(mode));
            for reverse in [false, true] {
                settings.set_reverse(reverse);
                s.install_scroll_preferences(settings).unwrap();
                let axis = s.display_projection(0., 600., 300.).unwrap();
                let delta = axis.note_y(48).unwrap() - axis.note_y(0).unwrap();
                let expected = if reverse { -expected } else { expected };
                assert!(
                    (delta - expected).abs() < 1e-8,
                    "{mode:?} reverse={reverse}: {delta}"
                );
            }
        }
    }
}
