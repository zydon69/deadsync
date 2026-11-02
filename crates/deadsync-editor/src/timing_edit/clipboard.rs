//! Typed timing payloads for the mixed event clipboard. All changed timing tags
//! enter one Target transaction, so attacks are anchored against the final clock.
use super::{Scope, Target, combo, label, pause, region, scroll, signature, speed, tempo, tick};
use crate::event_clipboard::{Event, TimingKind};
use crate::{Error,  history::Session};
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
impl Value {
    fn kind(&self) -> TimingKind {
        match self {
            Self::Tempo(_) => TimingKind::Tempo,
            Self::Stop(_) => TimingKind::Stop,
            Self::Delay(_) => TimingKind::Delay,
            Self::Signature(_) => TimingKind::Signature,
            Self::Tick(_) => TimingKind::Tick,
            Self::Combo(_) => TimingKind::Combo,
            Self::Warp(_) => TimingKind::Warp,
            Self::Fake(_) => TimingKind::Fake,
            Self::Scroll(_) => TimingKind::Scroll,
            Self::Speed(_) => TimingKind::Speed,
            Self::Label(_) => TimingKind::Label,
        }
    }
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
        let models=Self::read(session,target.origin(),super::TempoDraft::capture(session,chart,scope)?.tempos().clone())?;
        Ok((target,models))
    }
    fn read(session:&Session,origin:Option<usize>,tempo:tempo::Tempos)->Result<Self,Error>{
        let properties=super::resident::Properties::capture(session.document(),origin)?;
        let text=|tag|properties.text(tag);
        let extended=|tag|properties.text(tag);
        let models = Self {
            tempo,
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
        Ok(models)
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
    pub(crate) fn is_empty(&self)->bool{self.events.is_empty()}
    pub(crate) fn display_values(&self)->impl Iterator<Item=(Event,String)>+'_ {
        self.events.iter().map(|(row,value)|{let text=match value{
            Value::Tempo(v)|Value::Stop(v)|Value::Delay(v)|Value::Scroll(v)=>v.to_string(),
            Value::Tick(v)=>v.to_string(),Value::Warp(v)|Value::Fake(v)=>(*v as f64/48.0).to_string(),
            Value::Signature(v)=>format!("{}/{}",v.numerator,v.denominator),Value::Combo(v)=>format!("{}/{}",v.hit,v.miss),
            Value::Speed(v)=>v.rate.to_string(),Value::Label(v)=>v.clone(),
        };(Event::Timing{row:*row,kind:value.kind()},text)})
    }
    pub(crate) fn keys(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.iter().map(|(row, value)| Event::Timing {
            row: *row,
            kind: value.kind(),
        })
    }
    pub(crate) fn retain(&mut self, selected: &std::collections::BTreeSet<Event>) {
        self.events.retain(|(row, value)| {
            selected.contains(&Event::Timing {
                row: *row,
                kind: value.kind(),
            })
        });
    }
    pub(crate) fn capture(
        session: &Session,
        chart: usize,
        scope: Scope,
        start: u64,
        end: u64,
    ) -> Result<Self, Error> {
        let (_, models) = Models::capture(session, chart, scope)?;
        Self::from_models(models,start,end)
    }
    /// Read-only identities use the effective playback timing. Legacy SSC local
    /// timing is retained as opaque data until explicit migration; it is never
    /// exposed here as though it were active. Mutation constructors still use
    /// Target::capture and retain the legacy migration guard.
    pub(crate) fn capture_inventory(session:&Session,chart:usize,scope:Scope,start:u64,end:u64)->Result<Self,Error>{
        let document=session.document();
        if chart>=document.charts().len(){return Err(crate::Error::InvalidEdit("Timing inventory chart does not exist".into()));}
        if scope==Scope::Chart&&!super::supports_resident_chart_timing(document){return Err(crate::Error::InvalidEdit("Legacy timing inventory requires the effective song timing".into()));}
        let origin=(scope==Scope::Chart&&super::uses_chart_timing(document,chart)).then_some(chart);
        let raw=super::resident::text(document,origin,"BPMS")?;
        let mut tempo=tempo::Tempos::parse(&raw,120.)?;
        if tempo.source_lacked_initial(){let fallback=if scope==Scope::Chart{super::chart_fallback(document,chart)?}else{super::song_fallback(document)};tempo=tempo::Tempos::parse(&raw,fallback)?;}
        Self::from_models(Models::read(session,origin,tempo)?,start,end)
    }
    fn from_models(models:Models,start:u64,end:u64)->Result<Self,Error>{
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
