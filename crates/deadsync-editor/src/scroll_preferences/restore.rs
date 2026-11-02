//! One original PreferencesScroll Restore action over existing typed owners.
//! The source group has14 business fields plus a retained window visibility bit.
use crate::{
    Error, audio_preferences::File, scroll_preferences::Preferences as Scroll,
    scroll_preferences::spacing::Preference as Spacing, waveform_spacing::ScrollMode,
    zoom_preferences::Preferences as Zoom,
};
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub scroll: Scroll,
    pub zoom: Zoom,
    pub spacing: Spacing,
    pub waveform_mode: ScrollMode,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            scroll: Scroll::default(),
            zoom: Zoom::default(),
            spacing: Spacing::ConstantTime,
            waveform_mode: ScrollMode::MostCommonTempo,
        }
    }
}
impl Snapshot {
    pub fn capture(file: &File) -> Self {
        Self {
            scroll: file.scroll(),
            zoom: file.zoom(),
            spacing: file.spacing_preference(),
            waveform_mode: file.waveform().scroll_mode(),
        }
    }
    pub fn restored(&self) -> Self {
        Self::default()
    }
    /// Source IsUsingDefaults compares SizeCap to itself. Preserve its exact
    /// cap-only no-op behavior; a real restore still resets the cap with all fields.
    pub fn restoration_needed(&self) -> bool {
        let mut fields = Map::new();
        self.write(&mut fields);
        let mut defaults = Map::new();
        Self::default().write(&mut defaults);
        fields.into_iter().any(|(key, value)| {
            key != "SizeCap"
                && match (value, defaults.get(&key)) {
                    (Value::Number(a), Some(Value::Number(b))) => {
                        !crate::number::reference_equal(a.as_f64().unwrap(), b.as_f64().unwrap())
                    }
                    (value, Some(default)) => &value != default,
                    _ => true,
                }
        })
    }
    pub fn write(&self, fields: &mut Map<String, Value>) {
        self.scroll.write(fields);
        self.zoom.write(fields);
        fields.insert("SpacingMode".into(), self.spacing.source_name().into());
        fields.insert(
            "RowBasedWaveFormScrollMode".into(),
            self.waveform_mode.name().into(),
        );
    }
    fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        Ok(Self {
            scroll: Scroll::read(fields)?,
            zoom: Zoom::read(fields)?,
            spacing: Spacing::read_native(fields.get("SpacingMode"))?,
            waveform_mode: crate::waveform_preferences::Preferences::read(fields)?.scroll_mode(),
        })
    }
    pub fn install(&self, file: &mut File) {
        file.set_scroll(self.scroll);
        file.set_zoom(self.zoom);
        file.set_spacing_preference(self.spacing);
        let mut waveform = file.waveform();
        waveform.set_scroll_mode(self.waveform_mode);
        file.set_waveform(waveform);
    }
    fn merged(current: &File, expected: &Self, target: &Self) -> Result<(Self, File), Error> {
        let (mut fields, mut before, mut after) = (Map::new(), Map::new(), Map::new());
        Self::capture(current).write(&mut fields);
        expected.write(&mut before);
        target.write(&mut after);
        crate::audio_preferences::publication::merge_fields(&mut fields, &before, &after)?;
        let merged = Self::read(&fields)?;
        let mut file = current.clone();
        merged.install(&mut file);
        Ok((merged, file))
    }
}
pub struct Update {
    pub preferences: Snapshot,
    pub file: Option<File>,
}
pub(crate) struct Change {
    pub before: Snapshot,
    pub after: Snapshot,
    pub file: Option<File>,
    pub initial: Option<Snapshot>,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestoreScroll").finish_non_exhaustive()
    }
}
impl Change {
    pub fn apply(&mut self, forward: bool, live: Option<&File>) -> Result<Update, Error> {
        let (expected, target) = if forward {
            (&self.before, &self.after)
        } else {
            (&self.after, &self.before)
        };
        // Initial mixed publication includes direct SizeCap once; history
        // thereafter merges only the actual source Undo-owned leaves.
        let target=if forward{self.initial.as_ref().unwrap_or(target)}else{target};
        let mut preferences = *target;
        preferences = crate::audio_preferences::publication::publish(
            &mut self.file, live, preferences,
            |current| Snapshot::merged(current, expected, target),
        )?;
        self.initial=None;
        Ok(Update {
            preferences,
            file: self.file.clone(),
        })
    }
}
