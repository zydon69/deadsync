use super::State;
use crate::i18n;

impl State {
    pub(super) fn paste_selected_events(&mut self) {
        match self.workspace.paste_events_with_report() {
            Ok(report) if report.patterns_skipped > 0 || report.notes_skipped > 0 => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    if report.changed {
                        "EventPasteAppliedSkipped"
                    } else {
                        "EventPasteUnchangedSkipped"
                    },
                    &[
                        ("patterns", &report.patterns_skipped.to_string()),
                        ("notes", &report.notes_skipped.to_string()),
                    ],
                );
            }
            Ok(report) => {
                self.status = i18n::tr(
                    "Edition",
                    if report.changed {
                        "EventPasteApplied"
                    } else {
                        "EventPasteNoChange"
                    },
                )
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
        self.refresh();
    }
}
