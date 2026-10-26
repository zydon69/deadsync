//! StepF2 co-op notation from Fumen SMCommon / SMPropertyParsers / SMWriterBase.
//!
//! The compound decoration is kept in the note's opaque suffix, so ordinary
//! note moves, clipboard operations and history retain it. Unlike GrooveAuthor,
//! loading an editable chart does not discard visibility or reserved flags.
use crate::{
    Error,
    notes::{Grid, Kind, Note, Position, ROWS_PER_MEASURE, split_outer},
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Normal,
    Sudden,
    Vanish,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attributes {
    pub visibility: Visibility,
    pub fake: bool,
    /// The fourth StepF2 compound character is reserved; preserve it verbatim.
    pub reserved: char,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            visibility: Visibility::Normal,
            fake: false,
            reserved: '0',
        }
    }
}

impl Attributes {
    /// Read a compound decoration attached to a canonical note, leaving any
    /// keysound/extension suffix untouched. A normal non-StepF2 suffix is None.
    pub fn read(suffix: &str) -> Result<Option<(Self, &str)>, Error> {
        if !suffix.starts_with('{') || !suffix.as_bytes().get(2).is_some_and(|c| *c == b'|') {
            return Ok(None);
        }
        let bytes = suffix.as_bytes();
        if bytes.len() < 9
            || bytes[4] != b'|'
            || bytes[6] != b'|'
            || bytes[8] != b'}'
            || !bytes[..9].is_ascii()
            || !matches!(bytes[5], b'0' | b'1')
        {
            return Err(invalid("Malformed StepF2 compound note"));
        }
        let visibility = match bytes[3] {
            b'n' => Visibility::Normal,
            b's' => Visibility::Sudden,
            b'v' => Visibility::Vanish,
            b'h' => Visibility::Hidden,
            _ => return Err(invalid("Unknown StepF2 visibility")),
        };
        if !reserved_valid(bytes[7] as char) {
            return Err(invalid("Invalid StepF2 reserved character"));
        }
        Ok(Some((
            Self {
                visibility,
                fake: bytes[5] == b'1',
                reserved: bytes[7] as char,
            },
            &suffix[9..],
        )))
    }

    pub fn suffix(self, symbol: char, trailing: &str) -> Result<String, Error> {
        if !reserved_valid(self.reserved) {
            return Err(invalid("Invalid StepF2 reserved character"));
        }
        let visibility = match self.visibility {
            Visibility::Normal => 'n',
            Visibility::Sudden => 's',
            Visibility::Vanish => 'v',
            Visibility::Hidden => 'h',
        };
        Ok(format!(
            "{{{symbol}|{visibility}|{}|{}}}{trailing}",
            u8::from(self.fake),
            self.reserved
        ))
    }
}

fn invalid(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
fn reserved_valid(value: char) -> bool {
    value.is_ascii_graphic() && !matches!(value, '{' | '}' | '|' | ';' | '#' | '\\')
}

/// The neutral marker belongs to the decoded StepF2 model, not standard
/// StepMania attack syntax. Refuse cross-notation export until an explicit
/// conversion policy can preserve or deliberately remove its decorations.
pub(crate) fn require_standard(note: &Note) -> Result<(), Error> {
    if [&note.attributes, &note.tail_attributes]
        .iter()
        .any(|suffix| suffix.starts_with("{N|"))
    {
        return Err(Error::Unsupported(
            "StepF2 decorations require explicit conversion before using standard note notation"
                .into(),
        ));
    }
    Ok(())
}

/// Detection follows Fumen: X/x/Y/y/Z/z distinguish co-op from normal Pump.
/// A plain 1 or 2 alone is ambiguous and must keep its standard P1 meaning.
pub fn is_coop(bytes: &[u8], layout: &str) -> bool {
    if !matches!(layout, "pump-double" | "pump-routine") {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let Ok(blocks) = split_outer(text, ',') else {
        return false;
    };
    blocks
        .iter()
        .flat_map(|block| block.lines())
        .filter_map(|line| cells(line).ok())
        .flatten()
        .any(|cell| matches!(cell.symbol, 'X' | 'x' | 'Y' | 'y' | 'Z' | 'z'))
}

/// Explicit co-op decoder. All four owners share one physical lane stream.
pub fn parse(bytes: &[u8]) -> Result<Grid, Error> {
    parse_for_layout(bytes, "pump-routine")
}

/// Fumen initializes the participant count from the *source* layout before
/// discovering co-op symbols and promoting Pump Double to Routine. Thus a
/// Double chart with only P1 co-op symbols has one participant, while the same
/// note stream in a Routine chart starts with two. Do not infer this minimum
/// from the effective (promoted) layout.
pub fn parse_for_layout(bytes: &[u8], layout: &str) -> Result<Grid, Error> {
    let mut minimum_players = match layout {
        "pump-double" => 1,
        "pump-routine" => 2,
        _ => {
            return Err(Error::Unsupported(
                "StepF2 co-op decoding requires a Pump Double or Routine source layout".into(),
            ));
        }
    };
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("Non-UTF8 StepF2 note data"))?;
    if split_outer(text, '&')?.len() != 1 {
        return Err(invalid("StepF2 co-op uses one shared note stream"));
    }
    let mut grid = Grid::new(10, 4)?;
    let mut heads: BTreeMap<u16, (Position, Note)> = BTreeMap::new();
    for (measure, data) in split_outer(text, ',')?.iter().enumerate() {
        let lines: Vec<_> = data
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        for (index, line) in lines.iter().enumerate() {
            let cells = cells(line)?;
            if cells.len() != 10 {
                return Err(invalid("StepF2 Pump co-op needs ten lanes"));
            }
            let numerator = index as u64 * ROWS_PER_MEASURE;
            if numerator % lines.len() as u64 != 0 && cells.iter().any(|cell| cell.symbol != '0') {
                return Err(Error::Unsupported(
                    "Subdivision cannot be represented at 48 rows per beat".into(),
                ));
            }
            let row = (measure as u64)
                .checked_mul(ROWS_PER_MEASURE)
                .and_then(|base| base.checked_add(numerator / lines.len() as u64))
                .ok_or_else(|| invalid("StepF2 note position overflow"))?;
            for (lane, cell) in cells.into_iter().enumerate() {
                let lane = lane as u16;
                let (owner, kind) = match cell.symbol {
                    '0' => {
                        if !cell.suffix.is_empty() {
                            return Err(invalid("Decorated empty StepF2 cell"));
                        }
                        continue;
                    }
                    'X' => (0, Kind::Tap),
                    'x' => (0, Kind::Hold),
                    'Y' => (1, Kind::Tap),
                    'y' => (1, Kind::Hold),
                    'Z' => (2, Kind::Tap),
                    'z' => (2, Kind::Hold),
                    '1' => (3, Kind::Tap),
                    '2' => (3, Kind::Hold),
                    'M' => (0, Kind::Mine),
                    'F' => (0, Kind::Fake),
                    'S' | 'V' | 'H' => (0, Kind::Tap),
                    '3' => {
                        let (position, mut note) = heads
                            .remove(&lane)
                            .ok_or_else(|| invalid("StepF2 tail has no head"))?;
                        note.end = Some(row);
                        note.tail_attributes = Arc::from(cell.suffix);
                        grid.set(position, Some(note))?;
                        continue;
                    }
                    _ => {
                        return Err(Error::Unsupported(format!(
                            "Unsupported StepF2 note symbol {:?}",
                            cell.symbol
                        )));
                    }
                };
                if heads.contains_key(&lane) {
                    return Err(invalid("StepF2 note inside an open sustain"));
                }
                minimum_players = minimum_players.max(owner + 1);
                let mut suffix = cell.suffix;
                if matches!(cell.symbol, 'S' | 'V' | 'H') {
                    let visibility = match cell.symbol {
                        'S' => Visibility::Sudden,
                        'V' => Visibility::Vanish,
                        _ => Visibility::Hidden,
                    };
                    suffix = if let Some((mut attributes, trailing)) = Attributes::read(&suffix)? {
                        if attributes.visibility != Visibility::Normal
                            && attributes.visibility != visibility
                        {
                            return Err(Error::Unsupported("Combined StepF2 visibility flags cannot yet be represented without loss".into()));
                        }
                        attributes.visibility = visibility;
                        attributes.suffix('N', trailing)?
                    } else {
                        Attributes {
                            visibility,
                            ..Attributes::default()
                        }
                        .suffix('N', &suffix)?
                    };
                }
                let kind = if kind == Kind::Tap
                    && Attributes::read(&suffix)?.is_some_and(|(attrs, _)| attrs.fake)
                {
                    Kind::Fake
                } else {
                    kind
                };
                if kind == Kind::Fake && Attributes::read(&suffix)?.is_none() {
                    suffix = Attributes {
                        fake: true,
                        ..Attributes::default()
                    }
                    .suffix('N', &suffix)?;
                }
                let note = Note {
                    kind,
                    end: None,
                    attributes: Arc::from(suffix),
                    tail_attributes: Arc::from(""),
                };
                let position = Position {
                    player: owner,
                    lane,
                    row,
                };
                if kind == Kind::Hold {
                    heads.insert(lane, (position, note));
                } else {
                    grid.set(position, Some(note))?;
                }
            }
        }
    }
    if !heads.is_empty() {
        return Err(invalid("StepF2 sustain has no tail"));
    }
    grid.set_players(minimum_players)?;
    grid.stepf2 = true;
    Ok(grid)
}

struct Cell {
    symbol: char,
    suffix: String,
}
fn cells(line: &str) -> Result<Vec<Cell>, Error> {
    let bytes = line.as_bytes();
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if !bytes[cursor].is_ascii() {
            return Err(invalid("Non-ASCII StepF2 symbol"));
        }
        let mut suffix = String::new();
        let symbol;
        if bytes[cursor] == b'{' {
            let rest = &line[cursor..];
            let (attributes, _tail) =
                Attributes::read(rest)?.ok_or_else(|| invalid("Malformed StepF2 compound note"))?;
            symbol = bytes[cursor + 1] as char;
            // The embedded source symbol is redundant once kind/owner are
            // decoded. A neutral marker keeps moves and player swaps stable.
            suffix.push_str(&attributes.suffix('N', "")?);
            cursor += 9;
        } else {
            symbol = bytes[cursor] as char;
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && matches!(bytes[cursor], b'[' | b'<') {
            let close = if bytes[cursor] == b'[' { b']' } else { b'>' };
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != close {
                if bytes[cursor] == b'\\' {
                    cursor += 1;
                }
                cursor += 1;
            }
            if cursor >= bytes.len() {
                return Err(invalid("Unterminated StepF2 suffix"));
            }
            cursor += 1;
        }
        suffix.push_str(&line[start..cursor]);
        result.push(Cell { symbol, suffix });
    }
    Ok(result)
}

/// Strict export: errors are returned before any destination is written.
/// GrooveAuthor warns and flattens non-P1 mines; here that data loss is refused.
pub fn serialize(grid: &Grid) -> Result<Vec<u8>, Error> {
    if grid.columns() != 10 || grid.players() > 4 {
        return Err(Error::InvalidEdit(
            "StepF2 needs ten lanes and at most four participants".into(),
        ));
    }
    let mut rows: BTreeMap<u64, BTreeMap<u16, String>> = BTreeMap::new();
    let mut occupied: BTreeMap<u16, BTreeMap<u64, u64>> = BTreeMap::new();
    for (position, note) in grid.iter() {
        validate_attributes(note)?;
        let end = note.end.unwrap_or(position.row);
        let intervals = occupied.entry(position.lane).or_default();
        if intervals
            .range(..=end)
            .next_back()
            .is_some_and(|(_, &stop)| stop >= position.row)
        {
            return Err(Error::InvalidEdit(
                "StepF2 cannot encode overlapping participants on one lane".into(),
            ));
        }
        intervals.insert(position.row, end);
        let symbol = match note.kind {
            Kind::Tap | Kind::Fake => ['X', 'Y', 'Z', '1'][position.player as usize],
            Kind::Hold => ['x', 'y', 'z', '2'][position.player as usize],
            Kind::Mine if position.player == 0 => 'M',
            _ => {
                return Err(Error::Unsupported(
                    "StepF2 cannot losslessly encode this note kind or owner".into(),
                ));
            }
        };
        let head_token = token(symbol, &note.attributes, note.kind == Kind::Fake)?;
        rows.entry(position.row)
            .or_default()
            .insert(position.lane, head_token);
        if let Some(end) = note.end {
            rows.entry(end)
                .or_default()
                .insert(position.lane, token('3', &note.tail_attributes, false)?);
        }
    }
    let last_measure = rows
        .last_key_value()
        .map_or(0, |(&row, _)| row / ROWS_PER_MEASURE);
    // Same allocation budget as the canonical Grid serializer, checked before
    // traversing long spans and after every emitted row.
    const MAX_BYTES: usize = 128 * 1024 * 1024;
    if last_measure
        .checked_add(1)
        .and_then(|count| count.checked_mul(44))
        .is_none_or(|size| size > MAX_BYTES as u64)
    {
        return Err(Error::InvalidEdit(
            "Serialized note data exceeds 128 MiB".into(),
        ));
    }
    let mut output = String::from("\n");
    for measure in 0..=last_measure {
        if measure > 0 {
            output.push_str(",\n");
        }
        let start = measure * ROWS_PER_MEASURE;
        let step = rows
            .range(start..start + ROWS_PER_MEASURE)
            .fold(48, |mut step, (&row, _)| {
                let mut offset = row - start;
                while offset != 0 {
                    (step, offset) = (offset, step % offset);
                }
                step
            });
        for offset in (0..ROWS_PER_MEASURE).step_by(step as usize) {
            let cells = rows.get(&(start + offset));
            for lane in 0..10 {
                output.push_str(
                    cells
                        .and_then(|cells| cells.get(&lane))
                        .map_or("0", String::as_str),
                );
            }
            output.push('\n');
            if output.len() > MAX_BYTES {
                return Err(Error::InvalidEdit(
                    "Serialized note data exceeds 128 MiB".into(),
                ));
            }
        }
    }
    // If every exclusive P1/P2/P3 marker was deleted, plain 1/2 would load as
    // standard P1 notes. Refuse ambiguity rather than changing ownership.
    if !is_coop(output.as_bytes(), "pump-routine")
        && grid.iter().any(|(position, _)| position.player != 0)
    {
        return Err(Error::InvalidEdit(
            "StepF2 output has no marker distinguishing co-op ownership; use standard notation"
                .into(),
        ));
    }
    Ok(output.into_bytes())
}

/// Validate edited opaque suffixes with the same endpoint codec used by the
/// StepF2 writer, before a draft is allowed to create a history entry.
pub(crate) fn validate_attributes(note: &Note) -> Result<(), Error> {
    if matches!(note.kind, Kind::Tap | Kind::Fake) {
        if let Some((attributes, _)) = Attributes::read(&note.attributes)? {
            if attributes.fake != (note.kind == Kind::Fake) {
                return Err(Error::InvalidEdit(
                    "StepF2 fake flag disagrees with the note kind".into(),
                ));
            }
        }
    }
    let symbol = if matches!(note.kind, Kind::Hold | Kind::Roll) {
        'x'
    } else {
        'X'
    };
    token(symbol, &note.attributes, note.kind == Kind::Fake)?;
    token('3', &note.tail_attributes, false)?;
    Ok(())
}

fn token(symbol: char, suffix: &str, fake: bool) -> Result<String, Error> {
    let encoded = if let Some((attrs, trailing)) = Attributes::read(suffix)? {
        attrs.suffix(symbol, trailing)?
    } else if fake {
        Attributes {
            fake: true,
            ..Attributes::default()
        }
        .suffix(symbol, suffix)?
    } else {
        format!("{symbol}{suffix}")
    };
    if cells(&encoded)?.len() != 1 {
        return Err(Error::Unsupported(
            "A StepF2 suffix cannot contain another note or compound cell".into(),
        ));
    }
    Ok(encoded)
}
