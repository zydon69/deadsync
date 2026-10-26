use super::State;
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};

impl State {
    pub(super) fn open_chart_list(&mut self) {
        self.stop_audio();
        self.panel = None;
        self.queue_chart_list();
    }
    pub(super) fn chart_list_key(&mut self, code: KeyCode) {
        if code == KeyCode::Escape {
            self.panel = None;
            return;
        }
        if !self
            .chart_list_cache
            .is_current(&self.workspace, i18n::revision())
        {
            // Never commit a stale row, even when the chart index still exists.
            self.open_chart_list();
            return;
        }
        let rows = self.chart_list_cache.rows();
        match code {
            KeyCode::ArrowUp => self.chart_list_selection.move_by(rows, -1),
            KeyCode::ArrowDown => self.chart_list_selection.move_by(rows, 1),
            KeyCode::Tab => self
                .chart_list_selection
                .move_by(rows, if self.shift { -1 } else { 1 }),
            KeyCode::PageUp => self.chart_list_selection.move_by(rows, -8),
            KeyCode::PageDown => self.chart_list_selection.move_by(rows, 8),
            KeyCode::Home => {
                self.chart_list_selection = Default::default();
                if let Some(first) = rows.first() {
                    self.chart_list_selection
                        .reconcile(rows, first.source_index);
                }
            }
            KeyCode::End => {
                self.chart_list_selection = Default::default();
                if let Some(last) = rows.last() {
                    self.chart_list_selection.reconcile(rows, last.source_index);
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(source) = self.chart_list_selection.selected_source(rows) {
                    self.panel = None;
                    match self.chart_focus_music_time() {
                        Ok(time) => self.focus_chart_at_time(source, time),
                        Err(_) => self.status = i18n::tr("Edition", "ChartOrderFailed"),
                    }
                }
            }
            _ => {}
        }
    }
    pub(super) fn chart_list_actors(&self, actors: &mut Vec<Actor>) {
        let width = (screen_width() - 40.0).min(760.0);
        let center = screen_width() * 0.5;
        let top = (screen_height() - 370.0).max(10.0) * 0.5;
        actors.push(act!(quad: xy(center,top+185.0): zoomto(width,370.0): diffuse(0.08,0.1,0.15,1.0): z(8020)));
        let label = |actors: &mut Vec<Actor>, value: String, y: f32, color: [f32; 4]| {
            actors.push(act!(text: font("miso"): settext(value): align(0.0,0.0): xy(center-width*0.5+16.0,y): zoom(0.65): maxwidth(width-32.0): diffuse(color[0],color[1],color[2],color[3]): z(8021)));
        };
        label(
            actors,
            i18n::tr("Edition", "ChartListTitle").to_string(),
            top + 12.0,
            [1.0; 4],
        );
        let rows = self.chart_list_cache.rows();
        if rows.is_empty() {
            label(
                actors,
                i18n::tr("Edition", "ChartListEmpty").to_string(),
                top + 60.0,
                [0.7, 0.8, 0.9, 1.0],
            );
        } else if !self
            .chart_list_cache
            .is_current(&self.workspace, i18n::revision())
        {
            label(
                actors,
                i18n::tr("Edition", "ChartListRefresh").to_string(),
                top + 60.0,
                [1.0, 0.8, 0.4, 1.0],
            );
        } else {
            let selected = self.chart_list_selection.position(rows).unwrap_or(0);
            let start = selected / 8 * 8;
            for (position, row) in rows.iter().enumerate().skip(start).take(8) {
                let name = if row.key.name.is_empty() {
                    &row.key.description
                } else {
                    &row.key.name
                };
                let name: String = name
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(64)
                    .collect();
                let difficulty = [
                    "DifficultyBeginner",
                    "DifficultyEasy",
                    "DifficultyMedium",
                    "DifficultyHard",
                    "DifficultyChallenge",
                    "DifficultyEdit",
                ]
                .get(row.key.difficulty as usize)
                .map(|key| i18n::tr("Edition", key).to_string())
                .unwrap_or_else(|| row.key.difficulty.to_string());
                let summary = format!(
                    "{} {}/{}   {}   {} [{}]   {}",
                    if position == selected { ">" } else { " " },
                    position + 1,
                    rows.len(),
                    row.key.steps_type,
                    difficulty,
                    row.key.rating,
                    name
                );
                label(
                    actors,
                    summary,
                    top + 50.0 + (position - start) as f32 * 32.0,
                    if position == selected {
                        [1.0, 0.9, 0.4, 1.0]
                    } else {
                        [0.8, 0.85, 0.95, 1.0]
                    },
                );
            }
        }
        label(
            actors,
            i18n::tr("Edition", "ChartListHelp").to_string(),
            top + 327.0,
            [0.7, 0.8, 0.9, 1.0],
        );
    }
}
