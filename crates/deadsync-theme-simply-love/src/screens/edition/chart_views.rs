//! Keyboard front end and headers for the shared open-view registry.
//! Simultaneous chart lanes are rendered by `view_render_screen`.
use super::State;
use crate::{act, i18n};
use deadlib_present::{actors::Actor, space::screen_width};
use deadsync_editor::{Error, chart_views::ActiveChartViews, generation::metadata::Guid};
pub(super) enum ViewAction {
    Pin,
    Close,
    Focus(bool),
    Move(bool),
}
impl State {
    pub fn open_chart_views(&self) -> Option<&ActiveChartViews> {
        self.active_views.as_ref()
    }
    pub(super) fn refresh_chart_views(&mut self) {
        let result = (|| -> Result<ActiveChartViews, Error> {
            let session = &self.workspace.file().session;
            let time = self.chart_focus_time()?;
            let mut views = match &self.active_views {
                Some(views) if views.belongs_to(session) => views.clone(),
                _ => ActiveChartViews::empty(session, time)?,
            };
            views.synchronize(session, self.workspace.active_chart(), time)?;
            Ok(views)
        })();
        if let Ok(views) = result {
            let closed:Vec<_>=self.active_views.as_ref().into_iter().flat_map(|old|old.entries())
                .filter(|old| !views.entries().iter().any(|new|new.identity()==old.identity()))
                .filter_map(|old|self.workspace.file().session.chart_ids().iter().position(|id|id==old.identity()))
                .collect();
            for chart in closed {self.workspace.forget_chart_attack(chart);}
            self.active_views = Some(views);
        }
    }
    pub(super) fn chart_views_focus(&mut self, previous: bool) {
        self.chart_views_apply(ViewAction::Focus(previous));
    }
    pub(super) fn chart_views_apply(&mut self, action: ViewAction) {
        self.refresh_chart_views();
        let Some(mut next) = self.active_views.clone() else {
            return;
        };
        let Some(focused) = next.focused() else {
            return;
        };
        let previous_left=self.first_chart_canvas_left();
        let id: Guid = focused.identity().clone();
        let chart = focused.source_index();
        let session = &self.workspace.file().session;
        // Source CloseChart restores the old focal point after choosing its
        // neighbor; only explicit focus cycling preserves the group left edge.
        let preserve_left=matches!(action,ViewAction::Focus(_));
        let result = match action {
            // Source KeepChartOpen always pins; repeated presses cannot unpin.
            ViewAction::Pin => next.set_dedicated(session, chart, true),
            ViewAction::Close => next.close(session, &id),
            ViewAction::Focus(previous) => next.cycle_focus(session, previous),
            ViewAction::Move(right) => next.move_view(session, &id, right),
        };
        if result.is_err() {
            self.status = i18n::tr("Edition", "ChartOrderFailed");
            return;
        }
        if let Some(focused) = next.focused() {
            let target = focused.source_index();
            if self.workspace.active_chart() != Some(target)
                && !self.focus_chart_at_time_with_placement(target, next.music_time(),preserve_left)
            {
                return;
            }
        } else {
            self.close_focused_audio_view();
            self.workspace.clear_active_chart();
            self.clear_unfocused_chart_selection();
            self.focus_position = None;
        }
        let focus_changed=next.focused().is_some_and(|view|view.source_index()!=chart);
        if !next.entries().iter().any(|view|view.identity()==&id) {
            self.workspace.forget_chart_attack(chart);
        }
        self.active_views = Some(next);
        self.refresh();
        if focus_changed&&preserve_left{if let Err(error)=self.preserve_chart_canvas_left(previous_left){self.status=crate::i18n::editor_error(&error).into();return;}}
        self.status = i18n::tr("Edition", "ChartViewsHelp");
    }
    pub(super) fn chart_views_actors(&self, actors: &mut Vec<Actor>) {
        let Some(views) = self.active_views.as_ref() else {
            return;
        };
        if views.entries().is_empty() || !views.belongs_to(&self.workspace.file().session) {
            return;
        }
        let focused = views.focused().map(|view| view.identity());
        let selected = views
            .entries()
            .iter()
            .position(|view| Some(view.identity()) == focused)
            .unwrap_or(0);
        let count = views.entries().len().min(5);
        let start = (selected / 5) * 5;
        let width = (screen_width() - 36.0) / count as f32;
        for (slot, view) in views.entries().iter().enumerate().skip(start).take(5) {
            let x = 18.0 + (slot - start) as f32 * width;
            let active = Some(view.identity()) == focused;
            let color = if active {
                [0.25, 0.45, 0.6, 1.0]
            } else {
                [0.10, 0.16, 0.22, 1.0]
            };
            actors.push(act!(quad: xy(x+width*0.5,94.0): zoomto(width-3.0,18.0): diffuse(color[0],color[1],color[2],color[3]): z(8007)));
            let Some(descriptor) = self
                .workspace
                .file()
                .session
                .document()
                .charts()
                .get(view.source_index())
            else {
                continue;
            };
            if self
                .workspace
                .file()
                .session
                .chart_ids()
                .get(view.source_index())
                != Some(view.identity())
            {
                continue;
            }
            let name = self
                .workspace
                .file()
                .session
                .document()
                .text(Some(view.source_index()), "CHARTNAME")
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| descriptor.steps_type.clone());
            let name: String = name
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(28)
                .collect();
            let kind = i18n::tr(
                "Edition",
                if view.dedicated() {
                    "ChartViewPinned"
                } else {
                    "ChartViewTemporary"
                },
            );
            let label = format!(
                "{} {}/{} {} ({})",
                if active { ">" } else { "" },
                slot + 1,
                views.entries().len(),
                name,
                kind
            );
            actors.push(act!(text: font("miso"): settext(label): align(0.0,0.5): xy(x+4.0,94.0): zoom(0.40): maxwidth(width-10.0): z(8008)));
        }
    }
}

#[cfg(test)]mod focus_geometry_tests{
 use super::*;use super::super::{KeyCode,Command};
 fn fixture(disk:bool)->(tempfile::TempDir,State){crate::i18n::init_for_tests();deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.));let root=tempfile::tempdir().unwrap();let path=root.path().join("focus.ssc");std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#NOTES:0100;").unwrap();let mut state=State::open(&path,0).unwrap();state.audio_preferences_file=None;state.audio_preferences_path=None;if disk{let preferences=root.path().join("prefs.json");std::fs::write(&preferences,b"{\"Version\":1}").unwrap();state.configure_audio_preferences(&preferences).unwrap();}state.refresh_chart_views();let session=&state.workspace.file().session;let views=state.active_views.as_mut().unwrap();views.set_dedicated(session,0,true).unwrap();views.show(session,1,true).unwrap();state.refresh();(root,state)}
 fn source_expected_raw(center:bool,locked:bool)->i32{let rows:serde_json::Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../deadsync-editor/tests/fixtures/chart-focus-layout/source.expected.json"))).unwrap();let row=rows.as_array().unwrap().iter().find(|r|r["width0"]==192&&r["width1"]==192&&r["old"]==0&&r["next"]==1&&r["center"]==center&&r["locked"]==locked).unwrap();let raw=row["rawX"].as_i64().unwrap()as i32;if center{842+raw-900}else{raw}}
 fn field_positions(state:&State)->Vec<(usize,f32)>{state.visible_view_geometry().iter().map(|g|(g.source,g.left)).collect()}
 #[test]fn focused_tabs_preserve_group_position_ignore_axis_lock_and_keep_unowned_history_positions(){for disk in [false,true]{let(root,mut state)=fixture(disk);let before=state.workspace.file().session.serialize().unwrap();state.open_receptor_settings();state.receptor_settings_key(0,KeyCode::Enter);state.receptor_settings_key(13,KeyCode::ArrowRight);state.receptor_settings_key(14,KeyCode::Enter);state.receptor_settings_key(9,KeyCode::Enter);assert!(state.receptor_preferences().placement.lock_x);let original=field_positions(&state);let time=state.chart_focus_time().unwrap();let undo=state.workspace.file().session.undo_label().map(str::to_owned);
 state.chart_views_focus(false);assert_eq!(state.workspace.chart(),1);assert_eq!(field_positions(&state),original);assert_eq!(state.receptor_preferences().placement.x,source_expected_raw(false,true));assert_eq!(state.chart_focus_time().unwrap().to_bits(),time.to_bits());assert_eq!(state.workspace.file().session.undo_label(),undo.as_deref());assert_eq!(state.capture_application_preferences_file().unwrap().receptors().placement.x,1106);
 state.command(Command::Undo);assert_eq!(state.receptor_preferences().placement.x,1106,"flag/Y undo must preserve the later non-undoable focus X");assert_eq!(state.receptor_preferences().placement.y,110);assert!(state.receptor_preferences().hide_arrows);state.command(Command::Redo);assert_eq!(state.receptor_preferences().placement.x,1106);assert_eq!(state.receptor_preferences().placement.y,111);state.chart_views_focus(true);assert_eq!(field_positions(&state),original);assert_eq!(state.receptor_preferences().placement.x,842);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
 let focal=state.receptor_canvas_position();state.chart_views_apply(ViewAction::Close);assert_eq!(state.receptor_canvas_position(),focal,"CloseChart preserves focal point, not the former group edge");if disk{assert!(state.request_host_close());let file=deadsync_editor::audio_preferences::File::open(&root.path().join("prefs.json")).unwrap();assert_eq!(file.receptors(),state.receptor_preferences());}else{assert!(state.audio_preferences_file.is_none());assert!(state.audio_preferences_path.is_none());assert_eq!(state.source_preferences_import.memory.as_ref().unwrap().path(),std::path::Path::new(""));}
 }}
 #[test]fn centered_focus_repositions_group_and_keeps_source_hidden_x_for_uncentering(){let(_root,mut state)=fixture(false);let mut p=state.receptor_preferences();p.placement.center=true;state.configure_receptor_preferences(p);let old=field_positions(&state);state.chart_views_focus(false);assert_ne!(field_positions(&state),old);assert_eq!(state.receptor_canvas_position().0,427.);assert_eq!(state.receptor_preferences().placement.x,source_expected_raw(true,false));assert_eq!(state.first_chart_canvas_left(),Some(67.));state.open_receptor_settings();state.receptor_settings_key(11,KeyCode::Enter);state.receptor_settings_key(9,KeyCode::Enter);assert_eq!(field_positions(&state),old,"uncentering reveals the source hidden focal-point update");assert_eq!(state.receptor_canvas_position().0,691.);}
}
