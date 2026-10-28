//! Source quick-add defaults, detached from I/O and native keyboard dispatch.
//! Existing events are never replaced by a quick-add action.
use crate::{
    Error,
    generation::metadata::{ChartData, Definition, Guid},
    history::{Session, file_intent::IntentGuard},
    timing_edit::{self, Scope},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tempo,
    Signature,
    Scroll,
    Speed,
    Stop,
    Delay,
    Warp,
    Fake,
    Ticks,
    Combo,
    Label,
    Attack,
    Pattern,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Occupied,
    InvalidPosition,
    UnsupportedLayout,
    Stale,
    Invalid,
}
#[derive(Debug)]
pub struct Failure {
    pub reason: Refusal,
    pub error: Error,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            reason: Refusal::Invalid,
            error,
        }
    }
}
fn refuse(reason: Refusal, message: &str) -> Failure {
    Failure {
        reason,
        error: Error::InvalidEdit(message.into()),
    }
}
/// Original EditorPosition.GetNearestRow rounds exact positive halves down;
/// UIEditEvents then clamps negative positions to zero.
pub fn add_row(position: f64) -> Result<u64, Failure> {
    if !position.is_finite() {
        return Err(refuse(
            Refusal::InvalidPosition,
            "Quick-add position must be finite",
        ));
    }
    if position <= 0.0 {
        return Ok(0);
    }
    let integral = position.trunc();
    let row = integral + if position - integral > 0.5 { 1.0 } else { 0.0 };
    if row > i32::MAX as f64 {
        return Err(refuse(
            Refusal::InvalidPosition,
            "Quick-add position exceeds source row limit",
        ));
    }
    Ok(row as u64)
}
enum Draft {
    Tempo(timing_edit::TempoDraft),
    Signature(timing_edit::signature::Draft),
    Scroll(timing_edit::scroll::Draft),
    Speed(timing_edit::speed::Draft),
    Pause(timing_edit::pause::Draft),
    Region(timing_edit::region::Draft),
    Ticks(timing_edit::tick::Draft),
    Combo(timing_edit::combo::Draft),
    Label(timing_edit::label::Draft),
    Attack(crate::attacks::inspector::Draft),
    Pattern(ChartData),
}
pub struct Plan {
    guard: IntentGuard,
    chart: usize,
    identity: Guid,
    pub kind: Kind,
    pub row: u64,
    draft: Draft,
}
impl Plan {
    pub fn prepare(
        session: &Session,
        chart: usize,
        kind: Kind,
        position: f64,
    ) -> Result<Self, Failure> {
        // A seed is generated once per detached action, and retained on Undo/Redo.
        let bytes = uuid::Uuid::new_v4().into_bytes();
        let seed = (u32::from_le_bytes(bytes[..4].try_into().unwrap()) % i32::MAX as u32) as i32;
        Self::prepare_with_seed(session, chart, kind, position, seed)
    }
    pub fn prepare_with_seed(
        session: &Session,
        chart: usize,
        kind: Kind,
        position: f64,
        seed: i32,
    ) -> Result<Self, Failure> {
        let row = add_row(position)?;
        let identity = session
            .chart_ids()
            .get(chart)
            .ok_or_else(|| refuse(Refusal::Invalid, "Quick-add chart does not exist"))?
            .clone();
        let scope = if timing_edit::supports_chart_timing(session.document()) {
            Scope::Chart
        } else {
            Scope::Song
        };
        let tempo = timing_edit::TempoDraft::capture(session, chart, scope)?;
        let bpm = tempo.tempos().at(row);
        let seconds = 60.0 / bpm;
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(refuse(
                Refusal::Invalid,
                "Quick-add needs a positive active tempo",
            ));
        }
        let occupied = || {
            refuse(
                Refusal::Occupied,
                "An event of this type already exists at the quick-add row",
            )
        };
        let draft = match kind {
            Kind::Tempo => {
                let mut d = timing_edit::TempoDraft::capture(session, chart, scope)?;
                if d.tempos().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(row, bpm)?;
                Draft::Tempo(d)
            }
            Kind::Signature => {
                let mut d = timing_edit::signature::Draft::capture(session, chart, scope)?;
                if d.signatures().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(
                    row,
                    timing_edit::signature::Signature {
                        numerator: 4,
                        denominator: 4,
                    },
                )?;
                Draft::Signature(d)
            }
            Kind::Scroll => {
                let mut d = timing_edit::scroll::Draft::capture(session, chart, scope)?;
                if d.scrolls().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(row, 1.0)?;
                Draft::Scroll(d)
            }
            Kind::Speed => {
                let mut d = timing_edit::speed::Draft::capture(session, chart, scope)?;
                if d.speeds().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(
                    row,
                    timing_edit::speed::Speed {
                        rate: 1.0,
                        period: timing_edit::speed::Period::Rows(48),
                    },
                )?;
                Draft::Speed(d)
            }
            Kind::Ticks => {
                let mut d = timing_edit::tick::Draft::capture(session, chart, scope)?;
                if d.counts().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(row, 4)?;
                Draft::Ticks(d)
            }
            Kind::Combo => {
                let mut d = timing_edit::combo::Draft::capture(session, chart, scope)?;
                if d.combos().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(row, timing_edit::combo::Multipliers { hit: 1, miss: 1 })?;
                Draft::Combo(d)
            }
            Kind::Label => {
                let mut d = timing_edit::label::Draft::capture(session, chart, scope)?;
                if d.labels().values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(row, "New Label")?;
                Draft::Label(d)
            }
            Kind::Stop | Kind::Delay => {
                let k = if kind == Kind::Stop {
                    timing_edit::pause::Kind::Stop
                } else {
                    timing_edit::pause::Kind::Delay
                };
                let mut d = timing_edit::pause::Draft::capture(session, chart, scope)?;
                if d.pauses(k).values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(k, row, seconds)?;
                Draft::Pause(d)
            }
            Kind::Warp | Kind::Fake => {
                let k = if kind == Kind::Warp {
                    timing_edit::region::Kind::Warp
                } else {
                    timing_edit::region::Kind::Fake
                };
                let mut d = timing_edit::region::Draft::capture(session, chart, scope)?;
                if d.regions(k).values().contains_key(&row) {
                    return Err(occupied());
                }
                d.set(k, row, 48)?;
                Draft::Region(d)
            }
            Kind::Attack => {
                let mut d = crate::attacks::inspector::Draft::capture(session, chart, scope)?;
                if d.attacks().events().iter().any(|e| e.row == row) {
                    return Err(occupied());
                }
                d.add(
                    row,
                    crate::attacks::Modifier {
                        length: seconds,
                        ..Default::default()
                    },
                )?;
                Draft::Attack(d)
            }
            Kind::Pattern => {
                if !crate::generation::job::TARGET_LAYOUTS
                    .contains(&session.document().charts()[chart].steps_type.as_str())
                {
                    return Err(refuse(
                        Refusal::UnsupportedLayout,
                        "This chart layout does not support source pattern generation",
                    ));
                }
                if seed < 0 || row.checked_add(192).is_none_or(|end| end > i32::MAX as u64) {
                    return Err(refuse(
                        Refusal::Invalid,
                        "Pattern seed or endpoint exceeds source limits",
                    ));
                }
                let mut d = ChartData::existing_or_initial(session.document(), chart)?;
                let patterns = d.patterns.get_or_insert_default();
                if patterns.contains_key(&row) {
                    return Err(occupied());
                }
                patterns.insert(
                    row,
                    Definition {
                        pattern_config_guid: "03de57bd-4329-4de6-a722-a171c11fdd16".parse()?,
                        performed_chart_config_guid: "41a7f76e-596a-4fbe-bf6e-6a77a898e4a1"
                            .parse()?,
                        length: 192,
                        random_seed: seed,
                        start_position_inclusive: true,
                        end_position_inclusive: false,
                        ignore_preceding_distribution: false,
                        ..Default::default()
                    },
                );
                Draft::Pattern(d)
            }
        };
        Ok(Self {
            guard: IntentGuard::capture(session),
            chart,
            identity,
            kind,
            row,
            draft,
        })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && self.guard.is_current(session)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Failure> {
        if !self.is_current(session, chart) {
            return Err(refuse(
                Refusal::Stale,
                "Quick-add belongs to another chart or history lifetime",
            ));
        }
        Ok(match &self.draft {
            Draft::Tempo(d) => d.apply(session, chart)?,
            Draft::Signature(d) => d.apply(session, chart)?,
            Draft::Scroll(d) => d.apply(session, chart)?,
            Draft::Speed(d) => d.apply(session, chart)?,
            Draft::Pause(d) => d.apply(session, chart)?,
            Draft::Region(d) => d.apply(session, chart)?,
            Draft::Ticks(d) => d.apply(session, chart)?,
            Draft::Combo(d) => d.apply(session, chart)?,
            Draft::Label(d) => d.apply(session, chart)?,
            Draft::Attack(d) => d.apply(session, chart)?,
            Draft::Pattern(d) => d.save(session, chart)?,
        })
    }
}
