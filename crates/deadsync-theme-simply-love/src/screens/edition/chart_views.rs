//! Keyboard front end for the shared open-view registry. Headers and focused
//! editing are implemented here; simultaneous lane rendering is a later step.
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
            self.active_views = Some(views);
        }
    }
    pub(super) fn chart_views_focus(&mut self, previous: bool) {
        self.chart_views_apply(ViewAction::Focus(previous));
    }
    pub(super) fn chart_views_apply(&mut self, action: ViewAction) {
        self.stop_audio();
        self.refresh_chart_views();
        let Some(mut next) = self.active_views.clone() else {
            return;
        };
        let Some(focused) = next.focused() else {
            return;
        };
        let id: Guid = focused.identity().clone();
        let chart = focused.source_index();
        let dedicated = focused.dedicated();
        let session = &self.workspace.file().session;
        let result = match action {
            ViewAction::Pin => next.set_dedicated(session, chart, !dedicated),
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
                && !self.focus_chart_at_time(target, next.music_time())
            {
                return;
            }
        } else {
            self.workspace.clear_active_chart();
            self.focus_position = None;
        }
        self.active_views = Some(next);
        self.refresh();
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
