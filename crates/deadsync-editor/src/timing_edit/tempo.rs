//! Fumen AddTempos and GrooveAuthor tempo-node rules, pinned by the editor oracle.
use crate::Error;
use std::collections::BTreeMap;
pub const MIN_BPM: f64 = 0.000001;
#[derive(Clone, Debug, PartialEq)]
pub struct Tempos {
    values: BTreeMap<u64, f64>,
    source_lacked_initial: bool,
}
impl Tempos {
    pub fn parse(text: &str, fallback: f64) -> Result<Self, Error> {
        validate_bpm(fallback)?;
        let distinct = parse_pairs(text)?;
        let mut rows = BTreeMap::new();
        for (beat, bpm) in distinct {
            let row = beat_row(beat)?;
            if row >= 0 {
                rows.insert(row as u64, bpm);
            }
        }
        let replacement = rows
            .values()
            .copied()
            .find(|bpm| *bpm > 0.0)
            .unwrap_or(60.0);
        let mut values = BTreeMap::new();
        for (index, (row, bpm)) in rows.into_iter().enumerate() {
            let bpm = if bpm <= 0.0 {
                if index == 0 { replacement } else { continue }
            } else {
                bpm
            };
            let bpm = bpm.max(MIN_BPM);
            validate_bpm(bpm)?;
            values.insert(row, bpm);
        }
        let source_lacked_initial = !values.contains_key(&0);
        values.entry(0).or_insert(fallback);
        Ok(Self {
            values,
            source_lacked_initial,
        })
    }
    pub fn source_lacked_initial(&self) -> bool {
        self.source_lacked_initial
    }
    pub fn values(&self) -> &BTreeMap<u64, f64> {
        &self.values
    }
    pub fn at(&self, row: u64) -> f64 {
        *self.values.range(..=row).next_back().unwrap().1
    }
    pub fn set(&mut self, row: u64, bpm: f64) -> Result<bool, Error> {
        valid_row(row)?;
        validate_bpm(bpm)?;
        if self
            .values
            .get(&row)
            .is_some_and(|old| crate::number::reference_equal(*old, bpm))
        {
            return Ok(false);
        }
        self.values.insert(row, bpm);
        Ok(true)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        valid_row(row)?;
        if row == 0 {
            return Err(invalid("The initial BPM cannot be deleted"));
        }
        Ok(self.values.remove(&row).is_some())
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        valid_row(from)?;
        valid_row(to)?;
        if from == 0 && to != 0 {
            return Err(invalid("The initial BPM cannot be moved"));
        }
        let bpm = *self
            .values
            .get(&from)
            .ok_or_else(|| invalid("BPM node does not exist"))?;
        if from == to {
            return Ok(false);
        }
        if self.values.contains_key(&to) {
            return Err(invalid("Another BPM node already occupies this row"));
        }
        self.values.remove(&from);
        self.values.insert(to, bpm);
        Ok(true)
    }
    pub fn serialize(&self) -> String {
        self.values
            .iter()
            .map(|(row, bpm)| format!("{}={bpm}", *row as f64 / 48.0))
            .collect::<Vec<_>>()
            .join(",")
    }
}
pub(crate) fn parse_pairs(text: &str) -> Result<Vec<(f64, f64)>, Error> {
    let cleaned = text
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(value, _)| value))
        .collect::<Vec<_>>()
        .join("");
    let mut beats = Vec::new();
    for part in cleaned.split(',').filter(|part| !part.trim().is_empty()) {
        let (beat, bpm) = part
            .trim()
            .split_once('=')
            .ok_or_else(|| invalid("Invalid BPM segment"))?;
        let beat = beat
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid("Invalid BPM beat"))?;
        let bpm = bpm
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid("Invalid BPM value"))?;
        if !beat.is_finite() || !bpm.is_finite() {
            return Err(invalid("BPM positions and values must be finite"));
        }
        beats.push((beat, bpm));
    }
    beats.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut distinct: Vec<(f64, f64)> = Vec::new();
    for pair in beats {
        if let Some(last) = distinct.last_mut().filter(|last| last.0 == pair.0) {
            *last = pair;
        } else {
            distinct.push(pair);
        }
    }
    Ok(distinct)
}
/// Original nearest subdivision selection, including its negative-fraction and
/// half-row behavior. Keep its two stages and operation order for f64 parity.
pub(crate) fn beat_row(beat: f64) -> Result<i64, Error> {
    let whole = beat.trunc();
    if !whole.is_finite()
        || whole < (i32::MIN as f64 / 48.0) - 1.0
        || whole > (i32::MAX as f64 / 48.0) + 1.0
    {
        return Err(invalid("BPM position exceeds the reference row range"));
    }
    let fraction = beat - whole;
    let mut closest = 0;
    if fraction >= 47.0 / 48.0 {
        closest = 47;
    } else if fraction > 0.0 {
        let mut distance = fraction.abs();
        for index in 1..48 {
            let candidate = (fraction - index as f64 / 48.0).abs();
            if candidate <= distance {
                distance = candidate;
                closest = index;
            }
        }
    }
    let lower = whole + closest as f64 / 48.0;
    let higher = whole + (closest + 1) as f64 / 48.0;
    if (beat - lower).abs() > (beat - higher).abs() {
        closest += 1;
    }
    let row = whole * 48.0 + closest as f64;
    if row < i32::MIN as f64 || row > i32::MAX as f64 {
        return Err(invalid("BPM position exceeds the reference row range"));
    }
    Ok(row as i64)
}
fn validate_bpm(bpm: f64) -> Result<(), Error> {
    if !bpm.is_finite()
        || !(bpm as f32).is_finite()
        || bpm < MIN_BPM
        || !(60.0 / bpm / 48.0).is_finite()
        || !(1.0 / (60.0 / bpm / 48.0)).is_finite()
    {
        return Err(invalid(
            "BPM must be finite, at least 0.000001 and representable by gameplay timing",
        ));
    }
    Ok(())
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row > i32::MAX as u64 {
        Err(invalid("BPM position exceeds the reference row range"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
