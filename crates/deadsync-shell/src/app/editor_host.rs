//! Native editor host adapter. No gameplay session, song or chart is fabricated.
use super::*;
use crate::editor_entry::EditorLaunch;
use screens::edition::{EditorEntryRequest, State as Editor};

impl ScreensState {
    pub(super) fn active_editor(&self) -> Option<&Editor> {
        match self.current_screen {
            CurrentScreen::Edition => self.edition_state.as_ref(),
            CurrentScreen::Practice => self.practice_state.as_ref().and_then(practice::editor),
            _ => None,
        }
    }
    pub(super) fn active_editor_mut(&mut self) -> Option<&mut Editor> {
        match self.current_screen {
            CurrentScreen::Edition => self.edition_state.as_mut(),
            CurrentScreen::Practice => self.practice_state.as_mut().and_then(practice::editor_mut),
            _ => None,
        }
    }
}
impl App {
    pub(super) fn prepare_standalone_editor(&mut self) -> bool {
        if self.state.screens.edition_state.is_some() {
            return true;
        }
        let request = match self
            .state
            .screens
            .editor_launch
            .take()
            .unwrap_or(EditorLaunch::Startup)
        {
            EditorLaunch::Startup => EditorEntryRequest::Startup,
            EditorLaunch::Empty => EditorEntryRequest::Empty,
            EditorLaunch::File(path) => EditorEntryRequest::Explicit(path),
        };
        match Editor::prepare_entry(&self.dirs.data_dir, request) {
            Ok(state) => {
                self.gameplay_sfx.prepare_editor_ticks(&mut self.audio);
                self.state.screens.edition_state = Some(state);
                true
            }
            Err(error) => {
                log::error!("Cannot prepare standalone editor: {error}");
                self.state.shell.interaction.show_message(
                    deadsync_theme_simply_love::i18n::tr("Edition", "EntryFailed").to_string(),
                    Instant::now(),
                );
                false
            }
        }
    }
    pub(super) fn flush_active_editor_audio(&mut self) {
        if let Some(editor) = self.state.screens.active_editor_mut() {
            crate::gameplay_runtime::flush_standalone_editor(
                editor,
                &mut self.audio,
                &self.gameplay_sfx,
                &mut self.music_clock,
            );
        }
    }
    pub(super) fn authorize_editor_departure(&mut self, target: CurrentScreen) -> bool {
        if consume_departure_authority(&mut self.state.screens.editor_departure_authority,target) {
            return true;
        }
        // A Close under review or awaiting restoration owns its original
        // destination. Cancel it before asking for a different navigation.
        if self.state.screens.active_editor().is_some_and(|editor|editor.host_close_pending()) {
            return false;
        }
        let Some(editor) = self.state.screens.active_editor_mut() else {
            return true;
        };
        if editor.request_host_close() {
            editor.release_host_audio();
            self.flush_active_editor_audio();
            if self.state.screens.current_screen==CurrentScreen::Practice {
                if let Some(state)=self.state.screens.practice_state.as_mut(){practice::finish_editor_host_close(state);}
            }
            true
        } else {
            self.state.screens.editor_departure_target =
                editor.host_close_pending().then_some(target);
            self.flush_active_editor_audio();
            false
        }
    }
    pub(super) fn handle_standalone_raw_key(&mut self, event: &RawKeyboardEvent) {
        let Some(editor) = self.state.screens.active_editor_mut() else {
            return;
        };
        let close = editor.raw_key(event);
        if close {
            editor.release_host_audio();
        }
        self.flush_active_editor_audio();
        if close {
            let target = self
                .state
                .screens
                .editor_departure_target
                .take()
                .unwrap_or(CurrentScreen::Menu);
            self.state.screens.editor_departure_authority=Some(target);
            self.handle_navigation_action(target);
        }
    }
}

/// A completed close authorizes its exact destination once, including Practice
/// whose editor State was already removed by its audio-ack update.
fn consume_departure_authority(authority:&mut Option<CurrentScreen>,target:CurrentScreen,
)->bool {
    authority.take()==Some(target)
}
#[cfg(test)]
mod departure_authority_tests {
    use super::*;
    #[test]
    fn exact_destination_authority_is_consumed_once_without_an_active_editor() {
        let mut authority=Some(CurrentScreen::Options);
        assert!(consume_departure_authority(&mut authority,CurrentScreen::Options));
        assert!(authority.is_none());
        assert!(!consume_departure_authority(&mut authority,CurrentScreen::Menu));
    }
    #[test]
    fn a_different_destination_revokes_the_old_authority() {
        let mut authority=Some(CurrentScreen::Options);
        assert!(!consume_departure_authority(&mut authority,CurrentScreen::Edition));
        assert!(authority.is_none());
        assert!(!consume_departure_authority(&mut authority,CurrentScreen::Options));
    }
}

/// Removing the Practice editor does not remove its one-turn close authority.
pub(super) fn requires_departure_check(from:CurrentScreen,target:CurrentScreen,active_editor:bool,authority:bool,
)->bool {
    target!=from && (active_editor||authority)
}
#[cfg(test)]
mod route_after_audio_ack_tests {
    use super::*;
    #[test]
    fn practice_removal_still_consumes_authority_before_next_editor_entry() {
        let mut authority=Some(CurrentScreen::Menu);
        assert!(requires_departure_check(CurrentScreen::Practice,CurrentScreen::Menu,false,authority.is_some()));
        assert!(consume_departure_authority(&mut authority,CurrentScreen::Menu));
        assert!(!requires_departure_check(CurrentScreen::Menu,CurrentScreen::Edition,false,authority.is_some()));
        assert!(requires_departure_check(CurrentScreen::Edition,CurrentScreen::Options,true,authority.is_some()));
        assert!(!consume_departure_authority(&mut authority,CurrentScreen::Options));
    }
}

impl App{pub(super) fn sync_editor_ui_dpi(&mut self){let Some((monitor,size))=self.window.as_ref().map(|window|(window.scale_factor(),window.inner_size()))else{return;};if let Some(editor)=self.state.screens.active_editor_mut(){editor.set_ui_dpi_viewport(size.width,size.height);editor.update_host_ftue();if !editor.ui_dpi_is_bound(){if let Err(error)=editor.bind_ui_dpi(monitor){log::warn!("Editor UI DPI: {error}");}}}}}

impl App{pub(super) fn sync_editor_notifications(&mut self){
 let snapshot=deadsync_updater::state::editor_snapshot();
 let (checked,available)=match snapshot{deadsync_updater::state::EditorSnapshot::Unknown=>(false,None),deadsync_updater::state::EditorSnapshot::UpToDate=>(true,None),deadsync_updater::state::EditorSnapshot::Available{tag,url}=> {
                (false,Some((tag,url)))
            }
        };
 let request=if let Some(editor)=self.state.screens.active_editor_mut(){editor.observe_editor_about_update(checked,available.clone());if let Some((tag,url))=available{editor.offer_available_update(&tag,&url);}editor.take_notification_url()}else{None};
 crate::editor_external_open::poll();
 if let Some(url)=request{if !crate::editor_external_open::request(crate::editor_external_open::Target::Url(url)){log::warn!("Editor external opener busy or target rejected");}}
 let request=self.state.screens.active_editor_mut().and_then(|editor|editor.take_editor_external_open());
 if let Some(request)=request{let target=match request{screens::edition::editor_help::ExternalOpen::Url(url)=> {
                    Some(crate::editor_external_open::Target::Url(url.into()))
                }
                screens::edition::editor_help::ExternalOpen::LogFile=> {
                    deadlib_platform::logging::current_log_path().map(crate::editor_external_open::Target::File)}
            };if let Some(target)=target{if !crate::editor_external_open::request(target){if let Some(editor)=self.state.screens.active_editor_mut(){editor.external_open_refused(screens::edition::editor_help::ExternalOpenRefusal::Busy,
                        );}log::warn!("Editor external opener busy or target rejected");}}else{if let Some(editor)=self.state.screens.active_editor_mut(){editor.external_open_refused(screens::edition::editor_help::ExternalOpenRefusal::Unavailable,
                    );}log::warn!("No configured editor log file");}}
 self.sync_editor_log_view();
 // Host notification side effect only: no actor traversal, gameplay audio
 // command, lease change, or render callback. Candidates are never consumed.
 if self.window.as_ref().is_some_and(|window| {
            window.is_visible()!=Some(false)&&window.is_minimized()!=Some(true)
        }) {
  if let Some(editor)=self.state.screens.active_editor_mut(){
                if let Some(path) = editor.take_editor_folder_request() {
                    editor.complete_folder_request(
                        deadlib_platform::open_path::reveal(&path)
                            .map_err(|error| error.to_string()),
                    );
                }
                if let Some(alert)=editor.take_editor_system_alert(){deadlib_platform::system_alert::play(alert);}
  }
 }
}}


impl App{pub(super) fn sync_editor_log_view(&mut self){if let Some(editor)=self.state.screens.active_editor_mut(){if editor.log_formatting_needed(){let locale=deadsync_theme_simply_love::i18n::current_locale();let mut formatter=crate::editor_log_formatter::NativeLogFormatter::new(&locale,editor.log_preferences().date,
                );editor.process_log_formatting(&mut formatter);}}}}

impl App { pub(super) fn sync_editor_window_layout(&mut self,event_loop:&winit::event_loop::ActiveEventLoop,
    ){
 if !self.state.screens.active_editor_mut().is_some_and(|editor| {
                editor.editor_resolution_menu_open()||editor.editor_resolution_requested()
            }){return;}
 let modes=crate::graphics::available_monitor_specs(event_loop).get(self.state.shell.display_monitor).map(|monitor| {
                monitor.modes.iter().map(|mode|(mode.width,mode.height)).collect::<Vec<_>>()
            }).unwrap_or_default();
 let request=if let Some(editor)=self.state.screens.active_editor_mut(){editor.set_editor_resolutions(modes.clone());editor.take_editor_resolution()}else{None};
 if let Some((width,height))=request{let result=if modes.contains(&(width,height)){self.apply_resolution(width,height,event_loop).map_err(|error|error.to_string())}else{Err("Monitor mode no longer available".into())};
 if result.is_ok(){deadsync_config::runtime_update::update_display_resolution(width,height);deadsync_theme_simply_love::screens::options::sync_display_resolution(&mut self.state.screens.options_state,width,height,
                );}
 if let Some(editor)=self.state.screens.active_editor_mut(){editor.editor_resolution_result(result);}
 }
}}
