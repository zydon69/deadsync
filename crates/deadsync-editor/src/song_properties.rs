//! GrooveAuthor song-level editor metadata. SyncOffset affects preview only.
mod custom;
/// Prospective row-preserving main-music/offset publication.
pub mod live {
    pub use crate::history::song_live::{Application, Plan};
}
mod selectable;
pub use crate::history::song_presentation::{Application, PresentationReceipt};
use crate::{
    Error,
    document::Document,
    history::{ContentToken, Session},
};
pub use selectable::Selectable;
use serde_json::{Map, Value};
pub const DEFAULT_SYNC_OFFSET: f64 = 0.009;
const VERSION: &str = "GASongDataVersion";
const DATA: &str = "GASongData";

fn read(document: &Document) -> Result<(Map<String, Value>, f64), Error> {
    let fields = match (
        custom::text(document, VERSION),
        custom::text(document, DATA),
    ) {
        (None, None) => return Ok((Map::new(), document.load_context().open_sync_offset())),
        (Some(version), Some(data)) if version.trim().parse::<i32>().ok() == Some(1) => {
            let data = crate::generation::jsonc::strip_comments(data.as_bytes())?;
            match serde_json::from_slice(&data).map_err(|e| Error::InvalidFormat(e.to_string()))? {
                Value::Object(fields) => fields,
                _ => return invalid("GrooveAuthor song metadata must be an object"),
            }
        }
        (Some(_), Some(_)) => {
            return Err(Error::Unsupported(
                "Unknown GrooveAuthor song metadata version".into(),
            ));
        }
        _ => return invalid("Incomplete GrooveAuthor song metadata"),
    };
    // V1's missing field uses the C# double default, not the open-song preference.
    let offset = fields
        .get("SyncOffset")
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| Error::InvalidFormat("Invalid editor sync offset".into()))
        })
        .transpose()?
        .unwrap_or(0.0);
    // Keep the complete object, including future fields and default chart selection.
    Ok((fields, offset))
}
pub(crate) fn validate_structural_deletion(document: &Document) -> Result<(), Error> {
    if custom::text(document, VERSION).is_none() && custom::text(document, DATA).is_none() {
        Ok(())
    } else {
        read(document).map(|_| ())
    }
}
pub(crate) fn timing_custom_fields(document: &Document) -> Option<Map<String, Value>> {
    read(document).ok().map(|(fields, _)| fields)
}
pub fn sync_offset(document: &Document) -> Result<f64, Error> {
    read(document).map(|(_, offset)| offset)
}

/// Song-level widgets moved by the source keyboard callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Preview,
    EndHint,
}
pub struct SampleAudio {
    pub path: std::path::PathBuf,
    pub start: f64,
    /// None means the complete dedicated PREVIEW file.
    pub length: Option<f64>,
}

pub struct Draft {
    token: ContentToken,
    timing_choice: crate::song_timing_chart::Choice,
    original_timing_choice: crate::song_timing_chart::Choice,
    original_selectable: Selectable,
    original_selectable_raw: Option<String>,
    selectable_explicit: bool,
    selectable: Selectable,
    original: f64,
    offset: f64,
    original_music_offset: f64,
    music_offset: f64,
    last_second_hint: [f64; 2],
    original_sample: [f64; 2],
    sample: [f64; 2],
    fields: Map<String, Value>,
    text: crate::song_text::Text,
}
impl Draft {
    pub fn capture(session: &Session) -> Result<Self, Error> {
        let (fields, offset) = read(session.document())?;
        let music_offset = session
            .document()
            .text(None, "OFFSET")
            .map(|value| {
                value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| Error::InvalidEdit("Invalid song music offset".into()))
            })
            .transpose()?
            .unwrap_or(0.0);
        let mut sample = [0.0; 2];
        for (index, name) in ["SAMPLESTART", "SAMPLELENGTH"].into_iter().enumerate() {
            sample[index] = session
                .document()
                .text(None, name)
                .map(|value| {
                    value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| Error::InvalidEdit(format!("Invalid {name}")))
                })
                .transpose()?
                .unwrap_or(0.0);
        }
        let last_second_hint = {
            let value = session
                .document()
                .text(None, "LASTSECONDHINT")
                .map(|value| {
                    value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| Error::InvalidEdit("Invalid last second hint".into()))
                })
                .transpose()?
                .unwrap_or(0.0);
            [value, value]
        };
        let selectable = Selectable::read(session.document().text(None, "SELECTABLE").as_deref());
        Ok(Self {
            original_selectable: selectable,
            original_selectable_raw: session.document().text(None, "SELECTABLE"),
            selectable_explicit: false,
            selectable,
            token: session.content_token(),
            timing_choice: session.song_timing_choice()?,
            original_timing_choice: session.song_timing_choice()?,
            original: offset,
            offset,
            original_music_offset: music_offset,
            music_offset,
            last_second_hint,
            original_sample: sample,
            sample,
            fields,
            text: crate::song_text::Text::try_read(session.document())?,
        })
    }
    pub fn stage_assets(
        &mut self,
        assets: &crate::song_assets::Assets,
        only_unset: bool,
    ) -> Result<usize, Error> {
        assets.stage(&mut self.text, only_unset)
    }
    pub fn text(&self, field: crate::song_text::Field) -> &str {
        self.text.get(field)
    }
    pub fn supports_text(&self, field: crate::song_text::Field) -> bool {
        self.text.supports(field)
    }
    pub fn set_text(&mut self, field: crate::song_text::Field, value: &str) -> Result<(), Error> {
        self.text.set(field, value)
    }
    pub fn selectable(&self) -> Selectable {
        self.selectable
    }
    pub fn set_selectable(&mut self, value: Selectable) {
        self.selectable = value;
        self.selectable_explicit = true;
    }
    fn selectable_changed(&self) -> bool {
        self.selectable != self.original_selectable
            || self.selectable_explicit
                && self.original_selectable_raw.as_deref()
                    != Some(self.selectable.source_name().as_str())
    }
    pub fn has_changes(&self) -> bool {
        if self.selectable_changed() {
            return true;
        }
        if self.timing_choice != self.original_timing_choice {
            return true;
        }
        !crate::number::reference_equal(self.last_second_hint[0], self.last_second_hint[1])
            || self.text.has_changes()
            || !crate::number::reference_equal(self.original, self.offset)
            || !crate::number::reference_equal(self.original_music_offset, self.music_offset)
            || self
                .sample
                .iter()
                .zip(self.original_sample)
                .any(|(value, original)| !crate::number::reference_equal(*value, original))
    }
    pub fn supports_last_second_hint(&self) -> bool {
        true
    }
    pub fn last_second_hint(&self) -> f64 {
        self.last_second_hint[1]
    }
    pub fn set_last_second_hint(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return invalid("Last second hint must be finite");
        }
        let values = &mut self.last_second_hint;
        if !crate::number::reference_equal(values[1], value) {
            values[1] = value;
        }
        Ok(())
    }
    pub fn clock(&self, session: &Session, chart: usize) -> Result<crate::playback::Clock, Error> {
        if !session.accepts_content_token(&self.token) {
            return invalid("Song properties draft is stale or belongs to another document");
        }
        let chart_properties = crate::chart_properties::Draft::capture(session, chart)?;
        let offset = if chart_properties.uses_chart_offset() {
            chart_properties.music_offset()
        } else {
            self.music_offset
        };
        Ok(crate::playback::Clock::capture(session, chart)?.with_song_draft(offset, self.offset))
    }
    pub fn move_marker_to_time(&mut self, marker: Marker, seconds: f64) -> Result<(), Error> {
        if !seconds.is_finite() {
            return invalid("Song marker time must be finite");
        }
        match marker {
            Marker::Preview => self.set_sample_start_from_time(seconds),
            Marker::EndHint => self.set_last_second_hint(seconds.max(0.0)),
        }
    }
    pub fn set_sample_start_from_time(&mut self, seconds: f64) -> Result<(), Error> {
        if !seconds.is_finite() {
            return invalid("Preview position must be finite");
        }
        self.set_sample_start(seconds.max(0.0))
    }
    pub fn set_sample_end_from_time(&mut self, seconds: f64) -> Result<(), Error> {
        if !seconds.is_finite() {
            return invalid("Preview position must be finite");
        }
        self.set_sample_length((seconds.max(0.0) - self.sample_start()).max(0.0))
    }
    /// Resolve preview media and raw audio-file cut without applying chart offsets.
    pub fn sample_audio(
        &self,
        session: &Session,
        chart: usize,
        simfile: &std::path::Path,
    ) -> Result<SampleAudio, Error> {
        self.sample_audio_at_directory(
            session,
            chart,
            simfile.parent().unwrap_or(std::path::Path::new(".")),
        )
    }
    pub fn sample_audio_from_directory(
        &self,
        session: &Session,
        chart: usize,
        directory: &std::path::Path,
    ) -> Result<SampleAudio, Error> {
        if !directory.is_absolute() {
            return invalid("Preview requires an absolute resource directory");
        }
        self.sample_audio_at_directory(session, chart, directory)
    }
    fn sample_audio_at_directory(
        &self,
        session: &Session,
        chart: usize,
        directory: &std::path::Path,
    ) -> Result<SampleAudio, Error> {
        if !session.accepts_content_token(&self.token) {
            return invalid("Song properties draft is stale or belongs to another document");
        }
        let dedicated = Some(self.text(crate::song_text::Field::PreviewMusic).to_owned())
            .filter(|value| !value.is_empty());
        let (name, start, length) = if let Some(name) = dedicated {
            (name, 0.0, None)
        } else {
            if self.sample_length() <= 0.0 {
                return invalid("Preview length must be positive");
            }
            self.sample_end()?;
            // Song previews remain valid with no charts. A chart override is
            // only meaningful when a chart actually exists; invalid indices in
            // a nonempty document still go through the typed validation.
            let local = if session.document().charts().is_empty() {
                None
            } else {
                crate::chart_text::local_value(
                    session.document(),
                    chart,
                    crate::chart_text::Field::Music,
                )?
            };
            let name = local
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| self.text(crate::song_text::Field::Music).to_owned());
            (name, self.sample_start(), Some(self.sample_length()))
        };
        if name.is_empty() {
            return invalid("No preview music selected");
        }
        let path = directory.join(name.replace('\\', "/"));
        if !path.is_file() {
            return invalid("Preview music file not found");
        }
        Ok(SampleAudio {
            path,
            start,
            length,
        })
    }
    /// Fractional chart positions of the preview region; no cursor quantization.
    pub fn widgets(
        &self,
        session: &Session,
        chart: usize,
    ) -> Result<Vec<crate::event_clipboard::SongWidget>, Error> {
        crate::event_clipboard::song_widget::capture_draft(self, session, chart)
    }
    pub fn sample_positions(&self, session: &Session, chart: usize) -> Result<[f64; 2], Error> {
        let clock = self.clock(session, chart)?;
        Ok([
            clock.position(self.sample_start())?,
            clock.position(self.sample_end()?)?,
        ])
    }
    pub fn sample_end(&self) -> Result<f64, Error> {
        let end = self.sample_start() + self.sample_length();
        if end.is_finite() {
            Ok(end)
        } else {
            invalid("Preview end must be finite")
        }
    }
    pub fn sample_start(&self) -> f64 {
        self.sample[0]
    }
    pub fn sample_length(&self) -> f64 {
        self.sample[1]
    }
    pub fn set_sample_start(&mut self, value: f64) -> Result<(), Error> {
        self.set_sample(0, value)
    }
    pub fn set_sample_length(&mut self, value: f64) -> Result<(), Error> {
        self.set_sample(1, value)
    }
    fn set_sample(&mut self, index: usize, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return invalid("Preview sample time must be finite");
        }
        if !crate::number::reference_equal(self.sample[index], value) {
            self.sample[index] = value;
        }
        Ok(())
    }
    pub fn music_offset(&self) -> f64 {
        self.music_offset
    }
    pub fn set_music_offset(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return invalid("Music offset must be finite");
        }
        if !crate::number::reference_equal(self.music_offset, value) {
            self.music_offset = value;
        }
        Ok(())
    }
    /// The source inspector enables its migration button only at zero SyncOffset
    /// using Fumen DoubleEquals (one ULP, signed zero equal).
    pub fn can_apply_itg_offset(&self) -> bool {
        crate::number::reference_equal(self.sync_offset(), 0.0)
    }
    /// GrooveAuthor's explicit, non-idempotent ITG migration action.
    pub fn apply_itg_offset(&mut self) -> Result<(), Error> {
        self.set_music_offset(self.music_offset + DEFAULT_SYNC_OFFSET)?;
        self.set_sync_offset(DEFAULT_SYNC_OFFSET)
    }
    pub fn timing_chart(&self, session: &Session) -> Result<Option<usize>, Error> {
        self.timing_choice
            .identity()
            .map(|id| {
                session
                    .chart_ids()
                    .iter()
                    .position(|candidate| candidate == id)
                    .ok_or_else(|| Error::InvalidEdit("Song timing chart no longer exists".into()))
            })
            .transpose()
    }
    pub fn set_timing_chart(
        &mut self,
        session: &Session,
        chart: Option<usize>,
    ) -> Result<bool, Error> {
        if !session.accepts_content_token(&self.token) {
            return invalid("Song properties draft is stale");
        }
        self.timing_choice
            .set(&session.song_timing_snapshot()?, chart)
    }
    pub fn sync_offset(&self) -> f64 {
        self.offset
    }
    pub fn set_sync_offset(&mut self, offset: f64) -> Result<(), Error> {
        if !offset.is_finite() {
            return invalid("Editor sync offset must be finite");
        }
        self.offset = offset;
        Ok(())
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_content_token(&self.token)
    }
    /// Only a closed chart-event copy may carry an unchanged song draft across
    /// its chart-only content transition; this is not a clock receipt.
    pub fn rebind_chart_event_copy(
        &mut self,
        session: &Session,
        receipt: &crate::chart_event_copy::live::Receipt,
    ) -> Result<(), Error> {
        self.token = receipt.rebind_token(&self.token, session)?;
        Ok(())
    }
    pub fn requires_live_handoff(&self) -> bool {
        !self.preserves_presentation_clock() || !self.preserves_presentation_audio()
    }
    pub(crate) fn preserves_presentation_clock(&self) -> bool {
        crate::number::reference_equal(self.original, self.offset)
            && crate::number::reference_equal(self.original_music_offset, self.music_offset)
    }
    pub(crate) fn preserves_presentation_audio(&self) -> bool {
        use crate::song_text::Field;
        // Preview changes reload/stop the dedicated audition, not main music.
        self.text.original(Field::Music) == self.text.get(Field::Music)
    }
    /// Mint a presentation proof only for a real commit which leaves the clock intact.
    /// No public constructor or arbitrary-edit receipt exists.
    pub fn apply_with_presentation_receipt(
        &self,
        session: &mut Session,
    ) -> Result<Application, Error> {
        crate::history::song_presentation::apply(self, session)
    }
    pub fn apply(&self, session: &mut Session) -> Result<bool, Error> {
        if !session.accepts_content_token(&self.token) {
            return invalid("Song properties draft is stale or belongs to another document");
        }
        if !self.has_changes() {
            return Ok(false);
        }
        let text_changes = self.text.changes();
        let mut properties = text_changes
            .iter()
            .map(|(name, value)| -> Result<_, Error> {
                Ok((
                    None,
                    if *name == "PREVIEW"
                        && session.document().format == crate::document::Format::Sm
                    {
                        crate::song_text::resident::TAG.to_owned()
                    } else {
                        (*name).to_owned()
                    },
                    if *name == "PREVIEW"
                        && session.document().format == crate::document::Format::Sm
                    {
                        crate::song_text::resident::encode(value)?
                    } else {
                        std::sync::Arc::<[u8]>::from(crate::document::escape_text(value))
                    },
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.selectable_changed() {
            properties.push((
                None,
                "SELECTABLE".into(),
                std::sync::Arc::from(self.selectable.source_name().into_bytes()),
            ));
        }
        let hint_changed =
            !crate::number::reference_equal(self.last_second_hint[0], self.last_second_hint[1]);
        if hint_changed {
            properties.push((
                None,
                "LASTSECONDHINT".to_owned(),
                std::sync::Arc::from(self.last_second_hint().to_string().into_bytes()),
            ));
        }
        let mut sample_changed = false;
        for (index, name) in ["SAMPLESTART", "SAMPLELENGTH"].into_iter().enumerate() {
            if !crate::number::reference_equal(self.sample[index], self.original_sample[index]) {
                sample_changed = true;
                properties.push((
                    None,
                    name.to_owned(),
                    std::sync::Arc::from(self.sample[index].to_string().into_bytes()),
                ));
            }
        }
        let music_changed =
            !crate::number::reference_equal(self.original_music_offset, self.music_offset);
        if music_changed {
            properties.push((
                None,
                "OFFSET".to_owned(),
                std::sync::Arc::from(self.music_offset.to_string().into_bytes()),
            ));
        }
        if !crate::number::reference_equal(self.original, self.offset)
            || self.timing_choice != self.original_timing_choice
        {
            let mut fields = self.fields.clone();
            fields.insert("SyncOffset".into(), Value::from(self.offset));
            if self.timing_choice != self.original_timing_choice {
                let snapshot = session.song_timing_snapshot()?;
                fields.extend(
                    self.timing_choice
                        .output_fields(&snapshot, snapshot.source_indices())?,
                );
            }
            let json =
                serde_json::to_string(&fields).map_err(|e| Error::InvalidFormat(e.to_string()))?;
            properties.push((
                None,
                custom::key(session.document(), VERSION),
                std::sync::Arc::from(&b"1"[..]),
            ));
            properties.push((
                None,
                custom::key(session.document(), DATA),
                std::sync::Arc::from(crate::document::escape_text(&json)),
            ));
        }
        // EditorChart holds the reader's inherited author at load time; changing
        // the file credit does not reassign those existing chart authors.
        let old_credit = self.text.original(crate::song_text::Field::Credit);
        if session.document().format == crate::document::Format::Ssc
            && !old_credit.is_empty()
            && old_credit != self.text.get(crate::song_text::Field::Credit)
        {
            for (index, chart) in session.document().charts().iter().enumerate() {
                if crate::notes::layout_columns(&chart.steps_type).is_some()
                    && session
                        .document()
                        .text(Some(index), "CREDIT")
                        .is_none_or(|value| value.is_empty())
                {
                    properties.push((
                        Some(index),
                        "CREDIT".to_owned(),
                        std::sync::Arc::from(crate::document::escape_text(old_credit)),
                    ));
                }
            }
        }
        let changed = session.set_song_properties_and_timing(
            &properties,
            self.timing_choice.clone(),
            if text_changes.is_empty() && !music_changed && !sample_changed && !hint_changed {
                "Change editor sync offset"
            } else {
                "Change song properties"
            },
        )?;
        if changed && self.requires_live_handoff() {
            session.wrap_song_properties_live();
        }
        Ok(changed)
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}

pub(crate) fn custom_property_key(document: &Document, name: &str) -> String {
    custom::key(document, name)
}

#[derive(Clone, Copy)]
pub(crate) enum OutputContext {
    NativeLossless,
    SourceCompatible,
}
enum OutputMetadata {
    Absent,
    Known(Map<String, Value>, f64),
    Opaque(Error),
}
fn output_metadata(document: &Document) -> Result<OutputMetadata, Error> {
    match (
        custom::text(document, VERSION),
        custom::text(document, DATA),
    ) {
        (None, None) => Ok(OutputMetadata::Absent),
        (Some(version), Some(_)) if version.trim().parse::<i32>().ok() != Some(1) => {
            Ok(OutputMetadata::Opaque(Error::Unsupported(
                "Unknown GrooveAuthor song metadata version".into(),
            )))
        }
        (Some(_), Some(_)) => match read(document) {
            Ok((fields, offset)) => Ok(OutputMetadata::Known(fields, offset)),
            Err(error @ Error::InvalidFormat(_)) | Err(error @ Error::InvalidEdit(_)) => {
                Ok(OutputMetadata::Opaque(error))
            }
            Err(error) => Err(error),
        },
        _ => Ok(OutputMetadata::Opaque(Error::InvalidFormat(
            "Incomplete GrooveAuthor song metadata".into(),
        ))),
    }
}
pub(crate) fn materialize_output_context(
    document: &mut Document,
    timing: Map<String, Value>,
    mode: OutputContext,
) -> Result<(), Error> {
    let (mut metadata, offset) = match output_metadata(document)? {
        OutputMetadata::Absent => (Map::new(), document.load_context().open_sync_offset()),
        OutputMetadata::Known(fields, offset) => (fields, offset),
        OutputMetadata::Opaque(error) => {
            return match mode {
                OutputContext::NativeLossless => Ok(()),
                OutputContext::SourceCompatible => Err(error),
            };
        }
    };
    metadata
        .entry("SyncOffset")
        .or_insert_with(|| Value::from(offset));
    metadata.extend(timing);
    install_output_custom(document, VERSION, std::sync::Arc::from(b"1".as_slice()))?;
    install_output_custom(
        document,
        DATA,
        std::sync::Arc::from(crate::document::escape_text(
            &serde_json::to_string(&metadata).map_err(|e| Error::InvalidEdit(e.to_string()))?,
        )),
    )?;
    Ok(())
}

fn install_output_custom(
    document: &mut Document,
    name: &str,
    bytes: std::sync::Arc<[u8]>,
) -> Result<(), Error> {
    let key = custom::key(document, name);
    if document.added_property(None, &key).is_some() {
        document.set_added_property(None, &key, Some(bytes))
    } else if let Some(tag) = document.tag(None, &key) {
        document.replace(tag.value.clone(), Some(bytes))
    } else {
        document.set_added_property(None, &key, Some(bytes))
    }
}
