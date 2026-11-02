//! User-selected table comparers from GrooveAuthor48e18f UI*Comparer.cs.
//! Initial cultural ordering remains the host's responsibility; an empty list
//! deliberately leaves that order untouched. Explicit text columns are ordinal.
use super::{Catalog, Entry};
use crate::{
    Error,
    generation::{
        config::*,
        profiles::{Config, Kind},
    },
};
use std::cmp::Ordering;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    Abbreviation,
    NoteType,
    RepetitionLimit,
    StepType,
    StepTypeCheckPeriod,
    StartingFoot,
    StartingFooting,
    EndingFooting,
    StepSpeedMin,
    StepDistanceMin,
    StepStretchMin,
    LateralSpeed,
    LateralRelativeNps,
    LateralAbsoluteNps,
    TransitionMin,
    TransitionMax,
    FacingInwardLimit,
    FacingOutwardLimit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub column: Column,
    pub descending: bool,
}
pub const PATTERN: [Column; 9] = [
    Column::NoteType,
    Column::RepetitionLimit,
    Column::StepType,
    Column::StepTypeCheckPeriod,
    Column::StartingFoot,
    Column::StartingFooting,
    Column::EndingFooting,
    Column::Abbreviation,
    Column::Name,
];
pub const PERFORMED: [Column; 12] = [
    Column::StepSpeedMin,
    Column::StepDistanceMin,
    Column::StepStretchMin,
    Column::LateralSpeed,
    Column::LateralRelativeNps,
    Column::LateralAbsoluteNps,
    Column::TransitionMin,
    Column::TransitionMax,
    Column::FacingInwardLimit,
    Column::FacingOutwardLimit,
    Column::Abbreviation,
    Column::Name,
];
pub fn columns(kind: Kind) -> &'static [Column] {
    match kind {
        Kind::Pattern => &PATTERN,
        Kind::Performed => &PERFORMED,
        Kind::Expressed => &[],
    }
}
#[derive(Clone)]
enum Value {
    Number(f64),
    Text(String),
    Foot(i32, i32),
}
impl Value {
    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => {
                a.partial_cmp(b).expect("validated finite profile sort")
            }
            (Self::Text(a), Self::Text(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Self::Foot(a, b), Self::Foot(c, d)) => (a, b).cmp(&(c, d)),
            _ => unreachable!("same column has same key type"),
        }
    }
}
fn number(value: f64) -> Result<Value, Error> {
    if value.is_finite() {
        Ok(Value::Number(value))
    } else {
        Err(super::invalid("Nonfinite profile sort value"))
    }
}
fn enabled(value: f64, on: Option<bool>) -> f64 {
    if on == Some(true) { value } else { 0. }
}
fn start(c: StartFootChoice, lane: i32) -> String {
    match c {
        StartFootChoice::SpecifiedLane => lane.to_string(),
        StartFootChoice::AutomaticNewLane => "N".into(),
        StartFootChoice::AutomaticSameLane => "S".into(),
        StartFootChoice::AutomaticSameOrNewLane => "A".into(),
    }
}
fn end(c: EndFootChoice, lane: i32) -> String {
    match c {
        EndFootChoice::SpecifiedLane => lane.to_string(),
        EndFootChoice::AutomaticIgnoreFollowingSteps => "I".into(),
        EndFootChoice::AutomaticNewLaneToFollowing => "N".into(),
        EndFootChoice::AutomaticSameLaneToFollowing => "S".into(),
        EndFootChoice::AutomaticSameOrNewLaneAsFollowing => "A".into(),
    }
}
fn key(entry: &Entry, column: Column) -> Result<Value, Error> {
    use Column::*;
    if column == Name {
        return Ok(Value::Text(entry.source_name().into()));
    }
    if column == Abbreviation {
        let display = entry
            .display
            .as_deref()
            .ok_or_else(|| super::invalid("Profile culture presentation is not ready"))?;
        let name = entry.source_name();
        let abbreviation = if name.is_empty() {
            display
        } else if display == name {
            ""
        } else {
            display
                .strip_prefix(name)
                .and_then(|s| s.strip_prefix(": "))
                .ok_or_else(|| super::invalid("Profile presentation name mismatch"))?
        };
        return Ok(Value::Text(abbreviation.into()));
    }
    match &entry.config {
        Config::Pattern(c) => match column {
            NoteType => number(c.beat_sub_division as f64),
            RepetitionLimit => number(if c.limit_same_arrows_in_a_row_per_foot {
                c.max_same_arrows_in_a_row_per_foot as f64
            } else {
                0.
            }),
            StepType => number(c.normalized_step_weights()[0]),
            StepTypeCheckPeriod => number(c.step_type_check_period as f64),
            StartingFoot => Ok(Value::Foot(
                match c.starting_foot_choice {
                    StartingFootChoice::Random => 0,
                    StartingFootChoice::Automatic => 1,
                    StartingFootChoice::Specified => 2,
                },
                if c.starting_foot_choice == StartingFootChoice::Specified {
                    c.starting_foot_specified
                } else {
                    0
                },
            )),
            StartingFooting => Ok(Value::Text(format!(
                "[{}|{}]",
                start(c.left_foot_start_choice, c.left_foot_start_lane_specified),
                start(c.right_foot_start_choice, c.right_foot_start_lane_specified)
            ))),
            EndingFooting => Ok(Value::Text(format!(
                "[{}|{}]",
                end(c.left_foot_end_choice, c.left_foot_end_lane_specified),
                end(c.right_foot_end_choice, c.right_foot_end_lane_specified)
            ))),
            _ => Err(super::invalid("Column does not belong to Pattern table")),
        },
        Config::Performed(c) => match column {
            StepSpeedMin => number(
                if c.step_tightening.speed_tightening_enabled == Some(true) {
                    1.
                } else {
                    0.
                },
            ),
            StepDistanceMin => number(enabled(
                c.step_tightening.distance_min,
                c.step_tightening.distance_tightening_enabled,
            )),
            StepStretchMin => number(enabled(
                c.step_tightening.stretch_distance_min,
                c.step_tightening.stretch_tightening_enabled,
            )),
            LateralSpeed => number(enabled(
                c.lateral_tightening.speed,
                c.lateral_tightening.enabled,
            )),
            LateralRelativeNps => number(enabled(
                c.lateral_tightening.relative_nps,
                c.lateral_tightening.enabled,
            )),
            LateralAbsoluteNps => number(enabled(
                c.lateral_tightening.absolute_nps,
                c.lateral_tightening.enabled,
            )),
            TransitionMin => number(enabled(
                c.transitions.steps_per_transition_min as f64,
                c.transitions.enabled,
            )),
            TransitionMax => number(enabled(
                c.transitions.steps_per_transition_max as f64,
                c.transitions.enabled,
            )),
            FacingInwardLimit => number(c.facing.max_inward_percentage),
            FacingOutwardLimit => number(c.facing.max_outward_percentage),
            _ => Err(super::invalid("Column does not belong to Performed table")),
        },
        Config::Expressed(_) => Err(super::invalid("Expressed table has no user sort columns")),
    }
}
impl Catalog {
    pub fn apply_source_sort(&mut self, kind: Kind, specs: &[Spec]) -> Result<(), Error> {
        if specs.is_empty() {
            return Ok(());
        }
        let mut seen = std::collections::BTreeSet::new();
        for spec in specs {
            if !columns(kind).contains(&spec.column) || !seen.insert(spec.column as u8) {
                return Err(super::invalid("Invalid or repeated profile sort column"));
            }
        }
        let entries = self.entries(kind);
        let keys = entries
            .iter()
            .map(|entry| {
                specs
                    .iter()
                    .map(|s| key(entry, s.column))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut order = (0..entries.len()).collect::<Vec<_>>();
        // Source StepSpeedMin compares ep1.TravelSpeedMinBPM on BOTH sides.
        // Preserve that source bug: when both flags are enabled the column ties.
        // For unequal flags its sign also depends on ep1's raw editor BPM, including0.
        let bpm = |index: usize| {
            self.custom_profiles
                .get(&(kind, entries[index].guid.clone()))
                .and_then(|p| p.travel_speed())
                .map_or(125, |s| s.min_bpm)
        };
        super::super::reference_sort::sort(&mut order, |a, b| {
            for (i, spec) in specs.iter().enumerate() {
                let cmp = if spec.column == Column::StepSpeedMin {
                    let a_on = matches!(keys[*a][i],Value::Number(v)if v!=0.);
                    let b_on = matches!(keys[*b][i],Value::Number(v)if v!=0.);
                    (if a_on { bpm(*a) } else { 0 }).cmp(&(if b_on { bpm(*a) } else { 0 }))
                } else {
                    keys[*a][i].compare(&keys[*b][i])
                };
                if cmp != Ordering::Equal {
                    return if spec.descending { cmp.reverse() } else { cmp };
                }
            }
            entries[*a].guid.cmp(&entries[*b].guid)
        });
        let sorted = order.into_iter().map(|i| entries[i].clone()).collect();
        self.entries.insert(kind, sorted);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        generation::metadata::Guid,
        history::Session,
    };
    fn entry(id: u8, name: &str, config: Config) -> Entry {
        Entry {
            guid: format!("00000000-0000-0000-0000-{id:012}").parse().unwrap(),
            name: Some(name.into()),
            builtin: None,
            config,
            display: Some(format!("{name}: summary")),
        }
    }
    fn catalog(kind: Kind, entries: Vec<Entry>) -> Catalog {
        let session = Session::new(
            Document::parse(
                b"#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
                Format::Ssc,
            )
            .unwrap(),
        );
        Catalog {
            token: session.revision_token(),
            context: crate::generation::profile_catalog::Context::Chart(0),
            entries: std::collections::BTreeMap::from([(kind, entries)]),
            dependencies: Default::default(),
            custom_profiles: Default::default(),
            issues: Vec::new(),
        }
    }
    fn ids(catalog: &Catalog, kind: Kind) -> Vec<Guid> {
        catalog
            .entries(kind)
            .iter()
            .map(|e| e.guid.clone())
            .collect()
    }
    #[test]
    fn explicit_name_uses_utf16_ordinal_and_guid_tie_even_descending_and_empty_specs_leave_initial_order()
     {
        let config = Config::Pattern(PatternConfig::sixteenths());
        let mut c = catalog(
            Kind::Pattern,
            vec![
                entry(3, "\u{e000}", config.clone()),
                entry(2, "\u{10000}", config.clone()),
                entry(1, "\u{10000}", config),
            ],
        );
        let initial = ids(&c, Kind::Pattern);
        c.apply_source_sort(Kind::Pattern, &[]).unwrap();
        assert_eq!(ids(&c, Kind::Pattern), initial);
        c.apply_source_sort(
            Kind::Pattern,
            &[Spec {
                column: Column::Name,
                descending: false,
            }],
        )
        .unwrap();
        assert_eq!(
            ids(&c, Kind::Pattern),
            vec![initial[2].clone(), initial[1].clone(), initial[0].clone()]
        );
        c.apply_source_sort(
            Kind::Pattern,
            &[Spec {
                column: Column::Name,
                descending: true,
            }],
        )
        .unwrap();
        assert_eq!(
            ids(&c, Kind::Pattern),
            vec![initial[0].clone(), initial[2].clone(), initial[1].clone()]
        );
    }
    #[test]
    fn pattern_normalized_weight_and_multicolumn_precedence_match_source_not_raw_weight() {
        let mut a = PatternConfig::sixteenths();
        a.same_arrow_step_weight = 2;
        a.new_arrow_step_weight = 2;
        a.step_type_check_period = 4;
        let mut b = a.clone();
        b.same_arrow_step_weight = 9;
        b.new_arrow_step_weight = 9;
        b.step_type_check_period = 2;
        let mut c = catalog(
            Kind::Pattern,
            vec![
                entry(1, "a", Config::Pattern(a)),
                entry(2, "b", Config::Pattern(b)),
            ],
        );
        let initial = ids(&c, Kind::Pattern);
        c.apply_source_sort(
            Kind::Pattern,
            &[
                Spec {
                    column: Column::StepType,
                    descending: false,
                },
                Spec {
                    column: Column::StepTypeCheckPeriod,
                    descending: false,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            ids(&c, Kind::Pattern),
            vec![initial[1].clone(), initial[0].clone()]
        );
        let before = ids(&c, Kind::Pattern);
        assert!(
            c.apply_source_sort(
                Kind::Pattern,
                &[
                    Spec {
                        column: Column::Name,
                        descending: false
                    },
                    Spec {
                        column: Column::Name,
                        descending: true
                    }
                ]
            )
            .is_err()
        );
        assert_eq!(ids(&c, Kind::Pattern), before);
    }
    #[test]
    fn performed_speed_preserves_source_ep1_bug_and_disabled_values_are_zero() {
        let mut a = PerformedConfig::balanced();
        a.step_tightening.speed_tightening_enabled = Some(true);
        a.step_tightening.distance_min = 9.;
        a.step_tightening.distance_tightening_enabled = None;
        let mut b = a.clone();
        b.step_tightening.speed_min_time_seconds = 0.01;
        b.step_tightening.distance_tightening_enabled = Some(true);
        b.step_tightening.distance_min = 1.;
        let mut c = catalog(
            Kind::Performed,
            vec![
                entry(2, "b", Config::Performed(b)),
                entry(1, "a", Config::Performed(a)),
            ],
        );
        let initial = ids(&c, Kind::Performed);
        c.apply_source_sort(
            Kind::Performed,
            &[Spec {
                column: Column::StepSpeedMin,
                descending: true,
            }],
        )
        .unwrap();
        assert_eq!(
            ids(&c, Kind::Performed),
            vec![initial[1].clone(), initial[0].clone()],
            "both enabled tie even with distinct speed: source ep1 appears twice"
        );
        c.apply_source_sort(
            Kind::Performed,
            &[Spec {
                column: Column::StepDistanceMin,
                descending: false,
            }],
        )
        .unwrap();
        assert_eq!(ids(&c, Kind::Performed)[0], initial[1]);
    }
    #[test]
    fn every_source_column_is_admitted_and_sort_is_presentation_only() {
        for (kind, config) in [
            (Kind::Pattern, Config::Pattern(PatternConfig::sixteenths())),
            (
                Kind::Performed,
                Config::Performed(PerformedConfig::balanced()),
            ),
        ] {
            let mut c = catalog(
                kind,
                vec![entry(2, "b", config.clone()), entry(1, "a", config)],
            );
            let before = c
                .entries(kind)
                .iter()
                .map(|entry| (entry.guid.clone(), entry.config.json().unwrap()))
                .collect::<std::collections::BTreeMap<_, _>>();
            for column in columns(kind) {
                for descending in [false, true] {
                    c.apply_source_sort(
                        kind,
                        &[Spec {
                            column: *column,
                            descending,
                        }],
                    )
                    .unwrap();
                }
            }
            assert_eq!(
                c.entries(kind)
                    .iter()
                    .map(|entry| (entry.guid.clone(), entry.config.json().unwrap()))
                    .collect::<std::collections::BTreeMap<_, _>>(),
                before
            );
            assert!(c.dependencies.is_empty());
            assert!(c.custom_profiles.is_empty());
        }
    }
}

#[cfg(test)]
mod source_oracle_tests;
