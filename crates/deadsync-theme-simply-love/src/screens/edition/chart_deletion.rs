use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::workspace::Command;
impl State {
    pub(super) fn open_chart_deletion(&mut self) {
        match self.workspace.deletion_draft() {
            Ok(draft) => {
                self.deletion = Some(draft);
                self.queue_song_timing_order(super::song_timing_chart::Purpose::Deletion);
                self.panel = Some(Panel::DeleteChart(0));
                self.status = i18n::tr("Edition", "DeleteChartHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn deletion_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::DeleteChart(1 - index));
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 0 {
                self.deletion = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "Ready");
                return;
            }
            if self.song_timing_request.is_some(){self.status=i18n::tr("Edition","SongTimingLoading");return;}
            let Some(draft) = self.deletion.as_ref() else {
                return;
            };
            let presentation=match self.capture_structural_chart_presentation(){Ok(value)=>value,Err(error)=>{self.status=i18n::editor_error(&error).into();return;}};
            match self.workspace.delete_chart(draft) {
                Ok(()) => {
                    self.deletion = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "ChartDeleted");
                    if let Err(error)=self.finish_structural_chart_presentation(presentation){self.status=i18n::editor_error(&error).into();}
                    self.refresh();
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                    )
                }
            }
        }
    }
    pub(super) fn deletion_summary(&self) -> std::sync::Arc<str> {
        let Some(draft) = &self.deletion else {
            return i18n::tr("Edition", "DeleteChartHelp");
        };
        i18n::tr_fmt(
            "Edition",
            if draft.is_last() {
                "DeleteLastChartSummary"
            } else {
                "DeleteChartSummary"
            },
            &[
                ("layout", draft.layout()),
                ("description", draft.description()),
            ],
        )
    }
    pub(super) fn no_chart_key(&mut self, _code: KeyCode) -> bool {
        self.status = i18n::tr("Edition", "NoChartHelp");
        false
    }
    pub(super) fn empty_menu_key(&mut self, index: usize, code: KeyCode) -> bool {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::EmptyMenu(
                (index + if code == KeyCode::ArrowUp { super::EMPTY_MENU_KEYS.len()-1 } else { 1 }) % super::EMPTY_MENU_KEYS.len(),
            ));
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            self.panel = None;
            match index {
                0 => self.open_new_chart(),
                1 => self.command(Command::Undo),
                2 => self.command(Command::Redo),
                3 => self.command(Command::Save),
                4 => self.command(Command::SaveRecovery),
                5 => self.command(Command::Recover),
                7 => self.open_pack_properties(),
                8 => self.open_pack_song_list(),
                9 => self.open_save_as(),
                10 => self.open_output_settings(),
                11 => self.open_file_menu(),
                12 => self.open_density_settings(),
                13 => self.open_dark_settings(),
                14 => self.open_stream_settings(),
                15=>self.open_display_options(),
                16=>self.open_general_options(),
                17=>self.open_log(),
                18=>self.open_editor_help(),
                19=>self.open_performance(),
                20=>self.open_profile_manager(),
                21=>self.open_assistant(),
                _ => return self.request_close(),
            }
        }
        false
    }
    pub(super) fn no_chart_actors(&self, actors: &mut Vec<Actor>) {
        let help = i18n::tr("Edition", if self.has_song(){"NoChartHelp"}else{"FileNoSongHelp"},
        );
        actors.push(act!(text: font("miso"): settext(help): align(0.0,0.0): xy(30.0,screen_height()*0.4): zoom(0.65): wrapwidthpixels((screen_width()-60.0)/0.65): maxwidth(screen_width()-60.0): maxheight(100.0): z(8006)));
        if let Some(panel) = self.panel {
            self.panel_actors(panel, actors);
        }
    }
}

#[cfg(test)]
mod profile_manager_no_focus_tests {
    use super::super::{ProfileTarget, RawKeyboardEvent, chart_views::ViewAction};
    use super::*;
    fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0,
            });}}
    #[test]
    fn close_last_view_then_keyboard_menu_opens_every_global_inspector_without_changing_song(){
        for language in ["en","fr"]{
            crate::i18n::init_for_tests();crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            ).unwrap();let mut state=State::open(&path,0).unwrap();state.profile_directory=Some(root.path().to_owned());
            state.chart_views_apply(ViewAction::Close);assert!(state.has_open_song());assert!(!state.workspace.has_chart());assert_eq!(state.workspace.file().session.document().charts().len(),1);let before=state.workspace.file().session.serialize().unwrap();let ids=state.workspace.file().session.chart_ids().to_vec();let revision=state.workspace.file().session.revision();
            press(&mut state,KeyCode::F1);assert!(matches!(state.panel,Some(Panel::EmptyMenu(0))));
            press(&mut state, KeyCode::ArrowUp);
            assert!(matches!(state.panel, Some(Panel::EmptyMenu(21))));
            // Assistant is appended after the application profile manager.
            press(&mut state,KeyCode::ArrowUp);assert!(matches!(state.panel,Some(Panel::EmptyMenu(20))));press(&mut state,KeyCode::Enter);
            let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);while state.catalog_job.is_some(){state.poll_profile_catalog();assert!(std::time::Instant::now()<deadline,"{}",state.status);std::thread::yield_now();}
            assert!(matches!(state.catalog_target,ProfileTarget::Application));assert!(matches!(state.panel,Some(Panel::Profiles(0))));
            for row in 0..3 {
                assert!(matches!(state.panel,Some(Panel::Profiles(index))if index==row));press(&mut state,KeyCode::KeyE);assert!(matches!(state.panel,Some(Panel::ProfileEdit(_))),"{}",state.status);press(&mut state,KeyCode::Escape);assert!(matches!(state.panel,Some(Panel::Profiles(index))if index==row));if row<2{press(&mut state,KeyCode::ArrowDown);}
            }
            press(&mut state,KeyCode::Escape);assert!(matches!(state.panel,Some(Panel::EmptyMenu(20))));press(&mut state,KeyCode::ArrowDown);assert!(matches!(state.panel,Some(Panel::EmptyMenu(21))));
            press(&mut state, KeyCode::ArrowDown);
            assert!(matches!(state.panel, Some(Panel::EmptyMenu(0))));assert!(!state.workspace.has_chart());assert!(state.focus_position.is_none());assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(state.workspace.file().session.chart_ids(),ids);assert_eq!(state.workspace.file().session.revision(),revision);assert!(state.workspace.file().session.undo_label().is_none());assert!(!state.workspace.file().session.is_dirty());assert_eq!(std::fs::read_dir(root.path()).unwrap().count(),1);
        }
    }
}
