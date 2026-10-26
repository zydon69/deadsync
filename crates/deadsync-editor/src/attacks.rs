//! Attack events use absolute song-file seconds and have independent SSC inheritance.
//! Reference: Fumen AttackPropertyParser / GrooveAuthor UIAttackEvent.
pub mod placement;
pub mod inspector;
pub(crate) mod anchoring;
pub(crate) mod resident;
use crate::{
    Error,
    document::Format,
    history::{RevisionToken, Session},
    timing_edit::Scope,
};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modifier {
    pub name: String,
    pub level: f64,
    pub speed: f64,
    pub length: f64,
}
impl Default for Modifier {
    fn default() -> Self {
        Self {
            name: "Dizzy".into(),
            level: 100.0,
            speed: 1.0,
            length: 1.0,
        }
    }
}
impl Modifier {
    fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty() || self.name.chars().any(char::is_control) {
            return Err(invalid("An attack modifier needs a single-line name"));
        }
        // A leading numeric token, 'no', or '*' has special meaning in PlayerOptions.
        if self
            .name
            .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '*')
            || self.name == "no"
        {
            return Err(invalid(
                "Modifier name conflicts with a numeric or control token",
            ));
        }
        if !self.level.is_finite()
            || !self.speed.is_finite()
            || !self.length.is_finite()
            || self.length < 0.0
        {
            return Err(invalid(
                "Attack values must be finite and duration nonnegative",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub time: f64,
    pub modifiers: Vec<Modifier>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attacks {
    events: Vec<Event>,
}
impl Attacks {
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn parse(raw: &[u8]) -> Result<Self, Error> {
        Self::parse_order(raw, true)
    }
    pub(super) fn parse_source_order(raw: &[u8]) -> Result<Self, Error> {
        Self::parse_order(raw, false)
    }
    fn parse_order(raw: &[u8], sort: bool) -> Result<Self, Error> {
        let raw = without_comments(raw);
        let mut out = Self::default();
        let mut by_time = std::collections::BTreeMap::<u64, usize>::new();
        let (mut time, mut length, mut end) = (0.0, 0.0, None);
        let mut pending = false;
        for part in split(&raw, b':') {
            if part.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Some(index) = part.iter().position(|&b| b == b'=') else {
                return Err(invalid("Malformed attack property"));
            };
            let key = decode(&part[..index]).to_ascii_lowercase();
            let value = &part[index + 1..];
            pending = true;
            match key.as_str() {
                "time" => time = number(&decode(value))?,
                "len" => length = number(&decode(value))?,
                "end" => end = Some(number(&decode(value))?),
                "mods" => {
                    if let Some(stop) = end.take() {
                        length = stop - time;
                    }
                    let mut modifiers = Vec::new();
                    for raw_mod in split(value, b',') {
                        let words = words(raw_mod);
                        if words.is_empty() {
                            continue;
                        }
                        // Fumen defaults a missing level to 1, and uses percent units directly.
                        let mut modifier = Modifier {
                            name: decode(words[words.len() - 1]),
                            level: 1.0,
                            speed: 1.0,
                            length,
                        };
                        for word in words {
                            let word = decode(word);
                            if word == "no" {
                                modifier.level = 0.0;
                            } else if let Some(speed) = word.strip_prefix('*') {
                                modifier.speed = number(speed)?;
                            } else if word.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                                modifier.level = if let Some(ms) = word.strip_suffix("ms") {
                                    number(ms)? * 0.001
                                } else {
                                    number(word.strip_suffix('%').unwrap_or(&word))?
                                };
                            }
                        }
                        if !modifier.level.is_finite()
                            || !modifier.speed.is_finite()
                            || !modifier.length.is_finite()
                        {
                            return Err(invalid("Nonfinite attack value"));
                        }
                        modifiers.push(modifier);
                    }
                    if !time.is_finite() || !length.is_finite() {
                        return Err(invalid("Nonfinite attack time"));
                    }
                    let key = if time == 0.0 { 0 } else { time.to_bits() };
                    if let Some(&index) = by_time.get(&key) {
                        out.events[index].modifiers.extend(modifiers);
                    } else {
                        by_time.insert(key, out.events.len());
                        out.events.push(Event { time, modifiers });
                    }
                    time = 0.0;
                    length = 0.0;
                    pending = false;
                }
                _ => return Err(Error::Unsupported(format!("Unknown attack field: {key}"))),
            }
        }
        if pending {
            return Err(invalid("An attack entry must finish with MODS"));
        }
        if sort { out.events.sort_by(|a, b| a.time.total_cmp(&b.time)); }
        Ok(out)
    }
    pub fn add(&mut self, time: f64, modifier: Modifier) -> Result<usize, Error> {
        valid_time(time)?;
        modifier.validate()?;
        if let Some(index) = self.events.iter().position(|event| event.time == time) {
            self.events[index].modifiers.push(modifier);
            return Ok(index);
        }
        let index = self.events.partition_point(|event| event.time < time);
        self.events.insert(
            index,
            Event {
                time,
                modifiers: vec![modifier],
            },
        );
        Ok(index)
    }
    pub fn set(
        &mut self,
        event: usize,
        modifier: usize,
        time: f64,
        value: Modifier,
    ) -> Result<usize, Error> {
        valid_time(time)?;
        value.validate()?;
        let Some(original) = self.events.get(event) else {
            return Err(invalid("Attack event does not exist"));
        };
        if modifier >= original.modifiers.len() {
            return Err(invalid("Attack modifier does not exist"));
        }
        if self
            .events
            .iter()
            .enumerate()
            .any(|(i, e)| i != event && e.time == time)
        {
            return Err(invalid("Another attack already occupies that time"));
        }
        self.events[event].time = time;
        self.events[event].modifiers[modifier] = value;
        self.events.sort_by(|a, b| a.time.total_cmp(&b.time));
        Ok(self
            .events
            .iter()
            .position(|event| event.time == time)
            .expect("Updated attack remains present"))
    }
    pub fn remove_modifier(&mut self, event: usize, modifier: usize) -> Result<(), Error> {
        let Some(entry) = self.events.get_mut(event) else {
            return Err(invalid("Attack event does not exist"));
        };
        if modifier >= entry.modifiers.len() {
            return Err(invalid("Attack modifier does not exist"));
        }
        entry.modifiers.remove(modifier);
        if entry.modifiers.is_empty() {
            self.events.remove(event);
        }
        Ok(())
    }
    pub fn remove(&mut self, event: usize) -> Result<(), Error> {
        if event >= self.events.len() {
            return Err(invalid("Attack event does not exist"));
        }
        self.events.remove(event);
        Ok(())
    }
    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        self.serialize_options(false)
    }
    pub(super) fn serialize_preserving_empty(&self) -> Result<Vec<u8>, Error> {
        self.serialize_options(true)
    }
    fn serialize_options(&self, retain_empty: bool) -> Result<Vec<u8>, Error> {
        let mut entries = Vec::new();
        for event in &self.events {
            valid_time(event.time)?;
            if retain_empty && event.modifiers.is_empty() {
                entries.push(format!("TIME={}:LEN=0:MODS=", event.time));
            }
            for modifier in &event.modifiers {
                modifier.validate()?;
                // Always write explicit level and speed: Fumen's absent-level default is 1, not 100.
                let name = escape_name(&modifier.name);
                entries.push(format!(
                    "TIME={}:LEN={}:MODS=*{} {}% {}",
                    event.time, modifier.length, modifier.speed, modifier.level, name
                ));
            }
        }
        Ok(entries.join(":").into_bytes())
    }
}
pub struct Draft {
    token: RevisionToken,
    chart: usize,
    scope: Scope,
    inherited: bool,
    song_has_attacks: bool,
    original: Attacks,
    current: Attacks,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let doc = session.document();
        if chart >= doc.charts().len() {
            return Err(invalid("Attack chart does not exist"));
        }
        if scope == Scope::Chart
            && (doc.format != Format::Ssc || !crate::timing_edit::supports_chart_timing(doc))
        {
            return Err(invalid("Chart attacks require modern SSC"));
        }
        let song = Attacks::parse(doc.raw_property(None, "ATTACKS").unwrap_or_default())?;
        let local = if scope == Scope::Chart {
            Attacks::parse(doc.raw_property(Some(chart), "ATTACKS").unwrap_or_default())?
        } else {
            Attacks::default()
        };
        let inherited = scope == Scope::Chart && local.events.is_empty();
        let current = if scope == Scope::Song || inherited {
            song.clone()
        } else {
            local
        };
        Ok(Self {
            token: session.revision_token(),
            chart,
            scope,
            inherited,
            song_has_attacks: !song.events.is_empty(),
            original: current.clone(),
            current,
        })
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn inherited(&self) -> bool {
        self.inherited
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn attacks(&self) -> &Attacks {
        &self.current
    }
    pub fn attacks_mut(&mut self) -> &mut Attacks {
        &mut self.current
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid("Attack draft is stale"));
        }
        if !self.has_changes() {
            return Ok(false);
        }
        if self.scope == Scope::Chart && self.current.events.is_empty() && self.song_has_attacks {
            return Err(invalid(
                "An empty chart attack list inherits song attacks; remove them in song scope",
            ));
        }
        let bytes = self.current.serialize()?;
        // Colons are structure here; set_text would escape them and flatten the MSD parameters.
        session.set_property_bytes(
            if self.scope == Scope::Chart {
                Some(chart)
            } else {
                None
            },
            "ATTACKS",
            Arc::from(bytes),
        )
    }
}
/// Current default is one measure; the reference uses one beat (see docs/editor).
pub fn default_length(session: &Session, chart: usize, row: u64) -> Result<f64, Error> {
    let clock = crate::playback::Clock::capture(session, chart)?;
    let rate = clock
        .rate_mappings()
        .filter(|(_, rate)| rate.row() <= row as f64)
        .last()
        .map(|(_, rate)| rate)
        .ok_or_else(|| invalid("No tempo available for an attack duration"))?;
    let length = rate.seconds_per_row() * crate::notes::ROWS_PER_MEASURE as f64;
    valid_time(length)?;
    Ok(length)
}
/// File ATTACKS seconds omit the editor's synchronization offset.
pub fn cursor_time(session: &Session, chart: usize, row: u64) -> Result<f64, Error> {
    let clock = crate::playback::Clock::capture(session, chart)?;
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    let time = clock.chart_time(row)? - offset;
    valid_time(time)?;
    Ok(time)
}
/// Mirrors UIAttackEvent.SetModLengthFromCurrentTime, including pre-zero attack clamping.
pub fn length_to_cursor(
    session: &Session,
    chart: usize,
    row: u64,
    start: f64,
) -> Result<f64, Error> {
    valid_time(start)?;
    let clock = crate::playback::Clock::capture(session, chart)?;
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    let start = start + offset;
    valid_time(start)?;
    Ok((clock.chart_time(row)?.max(0.0) - start.max(0.0)).max(0.0))
}
/// Map a modifier endpoint back to the integral editor cursor, without opening audio.
pub fn endpoint_row(
    session: &Session,
    chart: usize,
    start: f64,
    length: f64,
) -> Result<u64, Error> {
    valid_time(start)?;
    if !length.is_finite() || length < 0.0 {
        return Err(invalid("Invalid attack duration"));
    }
    let clock = crate::playback::Clock::capture(session, chart)?;
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    let start = start + offset;
    valid_time(start)?;
    let end = start.max(0.0) + length;
    valid_time(end)?;
    let row = clock.position(clock.chart_time_to_music(end)?)?;
    if !row.is_finite() || row > i32::MAX as f64 {
        return Err(invalid("Attack endpoint lies outside editor rows"));
    }
    Ok(row.max(0.0).round() as u64)
}
fn number(value: &str) -> Result<f64, Error> {
    let n = value
        .parse::<f64>()
        .map_err(|_| invalid("Invalid attack number"))?;
    if n.is_finite() {
        Ok(n)
    } else {
        Err(invalid("Nonfinite attack number"))
    }
}
fn valid_time(time: f64) -> Result<(), Error> {
    if time.is_finite() {
        Ok(())
    } else {
        Err(invalid("Attack time must be finite"))
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn decode(raw: &[u8]) -> String {
    rssp::parse::decode_unescape_trim(raw).into_owned()
}
fn split(raw: &[u8], separator: u8) -> Vec<&[u8]> {
    let (mut start, mut pos) = (0, 0);
    let mut out = Vec::new();
    while pos < raw.len() {
        if raw[pos] == b'\\' {
            pos += 2;
            continue;
        }
        if raw[pos] == separator {
            out.push(&raw[start..pos]);
            start = pos + 1;
        }
        pos += 1;
    }
    out.push(&raw[start..]);
    out
}
fn words(raw: &[u8]) -> Vec<&[u8]> {
    let (mut start, mut pos) = (0, 0);
    let mut out = Vec::new();
    while pos < raw.len() {
        if raw[pos] == b'\\' {
            pos += 2;
            continue;
        }
        if raw[pos].is_ascii_whitespace() {
            if start < pos {
                out.push(&raw[start..pos]);
            }
            start = pos + 1;
        }
        pos += 1;
    }
    if start < raw.len() {
        out.push(&raw[start..]);
    }
    out
}
fn without_comments(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < raw.len() {
        if raw[pos] == b'\\' && pos + 1 < raw.len() {
            out.extend_from_slice(&raw[pos..pos + 2]);
            pos += 2;
        } else if raw[pos..].starts_with(b"//") {
            while pos < raw.len() && raw[pos] != b'\n' {
                pos += 1;
            }
        } else {
            out.push(raw[pos]);
            pos += 1;
        }
    }
    out
}
fn escape_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if matches!(c, '\\' | ':' | ';' | '#' | '/' | ',' | ' ' | '=') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
