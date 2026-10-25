//! GrooveAuthor display-tempo state. UI range editing is deliberately separate
//! from the serialized value; opening a property must never normalize its source.
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Actual,
    Random,
    Specified,
}

#[derive(Clone, Debug)]
pub struct Value {
    mode: Mode,
    min: f64,
    max: f64,
    separate_max: bool,
}

fn near(a: f64, b: f64) -> bool {
    if a.is_sign_negative() != b.is_sign_negative() {
        a == b
    } else {
        a.to_bits().abs_diff(b.to_bits()) <= 1
    }
}
fn equal(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}
fn number(text: &str) -> Option<f64> {
    text.trim().replace(',', "").parse().ok()
}
fn formatted(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else {
        format!("{value:.6}")
    }
}
impl Default for Value {
    fn default() -> Self {
        Self {
            mode: Mode::Actual,
            min: 0.0,
            max: 0.0,
            separate_max: true,
        }
    }
}
impl Value {
    /// Invalid source strings fall back to Actual, as in the original editor.
    /// The property owner retains the original bytes until a semantic edit.
    pub fn parse(text: &str) -> Self {
        if text == "*" {
            return Self {
                mode: Mode::Random,
                ..Self::default()
            };
        }
        let parts: Vec<_> = text.split(':').collect();
        let range = match parts.as_slice() {
            [one] => number(one).map(|v| (v, v)),
            [min, max] => number(min).zip(number(max)),
            _ => None,
        };
        match range {
            Some((min, max)) => Self {
                mode: Mode::Specified,
                min,
                max,
                separate_max: !near(min, max),
            },
            None => Self::default(),
        }
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn min(&self) -> f64 {
        self.min
    }
    pub fn max(&self) -> f64 {
        self.max
    }
    pub fn separate_max(&self) -> bool {
        self.separate_max
    }
    pub fn set_mode(&mut self, mode: Mode, actual: (f64, f64)) -> Result<(), Error> {
        if mode == Mode::Specified && self.mode != mode {
            if !actual.0.is_finite() || !actual.1.is_finite() {
                return Err(Error::InvalidEdit("Display tempo must be finite".into()));
            }
            self.min = actual.0;
            self.max = actual.1;
            if !near(self.min, self.max) {
                self.separate_max = true;
            }
        }
        self.mode = mode;
        Ok(())
    }
    pub fn set_min(&mut self, value: f64) -> Result<(), Error> {
        finite(value)?;
        self.min = value;
        if !self.separate_max {
            self.max = value;
        }
        Ok(())
    }
    pub fn set_max(&mut self, value: f64) -> Result<(), Error> {
        finite(value)?;
        if !self.separate_max {
            return Err(Error::InvalidEdit(
                "Separate maximum tempo is disabled".into(),
            ));
        }
        self.max = value;
        Ok(())
    }
    pub fn set_separate_max(&mut self, enabled: bool) {
        self.separate_max = enabled;
        if !enabled {
            self.max = self.min;
        }
    }
    pub fn same_serialized_value(&self, other: &Self) -> bool {
        self.mode == other.mode
            && (self.mode != Mode::Specified
                || (equal(self.min, other.min) && equal(self.max, other.max)))
    }
    pub fn serialize(&self) -> String {
        match self.mode {
            Mode::Actual => String::new(),
            Mode::Random => "*".into(),
            Mode::Specified if near(self.min, self.max) => formatted(self.min),
            Mode::Specified => format!("{}:{}", formatted(self.min), formatted(self.max)),
        }
    }
}
fn finite(value: f64) -> Result<(), Error> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidEdit("Display tempo must be finite".into()))
    }
}
