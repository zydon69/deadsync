//! Committed Unicode text is routed separately from physical editor shortcuts.
use super::{Panel, State};
impl State {
    pub fn wants_text_input(&self) -> bool {
        self.chart_text_field().is_some()
            || (matches!(self.panel, Some(Panel::Timing(2)))
                && self.timing.as_ref().is_some_and(|s| s.is_label()))
    }
    pub fn text_input(&mut self, text: &str) -> bool {
        if !self.wants_text_input()
            || self.control
            || text.is_empty()
            || text.chars().all(char::is_control)
        {
            return false;
        }
        if self.chart_text_field().is_some() {
            return self.chart_text_input(text);
        }
        let Some(settings) = self.timing.as_mut() else {
            return false;
        };
        if settings.fresh {
            settings.value.clear();
            settings.fresh = false;
        }
        settings
            .value
            .push_str(&deadsync_editor::timing_edit::label::sanitize(text));
        settings.pending = true;
        self.label_preedit.clear();
        true
    }
    pub fn text_preedit(&mut self, text: &str) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        self.label_preedit = if self.wants_text_input() {
            text.to_owned()
        } else {
            String::new()
        };
    }
}
