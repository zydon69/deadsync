//! Typed timing payloads for the mixed event clipboard. All changed timing tags
//! enter one Target transaction, so attacks are anchored against the final clock.
use super::{Scope, Target, combo, label, pause, region, scroll, signature, speed, tempo, tick};
use crate::{Error, document::Format, history::Session};
#[derive(Clone)]
enum Value {
    Tempo(f64),
    Stop(f64),
    Delay(f64),
    Signature(signature::Signature),
    Tick(i32),
    Combo(combo::Multipliers),
    Warp(u64),
    Fake(u64),
    Scroll(f64),
    Speed(speed::Speed),
    Label(String),
}
#[derive(Clone)]
pub(crate) struct Clipboard {
    events: Vec<(u64, Value)>,
}
#[derive(Clone)]
struct Models {
    tempo: tempo::Tempos,
    stop: pause::Pauses,
    delay: pause::Pauses,
    signature: signature::Signatures,
    tick: tick::TickCounts,
    combo: combo::Combos,
    warp: region::Regions,
    fake: region::Regions,
    scroll: scroll::Scrolls,
    speed: speed::Speeds,
    label: label::Labels,
}
impl Models {
    fn capture(session: &Session, chart: usize, scope: Scope) -> Result<(Target, Self), Error> {
        let target = Target::capture(session, chart, scope)?;
        let text = |tag| {
            session
                .document()
                .text(target.origin(), tag)
                .unwrap_or_default()
        };
        let extended = |tag| {
            if session.document().format == Format::Ssc {
                text(tag)
            } else {
                String::new()
            }
        };
        let models = Self {
            tempo: super::TempoDraft::capture(session, chart, scope)?
                .tempos()
                .clone(),
            stop: pause::Pauses::parse(pause::Kind::Stop, &text("STOPS"))?,
            delay: pause::Pauses::parse(pause::Kind::Delay, &text("DELAYS"))?,
            signature: signature::Signatures::parse(&text("TIMESIGNATURES"))?,
            tick: tick::TickCounts::parse(&text("TICKCOUNTS"))?,
            combo: combo::Combos::parse(&extended("COMBOS"))?,
            warp: region::Regions::parse(region::Kind::Warp, &extended("WARPS"))?,
            fake: region::Regions::parse(region::Kind::Fake, &extended("FAKES"))?,
            scroll: scroll::Scrolls::parse(&extended("SCROLLS"))?,
            speed: speed::Speeds::parse(&extended("SPEEDS"))?,
            label: label::Labels::parse(&extended("LABELS"))?,
        };
        Ok((target, models))
    }
    fn changes(&self, original: &Self) -> Vec<(&'static str, String)> {
        let mut changes = Vec::new();
        if self.tempo != original.tempo {
            changes.push(("BPMS", self.tempo.serialize()));
        }
        if self.stop != original.stop {
            changes.push(("STOPS", self.stop.serialize()));
        }
        if self.delay != original.delay {
            changes.push(("DELAYS", self.delay.serialize()));
        }
        if self.signature != original.signature {
            changes.push(("TIMESIGNATURES", self.signature.serialize()));
        }
        if self.tick != original.tick {
            changes.push(("TICKCOUNTS", self.tick.serialize()));
        }
        if self.combo != original.combo {
            changes.push(("COMBOS", self.combo.serialize()));
        }
        if self.warp != original.warp {
            changes.push(("WARPS", self.warp.serialize()));
        }
        if self.fake != original.fake {
            changes.push(("FAKES", self.fake.serialize()));
        }
        if self.scroll != original.scroll {
            changes.push(("SCROLLS", self.scroll.serialize()));
        }
        if self.speed != original.speed {
            changes.push(("SPEEDS", self.speed.serialize()));
        }
        if self.label != original.label {
            changes.push(("LABELS", self.label.serialize()));
        }
        changes
    }
}
impl Clipboard {
    pub(crate) fn capture(
        session: &Session,
        chart: usize,
        scope: Scope,
        start: u64,
        end: u64,
    ) -> Result<Self, Error> {
        let (_, models) = Models::capture(session, chart, scope)?;
        let mut events = Vec::new();
        events.extend(
            models
                .tempo
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Tempo(value.clone()))),
        );
        events.extend(
            models
                .stop
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Stop(value.clone()))),
        );
        events.extend(
            models
                .delay
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Delay(value.clone()))),
        );
        events.extend(
            models
                .signature
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Signature(value.clone()))),
        );
        events.extend(
            models
                .tick
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Tick(value.clone()))),
        );
        events.extend(
            models
                .combo
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Combo(value.clone()))),
        );
        events.extend(
            models
                .warp
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Warp(value.clone()))),
        );
        events.extend(
            models
                .fake
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Fake(value.clone()))),
        );
        events.extend(
            models
                .scroll
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Scroll(value.clone()))),
        );
        events.extend(
            models
                .speed
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Speed(value.clone()))),
        );
        events.extend(
            models
                .label
                .values()
                .range(start..=end)
                .map(|(&row, value)| (row, Value::Label(value.clone()))),
        );
        events.sort_by_key(|(row, _)| *row);
        Ok(Self { events })
    }
    pub(crate) fn first_row(&self) -> Option<u64> {
        self.events.first().map(|e| e.0)
    }
    pub(crate) fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        scope: Scope,
        origin: u64,
        destination: u64,
    ) -> Result<(), Error> {
        self.apply(session, chart, scope, Some((origin, destination)))
    }
    pub(crate) fn delete(
        &self,
        session: &mut Session,
        chart: usize,
        scope: Scope,
    ) -> Result<(), Error> {
        self.apply(session, chart, scope, None)
    }
    fn apply(
        &self,
        session: &mut Session,
        chart: usize,
        scope: Scope,
        placement: Option<(u64, u64)>,
    ) -> Result<(), Error> {
        if self.events.is_empty() {
            return Ok(());
        }
        let (target, original) = Models::capture(session, chart, scope)?;
        let mut models = original.clone();
        for (source, value) in &self.events {
            let row = match placement {
                Some((origin, destination)) => source
                    .checked_sub(origin)
                    .and_then(|offset| destination.checked_add(offset))
                    .filter(|row| *row <= i32::MAX as u64)
                    .ok_or_else(|| Error::InvalidEdit("Timing paste exceeds editor rows".into()))?,
                None => *source,
            };
            if session.document().format == Format::Sm
                && matches!(
                    value,
                    Value::Combo(..)
                        | Value::Warp(..)
                        | Value::Fake(..)
                        | Value::Scroll(..)
                        | Value::Speed(..)
                        | Value::Label(..)
                )
            {
                return Err(Error::Unsupported(
                    "These timing events require SSC export".into(),
                ));
            }
            match value {
                Value::Tempo(value) => {
                    if placement.is_some() {
                        models.tempo.set(row, *value)?;
                    } else if row != 0 {
                        models.tempo.remove(row)?;
                    }
                }
                Value::Stop(value) => {
                    if placement.is_some() {
                        models.stop.set(row, *value)?;
                    } else {
                        models.stop.remove(row)?;
                    }
                }
                Value::Delay(value) => {
                    if placement.is_some() {
                        models.delay.set(row, *value)?;
                    } else {
                        models.delay.remove(row)?;
                    }
                }
                Value::Signature(value) => {
                    if placement.is_some() {
                        models.signature.set(row, *value)?;
                    } else if row != 0 {
                        models.signature.remove(row)?;
                    }
                }
                Value::Tick(value) => {
                    if placement.is_some() {
                        models.tick.clone_at(row, *value)?;
                    } else {
                        models.tick.remove(row)?;
                    }
                }
                Value::Combo(value) => {
                    if placement.is_some() {
                        models.combo.clone_at(row, *value)?;
                    } else {
                        models.combo.remove(row)?;
                    }
                }
                Value::Warp(value) => {
                    if placement.is_some() {
                        models.warp.set(row, *value)?;
                    } else {
                        models.warp.remove(row)?;
                    }
                }
                Value::Fake(value) => {
                    if placement.is_some() {
                        models.fake.set(row, *value)?;
                    } else {
                        models.fake.remove(row)?;
                    }
                }
                Value::Scroll(value) => {
                    if placement.is_some() {
                        models.scroll.set(row, *value)?;
                    } else {
                        models.scroll.remove(row)?;
                    }
                }
                Value::Speed(value) => {
                    if placement.is_some() {
                        models.speed.clone_at(row, *value)?;
                    } else {
                        models.speed.remove(row)?;
                    }
                }
                Value::Label(value) => {
                    if placement.is_some() {
                        models.label.clone_at(row, value)?;
                    } else {
                        models.label.remove(row)?;
                    }
                }
            }
        }
        target.apply(
            session,
            chart,
            &models.changes(&original),
            "Edit selected timing events",
        )?;
        Ok(())
    }
}
