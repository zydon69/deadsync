//! Shared keyboard text editing for chart and song property drafts.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    chart_text::Field as ChartField,
    song_text::Field as SongField,
    text_buffer::{Buffer, Movement},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Chart(ChartField),
    Song(SongField),
}
impl Target {
    pub(super) fn is_music(self) -> bool {
        matches!(
            self,
            Self::Chart(ChartField::Music) | Self::Song(SongField::Music | SongField::PreviewMusic)
        )
    }
    pub(super) fn is_preview(self) -> bool {
        matches!(self, Self::Song(SongField::PreviewMusic))
    }
    pub(super) fn is_song(self) -> bool {
        matches!(self, Self::Song(_))
    }
    fn key(self) -> &'static str {
        match self {
            Self::Chart(field) => match field {
                ChartField::Name => "ChartName",
                ChartField::Description => "ChartDescription",
                ChartField::Credit => "ChartCredit",
                ChartField::Style => "ChartStyle",
                ChartField::Music => "ChartMusic",
            },
            Self::Song(field) => match field {
                SongField::Title => "SongTitle",
                SongField::Subtitle => "SongSubtitle",
                SongField::Artist => "SongArtist",
                SongField::Credit => "SongCredit",
                SongField::Genre => "SongGenre",
                SongField::Origin => "SongOrigin",
                SongField::TitleTransliteration => "SongTitleTranslit",
                SongField::SubtitleTransliteration => "SongSubtitleTranslit",
                SongField::ArtistTransliteration => "SongArtistTranslit",
                SongField::Music => "SongMusic",
                SongField::PreviewMusic => "SongPreviewMusic",
            },
        }
    }
}
impl State {
    fn field_at(&self, index: usize) -> Option<Target> {
        match self.panel {
            Some(Panel::ChartText(_)) => ChartField::ALL.get(index).copied().map(Target::Chart),
            Some(Panel::SongText(_)) => SongField::ALL.get(index).copied().map(Target::Song),
            _ => None,
        }
    }
    fn field_value(&self, target: Target) -> Option<&str> {
        match target {
            Target::Chart(f) => self.chart_properties.as_ref().map(|d| d.text(f)),
            Target::Song(f) => self.song_properties.as_ref().map(|d| d.text(f)),
        }
    }
    fn field_supported(&self, target: Target) -> bool {
        match target {
            Target::Chart(f) => self
                .chart_properties
                .as_ref()
                .is_some_and(|d| d.supports_text(f)),
            Target::Song(f) => self
                .song_properties
                .as_ref()
                .is_some_and(|d| d.supports_text(f)),
        }
    }
    fn set_field_value(
        &mut self,
        target: Target,
        value: &str,
    ) -> Result<(), deadsync_editor::Error> {
        let missing = || deadsync_editor::Error::InvalidEdit("No property draft is active".into());
        match target {
            Target::Chart(f) => self
                .chart_properties
                .as_mut()
                .ok_or_else(missing)?
                .set_text(f, value),
            Target::Song(f) => self
                .song_properties
                .as_mut()
                .ok_or_else(missing)?
                .set_text(f, value),
        }
    }
    pub(super) fn text_target(&self) -> Option<Target> {
        let index = match self.panel {
            Some(Panel::ChartText(i) | Panel::SongText(i)) => i,
            _ => return None,
        };
        self.field_at(index)
            .filter(|target| self.field_supported(*target))
    }
    pub(super) fn open_chart_text(&mut self) {
        let first = if self
            .chart_properties
            .as_ref()
            .is_some_and(|d| d.supports_text(ChartField::Name))
        {
            0
        } else {
            1
        };
        self.panel = Some(Panel::ChartText(first));
        self.focus_text();
        self.status = i18n::tr(
            "Edition",
            if first == 0 {
                "ChartTextHelpSsc"
            } else {
                "ChartTextHelpSm"
            },
        );
    }
    pub(super) fn open_song_text(&mut self) {
        self.panel = Some(Panel::SongText(0));
        self.focus_text();
        self.status = i18n::tr("Edition", "SongTextHelp");
    }
    fn focus_text(&mut self) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        self.text_buffer = self.text_target().and_then(|target| {
            self.field_value(target)
                .map(|value| Buffer::new(value, true))
        });
        self.label_preedit.clear();
    }
    pub(super) fn sync_text(&mut self) -> bool {
        let Some(target) = self.text_target() else {
            return true;
        };
        let Some(buffer) = self.text_buffer.as_ref() else {
            return true;
        };
        if self.field_value(target) == Some(buffer.value()) {
            return true;
        }
        let value = buffer.value().to_owned();
        if self.set_field_value(target, &value).is_err() {
            self.status = i18n::tr("Edition", "ChartTextSingleLine");
            return false;
        }
        true
    }
    pub(super) fn edit_text_input(&mut self, text: &str) -> bool {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let Some(buffer) = self.text_buffer.as_mut() else {
            return false;
        };
        buffer.insert(text);
        self.sync_text();
        self.label_preedit.clear();
        true
    }
    pub(super) fn text_field_key(&mut self, index: usize, code: KeyCode) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let song = matches!(self.panel, Some(Panel::SongText(_)));
        let count = if song {
            SongField::ALL.len()
        } else {
            ChartField::ALL.len()
        };
        if self.text_target().is_some_and(Target::is_music) {
            if code == KeyCode::F2 {
                if self.sync_text() {
                    self.queue_music_picker();
                }
                return;
            }
            if code == KeyCode::F3 {
                self.clear_music();
                return;
            }
        }
        if self.control && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV) {
            self.queue_text_clipboard(code);
            return;
        }
        if code == KeyCode::Escape
            || (index == count && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
        {
            let accepted = self.sync_text();
            self.label_preedit.clear();
            self.text_buffer = None;
            self.panel = Some(if song {
                Panel::SongProperties(3)
            } else {
                Panel::ChartProperties(9)
            });
            self.status = i18n::tr(
                "Edition",
                match (song, accepted) {
                    (true, true) => "SongTextStaged",
                    (true, false) => "SongTextInvalidDiscarded",
                    (false, true) => "ChartTextStaged",
                    (false, false) => "ChartTextInvalidDiscarded",
                },
            );
            return;
        }
        if matches!(
            code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Tab
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            if !self.sync_text() {
                return;
            }
            let total = count + 1;
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let delta = if backward { total - 1 } else { 1 };
            let mut next = match code {
                KeyCode::PageUp => index.saturating_sub(8) / 8 * 8,
                KeyCode::PageDown => (index / 8 * 8 + 8).min(count),
                _ => (index + delta) % total,
            };
            while next < count
                && !self
                    .field_at(next)
                    .is_some_and(|target| self.field_supported(target))
            {
                next = (next + delta) % total;
            }
            self.panel = Some(if song {
                Panel::SongText(next)
            } else {
                Panel::ChartText(next)
            });
            self.focus_text();
            return;
        }
        let Some(buffer) = self.text_buffer.as_mut() else {
            return;
        };
        match code {
            KeyCode::ArrowLeft => buffer.move_cursor(
                if self.control {
                    Movement::WordLeft
                } else {
                    Movement::Left
                },
                self.shift,
            ),
            KeyCode::ArrowRight => buffer.move_cursor(
                if self.control {
                    Movement::WordRight
                } else {
                    Movement::Right
                },
                self.shift,
            ),
            KeyCode::Home => buffer.move_cursor(Movement::Start, self.shift),
            KeyCode::End => buffer.move_cursor(Movement::End, self.shift),
            KeyCode::KeyA if self.control => buffer.select_all(),
            KeyCode::Backspace => {
                buffer.backspace(self.control);
            }
            KeyCode::Delete => {
                buffer.delete(self.control);
            }
            KeyCode::KeyZ if self.control => {
                if self.shift {
                    buffer.redo();
                } else {
                    buffer.undo();
                }
            }
            KeyCode::KeyY if self.control => {
                buffer.redo();
            }
            _ => return,
        }
        self.sync_text();
    }
    pub(super) fn text_field_label(&self, index: usize) -> std::sync::Arc<str> {
        let Some(target) = self.field_at(index) else {
            return std::sync::Arc::from("");
        };
        if !self.field_supported(target) {
            return i18n::tr_fmt(
                "Edition",
                target.key(),
                &[("value", &i18n::tr("Edition", "ChartTextUnsupportedSm"))],
            );
        }
        let mut value = if self.text_target() == Some(target) {
            self.text_buffer
                .as_ref()
                .map(|b| b.display(&self.label_preedit, 48))
                .unwrap_or_default()
        } else {
            let b = Buffer::new(self.field_value(target).unwrap_or_default(), false);
            let p = b.display("", 48);
            p.strip_suffix('|').unwrap_or(&p).to_owned()
        };
        if target.is_music()
            && self.field_value(target) == Some("")
            && (value.is_empty() || value == "|")
        {
            let hint = i18n::tr(
                "Edition",
                if target.is_preview() {
                    "SongPreviewMusicEmpty"
                } else if target.is_song() {
                    "SongMusicEmpty"
                } else {
                    "ChartMusicInherited"
                },
            );
            value = if value.is_empty() {
                hint.to_string()
            } else {
                format!("{value} {hint}")
            };
        }
        i18n::tr_fmt("Edition", target.key(), &[("value", &value)])
    }
}
