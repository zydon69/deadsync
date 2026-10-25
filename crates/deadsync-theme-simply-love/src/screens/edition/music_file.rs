use super::State;
use crate::i18n;
use deadsync_editor::{
    chart_text::Field,
    music_file::{self, Browse, Picker},
};
pub(super) struct Request {
    serial: u64,
}
impl State {
    pub(super) fn queue_music_picker(&mut self) {
        self.music_picker_request = Some(Request {
            serial: self.text_edit_serial,
        });
    }
    pub(super) fn clear_chart_music(&mut self) {
        if let Some(buffer) = self.chart_text_buffer.as_mut() {
            buffer.select_all();
            buffer.delete(false);
        }
        if self.sync_chart_text() {
            self.status = i18n::tr("Edition", "ChartMusicCleared");
        }
    }
    pub fn process_music_picker(&mut self, picker: &mut impl Picker) -> bool {
        let Some(request) = self.music_picker_request.take() else {
            return false;
        };
        if request.serial != self.text_edit_serial
            || self.chart_text_field() != Some(Field::Music)
            || !self.label_preedit.is_empty()
        {
            return false;
        }
        let current = self
            .chart_text_buffer
            .as_ref()
            .map(|b| b.value().to_owned())
            .unwrap_or_default();
        let browse = Browse {
            directory: self
                .workspace
                .file()
                .path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
            current,
        };
        let result = picker.choose(&browse).and_then(|path| {
            path.map(|path| music_file::selected_path(self.workspace.file().path(), &path))
                .transpose()
        });
        match result {
            Ok(Some(path)) => {
                if let Some(buffer) = self.chart_text_buffer.as_mut() {
                    buffer.select_all();
                }
                self.chart_text_input(&path);
                self.status = i18n::tr("Edition", "ChartMusicStaged");
            }
            Ok(None) => self.status = i18n::tr("Edition", "ChartMusicCancelled"),
            Err(_) => self.status = i18n::tr("Edition", "ChartMusicInvalid"),
        }
        true
    }
}
