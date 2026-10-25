//! GrooveAuthor song-level editor metadata. SyncOffset affects preview only.
use crate::{
    Error,
    document::Document,
    history::{RevisionToken, Session},
};
use serde_json::{Map, Value};
pub const DEFAULT_SYNC_OFFSET: f64 = 0.009;
const VERSION: &str = "GASongDataVersion";
const DATA: &str = "GASongData";

fn read(document: &Document) -> Result<(Map<String, Value>, f64), Error> {
    let fields = match (document.text(None, VERSION), document.text(None, DATA)) {
        (None, None) => return Ok((Map::new(), DEFAULT_SYNC_OFFSET)),
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
pub fn sync_offset(document: &Document) -> Result<f64, Error> {
    read(document).map(|(_, offset)| offset)
}

/// V1 restores timing-chart references from type-group file-load order before
/// EditorSong.UpdateChartSortInternal. Keep that load-time reference attached.
pub(crate) fn after_chart_deletion(
    document: &Document,
    deleted: usize,
) -> Result<Option<String>, Error> {
    if document.text(None, DATA).is_none() && document.text(None, VERSION).is_none() {
        return Ok(None);
    }
    let (mut fields, _) = read(document)?;
    let kind = match fields.get("DefaultChartType") {
        Some(Value::String(kind)) => kind.to_ascii_lowercase().replace('_', "-"),
        Some(Value::Number(kind)) => {
            // Numeric enum values in pinned Fumen ChartType; JsonStringEnumConverter
            // also accepts these values in existing GrooveAuthor song metadata.
            let kind = match kind.as_u64() {
                Some(0) => "dance-single",
                Some(1) => "dance-double",
                Some(2) => "dance-couple",
                Some(3) => "dance-solo",
                Some(4) => "dance-threepanel",
                Some(5) => "dance-routine",
                Some(6) => "pump-single",
                Some(7) => "pump-halfdouble",
                Some(8) => "pump-double",
                Some(9) => "pump-couple",
                Some(10) => "pump-routine",
                Some(38) => "smx-beginner",
                Some(39) => "smx-single",
                Some(40) => "smx-dual",
                Some(41) => "smx-full",
                Some(42) => "smx-team",
                _ => return Ok(None),
            };
            kind.to_owned()
        }
        _ => return Ok(None),
    };
    let selected = match fields.get("DefaultChartIndex") {
        None => 0,
        Some(value) => match value.as_u64() {
            Some(value) => value,
            None => return Ok(None),
        },
    };
    let chart = document
        .charts()
        .get(deleted)
        .ok_or_else(|| Error::InvalidEdit("Invalid deleted chart".into()))?;
    if chart.steps_type != kind {
        return Ok(None);
    }
    let typed = document
        .charts()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.steps_type == kind)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if usize::try_from(selected).is_err() || selected as usize >= typed.len() {
        return Ok(None);
    }
    let position = typed.iter().position(|i| *i == deleted).unwrap();
    if position as u64 > selected {
        return Ok(None);
    }
    if position as u64 == selected {
        fields.insert("DefaultChartType".into(), Value::Null);
        fields.insert("DefaultChartIndex".into(), Value::from(0));
    } else {
        fields.insert("DefaultChartIndex".into(), Value::from(selected - 1));
    }
    Ok(Some(
        serde_json::to_string(&fields).map_err(|e| Error::InvalidFormat(e.to_string()))?,
    ))
}

pub struct Draft {
    token: RevisionToken,
    original: f64,
    offset: f64,
    original_music_offset: f64,
    music_offset: f64,
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
        Ok(Self {
            token: session.revision_token(),
            original: offset,
            offset,
            original_music_offset: music_offset,
            music_offset,
            original_sample: sample,
            sample,
            fields,
            text: crate::song_text::Text::read(session.document()),
        })
    }
    pub fn text(&self, field: crate::song_text::Field) -> &str {
        self.text.get(field)
    }
    pub fn set_text(&mut self, field: crate::song_text::Field, value: &str) -> Result<(), Error> {
        self.text.set(field, value)
    }
    pub fn has_changes(&self) -> bool {
        self.text.has_changes()
            || !crate::number::reference_equal(self.original, self.offset)
            || !crate::number::reference_equal(self.original_music_offset, self.music_offset)
            || self
                .sample
                .iter()
                .zip(self.original_sample)
                .any(|(value, original)| !crate::number::reference_equal(*value, original))
    }
    pub fn clock(&self, session: &Session, chart: usize) -> Result<crate::playback::Clock, Error> {
        if !session.accepts_token(&self.token) {
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
    /// Fractional chart positions of the preview region; no cursor quantization.
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
    /// GrooveAuthor's explicit, non-idempotent ITG migration action.
    pub fn apply_itg_offset(&mut self) -> Result<(), Error> {
        self.set_music_offset(self.music_offset + DEFAULT_SYNC_OFFSET)?;
        self.set_sync_offset(DEFAULT_SYNC_OFFSET)
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
    pub fn apply(&self, session: &mut Session) -> Result<bool, Error> {
        if !session.accepts_token(&self.token) {
            return invalid("Song properties draft is stale or belongs to another document");
        }
        if !self.has_changes() {
            return Ok(false);
        }
        let text_changes = self.text.changes();
        let mut properties = text_changes
            .iter()
            .map(|(name, value)| {
                (
                    None,
                    (*name).to_owned(),
                    std::sync::Arc::<[u8]>::from(crate::document::escape_text(value)),
                )
            })
            .collect::<Vec<_>>();
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
        if !crate::number::reference_equal(self.original, self.offset) {
            let mut fields = self.fields.clone();
            fields.insert("SyncOffset".into(), Value::from(self.offset));
            let json =
                serde_json::to_string(&fields).map_err(|e| Error::InvalidFormat(e.to_string()))?;
            properties.push((None, VERSION.to_owned(), std::sync::Arc::from(&b"1"[..])));
            properties.push((
                None,
                DATA.to_owned(),
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
        session.set_property_bytes_atomic(
            &properties,
            if text_changes.is_empty() && !music_changed && !sample_changed {
                "Change editor sync offset"
            } else {
                "Change song properties"
            },
        )
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
