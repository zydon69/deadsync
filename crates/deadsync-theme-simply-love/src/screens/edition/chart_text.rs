use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::chart_text::Field;
impl State {
    pub(super) fn open_chart_text(&mut self) {
        let first = if self
            .chart_properties
            .as_ref()
            .is_some_and(|d| d.supports_text(Field::Name))
        {
            0
        } else {
            1
        };
        self.chart_text_fresh = true;
        self.label_preedit.clear();
        self.panel = Some(Panel::ChartText(first));
        self.status = i18n::tr(
            "Edition",
            if first == 0 {
                "ChartTextHelpSsc"
            } else {
                "ChartTextHelpSm"
            },
        );
    }
    pub(super) fn chart_text_field(&self) -> Option<Field> {
        let Some(Panel::ChartText(index)) = self.panel else {
            return None;
        };
        let field = Field::ALL.get(index).copied()?;
        self.chart_properties
            .as_ref()?
            .supports_text(field)
            .then_some(field)
    }
    pub(super) fn chart_text_input(&mut self, text: &str) -> bool {
        let Some(field) = self.chart_text_field() else {
            return false;
        };
        let Some(draft) = self.chart_properties.as_mut() else {
            return false;
        };
        let mut value = if self.chart_text_fresh {
            String::new()
        } else {
            draft.text(field).to_owned()
        };
        value.extend(text.chars().filter(|c| !c.is_control()));
        if draft.set_text(field, &value).is_err() {
            return false;
        }
        self.chart_text_fresh = false;
        self.label_preedit.clear();
        true
    }
    pub(super) fn chart_text_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::Escape) || (index == 4 && code == KeyCode::Enter) {
            self.label_preedit.clear();
            self.panel = Some(Panel::ChartProperties(9));
            self.status = i18n::tr("Edition", "ChartTextStaged");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            let delta = if code == KeyCode::ArrowUp { 4 } else { 1 };
            let mut next = (index + delta) % 5;
            while next < 4
                && self
                    .chart_properties
                    .as_ref()
                    .is_some_and(|d| !d.supports_text(Field::ALL[next]))
            {
                next = (next + delta) % 5;
            }
            self.panel = Some(Panel::ChartText(next));
            self.chart_text_fresh = true;
        } else if !self.control && matches!(code, KeyCode::Backspace | KeyCode::Delete) {
            if let Some(field) = self.chart_text_field() {
                if let Some(draft) = self.chart_properties.as_mut() {
                    let mut value = draft.text(field).to_owned();
                    if code == KeyCode::Delete {
                        value.clear();
                    } else {
                        value.pop();
                    }
                    let _ = draft.set_text(field, &value);
                    self.chart_text_fresh = false;
                }
            }
        }
    }
    pub(super) fn chart_text_label(&self, index: usize) -> std::sync::Arc<str> {
        let field = Field::ALL[index];
        let key = match field {
            Field::Name => "ChartName",
            Field::Description => "ChartDescription",
            Field::Credit => "ChartCredit",
            Field::Style => "ChartStyle",
        };
        let value = self
            .chart_properties
            .as_ref()
            .map(|d| {
                if !d.supports_text(field) {
                    return i18n::tr("Edition", "ChartTextUnsupportedSm").to_string();
                }
                if matches!(self.panel,Some(Panel::ChartText(selected)) if selected==index)
                    && !self.label_preedit.is_empty()
                {
                    format!(
                        "{}{}",
                        if self.chart_text_fresh {
                            ""
                        } else {
                            d.text(field)
                        },
                        self.label_preedit
                    )
                } else {
                    d.text(field).to_owned()
                }
            })
            .unwrap_or_default();
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
