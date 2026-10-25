//! Sparse, participant-aware note data. GrooveAuthor's 48 rows per beat is
//! retained; unsupported subdivisions fail explicitly instead of being rounded.
use crate::Error;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const ROWS_PER_BEAT: u64 = 48;
pub const ROWS_PER_MEASURE: u64 = 192;
const MAX_SERIALIZED_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tap,
    Hold,
    Roll,
    Mine,
    Lift,
    Fake,
    Keysound,
    Attack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub kind: Kind,
    pub end: Option<u64>,
    /// Keysound, attack, and extension attributes remain attached to the note.
    pub attributes: Arc<str>,
    pub tail_attributes: Arc<str>,
}

impl Note {
    pub fn tap() -> Self {
        Self {
            kind: Kind::Tap,
            end: None,
            attributes: Arc::from(""),
            tail_attributes: Arc::from(""),
        }
    }

    fn symbol(&self) -> char {
        match self.kind {
            Kind::Tap => '1',
            Kind::Hold => '2',
            Kind::Roll => '4',
            Kind::Mine => 'M',
            Kind::Lift => 'L',
            Kind::Fake => 'F',
            Kind::Keysound => 'K',
            Kind::Attack => 'A',
        }
    }

    fn validate(&self, row: u64) -> Result<(), Error> {
        if matches!(self.kind, Kind::Hold | Kind::Roll) != self.end.is_some()
            || self.end.is_some_and(|end| end <= row)
        {
            return Err(Error::InvalidEdit(
                "A sustain must end after its head".into(),
            ));
        }
        for attributes in [&self.attributes, &self.tail_attributes] {
            let mut escaped = false;
            let mut slash = false;
            for byte in attributes.bytes() {
                if escaped {
                    escaped = false;
                    slash = false;
                } else if byte == b'\\' {
                    escaped = true;
                    slash = false;
                } else if matches!(byte, b';' | b'#') || (slash && byte == b'/') {
                    return Err(Error::InvalidEdit(
                        "Unescaped simfile delimiter in note attributes".into(),
                    ));
                } else {
                    slash = byte == b'/';
                }
            }
            let token = format!("1{attributes}");
            let parsed = tokens(&token)?;
            if parsed.len() != 1 || parsed[0].1 != attributes.as_ref() {
                return Err(Error::InvalidEdit("Invalid note attributes".into()));
            }
        }
        if self.end.is_none() && !self.tail_attributes.is_empty() {
            return Err(Error::InvalidEdit(
                "Only sustains can have tail attributes".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub player: u32,
    pub lane: u16,
    pub row: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    columns: u16,
    players: u32,
    tracks: BTreeMap<(u32, u16), BTreeMap<u64, Note>>,
}

impl Grid {
    pub fn new(columns: u16, players: u32) -> Result<Self, Error> {
        if columns == 0 || players == 0 {
            return Err(Error::InvalidEdit(
                "A chart needs lanes and participants".into(),
            ));
        }
        Ok(Self {
            columns,
            players,
            tracks: BTreeMap::new(),
        })
    }

    pub fn columns(&self) -> u16 {
        self.columns
    }
    pub fn players(&self) -> u32 {
        self.players
    }
    pub fn get(&self, pos: Position) -> Option<&Note> {
        self.tracks.get(&(pos.player, pos.lane))?.get(&pos.row)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Position, &Note)> {
        self.tracks.iter().flat_map(|(&(player, lane), track)| {
            track
                .iter()
                .map(move |(&row, note)| (Position { player, lane, row }, note))
        })
    }

    /// Only query the visible interval of each lane. Include a sustain whose
    /// head precedes the viewport and whose tail is still visible.
    pub fn visible(&self, start: u64, end: u64) -> impl Iterator<Item = (Position, &Note)> {
        self.tracks
            .iter()
            .flat_map(move |(&(player, lane), track)| {
                let crossing = track
                    .range(..start)
                    .next_back()
                    .filter(|(_, note)| note.end.is_some_and(|tail| tail >= start));
                crossing
                    .into_iter()
                    .chain(track.range(start..=end))
                    .map(move |(&row, note)| (Position { player, lane, row }, note))
            })
    }

    /// Bounded overview query for far-zoomed-out rendering. For each occupied
    /// track/bin, retain the first and last head. The last head also preserves
    /// the sustain crossing the next bin (same-track sustains cannot overlap).
    /// An initial sustain crossing the viewport is included separately.
    /// This is presentation sampling only: the document is never modified.
    /// At one bin per row the result is identical to `visible`.
    pub fn visible_overview(
        &self,
        start: u64,
        end: u64,
        bins: usize,
    ) -> Result<Vec<(Position, &Note)>, Error> {
        if start > end || !(1..=4096).contains(&bins) {
            return Err(Error::InvalidEdit("Invalid chart overview bounds or bin count".into()));
        }
        // u128 includes the complete inclusive u64 domain without overflow.
        let span = u128::from(end) - u128::from(start) + 1;
        let count = (bins as u128).min(span);
        let mut result = Vec::new();
        for (&(player, lane), track) in &self.tracks {
            if let Some((&row, note)) = track.range(..start).next_back()
                .filter(|(_, note)| note.end.is_some_and(|tail| tail >= start))
            {
                result.push((Position { player, lane, row }, note));
            }
            for index in 0..count {
                let a = (u128::from(start) + span * index / count) as u64;
                let b = (u128::from(start) + span * (index + 1) / count - 1) as u64;
                let mut range = track.range(a..=b);
                if let Some((&row, note)) = range.next() {
                    result.push((Position { player, lane, row }, note));
                    if let Some((&row, note)) = range.next_back() {
                        result.push((Position { player, lane, row }, note));
                    }
                }
            }
        }
        Ok(result)
    }

    pub fn set(&mut self, pos: Position, note: Option<Note>) -> Result<Option<Note>, Error> {
        if pos.lane >= self.columns || pos.player >= self.players {
            return Err(Error::InvalidEdit(
                "Lane or participant lies outside this chart".into(),
            ));
        }
        if let Some(note) = &note {
            note.validate(pos.row)?;
            if let Some(track) = self.tracks.get(&(pos.player, pos.lane)) {
                if track
                    .range(..pos.row)
                    .next_back()
                    .is_some_and(|(_, previous)| previous.end.is_some_and(|end| end >= pos.row))
                {
                    return Err(Error::InvalidEdit(
                        "A note cannot overlap a sustain in the same lane".into(),
                    ));
                }
                if let Some(end) = note.end {
                    if track
                        .range((
                            std::ops::Bound::Excluded(pos.row),
                            std::ops::Bound::Included(end),
                        ))
                        .next()
                        .is_some()
                    {
                        return Err(Error::InvalidEdit(
                            "A sustain cannot overlap another note".into(),
                        ));
                    }
                }
            }
        }
        let key = (pos.player, pos.lane);
        if let Some(note) = note {
            Ok(self.tracks.entry(key).or_default().insert(pos.row, note))
        } else {
            let previous = self
                .tracks
                .get_mut(&key)
                .and_then(|track| track.remove(&pos.row));
            if self.tracks.get(&key).is_some_and(BTreeMap::is_empty) {
                self.tracks.remove(&key);
            }
            Ok(previous)
        }
    }

    pub fn parse(bytes: &[u8], columns: u16) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            Error::Unsupported(
                "Non-UTF8 note data remains preserved but cannot yet be edited".into(),
            )
        })?;
        let blocks = split_outer(text, '&')?;
        let players = u32::try_from(blocks.len())
            .map_err(|_| Error::InvalidFormat("Too many participants".into()))?;
        let mut grid = Self::new(columns, players)?;
        for (player, block) in blocks.iter().enumerate() {
            let mut heads: BTreeMap<u16, (u64, Note)> = BTreeMap::new();
            for (measure, data) in split_outer(block, ',')?.iter().enumerate() {
                let lines: Vec<_> = data
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .collect();
                if lines.is_empty() {
                    continue;
                }
                for (index, line) in lines.iter().enumerate() {
                    let cells = tokens(line)?;
                    if cells.len() != usize::from(columns) {
                        return Err(Error::InvalidFormat(format!(
                            "Expected {columns} lanes, found {}",
                            cells.len()
                        )));
                    }
                    let numerator = (index as u64)
                        .checked_mul(ROWS_PER_MEASURE)
                        .ok_or_else(|| Error::InvalidFormat("Note position overflow".into()))?;
                    if numerator % lines.len() as u64 != 0 && cells.iter().any(|cell| cell.0 != '0')
                    {
                        return Err(Error::Unsupported(
                            "Subdivision cannot be represented at 48 rows per beat".into(),
                        ));
                    }
                    let row = (measure as u64)
                        .checked_mul(ROWS_PER_MEASURE)
                        .and_then(|base| base.checked_add(numerator / lines.len() as u64))
                        .ok_or_else(|| Error::InvalidFormat("Note position overflow".into()))?;
                    for (lane, (symbol, attributes)) in cells.into_iter().enumerate() {
                        let lane = lane as u16;
                        let pos = Position {
                            player: player as u32,
                            lane,
                            row,
                        };
                        if symbol == '0' {
                            if !attributes.is_empty() {
                                return Err(Error::Unsupported(
                                    "Attributes on empty cells cannot yet be edited".into(),
                                ));
                            }
                            continue;
                        }
                        if symbol == '3' {
                            let (head, mut note) = heads.remove(&lane).ok_or_else(|| {
                                Error::InvalidFormat("Sustain tail has no head".into())
                            })?;
                            note.end = Some(row);
                            note.tail_attributes = Arc::from(attributes);
                            grid.set(Position { row: head, ..pos }, Some(note))?;
                            continue;
                        }
                        if heads.contains_key(&lane) {
                            return Err(Error::InvalidFormat("Note inside an open sustain".into()));
                        }
                        let kind = match symbol {
                            '1' => Kind::Tap,
                            '2' => Kind::Hold,
                            '4' => Kind::Roll,
                            'M' => Kind::Mine,
                            'L' => Kind::Lift,
                            'F' => Kind::Fake,
                            'K' => Kind::Keysound,
                            'A' => Kind::Attack,
                            _ => {
                                return Err(Error::Unsupported(format!(
                                    "Note symbol {symbol:?} is preserved but cannot yet be edited"
                                )));
                            }
                        };
                        let note = Note {
                            kind,
                            end: None,
                            attributes: Arc::from(attributes),
                            tail_attributes: Arc::from(""),
                        };
                        if matches!(kind, Kind::Hold | Kind::Roll) {
                            heads.insert(lane, (row, note));
                        } else {
                            grid.set(pos, Some(note))?;
                        }
                    }
                }
            }
            if !heads.is_empty() {
                return Err(Error::InvalidFormat("Sustain head has no tail".into()));
            }
        }
        Ok(grid)
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        let mut output = String::from("\n");
        for player in 0..self.players {
            if player > 0 {
                output.push_str("\n&\n");
            }
            let mut rows: BTreeMap<u64, BTreeMap<u16, String>> = BTreeMap::new();
            for (pos, note) in self.iter().filter(|(pos, _)| pos.player == player) {
                rows.entry(pos.row)
                    .or_default()
                    .insert(pos.lane, format!("{}{}", note.symbol(), note.attributes));
                if let Some(end) = note.end {
                    rows.entry(end)
                        .or_default()
                        .insert(pos.lane, format!("3{}", note.tail_attributes));
                }
            }
            let last_measure = rows
                .last_key_value()
                .map_or(0, |(&row, _)| row / ROWS_PER_MEASURE);
            let minimum = (last_measure + 1)
                .checked_mul(u64::from(self.columns) + 2)
                .ok_or_else(|| Error::InvalidEdit("Chart is too large to serialize".into()))?;
            if minimum > MAX_SERIALIZED_BYTES as u64 {
                return Err(Error::InvalidEdit(
                    "Serialized note data exceeds 128 MiB".into(),
                ));
            }
            for measure in 0..=last_measure {
                if measure > 0 {
                    output.push_str(",\n");
                }
                let start = measure * ROWS_PER_MEASURE;
                let end = start + ROWS_PER_MEASURE - 1;
                let step = rows
                    .range(start..=end)
                    .fold(48, |step, (&row, _)| gcd(step, row - start));
                for offset in (0..ROWS_PER_MEASURE).step_by(step as usize) {
                    let cells = rows.get(&(start + offset));
                    for lane in 0..self.columns {
                        if let Some(token) = cells.and_then(|cells| cells.get(&lane)) {
                            output.push_str(token);
                        } else {
                            output.push('0');
                        }
                    }
                    output.push('\n');
                    if output.len() > MAX_SERIALIZED_BYTES {
                        return Err(Error::InvalidEdit(
                            "Serialized note data exceeds 128 MiB".into(),
                        ));
                    }
                }
            }
        }
        Ok(output.into_bytes())
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Split only outside note attributes; commas in attacks and comments are not
/// measure separators. Removing comments here leaves every note token intact.
pub(crate) fn split_outer(text: &str, separator: char) -> Result<Vec<String>, Error> {
    let mut blocks = vec![String::new()];
    let mut brackets = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let block = blocks.last_mut().unwrap();
            block.push(c);
            block.push(
                chars
                    .next()
                    .ok_or_else(|| Error::InvalidFormat("Incomplete escape in note data".into()))?,
            );
            continue;
        }
        if brackets.is_empty() && c == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            blocks.last_mut().unwrap().push('\n');
            continue;
        }
        match c {
            '[' => brackets.push(']'),
            '{' => brackets.push('}'),
            '<' => brackets.push('>'),
            ']' | '}' | '>' => {
                if brackets.pop() != Some(c) {
                    return Err(Error::InvalidFormat("Unbalanced note attributes".into()));
                }
            }
            _ => {}
        }
        if brackets.is_empty() && c == separator {
            blocks.push(String::new());
        } else {
            blocks.last_mut().unwrap().push(c);
        }
    }
    if !brackets.is_empty() {
        return Err(Error::InvalidFormat("Unterminated note attributes".into()));
    }
    Ok(blocks)
}

fn tokens(line: &str) -> Result<Vec<(char, &str)>, Error> {
    let bytes = line.as_bytes();
    let mut result = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }
        if !bytes[pos].is_ascii() {
            return Err(Error::Unsupported("Non-ASCII note symbol".into()));
        }
        let symbol = bytes[pos] as char;
        pos += 1;
        let start = pos;
        while pos < bytes.len() && matches!(bytes[pos], b'[' | b'{' | b'<') {
            let close = match bytes[pos] {
                b'[' => b']',
                b'{' => b'}',
                _ => b'>',
            };
            pos += 1;
            while pos < bytes.len() && bytes[pos] != close {
                if bytes[pos] == b'\\' {
                    pos += 1;
                }
                pos += 1;
            }
            if pos >= bytes.len() {
                return Err(Error::InvalidFormat("Unterminated note attributes".into()));
            }
            pos += 1;
        }
        result.push((symbol, &line[start..pos]));
    }
    Ok(result)
}

/// Layout inventory follows GrooveAuthor/Fumen chart identifiers. Editing a
/// layout does not imply that DeadSync can play that layout.
pub fn layout_columns(steps_type: &str) -> Option<u16> {
    Some(match steps_type {
        "dance-single" | "pump-halfdouble" => {
            if steps_type == "dance-single" {
                4
            } else {
                6
            }
        }
        "dance-double" | "dance-couple" | "dance-routine" => 8,
        "dance-solo" => 6,
        "dance-threepanel" => 3,
        "pump-single" | "pump-couple" => {
            if steps_type == "pump-single" {
                5
            } else {
                10
            }
        }
        "pump-double" | "pump-routine" => 10,
        "smx-single" | "smx-beginner" => {
            if steps_type == "smx-single" {
                5
            } else {
                3
            }
        }
        "smx-dual" => 6,
        "smx-full" | "smx-team" => 10,
        _ => return None,
    })
}
