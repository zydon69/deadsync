//! One inline numeric entry session for preference panels. Text never mutates a draft.
use super::*;
use deadsync_editor::{audio_preferences::publication::EditBinding, minimap_viewport::Axis};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    SongMetadata,
    ChartOffset,
    Display,
    General,
    Receptor,
    Stream,
    Performance,
    Recent,
    Scroll,
    Speed,
    Zoom,
    Waveform,
    WaveColor,
    MiniViewport,
    MiniContent,
    Density,
    Dark,
    Note,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    kind: Kind,
    field: usize,
    row: usize,
}
#[derive(Clone, PartialEq)]
enum Snapshot {
    SongMetadata([u64; 5], Vec<String>, deadsync_editor::song_properties::Selectable, Option<usize>),
    ChartOffset(u64, bool, Vec<String>, deadsync_editor::generation::metadata::Guid, Option<deadsync_editor::chart_identity::Difficulty>, i32, (deadsync_editor::display_tempo::Mode, u64, u64, bool)),
    Display(deadsync_editor::options_preferences::Display),
    General(deadsync_editor::options_preferences::General),
    Receptor(deadsync_editor::receptor_preferences::Preferences, [i32; 2]),
    Stream(deadsync_editor::stream_preferences::Preferences),
    Performance(deadsync_editor::performance::preferences::Preferences),
    Recent(usize),
    Scroll(deadsync_editor::scroll_preferences::Preferences),
    Zoom(deadsync_editor::zoom_preferences::Preferences),
    CurrentZoom(u64, u64, bool),
    Waveform(deadsync_editor::waveform_preferences::Preferences),
    MiniViewport(deadsync_editor::minimap_preferences::Preferences),
    MiniContent(super::minimap_options_draft::Draft),
    Density(deadsync_editor::density_preferences::Preferences),
    Dark(deadsync_editor::dark_preferences::Preferences),
    Note(
        Arc<deadsync_editor::note_color_preferences::Preferences>,
        usize,
    ),
}
pub(super) struct Edit {
    target: Target,
    baseline: Snapshot,
    binding: EditBinding,
    content: Option<(deadsync_editor::chart_order::SessionBinding, Option<usize>)>,
    pub(super) text: String,
    initial: String,
}
fn invalid() -> deadsync_editor::Error {
    deadsync_editor::Error::InvalidEdit("Invalid preference numeric value".into())
}
fn float(text: &str) -> Result<f64, deadsync_editor::Error> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(invalid)
}
// ImGuiUtils.DragDouble applies Math.Max(min, value), then Math.Min(max, value).
// In particular a committed -0 input becomes +0 when the lower bound is +0.
fn source_drag_double(text: &str, min: f64, max: f64) -> Result<f64, deadsync_editor::Error> {
    let value = float(text)?.max(min).min(max);
    Ok(if value == 0.0 && min == 0.0 {
        0.0
    } else {
        value
    })
}
fn single(text: &str) -> Result<f32, deadsync_editor::Error> {
    let v = float(text)? as f32;
    if v.is_finite() { Ok(v) } else { Err(invalid()) }
}
fn integer(text: &str) -> Result<i32, deadsync_editor::Error> {
    text.trim().parse().map_err(|_| invalid())
}
fn uint(text: &str, min: u32, max: u32) -> Result<u32, deadsync_editor::Error> {
    text.trim()
        .parse::<i32>()
        .map(|v| (v as u32).clamp(min, max))
        .map_err(|_| invalid())
}
impl State {
    fn numeric_target(&self) -> Option<Target> {
        let (kind, field, row) = match self.panel? {
            Panel::SongProperties(i @ (0 | 4)) => (Kind::SongMetadata, usize::from(i == 4), i),
            Panel::SongSample(i @ 0..=2) => (Kind::SongMetadata, i + 2, i),
            Panel::ChartProperties(2) if self.chart_properties.as_ref()?.uses_chart_offset() => (Kind::ChartOffset, 0, 2),
            Panel::OptionsSettings(1) if self.options_field == 6 => (Kind::Display, 6, 1),
            Panel::GeneralOptions(1)
                if matches!(self.general_field, 0..=2)
                    || self.general_field == 8 && self.general_options.custom_dpi =>
            {
                (Kind::General, self.general_field, 1)
            }
            Panel::ReceptorSettings(i @ 12..=13) => {
                self.receptor_draft?;
                if !self.receptor_control_enabled(i) {
                    return None;
                }
                (Kind::Receptor, i, i)
            }
            Panel::StreamSettings(i @ 3..=4) => (Kind::Stream, i, i),
            Panel::Performance(i @ 2..=3) => {
                let draft = self.performance_numeric_draft()?;
                if i == 2
                    && draft.scale != deadsync_editor::performance::preferences::Scale::Explicit
                {
                    return None;
                }
                (Kind::Performance, i, i)
            }
            Panel::GeneralOptions(1) if self.general_field == 12 => (Kind::Recent, 12, 1),
            Panel::ScrollSettings(i @ 5..=6) => (Kind::Scroll, i, i),
            Panel::ScrollValues(i @ 0..=2) if self.source_scroll_speed_enabled(i) => {
                (Kind::Speed, i, i)
            }
            Panel::Hotbar(7) => (Kind::Zoom,3,7),
            Panel::ZoomSettings(i @ 0..=2) => (Kind::Zoom, i, i),
            Panel::WaveformSettings(i @ (2 | 3 | 8 | 10..=12)) => (Kind::Waveform, i, i),
            Panel::WaveformColors(i @ 1..=13) if self.waveform_color_control_enabled(i) => {
                (Kind::WaveColor, i, i)
            }
            Panel::MinimapSettings(i @ 1..=2) => (Kind::MiniViewport, i, i),
            Panel::MinimapOptions(i @ (3..=6 | 8 | 10 | 12)) => (Kind::MiniContent, i, i),
            Panel::DensitySettings(1) => {
                let field = self.density_draft?.field;
                if !matches!(field,4..=6|8..=19) {
                    return None;
                }
                (Kind::Density, field, 1)
            }
            Panel::DarkSettings(i @ 3..=6) => (Kind::Dark, i, i),
            Panel::NoteColorSettings(i @ 1..=3) => {
                let d = self.note_color_draft.as_ref()?;
                if d.field() > 30 || i > d.components() {
                    return None;
                }
                (Kind::Note, i - 1, i)
            }
            _ => return None,
        };
        Some(Target { kind, field, row })
    }
    fn numeric_snapshot(&self, kind: Kind) -> Option<Snapshot> {
        Some(match kind {
            Kind::SongMetadata => {
                let draft = self.song_properties.as_ref()?;
                Snapshot::SongMetadata([
                    draft.sync_offset(), draft.music_offset(), draft.sample_start(),
                    draft.sample_length(), draft.last_second_hint(),
                ].map(f64::to_bits),
                    deadsync_editor::song_text::Field::ALL.into_iter().map(|field| draft.text(field).to_owned()).collect(),
                    draft.selectable(), draft.timing_chart(&self.workspace.file().session).ok()?)
            }
            Kind::ChartOffset => {
                let draft = self.chart_properties.as_ref()?;
                Snapshot::ChartOffset(draft.music_offset().to_bits(), draft.uses_chart_offset(),
                    deadsync_editor::chart_text::Field::ALL.into_iter().map(|field| draft.text(field).to_owned()).collect(),
                    draft.expression().clone(), draft.difficulty(), draft.rating(), (draft.display_tempo().mode(), draft.display_tempo().min().to_bits(), draft.display_tempo().max().to_bits(), draft.display_tempo().separate_max()))
            }
            Kind::Display => Snapshot::Display(self.options_draft?),
            Kind::General => Snapshot::General(self.general_options.clone()),
            Kind::Receptor => Snapshot::Receptor(
                self.receptor_draft?,
                [
                    (screen_width() as i32).saturating_sub(1).max(0),
                    (screen_height() as i32).saturating_sub(1).max(0),
                ],
            ),
            Kind::Stream => Snapshot::Stream(self.stream_draft?),
            Kind::Performance => Snapshot::Performance(self.performance_numeric_draft()?),
            Kind::Recent => Snapshot::Recent(self.file_menu.recent_preferences().maximum),
            Kind::Scroll => Snapshot::Scroll(self.scroll_preferences_draft?),
            Kind::Speed => Snapshot::Scroll(self.scroll_preferences),
            Kind::Zoom => if matches!(self.panel,Some(Panel::Hotbar(7))){Snapshot::CurrentZoom(self.spacing_zoom().to_bits(),self.zoom.size_cap().to_bits(),self.zoom.limit_to_size())}else{Snapshot::Zoom(self.zoom_preferences_draft?)},
            Kind::Waveform | Kind::WaveColor => {
                Snapshot::Waveform(self.waveform_preferences_draft?)
            }
            Kind::MiniViewport => Snapshot::MiniViewport(self.minimap.draft?),
            Kind::MiniContent => Snapshot::MiniContent(self.minimap.options_draft?),
            Kind::Density => Snapshot::Density(self.density_draft?.preferences),
            Kind::Dark => Snapshot::Dark(self.dark_draft?),
            Kind::Note => {
                let d = self.note_color_draft.as_ref()?;
                Snapshot::Note(d.staged(), d.field())
            }
        })
    }
    pub(super) fn preference_numeric_is_current(&self) -> bool {
        let Some(edit) = self.preference_numeric.as_ref() else {
            return false;
        };
        let current=self.numeric_snapshot(edit.target.kind);
        // The source zoom animation may advance while typing. Its current value
        // is not an external edit; retain bounds/owner guards without cancelling
        // desired interpolation just by entering or cancelling this field.
        let snapshot_current=match (current.as_ref(),&edit.baseline) {
            (Some(Snapshot::CurrentZoom(_,cap,limited)),Snapshot::CurrentZoom(_,old_cap,old_limited))=>cap==old_cap&&limited==old_limited,
            _=>current.as_ref()==Some(&edit.baseline),
        };
        edit.content.as_ref().is_none_or(|(binding, chart)|
            binding.is_current(&self.workspace.file().session) && *chart == self.workspace.active_chart())
            && edit.binding.is_current(&self.workspace.file().session)
            && self.numeric_target() == Some(edit.target)
            && snapshot_current
    }
    pub(super) fn finish_preference_numeric(&mut self) {
        if self.preference_numeric.take().is_some() {
            self.command_bindings.release_input();
            self.text_buffer = None;
            self.label_preedit.clear();
            self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        }
    }
    pub(super) fn preference_numeric_label(&self, row: usize) -> Option<Arc<str>> {
        let edit = self.preference_numeric.as_ref()?;
        if row != edit.target.row {
            return None;
        }
        let value = self
            .text_buffer
            .as_ref()
            .map(|b| b.display(&self.label_preedit, 48))
            .unwrap_or_else(|| edit.text.clone());
        Some(i18n::tr_fmt(
            "Edition",
            "PreferenceNumericValue",
            &[("value", &value)],
        ))
    }
    pub(super) fn preference_numeric_key(&mut self, code: KeyCode) -> bool {
        if self.preference_numeric.is_some() {
            if !self.preference_numeric_is_current() {
                self.finish_preference_numeric();
                self.status = i18n::tr("Edition", "PreferenceNumericStale");
                return true;
            }
            if code == KeyCode::Escape {
                self.finish_preference_numeric();
                return true;
            }
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                if !self.label_preedit.is_empty() {
                    return true;
                }
                if !self.sync_text() {
                    return true;
                }
                let edit = self.preference_numeric.as_ref().unwrap();
                if edit.text.trim() == edit.initial.trim() {
                    self.finish_preference_numeric();
                    return true;
                }
                let target = edit.target;
                let baseline = edit.baseline.clone();
                let text = edit.text.clone();
                match self.numeric_accept(target, baseline, &text) {
                    Ok(()) => {
                        self.finish_preference_numeric();
                        self.status = i18n::tr("Edition", "PreferenceNumericHelp");
                    }
                    Err(error) => {
                        self.status = match error {
                            deadsync_editor::Error::InvalidEdit(_)
                            | deadsync_editor::Error::InvalidFormat(_) => {
                                i18n::tr("Edition", "PreferenceNumericInvalid")
                            }
                            error => i18n::editor_error(&error).into(),
                        };
                    }
                }
                return true;
            }
            if !matches!(
                code,
                KeyCode::Tab
                    | KeyCode::ArrowUp
                    | KeyCode::ArrowDown
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            ) {
                self.text_field_key(0, code);
            }
            return true;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            return false;
        }
        let Some(target) = self.numeric_target() else {
            return false;
        };
        let Some(baseline) = self.numeric_snapshot(target.kind) else {
            return false;
        };
        let text = numeric_value(target, &baseline);
        self.command_bindings.release_input();
        self.lane_entry_release_input();
        self.receptor_views.release_input();
        self.preference_numeric = Some(Edit {
            target,
            baseline,
            binding: EditBinding::capture(&self.workspace.file().session),
            content: matches!(target.kind, Kind::SongMetadata | Kind::ChartOffset).then(|| (
                deadsync_editor::chart_order::SessionBinding::capture(&self.workspace.file().session),
                self.workspace.active_chart(),
            )),
            initial: text.clone(),
            text,
        });
        self.focus_text();
        self.status = i18n::tr("Edition", "PreferenceNumericHelp");
        true
    }
    /// Consume every raw key while editing, including configurable global shortcuts.
    pub(super) fn preference_numeric_raw_key(&mut self, event: &RawKeyboardEvent) {
        let bit = match event.code {
            KeyCode::ControlLeft => 1,
            KeyCode::ControlRight => 2,
            KeyCode::SuperLeft => 4,
            KeyCode::SuperRight => 8,
            _ => 0,
        };
        if bit != 0 {
            if event.pressed {
                self.control_keys |= bit;
            } else {
                self.control_keys &= !bit;
            }
            self.control = self.control_keys != 0;
            return;
        }
        let bit = match event.code {
            KeyCode::ShiftLeft => 1,
            KeyCode::ShiftRight => 2,
            _ => 0,
        };
        if bit != 0 {
            if event.pressed {
                self.shift_keys |= bit;
            } else {
                self.shift_keys &= !bit;
            }
            self.shift = self.shift_keys != 0;
            return;
        }
        if event.pressed {
            self.preference_numeric_key(event.code);
        }
    }
    fn numeric_accept(
        &mut self,
        target: Target,
        baseline: Snapshot,
        text: &str,
    ) -> Result<(), deadsync_editor::Error> {
        let i = target.field;
        match baseline {
            Snapshot::SongMetadata(..) => {
                let minimum = if matches!(i, 0 | 3 | 4) { 0.0 } else { f64::MIN };
                let value = source_drag_double(text, minimum, f64::MAX)?;
                let draft = self.song_properties.as_mut().ok_or_else(invalid)?;
                match i {
                    0 => draft.set_sync_offset(value)?,
                    1 => draft.set_music_offset(value)?,
                    2 => draft.set_sample_start(value)?,
                    3 => draft.set_sample_length(value)?,
                    _ => draft.set_last_second_hint(value)?,
                }
                self.refresh_sample_region();
            }
            Snapshot::ChartOffset(..) => {
                let value = source_drag_double(text, f64::MIN, f64::MAX)?;
                self.chart_properties.as_mut().ok_or_else(invalid)?.set_music_offset(value)?;
            }
            Snapshot::Display(mut p) => {
                p.misc_width = integer(text)?.clamp(0, 200);
                self.options_draft = Some(p);
            }
            Snapshot::General(mut p) => {
                match i {
                    0 => p.undo_history_size = integer(text)?.clamp(1, 32768),
                    1 => p.new_sync = source_drag_double(text, 0., f64::MAX)?,
                    2 => p.open_sync = source_drag_double(text, 0., f64::MAX)?,
                    _ => p.dpi_scale = source_drag_double(text, 0.25, 8.)?,
                }
                self.commit_general_options(p)?;
            }
            Snapshot::Receptor(mut p, bounds) => {
                let axis = i - 12;
                let value = integer(text)?.clamp(0, bounds[axis]);
                if axis == 0 {
                    p.placement.x = value;
                } else {
                    p.placement.y = value;
                }
                self.receptor_draft = Some(p);
            }
            Snapshot::Stream(mut p) => {
                if i == 3 {
                    p.minimum_length = integer(text)?.clamp(0, 8);
                } else {
                    p.short_break_cutoff = integer(text)?.clamp(0, 64);
                }
                self.stream_draft = Some(p);
            }
            Snapshot::Performance(mut p) => {
                if i == 2 {
                    p.explicit_seconds = source_drag_double(text, 0., 1.)?;
                } else {
                    p.max_frames = integer(text)?.clamp(2, 1024);
                }
                self.set_performance_numeric_draft(p)?;
            }
            Snapshot::Recent(_) => {
                self.commit_recent_files_history_size(integer(text)?.clamp(0, 100) as usize)?;
            }
            Snapshot::Scroll(mut p) => {
                if target.kind == Kind::Speed {
                    return self.set_source_scroll_speed_input(i, f64::from(single(text)?));
                }
                if i == 5 {
                    p.set_wheel_time(float(text)?.clamp(0.0, 10.0))?;
                } else {
                    p.set_wheel_rows(integer(text)?.clamp(0, 384))?;
                }
                self.scroll_preferences_draft = Some(p);
            }
            Snapshot::CurrentZoom(..) => {self.set_hotbar_zoom(f64::from(single(text)?))?;}
            Snapshot::Zoom(mut p) => {
                match i {
                    0 => p.set_duration(float(text)?.clamp(0.0, 1.0))?,
                    1 => p.set_multiplier(float(text)?.clamp(1.0, 4.0))?,
                    _ => {
                        p.set_size_cap(float(text)?.clamp(deadsync_editor::zoom::MIN_ZOOM, 1.0))?
                    }
                }
                self.zoom_preferences_draft = Some(p);
            }
            Snapshot::Waveform(mut p) => {
                if target.kind == Kind::Waveform {
                    match i {
                        2 => p.set_channel_width(f64::from(single(text)?))?,
                        3 => p.set_dense_scale(f64::from(single(text)?))?,
                        8 => p.set_loading_parallelism(integer(text)?),
                        10 => p.set_subpix(f64::from(single(text)?))?,
                        11 => p.set_edge_threshold(f64::from(single(text)?))?,
                        _ => p.set_edge_threshold_min(f64::from(single(text)?))?,
                    }
                } else {
                    let mut palette = p.palette();
                    let value = single(text)?;
                    if i == 1 {
                        palette.set_darken(value)?;
                    } else {
                        let color = if i < 6 {
                            palette.dense()
                        } else if i < 10 {
                            palette.sparse()
                        } else {
                            palette.background()
                        }
                        .with_component((i - 2) % 4, value.clamp(0.0, 1.0))?;
                        if i < 6 {
                            palette.set_dense(color);
                        } else if i < 10 {
                            palette.set_sparse(color);
                        } else {
                            palette.set_background(color);
                        }
                    }
                    p.set_palette(palette);
                }
                self.waveform_preferences_draft = Some(p);
            }
            Snapshot::MiniViewport(mut p) => {
                let (axis, min, max) = if i == 1 {
                    (Axis::Time, 30, 300)
                } else {
                    (Axis::Row, 3072, 28800)
                };
                p.set_visible_range(axis, f64::from(uint(text, min, max)?))?;
                self.minimap.draft = Some(p);
            }
            Snapshot::MiniContent(mut p) => {
                match i {
                    3 => {
                        p.position
                            .set_offset(p.position.mount, integer(text)?.clamp(-1024, 1024));
                    }
                    4 => p.content.width = uint(text, 2, 128)?,
                    5 => p.content.note_width = uint(text, 1, 32)?,
                    6 => p.content.note_spacing = uint(text, 0, 32)?,
                    8 => p.content.patterns_width = uint(text, 1, 128)?,
                    10 => p.content.preview_width = uint(text, 1, 128)?,
                    _ => p.content.max_notes = uint(text, 0, 65535)? as usize,
                }
                self.minimap.options_draft = Some(p);
            }
            Snapshot::Density(mut p) => {
                match i {
                    4 => {
                        let index = p.position.index();
                        p.position_offsets[index] = integer(text)?.clamp(-1024, 1024);
                    }
                    5 => {
                        let index = p.position.index();
                        p.width_offsets[index] = integer(text)?.clamp(-1024, 1024);
                    }
                    6 => p.height = integer(text)?.clamp(16, 1024),
                    _ => {
                        let value = single(text)?.clamp(0.0, 1.0);
                        if i < 12 {
                            p.high[i - 8] = value;
                        } else if i < 16 {
                            p.low[i - 12] = value;
                        } else {
                            p.background[i - 16] = value;
                        }
                    }
                }
                p.validate()?;
                self.density_draft.as_mut().ok_or_else(invalid)?.preferences = p;
            }
            Snapshot::Dark(mut p) => {
                let mut rgba = p.color();
                rgba[i - 3] = single(text)?.clamp(0.0, 1.0);
                p.set_color(rgba)?;
                self.dark_draft = Some(p);
            }
            Snapshot::Note(_, _) => {
                let value = single(text)?;
                self.note_color_draft
                    .as_mut()
                    .ok_or_else(invalid)?
                    .set_component(i, value)?;
                self.note_color_preview();
            }
        }
        Ok(())
    }
}
fn numeric_value(target: Target, snapshot: &Snapshot) -> String {
    let i = target.field;
    match snapshot {
        Snapshot::SongMetadata(values, ..) => f64::from_bits(values[i]).to_string(),
        Snapshot::ChartOffset(value, ..) => f64::from_bits(*value).to_string(),
        Snapshot::Display(p) => p.misc_width.to_string(),
        Snapshot::General(p) => match i {
            0 => p.undo_history_size.to_string(),
            1 => p.new_sync.to_string(),
            2 => p.open_sync.to_string(),
            _ => p.dpi_scale.to_string(),
        },
        Snapshot::Receptor(p, _) => if i == 12 {
            p.placement.x
        } else {
            p.placement.y
        }
        .to_string(),
        Snapshot::Stream(p) => if i == 3 {
            p.minimum_length
        } else {
            p.short_break_cutoff
        }
        .to_string(),
        Snapshot::Performance(p) => {
            if i == 2 {
                p.explicit_seconds.to_string()
            } else {
                p.max_frames.to_string()
            }
        }
        Snapshot::Recent(value) => value.to_string(),
        Snapshot::Scroll(p) => {
            if target.kind == Kind::Speed {
                p.speed(i).unwrap().to_string()
            } else if i == 5 {
                p.wheel_time().to_string()
            } else {
                p.wheel_rows().to_string()
            }
        }
        Snapshot::CurrentZoom(bits, ..) => f64::from_bits(*bits).to_string(),
        Snapshot::Zoom(p) => match i {
            0 => p.duration(),
            1 => p.multiplier(),
            _ => p.size_cap(),
        }
        .to_string(),
        Snapshot::Waveform(p) => {
            if target.kind == Kind::Waveform {
                return match i {
                    2 => p.channel_width().to_string(),
                    3 => p.dense_scale().to_string(),
                    8 => p.loading_parallelism().to_string(),
                    10 => p.subpix().to_string(),
                    11 => p.edge_threshold().to_string(),
                    _ => p.edge_threshold_min().to_string(),
                };
            }
            let palette = p.palette();
            if i == 1 {
                palette.darken().to_string()
            } else {
                let rgba = if i < 6 {
                    palette.dense()
                } else if i < 10 {
                    palette.sparse()
                } else {
                    palette.background()
                }
                .rgba();
                rgba[(i - 2) % 4].to_string()
            }
        }
        Snapshot::MiniViewport(p) => p
            .visible_range(if i == 1 { Axis::Time } else { Axis::Row })
            .to_string(),
        Snapshot::MiniContent(p) => match i {
            3 => return p.position.offset(p.position.mount).to_string(),
            4 => p.content.width,
            5 => p.content.note_width,
            6 => p.content.note_spacing,
            8 => p.content.patterns_width,
            10 => p.content.preview_width,
            _ => return p.content.max_notes.to_string(),
        }
        .to_string(),
        Snapshot::Density(p) => match i {
            4 => p.position_offsets[p.position.index()].to_string(),
            5 => p.width_offsets[p.position.index()].to_string(),
            6 => p.height.to_string(),
            _ => if i < 12 {
                p.high[i - 8]
            } else if i < 16 {
                p.low[i - 12]
            } else {
                p.background[i - 16]
            }
            .to_string(),
        },
        Snapshot::Dark(p) => p.color()[i - 3].to_string(),
        Snapshot::Note(p, field) => {
            let mut draft = deadsync_editor::note_color_inspector::Draft::capture(Arc::clone(p))
                .expect("validated draft");
            draft.select(*field).expect("captured field");
            draft
                .component_value(i)
                .expect("numeric component")
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn state(language: &str, disk: bool) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        if disk {
            let path = root.path().join("preferences.json");
            std::fs::write(&path, br#"{"Version":1,"Vendor":{"keep":true}}"#).unwrap();
            state.configure_audio_preferences(&path).unwrap();
        }
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                br#"{"VendorSource":{"keep":[1,3]}}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &file, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
        (root, state)
    }
    fn open(state: &mut State, kind: Kind) -> (usize, usize, &'static str) {
        match kind {
            Kind::SongMetadata | Kind::ChartOffset => unreachable!("metadata uses its own workflow tests"),
            Kind::Display => {
                state.open_display_options();
                state.options_field = 6;
                state.panel = Some(Panel::OptionsSettings(1));
                (3, 4, "137")
            }
            Kind::General => {
                state.open_general_options();
                state.general_field = 1;
                state.panel = Some(Panel::GeneralOptions(1));
                (1, 1, "0.123456789")
            }
            Kind::Receptor => {
                state.open_receptor_settings();
                state.panel = Some(Panel::ReceptorSettings(12));
                (9, 10, "137")
            }
            Kind::Stream => {
                state.open_stream_settings();
                state.panel = Some(Panel::StreamSettings(3));
                (8, 9, "7")
            }
            Kind::Performance => {
                state.open_performance();
                state.panel = Some(Panel::Performance(3));
                (5, 7, "137")
            }
            Kind::Recent => {
                state.open_general_options();
                state.general_field = 12;
                state.panel = Some(Panel::GeneralOptions(1));
                (1, 1, "7")
            }
            Kind::Scroll => {
                state.open_scroll_settings();
                state.panel = Some(Panel::ScrollSettings(5));
                (3, 4, "0.123456")
            }
            Kind::Speed => {
                state.open_scroll_values();
                state.panel = Some(Panel::ScrollValues(0));
                (0, 0, "321.123456")
            }
            Kind::Zoom => {
                state.open_zoom_settings();
                state.panel = Some(Panel::ZoomSettings(0));
                (5, 6, "0.123456")
            }
            Kind::Waveform => {
                state.open_waveform_settings();
                state.panel = Some(Panel::WaveformSettings(2));
                (14, 15, "0.123456")
            }
            Kind::WaveColor => {
                state.open_waveform_settings();
                state.panel = Some(Panel::WaveformColors(2));
                (14, 15, "0.123456")
            }
            Kind::MiniViewport => {
                state.open_minimap_settings();
                state.panel = Some(Panel::MinimapSettings(1));
                (5, 6, "57")
            }
            Kind::MiniContent => {
                state.open_minimap_options();
                state.panel = Some(Panel::MinimapOptions(4));
                (14, 15, "57")
            }
            Kind::Density => {
                state.open_density_settings();
                state.density_draft.as_mut().unwrap().field = 6;
                state.panel = Some(Panel::DensitySettings(1));
                (3, 4, "137")
            }
            Kind::Dark => {
                state.open_dark_settings();
                state.panel = Some(Panel::DarkSettings(3));
                (8, 9, "0.123456")
            }
            Kind::Note => {
                state.open_note_color_settings();
                state.panel = Some(Panel::NoteColorSettings(1));
                (5, 6, "0.123456")
            }
        }
    }
    fn select_row(state: &mut State, row: usize) {
        state.panel = Some(match state.panel.unwrap() {
            Panel::ScrollSettings(_) => Panel::ScrollSettings(row),
            Panel::ZoomSettings(_) => Panel::ZoomSettings(row),
            Panel::WaveformSettings(_) | Panel::WaveformColors(_) => Panel::WaveformSettings(row),
            Panel::MinimapSettings(_) => Panel::MinimapSettings(row),
            Panel::MinimapOptions(_) => Panel::MinimapOptions(row),
            Panel::DensitySettings(_) => Panel::DensitySettings(row),
            Panel::DarkSettings(_) => Panel::DarkSettings(row),
            Panel::NoteColorSettings(_) => Panel::NoteColorSettings(row),
            _ => panic!("unexpected test panel"),
        });
    }
    fn live(state: &State, kind: Kind) -> Snapshot {
        match kind {
            Kind::SongMetadata | Kind::ChartOffset => unreachable!("metadata uses its own workflow tests"),
            Kind::Display => Snapshot::Display(state.display_options),
            Kind::General => Snapshot::General(state.general_options.clone()),
            Kind::Receptor => Snapshot::Receptor(
                state.receptor_preferences(),
                [
                    (screen_width() as i32).saturating_sub(1).max(0),
                    (screen_height() as i32).saturating_sub(1).max(0),
                ],
            ),
            Kind::Stream => Snapshot::Stream(state.stream_preferences()),
            Kind::Performance => Snapshot::Performance(state.performance_preferences()),
            Kind::Recent => Snapshot::Recent(state.file_menu.recent_preferences().maximum),
            Kind::Scroll | Kind::Speed => Snapshot::Scroll(state.scroll_preferences),
            Kind::Zoom => Snapshot::Zoom(state.zoom_preferences),
            Kind::Waveform | Kind::WaveColor => Snapshot::Waveform(state.waveform_preferences),
            Kind::MiniViewport => Snapshot::MiniViewport(state.minimap.preferences),
            Kind::MiniContent => {
                Snapshot::MiniContent(super::super::minimap_options_draft::Draft {
                    position: state.minimap.position,
                    content: state.minimap.content.options,
                })
            }
            Kind::Density => Snapshot::Density(state.density_preferences()),
            Kind::Dark => Snapshot::Dark(state.dark_preferences),
            Kind::Note => Snapshot::Note(Arc::clone(&state.note_colors), 0),
        }
    }
    #[test]
    fn raw_keyboard_entry_is_precise_cancellable_and_uses_canonical_undo_owners_without_a_chart() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                for kind in [
                    Kind::Scroll,
                    Kind::Zoom,
                    Kind::Waveform,
                    Kind::WaveColor,
                    Kind::MiniViewport,
                    Kind::MiniContent,
                    Kind::Density,
                    Kind::Dark,
                    Kind::Note,
                ] {
                    let (_root, mut state) = state(language, disk);
                    assert!(!state.workspace.has_chart());
                    let before = live(&state, kind);
                    let (save, cancel, text) = open(&mut state, kind);
                    let original = state.numeric_snapshot(kind).unwrap();
                    let song = state.workspace.file().session.serialize().unwrap();
                    let revision = state.workspace.file().session.revision();
                    press(&mut state, KeyCode::Enter);
                    assert!(state.wants_text_input());
                    assert!(state.text_input(text));
                    assert!(state.numeric_snapshot(kind).as_ref() == Some(&original));
                    press(&mut state, KeyCode::Escape);
                    assert!(state.preference_numeric.is_none());
                    assert!(state.numeric_snapshot(kind).as_ref() == Some(&original));
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(text));
                    press(&mut state, KeyCode::Enter);
                    assert!(state.preference_numeric.is_none());
                    assert!(state.numeric_snapshot(kind).as_ref() != Some(&original));
                    select_row(&mut state, cancel);
                    press(&mut state, KeyCode::Enter);
                    assert!(live(&state, kind) == before);
                    let (save_again, _, text) = open(&mut state, kind);
                    assert_eq!(save_again, save);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(text));
                    press(&mut state, KeyCode::Enter);
                    let staged = state.numeric_snapshot(kind).unwrap();
                    select_row(&mut state, save);
                    press(&mut state, KeyCode::Enter);
                    assert!(live(&state, kind) == staged);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
                    let published = state.capture_application_preferences_file().unwrap();
                    assert!(published.source_archive().is_some());
                    assert!(
                        !published.has_changes(),
                        "{language} disk={disk} kind={kind:?}: {}",
                        state.status
                    );
                    {
                        assert_ne!(state.workspace.file().session.revision(), revision);
                        state.command(Command::Undo);
                        assert!(live(&state, kind) == before);
                        state.command(Command::Redo);
                        assert!(live(&state, kind) == staged);
                    }
                    if disk {
                        let reopened =
                            deadsync_editor::audio_preferences::File::open(published.path())
                                .unwrap();
                        assert!(reopened.source_archive().is_some());
                        let root: serde_json::Value =
                            serde_json::from_slice(&std::fs::read(published.path()).unwrap())
                                .unwrap();
                        assert_eq!(root["Vendor"]["keep"], true);
                    }
                }
            }
        }
    }
    #[test]
    fn stale_session_panel_or_draft_and_global_shortcuts_cannot_publish_or_edit_notes() {
        let (_root, mut state) = state("en", false);
        open(&mut state, Kind::Waveform);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.123456"));
        let revision = state.workspace.file().session.revision();
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::ControlLeft,
            pressed: true,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
        press(&mut state, KeyCode::KeyZ);
        press(&mut state, KeyCode::KeyS);
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::ControlLeft,
            pressed: false,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
        // Explicitly release the modifier; physical global commands never reach the document.
        state.control = false;
        state.control_keys = 0;
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        state
            .waveform_preferences_draft
            .as_mut()
            .unwrap()
            .set_show(false);
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        open(&mut state, Kind::Waveform);
        press(&mut state, KeyCode::Enter);
        state.panel = Some(Panel::DarkSettings(3));
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        open(&mut state, Kind::Waveform);
        press(&mut state, KeyCode::Enter);
        state.command(Command::Undo);
        assert!(state.preference_numeric.is_none());
    }
    #[test]
    fn replacing_the_song_session_invalidates_equal_looking_numeric_drafts() {
        let (root, mut state) = state("en", false);
        open(&mut state, Kind::Waveform);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.123456"));
        let draft = state.waveform_preferences_draft;
        let replacement = State::without_song(root.path()).unwrap();
        state.workspace = replacement.workspace;
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        assert_eq!(state.waveform_preferences_draft, draft);
        assert!(!state.wants_text_input());
    }
    #[test]
    fn finite_and_integer_validation_is_atomic_and_source_clamps_are_control_specific() {
        let (_root, mut state) = state("en", false);
        for kind in [
            Kind::Scroll,
            Kind::Zoom,
            Kind::Waveform,
            Kind::MiniViewport,
            Kind::MiniContent,
            Kind::Density,
            Kind::Dark,
            Kind::Note,
        ] {
            open(&mut state, kind);
            let original = state.numeric_snapshot(kind).unwrap();
            for text in ["NaN", "inf", "1e999", "bad"] {
                press(&mut state, KeyCode::Enter);
                assert!(state.text_input(text));
                press(&mut state, KeyCode::Enter);
                assert!(state.preference_numeric.is_some());
                assert!(state.numeric_snapshot(kind).as_ref() == Some(&original));
                press(&mut state, KeyCode::Escape);
            }
        }
        open(&mut state, Kind::Scroll);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("12"));
        press(&mut state, KeyCode::Enter);
        assert_eq!(state.scroll_preferences_draft.unwrap().wheel_time(), 10.0);
        open(&mut state, Kind::MiniViewport);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0"));
        press(&mut state, KeyCode::Enter);
        assert_eq!(state.minimap.draft.unwrap().visible_range(Axis::Time), 30.0);
        open(&mut state, Kind::Waveform);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("1.25"));
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state.waveform_preferences_draft.unwrap().channel_width(),
            1.25
        );
    }
    #[test]
    fn both_new_wheel_controls_change_actual_wheel_intents_after_save() {
        use deadsync_editor::{
            scroll_preferences::spacing::Preference,
            scroll_preferences::wheel::{Input, Intent, decide},
        };
        let (_root, mut state) = state("en", false);
        open(&mut state, Kind::Scroll);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.123456"));
        press(&mut state, KeyCode::Enter);
        state.panel = Some(Panel::ScrollSettings(6));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("57"));
        press(&mut state, KeyCode::Enter);
        state.panel = Some(Panel::ScrollSettings(3));
        press(&mut state, KeyCode::Enter);
        let input = Input {
            detents: 1.0,
            zoom_held: false,
            spacing_held: false,
            playing: false,
            has_chart: true,
            snapped: false,
            spacing_zoom: 1.0,
            spacing_mode: Preference::ConstantTime,
        };
        assert_eq!(
            decide(state.scroll_preferences, input),
            Intent::InterpolateTime(-0.123456)
        );
        assert_eq!(
            decide(
                state.scroll_preferences,
                Input {
                    spacing_mode: Preference::ConstantRow,
                    ..input
                }
            ),
            Intent::InterpolateRows(-57.0)
        );
        state.command(Command::Undo);
        assert_eq!(state.scroll_preferences.wheel_time(), 0.25);
        assert_eq!(state.scroll_preferences.wheel_rows(), 48);
        state.command(Command::Redo);
        assert_eq!(state.scroll_preferences.wheel_time(), 0.123456);
        assert_eq!(state.scroll_preferences.wheel_rows(), 57);
    }
    #[test]
    fn confirming_unchanged_text_keeps_imported_raw_values_outside_control_bounds() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                let (_root, mut state) = state(language, disk);
                let current = state.capture_application_preferences_file().unwrap();
                let draft=state.workspace.prepare_source_preferences(&current,br#"{"PreferencesMiniMap":{"MiniMapVisibleTimeRange":0},"VendorRawZero":{"keep":[3,9]}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();
                state
                    .workspace
                    .import_source_preferences(draft, &current, true)
                    .unwrap();
                state.sync_source_preferences_history().unwrap();
                assert_eq!(state.minimap.preferences.visible_range(Axis::Time), 0.0);
                let owner = state.capture_application_preferences_file().unwrap();
                let archive = owner.source_archive().unwrap().clone();
                let disk_before = if disk {
                    Some(std::fs::read(owner.path()).unwrap())
                } else {
                    None
                };
                let revision = state.workspace.file().session.revision();
                state.open_minimap_settings();
                state.panel = Some(Panel::MinimapSettings(1));
                let before = state.minimap.draft;
                press(&mut state, KeyCode::Enter);
                press(&mut state, KeyCode::Enter);
                assert_eq!(state.minimap.draft, before);
                assert_eq!(state.workspace.file().session.revision(), revision);
                state.panel = Some(Panel::MinimapSettings(5));
                press(&mut state, KeyCode::Enter);
                assert_eq!(state.minimap.preferences.visible_range(Axis::Time), 0.0);
                let after = state.capture_application_preferences_file().unwrap();
                assert_eq!(after.source_archive(), Some(&archive));
                assert!(!after.has_changes());
                if let Some(bytes) = disk_before {
                    assert_eq!(std::fs::read(after.path()).unwrap(), bytes);
                }
                assert_eq!(state.workspace.file().session.revision(), revision);
            }
        }
    }
    #[test]
    fn original_numeric_wrapper_fixture_matches_native_cast_and_bounds_exactly() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/preference-numeric-controls.json"
        )))
        .unwrap();
        for case in cases.as_array().unwrap() {
            if case["kind"] == "integer" {
                let raw = case["raw"].as_i64().unwrap() as i32;
                assert_eq!(
                    uint(&raw.to_string(), 30, 300).unwrap(),
                    case["uintRange"].as_u64().unwrap() as u32
                );
                assert_eq!(raw.clamp(0, 384), case["intRange"].as_i64().unwrap() as i32);
            } else {
                let raw = case["raw"].as_f64().unwrap();
                let expected = case["result"].as_f64().unwrap();
                assert_eq!(
                    float(&raw.to_string()).unwrap().clamp(0.0, 10.0).to_bits(),
                    expected.to_bits()
                );
            }
        }
    }
    #[test]
    fn clipboard_and_ime_use_the_same_scratch_and_stale_requests_never_read_the_os() {
        struct Clipboard {
            reads: usize,
            text: String,
        }
        impl deadsync_editor::clipboard::TextClipboard for Clipboard {
            fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
                self.reads += 1;
                Ok(self.text.clone())
            }
            fn write_text(&mut self, text: &str) -> Result<(), deadsync_editor::clipboard::Error> {
                self.text = text.into();
                Ok(())
            }
        }
        let (_root, mut state) = state("fr", false);
        open(&mut state, Kind::Waveform);
        let before = state.waveform_preferences_draft;
        let mut clipboard = Clipboard {
            reads: 0,
            text: "0.123456".into(),
        };
        press(&mut state, KeyCode::Enter);
        state.control = true;
        press(&mut state, KeyCode::KeyV);
        state.control = false;
        press(&mut state, KeyCode::Escape);
        press(&mut state, KeyCode::Enter);
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        assert_eq!(state.waveform_preferences_draft, before);
        state.control = true;
        press(&mut state, KeyCode::KeyV);
        state.control = false;
        assert!(state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 1);
        assert_eq!(state.waveform_preferences_draft, before);
        state.text_preedit("12");
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_some());
        assert_eq!(state.waveform_preferences_draft, before);
        state.text_preedit("");
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state.waveform_preferences_draft.unwrap().channel_width(),
            f64::from(0.123456_f32)
        );
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.5"));
        state.release_input();
        assert!(state.preference_numeric.is_none());
        assert!(!state.wants_text_input());
    }
    #[test]
    fn numeric_entry_and_cancel_keep_real_logical_transport_owner_generation_and_lease() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:missing.wav;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state.editor_asset_owner();
        state.take_audio_commands();
        let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        // Match the host ordering: a lease is acknowledged before a drained
        // Start command can bind its decoder generation.
        state.update_audio(snapshot);
        state.take_audio_commands();
        state.navigate_exact_time(1.234567, false).unwrap();
        state.toggle_audio();
        state.take_audio_commands();
        state.bind_decoder_generation(Some(41));
        state.bind_decoder_asset_epoch(Some(owner), Some(53));
        assert_eq!(state.chart_audio_generation(), Some(41));
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 41,
            asset_epoch: 53,
            music_nanos: 1_234_567_000,
        });
        state.update_audio(snapshot);
        state.take_audio_commands();
        let time = state.transport_music_time();
        let output = state.audio_output_snapshot();
        assert!(output.available && output.leased);
        assert!(state.is_playing());
        for kind in [
            Kind::Scroll,
            Kind::Zoom,
            Kind::Waveform,
            Kind::WaveColor,
            Kind::MiniViewport,
            Kind::MiniContent,
            Kind::Density,
            Kind::Dark,
            Kind::Note,
        ] {
            open(&mut state, kind);
            press(&mut state, KeyCode::Enter);
            assert!(state.wants_text_input());
            assert!(state.text_input("0.123456"));
            press(&mut state, KeyCode::Escape);
            assert!(state.is_playing());
            assert_eq!(state.transport_music_time(), time);
            assert_eq!(state.editor_asset_owner(), owner);
            assert_eq!(state.chart_audio_generation(), Some(41));
            let current = state.audio_output_snapshot();
            assert_eq!(
                (
                    current.available,
                    current.leased,
                    current.requested_period,
                    current.negotiated_period,
                    current.ack
                ),
                (
                    output.available,
                    output.leased,
                    output.requested_period,
                    output.negotiated_period,
                    output.ack
                )
            );
            assert!(!state.take_audio_commands().iter().any(|c| matches!(
                c,
                deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                    | deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                    | deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic { .. }
                    | deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
            )));
        }
    }
    #[test]
    fn original_mode_gates_block_inactive_controls_without_changing_raw_owners() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/preference-mode-gates.json"
        )))
        .unwrap();
        assert_eq!(fixture.as_array().unwrap().len(), 10);
        for language in ["en", "fr"] {
            for disk in [false, true] {
                for case in fixture.as_array().unwrap() {
                    let (_root, mut state) = state(language, disk);
                    let raw = case["raw"].as_i64().unwrap() as i32;
                    let file = state.capture_application_preferences_file().unwrap();
                    let input=serde_json::json!({"PreferencesScroll":{"SpacingMode":raw.to_string()},"PreferencesWaveForm":{"WaveFormSparseColorOption":raw.to_string()},"VendorSource":{"keep":[1,3]}}).to_string();
                    let draft = state
                        .workspace
                        .prepare_source_preferences(
                            &file,
                            input.as_bytes(),
                            deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
                        )
                        .unwrap();
                    state
                        .workspace
                        .import_source_preferences(draft, &file, true)
                        .unwrap();
                    state.sync_source_preferences_history().unwrap();
                    let revision = state.workspace.file().session.revision();
                    let song = state.workspace.file().session.serialize().unwrap();
                    if case["kind"] == "scroll" {
                        state.open_scroll_values();
                        for index in 0..3 {
                            state.panel = Some(Panel::ScrollValues(index));
                            let before = state.scroll_preferences;
                            let enabled = case["enabled"][index].as_bool().unwrap();
                            assert_eq!(state.source_scroll_speed_enabled(index), enabled);
                            let disk_before = state
                                .audio_preferences_path
                                .as_ref()
                                .map(|p| std::fs::read(p).unwrap());
                            if enabled {
                                press(&mut state, KeyCode::Enter);
                                assert!(state.wants_text_input());
                                assert!(state.text_input("321.123456"));
                                press(&mut state, KeyCode::Enter);
                                assert_eq!(
                                    state.scroll_preferences.speed(index),
                                    Some(f64::from(321.123456_f32))
                                );
                                let mode = [
                                    deadsync_editor::zoom::SpacingMode::ConstantTime,
                                    deadsync_editor::zoom::SpacingMode::ConstantRow,
                                    deadsync_editor::zoom::SpacingMode::Variable,
                                ][index];
                                assert_eq!(state.zoom.spacing(mode), f64::from(321.123456_f32));
                            } else {
                                for key in [
                                    KeyCode::ArrowLeft,
                                    KeyCode::ArrowRight,
                                    KeyCode::Home,
                                    KeyCode::Enter,
                                ] {
                                    press(&mut state, key);
                                    assert!(state.preference_numeric.is_none());
                                    assert_eq!(state.scroll_preferences, before);
                                }
                                assert!(state.scroll_values_label(index).contains(
                                    if language == "en" {
                                        "unavailable"
                                    } else {
                                        "indisponible"
                                    }
                                ));
                                assert_eq!(
                                    state
                                        .audio_preferences_path
                                        .as_ref()
                                        .map(|p| std::fs::read(p).unwrap()),
                                    disk_before
                                );
                            }
                        }
                        assert_eq!(
                            state.workspace.file().session.revision(),
                            revision,
                            "direct source speed controls do not add Undo"
                        );
                        assert_eq!(
                            state.spacing_preference,
                            deadsync_editor::scroll_preferences::spacing::Preference::from_raw(raw)
                        );
                    } else {
                        state.open_waveform_settings();
                        let before = state.waveform_preferences;
                        for index in [1, 6, 7, 8, 9] {
                            let enabled = if index == 1 {
                                case["scale"].as_bool().unwrap()
                            } else {
                                case["color"].as_bool().unwrap()
                            };
                            state.panel = Some(Panel::WaveformColors(index));
                            let draft = state.waveform_preferences_draft.unwrap();
                            assert_eq!(state.waveform_color_control_enabled(index), enabled);
                            if enabled {
                                press(&mut state, KeyCode::Enter);
                                assert!(state.text_input("0.37"));
                                press(&mut state, KeyCode::Enter);
                                assert_ne!(state.waveform_preferences_draft.unwrap(), draft);
                            } else {
                                for key in [
                                    KeyCode::ArrowLeft,
                                    KeyCode::ArrowRight,
                                    KeyCode::Home,
                                    KeyCode::Enter,
                                ] {
                                    press(&mut state, key);
                                    assert!(state.preference_numeric.is_none());
                                    assert_eq!(state.waveform_preferences_draft.unwrap(), draft);
                                }
                                assert!(
                                    state
                                        .waveform_colors_label(index, "WaveformSparseRed")
                                        .contains(if language == "en" {
                                            "unavailable"
                                        } else {
                                            "indisponible"
                                        })
                                );
                            }
                        }
                        assert_eq!(
                            state.workspace.file().session.revision(),
                            revision,
                            "typing only changes the draft"
                        );
                        let staged = state.waveform_preferences_draft.unwrap();
                        state.panel = Some(Panel::WaveformSettings(14));
                        press(&mut state, KeyCode::Enter);
                        assert_eq!(state.waveform_preferences, staged);
                        assert!(state.waveform_preferences_draft.is_none());
                        if staged != before {
                            state.command(Command::Undo);
                            assert_eq!(state.waveform_preferences, before);
                            state.command(Command::Redo);
                            assert_eq!(state.waveform_preferences, staged);
                        } else {
                            assert_eq!(state.workspace.file().session.revision(), revision);
                        }
                    }
                    let owner = state.capture_application_preferences_file().unwrap();
                    assert!(!owner.has_changes());
                    assert!(owner.source_archive().is_some());
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
                }
            }
        }
    }
    #[test]
    fn original_remaining_numeric_wrappers_match_real_keyboard_controls_exactly() {
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854., 480.,
        ));
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/preference-remaining-numeric.json"
        )))
        .unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 63);
        for case in cases.as_array().unwrap() {
            let (_root, mut state) = state("en", false);
            let control = case["control"].as_str().unwrap();
            match control {
                "receptor-x" | "receptor-y" => {
                    state.open_receptor_settings();
                    state.panel = Some(Panel::ReceptorSettings(if control == "receptor-x" {
                        12
                    } else {
                        13
                    }));
                }
                "stream-min" | "stream-cutoff" => {
                    state.open_stream_settings();
                    state.panel = Some(Panel::StreamSettings(if control == "stream-min" {
                        3
                    } else {
                        4
                    }));
                }
                "history" | "sync" | "dpi" => {
                    if control == "dpi" {
                        let mut p = state.general_options.clone();
                        p.custom_dpi = true;
                        state.commit_general_options(p).unwrap();
                    }
                    state.open_general_options();
                    state.general_field = match control {
                        "history" => 0,
                        "sync" => 1,
                        _ => 8,
                    };
                    state.panel = Some(Panel::GeneralOptions(1));
                }
                "frames" | "explicit" => {
                    state.open_performance();
                    let mut p = state.performance_numeric_draft().unwrap();
                    p.scale = deadsync_editor::performance::preferences::Scale::Explicit;
                    state.set_performance_numeric_draft(p).unwrap();
                    state.panel = Some(Panel::Performance(if control == "frames" { 3 } else { 2 }));
                }
                _ => unreachable!(),
            }
            press(&mut state, KeyCode::Enter);
            assert!(state.wants_text_input(), "{control}");
            assert!(
                state.text_input(
                    &case["raw_text"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| case["raw"].to_string())
                )
            );
            press(&mut state, KeyCode::Enter);
            assert!(
                state.preference_numeric.is_none(),
                "{control}: {}",
                state.status
            );
            let actual = match control {
                "receptor-x" => f64::from(state.receptor_draft.unwrap().placement.x),
                "receptor-y" => f64::from(state.receptor_draft.unwrap().placement.y),
                "stream-min" => f64::from(state.stream_draft.unwrap().minimum_length),
                "stream-cutoff" => f64::from(state.stream_draft.unwrap().short_break_cutoff),
                "history" => f64::from(state.general_options.undo_history_size),
                "sync" => state.general_options.new_sync,
                "dpi" => state.general_options.dpi_scale,
                "frames" => f64::from(state.performance_numeric_draft().unwrap().max_frames),
                "explicit" => state.performance_numeric_draft().unwrap().explicit_seconds,
                _ => unreachable!(),
            };
            assert_eq!(
                actual.to_bits(),
                case["result_bits"]
                    .as_str()
                    .map(|bits| u64::from_str_radix(bits, 16).unwrap())
                    .unwrap_or_else(|| case["result"].as_f64().unwrap().to_bits()),
                "{case}"
            );
        }
    }
    #[test]
    fn miscellaneous_width_precise_entry_cancel_bounds_and_canonical_history() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/misc-width-numeric-source.json"
        )))
        .unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 7);
        for language in ["en", "fr"] {
            for disk in [false, true] {
                let (_root, mut state) = state(language, disk);
                let before = state.display_options;
                let song = state.workspace.file().session.serialize().unwrap();
                let revision = state.workspace.file().session.revision();
                open(&mut state, Kind::Display);
                press(&mut state, KeyCode::Enter);
                assert!(state.wants_text_input());
                assert!(state.text_input("137"));
                press(&mut state, KeyCode::Escape);
                assert_eq!(state.options_draft, Some(before));
                assert_eq!(state.workspace.file().session.revision(), revision);
                for case in cases.as_array().unwrap() {
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(&case["raw"].to_string()));
                    press(&mut state, KeyCode::Enter);
                    assert!(state.preference_numeric.is_none());
                    assert_eq!(
                        i64::from(state.options_draft.unwrap().misc_width),
                        case["result"].as_i64().unwrap()
                    );
                    assert_eq!(state.display_options, before);
                }
                press(&mut state, KeyCode::Enter);
                assert!(state.text_input("137"));
                press(&mut state, KeyCode::Enter);
                let mut expected = before;
                expected.misc_width = 137;
                assert_eq!(state.options_draft, Some(expected));
                state.panel = Some(Panel::OptionsSettings(3));
                press(&mut state, KeyCode::Enter);
                assert_eq!(state.display_options, expected);
                assert_eq!(state.misc_event_area_width(), 137.);
                let file = state.capture_application_preferences_file().unwrap();
                assert!(!file.has_changes());
                assert!(file.source_archive().is_some());
                state.command(Command::Undo);
                assert_eq!(state.display_options, before);
                state.command(Command::Redo);
                assert_eq!(state.display_options, expected);
                if disk {
                    assert_eq!(
                        deadsync_editor::audio_preferences::File::open(file.path())
                            .unwrap()
                            .display_options(),
                        expected
                    );
                }
                assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
            }
        }
    }
    #[test]
    fn remaining_numeric_groups_use_exact_drafts_cancellation_canonical_save_and_undo() {
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854., 480.,
        ));
        for language in ["en", "fr"] {
            for disk in [false, true] {
                for kind in [
                    Kind::General,
                    Kind::Receptor,
                    Kind::Stream,
                    Kind::Performance,
                ] {
                    let (_root, mut state) = state(language, disk);
                    let before = live(&state, kind);
                    let song = state.workspace.file().session.serialize().unwrap();
                    let (save, _, text) = open(&mut state, kind);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.wants_text_input());
                    assert!(state.text_input(text));
                    press(&mut state, KeyCode::Escape);
                    assert!(live(&state, kind) == before);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(text));
                    press(&mut state, KeyCode::Enter);
                    assert!(state.preference_numeric.is_none(), "{}", state.status);
                    let after = if kind == Kind::General {
                        live(&state, kind)
                    } else {
                        state.numeric_snapshot(kind).unwrap()
                    };
                    assert!(after != before);
                    if kind != Kind::General {
                        state.panel = Some(match kind {
                            Kind::Receptor => Panel::ReceptorSettings(save),
                            Kind::Stream => Panel::StreamSettings(save),
                            Kind::Performance => Panel::Performance(save),
                            _ => unreachable!(),
                        });
                        press(&mut state, KeyCode::Enter);
                    }
                    assert!(live(&state, kind) == after, "{}", state.status);
                    let file = state.capture_application_preferences_file().unwrap();
                    assert!(!file.has_changes());
                    assert!(file.source_archive().is_some());
                    state.command(Command::Undo);
                    assert!(live(&state, kind) == before, "{}", state.status);
                    state.command(Command::Redo);
                    assert!(live(&state, kind) == after, "{}", state.status);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
                    if disk {
                        let reopened =
                            deadsync_editor::audio_preferences::File::open(file.path()).unwrap();
                        assert!(!reopened.has_changes());
                        assert!(reopened.source_archive().is_some());
                    }
                }
            }
        }
    }
    #[test]
    fn remaining_numeric_source_bounds_enabled_controls_and_resize_invalidation_are_exact() {
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854., 480.,
        ));
        let (_root, mut state) = state("en", false);
        let enter = |state: &mut State, text: &str| {
            press(state, KeyCode::Enter);
            assert!(state.wants_text_input());
            assert!(state.text_input(text));
            press(state, KeyCode::Enter);
            assert!(state.preference_numeric.is_none(), "{}", state.status);
        };
        state.open_general_options();
        state.general_field = 0;
        state.panel = Some(Panel::GeneralOptions(1));
        enter(&mut state, "0");
        assert_eq!(state.general_options.undo_history_size, 1);
        assert_eq!(state.workspace.file().session.history_capacity(), 1);
        enter(&mut state, "99999");
        assert_eq!(state.workspace.file().session.history_capacity(), 32768);
        state.general_field = 1;
        enter(&mut state, "0.123456789012345");
        assert_eq!(state.general_options.new_sync, 0.123456789012345);
        state.general_field = 2;
        enter(&mut state, "-1");
        assert_eq!(state.general_options.open_sync, 0.);
        state.general_field = 8;
        let dpi = state.general_options.dpi_scale;
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        assert_eq!(state.general_options.dpi_scale, dpi);
        let mut p = state.general_options.clone();
        p.custom_dpi = true;
        state.commit_general_options(p).unwrap();
        enter(&mut state, "1.23456789");
        assert_eq!(state.general_options.dpi_scale, 1.23456789);
        enter(&mut state, "99");
        assert_eq!(state.general_options.dpi_scale, 8.);
        enter(&mut state, "0");
        assert_eq!(state.general_options.dpi_scale, 0.25);
        state.open_stream_settings();
        state.panel = Some(Panel::StreamSettings(3));
        enter(&mut state, "99");
        assert_eq!(state.stream_draft.unwrap().minimum_length, 8);
        state.panel = Some(Panel::StreamSettings(4));
        enter(&mut state, "99");
        assert_eq!(state.stream_draft.unwrap().short_break_cutoff, 64);
        enter(&mut state, "-2");
        assert_eq!(state.stream_draft.unwrap().short_break_cutoff, 0);
        state.panel = None;
        state.open_performance();
        state.panel = Some(Panel::Performance(2));
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        let mut p = state.performance_numeric_draft().unwrap();
        p.scale = deadsync_editor::performance::preferences::Scale::Explicit;
        state.set_performance_numeric_draft(p).unwrap();
        enter(&mut state, "0.123456789012345");
        assert_eq!(
            state.performance_numeric_draft().unwrap().explicit_seconds,
            0.123456789012345
        );
        enter(&mut state, "2");
        assert_eq!(
            state.performance_numeric_draft().unwrap().explicit_seconds,
            1.
        );
        state.panel = Some(Panel::Performance(3));
        enter(&mut state, "0");
        assert_eq!(state.performance_numeric_draft().unwrap().max_frames, 2);
        enter(&mut state, "99999");
        assert_eq!(state.performance_numeric_draft().unwrap().max_frames, 1024);
        state.open_receptor_settings();
        state.panel = Some(Panel::ReceptorSettings(12));
        let mut p = state.receptor_draft.unwrap();
        p.placement.lock_x = true;
        state.receptor_draft = Some(p);
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        p.placement.lock_x = false;
        p.placement.center = true;
        state.receptor_draft = Some(p);
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        p.placement.center = false;
        state.receptor_draft = Some(p);
        enter(&mut state, "2147483647");
        assert_eq!(
            state.receptor_draft.unwrap().placement.x,
            (screen_width() as i32 - 1).max(0)
        );
        state.panel = Some(Panel::ReceptorSettings(13));
        enter(&mut state, "-1");
        assert_eq!(state.receptor_draft.unwrap().placement.y, 0);
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric_is_current());
        let width = screen_width();
        let height = screen_height();
        deadlib_present::space::with_metrics(
            deadlib_present::space::Metrics::centered(width, height + 1.),
            || assert!(!state.preference_numeric_is_current()),
        );
        assert!(state.preference_numeric_is_current());
    }
    #[test]
    fn precision_reaches_receptor_projection_stream_notation_and_performance_monitor() {
        use std::sync::atomic::AtomicBool;
        use std::time::{Duration, Instant};
        i18n::init_for_tests();
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854., 480.,
        ));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("consumers.ssc");
        let stream = vec!["1000"; 16].join("\n");
        let gap = vec!["0000"; 16].join("\n");
        // Source density commits a run at an event boundary, not at an empty
        // measure with no events. The real Label at beat4 makes this a break.
        std::fs::write(&path,format!("#VERSION:0.83;#BPMS:0=120;#LABELS:4=Gap boundary;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{stream},\n{gap},\n{stream};")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let enter = |state: &mut State, text: &str| {
            press(state, KeyCode::Enter);
            assert!(state.text_input(text));
            press(state, KeyCode::Enter);
            assert!(state.preference_numeric.is_none(), "{}", state.status);
        };
        state.open_receptor_settings();
        state.panel = Some(Panel::ReceptorSettings(13));
        enter(&mut state, "137");
        state.panel = Some(Panel::ReceptorSettings(9));
        press(&mut state, KeyCode::Enter);
        let y = state.chart_viewport().cursor;
        assert_eq!(y, 187.);
        assert_eq!(
            state
                .row_projection(105., screen_height() - 85., y)
                .y(state.workspace.row()),
            f64::from(y)
        );
        state.command(Command::Undo);
        assert_eq!(state.chart_viewport().cursor, 160.);
        state.command(Command::Redo);
        assert_eq!(state.chart_viewport().cursor, 187.);
        let breakdown = |state: &State| {
            deadsync_editor::density::task::Task::capture(
                &state.workspace.file().session,
                state.workspace.chart(),
                state.source_density.stream,
            )
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap()
            .series
            .breakdown
        };
        let report=deadsync_editor::density::task::Task::capture(&state.workspace.file().session,state.workspace.chart(),state.source_density.stream).unwrap().run(&AtomicBool::new(false)).unwrap();
        assert_eq!(report.series.measures.iter().map(|m|m.steps).collect::<Vec<_>>(),vec![16,0,16]);
        assert_eq!(report.series.streams,vec![deadsync_editor::density::Stream{start:0,last:0},deadsync_editor::density::Stream{start:2,last:2}]);
        assert_eq!(breakdown(&state), Some("1-1".into()));
        state.open_stream_settings();
        state.panel = Some(Panel::StreamSettings(3));
        enter(&mut state, "2");
        state.panel = Some(Panel::StreamSettings(8));
        press(&mut state, KeyCode::Enter);
        assert_eq!(breakdown(&state), None);
        state.command(Command::Undo);
        assert_eq!(breakdown(&state), Some("1-1".into()));
        state.command(Command::Redo);
        assert_eq!(breakdown(&state), None);
        state.open_stream_settings();
        state.panel = Some(Panel::StreamSettings(3));
        enter(&mut state, "1");
        state.panel = Some(Panel::StreamSettings(4));
        enter(&mut state, "0");
        state.panel = Some(Panel::StreamSettings(8));
        press(&mut state, KeyCode::Enter);
        assert_eq!(breakdown(&state), Some("1|1".into()));
        state.panel = None;
        state.open_performance();
        let mut p = state.performance_numeric_draft().unwrap();
        p.scale = deadsync_editor::performance::preferences::Scale::Explicit;
        state.set_performance_numeric_draft(p).unwrap();
        state.panel = Some(Panel::Performance(2));
        enter(&mut state, "0.123456789");
        state.panel = Some(Panel::Performance(3));
        enter(&mut state, "137");
        state.panel = Some(Panel::Performance(5));
        press(&mut state, KeyCode::Enter);
        let now = Instant::now();
        for frame in 0..145 {
            state.begin_performance_frame(now + Duration::from_millis(frame * 16));
        }
        let summary = state.performance_summary().unwrap();
        assert_eq!(summary.frames.len(), 137);
        assert_eq!(summary.plot_scale, 0.123456789_f64 as f32);
    }
    #[test]
    fn limit_history_never_replaces_disjoint_live_records_with_the_disk_baseline() {
        let (_root, mut state) = state("en", true);
        let mut recent = state.file_menu.recent_preferences().clone();
        recent
            .record(&deadsync_editor::recent_songs::Song {
                file: "unsaved-recent".into(),
                title: None,
                pack: None,
                spacing_zoom: 1.,
                chart_position: 0.,
                active: vec![],
                focused: None,
            })
            .unwrap();
        let entries = recent.entries.clone();
        state.file_menu.install_recent(recent);
        assert!(
            state
                .capture_application_preferences_file()
                .unwrap()
                .has_changes()
        );
        state.commit_recent_files_history_size(2).unwrap();
        assert_eq!(state.file_menu.recent_preferences().entries, entries);
        assert_eq!(state.file_menu.recent_maximum(), 2);
        state.command(Command::Undo);
        assert_eq!(state.file_menu.recent_preferences().entries, entries);
        assert_eq!(state.file_menu.recent_maximum(), 20);
        state.command(Command::Redo);
        assert_eq!(state.file_menu.recent_preferences().entries, entries);
        assert_eq!(state.file_menu.recent_maximum(), 2);
        assert!(
            deadsync_editor::audio_preferences::File::open(
                state.application_preferences_owner().unwrap().path()
            )
            .unwrap()
            .recent_songs()
            .entries
            .is_empty(),
            "limit-only publication must not fabricate persistence of another owner's records"
        );
    }
    #[test]
    fn recent_limit_keyboard_cancel_publication_and_history_keep_existing_records() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                let (_root, mut state) = state(language, disk);
                let file = state.capture_application_preferences_file().unwrap();
                let draft=state.workspace.prepare_source_preferences(&file,br#"{"RecentFiles":[{"FileName":"A","ActiveCharts":[]},{"FileName":"B","ActiveCharts":[]},{"FileName":"C","ActiveCharts":[]},{"FileName":"D","ActiveCharts":[]}],"VendorSource":{"keep":[1,3]}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();
                state
                    .workspace
                    .import_source_preferences(draft, &file, true)
                    .unwrap();
                state.sync_source_preferences_history().unwrap();
                let entries = state.file_menu.recent_preferences().entries.clone();
                let song = state.workspace.file().session.serialize().unwrap();
                let revision = state.workspace.file().session.revision();
                open(&mut state, Kind::Recent);
                press(&mut state, KeyCode::Enter);
                assert!(state.wants_text_input());
                assert!(state.text_input("2"));
                press(&mut state, KeyCode::Escape);
                assert_eq!(state.file_menu.recent_maximum(), 20);
                assert_eq!(state.workspace.file().session.revision(), revision);
                press(&mut state, KeyCode::Enter);
                assert!(state.text_input("2"));
                press(&mut state, KeyCode::Enter);
                assert!(state.preference_numeric.is_none(), "{}", state.status);
                assert_eq!(state.file_menu.recent_maximum(), 2);
                assert_eq!(state.file_menu.recent_preferences().entries, entries);
                let owner = state.capture_application_preferences_file().unwrap();
                assert_eq!(owner.recent_songs().maximum, 2);
                assert!(!owner.has_changes());
                assert!(owner.source_archive().is_some());
                state.command(Command::Undo);
                assert_eq!(state.file_menu.recent_maximum(), 20);
                assert_eq!(state.file_menu.recent_preferences().entries, entries);
                state.command(Command::Redo);
                assert_eq!(state.file_menu.recent_maximum(), 2);
                assert_eq!(state.file_menu.recent_preferences().entries, entries);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
                if disk {
                    assert_eq!(
                        deadsync_editor::audio_preferences::File::open(owner.path())
                            .unwrap()
                            .recent_songs()
                            .maximum,
                        2
                    );
                }
                // The source slider bounds only an explicit UI edit, not imported raw.
                press(&mut state, KeyCode::Enter);
                assert!(state.text_input("999"));
                press(&mut state, KeyCode::Enter);
                assert_eq!(state.file_menu.recent_maximum(), 100);
                press(&mut state, KeyCode::Enter);
                assert!(state.text_input("-1"));
                press(&mut state, KeyCode::Enter);
                assert_eq!(state.file_menu.recent_maximum(), 0);
                assert_eq!(state.file_menu.recent_preferences().entries, entries);
                press(&mut state, KeyCode::Home);
                assert_eq!(state.file_menu.recent_maximum(), 20);
            }
        }
    }
    #[test]
    fn palette_preview_is_visible_but_not_a_canonical_publication_baseline() {
        for disk in [false, true] {
            let (_root, mut state) = state("en", disk);
            let committed = Arc::clone(&state.note_colors);
            open(&mut state, Kind::Note);
            press(&mut state, KeyCode::Enter);
            assert!(state.text_input("0.123456"));
            press(&mut state, KeyCode::Enter);
            assert_ne!(state.note_colors, committed);
            assert!(state.hold_colors.is_current(&state.note_colors));
            let captured = state.capture_application_preferences_file().unwrap();
            assert_eq!(captured.note_colors_arc(), committed);
            assert!(!captured.has_changes());
            select_row(&mut state, 6);
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.note_colors, committed);
            assert!(state.hold_colors.is_current(&committed));
        }
    }

    #[test]
    fn precise_note_color_entry_reaches_both_real_palette_caches_and_undo_redo() {
        use deadsync_editor::minimap_palette::Family;
        let (_root, mut state) = state("en", false);
        let original_main = state
            .hold_colors
            .tap("dance-single", 0, 0, 0, false)
            .unwrap();
        let original_map = state
            .minimap
            .content
            .palette()
            .colors(Family::Row, 0, 0, 0, false)
            .unwrap()
            .head;
        open(&mut state, Kind::Note);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.123456"));
        press(&mut state, KeyCode::Enter);
        select_row(&mut state, 5);
        press(&mut state, KeyCode::Enter);
        assert!(
            state.note_color_draft.is_none(),
            "Save failed: {}",
            state.status
        );
        assert!(state.hold_colors.is_current(&state.note_colors));
        assert!(!state.application_preferences_owner().unwrap().has_changes());
        let changed_main = state
            .hold_colors
            .tap("dance-single", 0, 0, 0, false)
            .unwrap();
        let changed_map = state
            .minimap
            .content
            .palette()
            .colors(Family::Row, 0, 0, 0, false)
            .unwrap()
            .head;
        assert_ne!(changed_main, original_main);
        assert_ne!(changed_map, original_map);
        state.command(Command::Undo);
        assert_eq!(
            state
                .hold_colors
                .tap("dance-single", 0, 0, 0, false)
                .unwrap(),
            original_main
        );
        assert_eq!(
            state
                .minimap
                .content
                .palette()
                .colors(Family::Row, 0, 0, 0, false)
                .unwrap()
                .head,
            original_map
        );
        state.command(Command::Redo);
        assert_eq!(
            state
                .hold_colors
                .tap("dance-single", 0, 0, 0, false)
                .unwrap(),
            changed_main
        );
        assert_eq!(
            state
                .minimap
                .content
                .palette()
                .colors(Family::Row, 0, 0, 0, false)
                .unwrap()
                .head,
            changed_map
        );
    }
    #[test]
    fn precise_minimap_range_reaches_the_real_viewport_projection_and_roundtrips_history() {
        use deadsync_editor::minimap_viewport::{Geometry, Viewport};
        let (_root, mut state) = state("en", false);
        let projection = |state: &State| {
            Geometry::build(
                Axis::Time,
                Viewport {
                    top: 0.0,
                    bottom: 600.0,
                    focal: 100.0,
                },
                300.0,
                5.0,
                (0.0, 600.0),
                state.minimap.preferences.visible_range(Axis::Time),
                0,
                600,
                false,
                false,
            )
            .unwrap()
            .projection
        };
        let before = projection(&state);
        open(&mut state, Kind::MiniViewport);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("57"));
        press(&mut state, KeyCode::Enter);
        select_row(&mut state, 5);
        press(&mut state, KeyCode::Enter);
        let after = projection(&state);
        assert_eq!(after.mini_range, 57.0);
        assert_ne!(before.mini_range, after.mini_range);
        state.command(Command::Undo);
        assert_eq!(projection(&state).mini_range, before.mini_range);
        state.command(Command::Redo);
        assert_eq!(projection(&state).mini_range, 57.0);
    }
    #[test]
    fn density_numeric_height_changes_real_mesh_and_dark_component_changes_real_quad() {
        use deadsync_editor::{
            dark_preferences::Order,
            density::layout::Orientation,
            density::{Plot, Sample},
        };
        let (_root, mut state) = state("fr", false);
        let plot = Plot {
            samples: vec![
                Sample {
                    has_steps: true,
                    time: 0.0,
                    rate: 4.0,
                    percent: 0.5,
                },
                Sample {
                    has_steps: true,
                    time: 2.0,
                    rate: 8.0,
                    percent: 1.0,
                },
            ],
            peak: 8.0,
            aggregated: false,
        };
        let mesh = |state: &State| {
            let p = state.density_preferences();
            super::super::density_render::Mesh::build(
                &plot,
                2.0,
                p,
                100,
                p.height,
                Orientation::Horizontal,
                false,
            )
            .vertices
            .iter()
            .map(|v| v.pos[1].to_bits())
            .collect::<Vec<_>>()
        };
        let before = mesh(&state);
        assert!(!before.is_empty());
        open(&mut state, Kind::Density);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("137"));
        press(&mut state, KeyCode::Enter);
        select_row(&mut state, 3);
        press(&mut state, KeyCode::Enter);
        let after = mesh(&state);
        assert_ne!(after, before);
        state.command(Command::Undo);
        assert_eq!(mesh(&state), before);
        state.command(Command::Redo);
        assert_eq!(mesh(&state), after);
        let mut dark = state.dark_preferences;
        dark.set_show(true);
        state.configure_dark_preferences(dark);
        let color = |state: &State| {
            let mut actors = Vec::new();
            state.dark_background_actors(&mut actors, Order::AfterBackground);
            actors
                .into_iter()
                .find_map(|a| match a {
                    Actor::Sprite { tint, .. } => Some(tint),
                    _ => None,
                })
                .expect("window dark quad")
        };
        let before = color(&state);
        open(&mut state, Kind::Dark);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.123456"));
        press(&mut state, KeyCode::Enter);
        select_row(&mut state, 8);
        press(&mut state, KeyCode::Enter);
        let after = color(&state);
        assert_ne!(after, before);
        state.command(Command::Undo);
        assert_eq!(color(&state), before);
        state.command(Command::Redo);
        assert_eq!(color(&state), after);
    }
    #[test]
    fn direct_speed_entry_preserves_source_float_precision_without_an_undo_action() {
        let (_root, mut state) = state("en", false);
        let revision = state.workspace.file().session.revision();
        open(&mut state, Kind::Speed);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("321.123456"));
        press(&mut state, KeyCode::Escape);
        assert_eq!(state.scroll_preferences.speed(0), Some(300.0));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("321.123456"));
        press(&mut state, KeyCode::Enter);
        let exact = f64::from(321.123456_f32);
        assert_eq!(state.scroll_preferences.speed(0), Some(exact));
        assert_eq!(
            state
                .zoom
                .spacing(deadsync_editor::zoom::SpacingMode::ConstantTime),
            exact
        );
        assert_eq!(state.workspace.file().session.revision(), revision);
    }
}

#[cfg(test)]
mod metadata_numeric_tests {
    use super::*;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code, pressed, repeat: false, timestamp: std::time::Instant::now(), host_nanos: 0,
            });
        }
    }
    fn state(language: &str) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path, "#VERSION:0.83;#TITLE:Keep;#OFFSET:0.125;#SAMPLESTART:0;#SAMPLELENGTH:1;#LASTSECONDHINT:2;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0,\"Vendor\":7};#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.25;#NOTES:1000;").unwrap();
        let state = State::open(&path, 0).unwrap();
        (root, state)
    }
    fn values(state: &State) -> [u64; 5] {
        let p = state.song_properties.as_ref().unwrap();
        [p.sync_offset(), p.music_offset(), p.sample_start(), p.sample_length(), p.last_second_hint()].map(f64::to_bits)
    }
    fn select(state: &mut State, field: usize) {
        state.panel = Some(match field {
            0 => Panel::SongProperties(0),
            1 => Panel::SongProperties(4),
            _ => Panel::SongSample(field - 2),
        });
    }
    #[test]
    fn precise_song_fields_stage_cancel_apply_undo_redo_and_actual_save_reopen_en_fr_chartless() {
        let input = [0.0123456789f64, -0.1234567891, -0.2345678912, 0.3456789123, 0.4567891234];
        for language in ["en", "fr"] {
            for chartless in [false, true] {
                let (root, mut state) = state(language);
                if chartless {
                    state.close_focused_audio_view();
                    state.workspace.clear_active_chart();
                    state.refresh();
                }
                state.open_song_properties();
                let original = values(&state);
                let bytes = state.workspace.file().session.serialize().unwrap();
                let revision = state.workspace.file().session.revision();
                for (field, value) in input.into_iter().enumerate() {
                    select(&mut state, field);
                    let before = values(&state);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.wants_text_input());
                    assert!(state.text_input(&value.to_string()));
                    assert_eq!(values(&state), before);
                    press(&mut state, KeyCode::Escape);
                    assert_eq!(values(&state), before);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(&value.to_string()));
                    press(&mut state, KeyCode::Enter);
                    assert!(state.preference_numeric.is_none());
                    assert_eq!(values(&state)[field], value.to_bits());
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
                    assert_eq!(state.workspace.file().session.revision(), revision);
                    let mut actors = Vec::new();
                    state.actors(&mut actors);
                    assert!(!actors.is_empty());
                }
                // Whole inspector cancellation also discards every accepted scalar.
                state.panel = Some(Panel::SongProperties(2));
                press(&mut state, KeyCode::Enter);
                state.open_song_properties();
                assert_eq!(values(&state), original);
                for (field, value) in input.into_iter().enumerate() {
                    select(&mut state, field);
                    press(&mut state, KeyCode::Enter);
                    assert!(state.text_input(&value.to_string()));
                    press(&mut state, KeyCode::Enter);
                }
                state.panel = Some(Panel::SongProperties(1));
                press(&mut state, KeyCode::Enter);
                assert!(state.song_properties.is_none(), "{}", state.status);
                state.open_song_properties();
                assert_eq!(values(&state), input.map(f64::to_bits));
                state.panel = None;
                state.command(Command::Undo);
                state.open_song_properties();
                assert_eq!(values(&state), original);
                state.panel = None;
                state.command(Command::Redo);
                state.open_song_properties();
                assert_eq!(values(&state), input.map(f64::to_bits));
                state.panel = None;
                state.workspace.execute(Command::Save).unwrap();
                let mut reopened = State::open(&root.path().join("song.ssc"), 0).unwrap();
                reopened.open_song_properties();
                assert_eq!(values(&reopened), input.map(f64::to_bits));
                assert_eq!(reopened.workspace.file().session.document().text(None, "TITLE").as_deref(), Some("Keep"));
                assert!(reopened.workspace.file().session.document().text(None, "GASongData").unwrap().contains("\"Vendor\":7"));
            }
        }
    }
    #[test]
    fn disabled_chart_offset_is_readonly_then_precise_entry_has_canonical_history_and_reopen() {
        for language in ["en", "fr"] {
            let (root, mut state) = state(language);
            state.open_chart_properties();
            state.chart_properties.as_mut().unwrap().set_use_chart_offset(false);
            let before = state.chart_properties.as_ref().unwrap().music_offset();
            state.panel = Some(Panel::ChartProperties(2));
            for code in [KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Home, KeyCode::Enter] {
                press(&mut state, code);
                assert!(state.preference_numeric.is_none());
                assert_eq!(state.chart_properties.as_ref().unwrap().music_offset().to_bits(), before.to_bits());
            }
            press(&mut state, KeyCode::ArrowUp);
            assert!(matches!(state.panel, Some(Panel::ChartProperties(1))));
            press(&mut state, KeyCode::ArrowRight);
            assert!(state.chart_properties.as_ref().unwrap().uses_chart_offset());
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            assert!(state.text_input("-0.987654321"));
            press(&mut state, KeyCode::Escape);
            assert_eq!(state.chart_properties.as_ref().unwrap().music_offset().to_bits(), before.to_bits());
            press(&mut state, KeyCode::Enter);
            assert!(state.text_input("-0.987654321"));
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.chart_properties.as_ref().unwrap().music_offset().to_bits(), (-0.987654321f64).to_bits());
            state.panel = Some(Panel::ChartProperties(4));
            press(&mut state, KeyCode::Enter);
            assert!(state.chart_properties.is_none(), "{}", state.status);
            assert_eq!(state.workspace.chart_properties().unwrap().music_offset().to_bits(), (-0.987654321f64).to_bits());
            state.command(Command::Undo);
            assert_eq!(state.workspace.chart_properties().unwrap().music_offset().to_bits(), before.to_bits());
            state.command(Command::Redo);
            assert_eq!(state.workspace.chart_properties().unwrap().music_offset().to_bits(), (-0.987654321f64).to_bits());
            state.workspace.execute(Command::Save).unwrap();
            let reopened = State::open(&root.path().join("song.ssc"), 0).unwrap();
            assert_eq!(reopened.workspace.chart_properties().unwrap().music_offset().to_bits(), (-0.987654321f64).to_bits());
        }
    }
    #[test]
    fn itg_button_is_enabled_at_source_zero_only_and_cannot_repeat() {
        for language in ["en", "fr"] {
            let (_root, mut state) = state(language);
            state.open_song_properties();
            state.panel = Some(Panel::SongProperties(5));
            let original = state.song_properties.as_ref().unwrap().music_offset();
            press(&mut state, KeyCode::Enter);
            let p = state.song_properties.as_ref().unwrap();
            assert_eq!(p.music_offset(), original + 0.009);
            assert_eq!(p.sync_offset(), 0.009);
            assert!(!p.can_apply_itg_offset());
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.song_properties.as_ref().unwrap().music_offset(), original + 0.009);
            press(&mut state, KeyCode::ArrowDown);
            assert!(matches!(state.panel, Some(Panel::SongProperties(6))));
            state.panel = Some(Panel::SongProperties(1));
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.workspace.song_properties().unwrap().music_offset(), original + 0.009);
            state.command(Command::Undo);
            assert_eq!(state.workspace.song_properties().unwrap().music_offset(), original);
            state.command(Command::Redo);
            assert_eq!(state.workspace.song_properties().unwrap().music_offset(), original + 0.009);
        }
    }
    #[test]
    fn metadata_numeric_rejects_stale_draft_focus_and_content_and_clamps_only_source_minima() {
        let (_root, mut state) = state("en");
        state.open_song_properties();
        state.panel = Some(Panel::SongSample(1));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("-3.1415926535"));
        press(&mut state, KeyCode::Enter);
        assert_eq!(state.song_properties.as_ref().unwrap().sample_length(), 0.0);
        state.panel = Some(Panel::SongProperties(4));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("nan"));
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_some());
        assert_eq!(state.song_properties.as_ref().unwrap().music_offset(), 0.125);
        press(&mut state, KeyCode::Escape);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.7654321"));
        state.song_properties.as_mut().unwrap().set_text(deadsync_editor::song_text::Field::Title, "Other draft").unwrap();
        assert!(!state.preference_numeric_is_current());
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        assert_eq!(state.song_properties.as_ref().unwrap().music_offset(), 0.125);
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.7654321"));
        state.open_song_properties();
        assert!(state.preference_numeric.is_none(), "replacing even an equivalent inspector closes its scratch");
        state.panel = Some(Panel::SongProperties(4));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.7654321"));
        state.workspace.execute(Command::Place(1)).unwrap();
        assert!(!state.preference_numeric_is_current());
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
        state.open_song_properties();
        state.panel = Some(Panel::SongProperties(4));
        press(&mut state, KeyCode::Enter);
        assert!(state.text_input("0.7654321"));
        state.workspace.clear_active_chart();
        assert!(!state.preference_numeric_is_current());
        press(&mut state, KeyCode::Enter);
        assert!(state.preference_numeric.is_none());
    }
}
