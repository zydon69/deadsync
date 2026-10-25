//! ActiveEditorChart.SetWaveFormPps policy (GrooveAuthor 48e18f71, MIT).
//! Returns the original unsigned temporal speed before view zoom/Reverse.
use crate::{
    Error,
    spacing::{Rate, Speeds},
    zoom::SpacingMode,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollMode {
    CurrentTempo,
    CurrentTempoAndRate,
    #[default]
    MostCommonTempo,
}

pub fn pixels_per_second(
    mode: SpacingMode,
    policy: ScrollMode,
    rate: Rate,
    speeds: Speeds,
    most_common_tempo: f64,
    interpolated_scroll: f64,
) -> Result<f64, Error> {
    let value = match mode {
        SpacingMode::ConstantTime => positive(speeds.time)?,
        SpacingMode::ConstantRow => {
            let mut value = positive(speeds.row)? * positive(rate.rows_per_second)?;
            if policy == ScrollMode::MostCommonTempo {
                value *= positive(most_common_tempo)? / positive(rate.tempo)?;
            }
            value
        }
        SpacingMode::Variable => {
            let tempo = if policy == ScrollMode::MostCommonTempo {
                most_common_tempo
            } else {
                rate.tempo
            };
            let mut value = positive(speeds.variable)? * (positive(tempo)? / 300.0);
            if policy == ScrollMode::CurrentTempoAndRate {
                finite(rate.scroll)?;
                finite(interpolated_scroll)?;
                let rate = rate.scroll * interpolated_scroll;
                value *= if rate <= 0.0 { 1.0 } else { rate };
            }
            value
        }
    };
    positive(value)
}
fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidEdit("Nonfinite waveform spacing".into()))
    }
}
fn positive(value: f64) -> Result<f64, Error> {
    finite(value)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(Error::InvalidEdit(
            "Waveform spacing must be positive".into(),
        ))
    }
}

/// Original RefreshRateAlteringEvents tempo-duration accounting. The final
/// duration ends at the last rate event, and equal durations keep insertion order.
pub fn most_common_tempo(index: &crate::spacing_index::Index) -> Result<Option<f64>, Error> {
    let Some(last) = index.sections().last() else {
        return Ok(None);
    };
    let mut positions = std::collections::BTreeMap::new();
    let mut durations: Vec<(f64, f64)> = Vec::new();
    let mut add = |tempo: f64, time: f64, change: f64| -> Result<(), Error> {
        let i = *positions.entry(tempo.to_bits()).or_insert_with(|| {
            durations.push((tempo, 0.0));
            durations.len() - 1
        });
        durations[i].1 = finite(durations[i].1 + time - change)?;
        Ok(())
    };
    let mut first = true;
    let mut last_tempo = 1.0;
    let mut last_change = 0.0;
    for section in index.sections().iter().filter(|section| section.order == 1) {
        if !first {
            add(last_tempo, section.mapping.time(), last_change)?;
            last_change = section.mapping.time();
        }
        last_tempo = section.tempo;
        first = false;
    }
    add(last_tempo, last.mapping.time(), last_change)?;
    let mut longest = -1.0;
    let mut common = 0.0;
    for (tempo, duration) in durations {
        if duration > longest {
            longest = duration;
            common = tempo;
        }
    }
    Ok(Some(common))
}
