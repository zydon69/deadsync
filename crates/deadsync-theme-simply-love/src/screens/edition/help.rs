//! Bounded keyboard help; opening it must not expose editing shortcuts beneath it.
use super::*;
impl State {
    fn help_lines(&self) -> Vec<String> {
        let mut lines: Vec<_> = i18n::tr("Edition", "Help")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect();
        lines.extend(self.key_binding_help_lines());
        for key in [
            "KeyBindingSettingsOpen",
            "HelpViews",
            "HelpMinimap",
            "HelpChartCopy",
            "HelpEventShift",
            "HelpShiftAll",
            "HelpQuickAdd",
            "HelpSelectedTransform",
            "PackHelpMenu",
            "PackSongsHelpMenu",
        ] {
            lines.push(i18n::tr("Edition", key).to_string());
        }
        lines
    }
    fn help_rows(&self) -> usize {
        (((screen_height() - 140.0).max(58.0) / 58.0) as usize).clamp(1, 8)
    }
    fn help_pages(&self) -> usize {
        self.help_lines().len().div_ceil(self.help_rows()).max(1)
    }
    pub(super) fn help_key(&mut self, event: &RawKeyboardEvent) {
        if !event.pressed {
            return;
        }
        let last = self.help_pages() - 1;
        match event.code {
            KeyCode::Escape | KeyCode::F1 if !event.repeat => {
                self.help = false;
                self.status = i18n::tr("Edition", "Ready");
            }
            KeyCode::ArrowRight | KeyCode::PageDown | KeyCode::ArrowDown => {
                self.help_page = self.help_page.saturating_add(1).min(last);
            }
            KeyCode::ArrowLeft | KeyCode::PageUp | KeyCode::ArrowUp => {
                self.help_page = self.help_page.saturating_sub(1);
            }
            KeyCode::Home => self.help_page = 0,
            KeyCode::End => self.help_page = last,
            _ => {}
        }
    }
    pub(super) fn help_actors(&self, actors: &mut Vec<Actor>) {
        let width = (screen_width() - 70.0).max(1.0);
        let page = self.help_page.min(self.help_pages() - 1);
        let title = i18n::tr_fmt(
            "Edition",
            "HelpPage",
            &[
                ("page", &(page + 1).to_string()),
                ("pages", &self.help_pages().to_string()),
            ],
        );
        actors.push(act!(quad: xy(screen_width()*0.5,screen_height()*0.5): setsize((screen_width()-30.0).max(1.0),(screen_height()-30.0).max(1.0)): diffuse(0.05,0.08,0.12,0.98): z(8049)));
        actors.push(act!(text: font("miso"): settext(title): align(0.0,0.0): xy(35.0,30.0): zoom(0.8): maxwidth(width): maxheight(35.0): z(8050)));
        for (index, line) in self
            .help_lines()
            .into_iter()
            .skip(page * self.help_rows())
            .take(self.help_rows())
            .enumerate()
        {
            actors.push(act!(text: font("miso"): settext(line): align(0.0,0.0): xy(35.0,80.0+index as f32*58.0): zoom(0.55): wrapwidthpixels(width/0.55): maxwidth(width): maxheight(52.0): z(8051)));
        }
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","HelpPagesControls")): align(0.0,0.0): xy(35.0,screen_height()-48.0): zoom(0.5): maxwidth(width): maxheight(30.0): z(8052)));
    }
}
