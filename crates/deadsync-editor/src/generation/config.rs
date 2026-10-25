//! GrooveAuthor generation configurations. All underlying library options are
//! retained; the native UI may disclose their advanced sections progressively.
//! Ported from StepManiaLibrary bdbc427f (MIT), *Config.cs and GrooveAuthor
//! 48e18f71 AutogenConfig/Editor*Config.cs for the named editor presets.
use crate::{Error, notes::layout_columns};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! configuration {
    ($name:ident { $($field:ident : $ty:ty = $default:expr => $json:literal),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name { $(#[serde(rename = $json)] pub $field: $ty),* }
        impl Default for $name { fn default() -> Self { Self { $($field: $default),* } } }
    };
}

macro_rules! choices {
    ($name:ident, $default:ident, [$($variant:ident),+ $(,)?]) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { $($variant),+ }
        impl Default for $name { fn default() -> Self { Self::$default } }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Value { Name(String), Number(usize) }
                let variants = [$(Self::$variant),+];
                match Value::deserialize(deserializer)? {
                    Value::Number(number) => variants.get(number).copied(),
                    Value::Name(name) => variants.into_iter().find(|choice| format!("{choice:?}").eq_ignore_ascii_case(&name)),
                }.ok_or_else(|| serde::de::Error::custom(concat!("Unknown ", stringify!($name))))
            }
        }
    };
}

choices!(BracketMethod, Balanced, [Aggressive, Balanced, NoBrackets]);
choices!(
    BracketDetermination,
    ChooseMethodDynamically,
    [ChooseMethodDynamically, UseDefaultMethod]
);
choices!(
    StartFootChoice,
    AutomaticSameLane,
    [
        AutomaticSameLane,
        AutomaticNewLane,
        AutomaticSameOrNewLane,
        SpecifiedLane
    ]
);
choices!(
    EndFootChoice,
    AutomaticIgnoreFollowingSteps,
    [
        AutomaticIgnoreFollowingSteps,
        AutomaticSameLaneToFollowing,
        AutomaticNewLaneToFollowing,
        AutomaticSameOrNewLaneAsFollowing,
        SpecifiedLane
    ]
);
choices!(StartingFootChoice, Random, [Random, Automatic, Specified]);

configuration!(ExpressedConfig {
    default_bracket_parsing_method: BracketMethod = BracketMethod::Balanced => "DefaultBracketParsingMethod",
    bracket_parsing_determination: BracketDetermination = BracketDetermination::ChooseMethodDynamically => "BracketParsingDetermination",
    min_level_for_brackets: i32 = 0 => "MinLevelForBrackets",
    aggressive_when_uncoverable: bool = false => "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets",
    balanced_bpm_for_aggressive: f64 = 0.0 => "BalancedBracketsPerMinuteForAggressiveBrackets",
    balanced_bpm_for_no_brackets: f64 = 0.0 => "BalancedBracketsPerMinuteForNoBrackets",
});

configuration!(FacingConfig {
    max_inward_percentage: f64 = -1.0 => "MaxInwardPercentage",
    inward_percentage_cutoff: f64 = -1.0 => "InwardPercentageCutoff",
    max_outward_percentage: f64 = -1.0 => "MaxOutwardPercentage",
    outward_percentage_cutoff: f64 = -1.0 => "OutwardPercentageCutoff",
});

configuration!(LateralConfig {
    enabled: Option<bool> = None => "Enabled",
    relative_nps: f64 = -1.0 => "RelativeNPS",
    absolute_nps: f64 = -1.0 => "AbsoluteNPS",
    speed: f64 = -1.0 => "Speed",
});

configuration!(StepConfig {
    speed_tightening_enabled: Option<bool> = None => "SpeedTighteningEnabled",
    speed_min_time_seconds: f64 = -1.0 => "SpeedMinTimeSeconds",
    speed_max_time_seconds: f64 = -1.0 => "SpeedMaxTimeSeconds",
    speed_tightening_min_distance: f64 = -1.0 => "SpeedTighteningMinDistance",
    distance_tightening_enabled: Option<bool> = None => "DistanceTighteningEnabled",
    distance_min: f64 = -1.0 => "DistanceMin",
    distance_max: f64 = -1.0 => "DistanceMax",
    stretch_tightening_enabled: Option<bool> = None => "StretchTighteningEnabled",
    stretch_distance_min: f64 = -1.0 => "StretchDistanceMin",
    stretch_distance_max: f64 = -1.0 => "StretchDistanceMax",
    lateral_min_panel_distance: f64 = -100.0 => "LateralMinPanelDistance",
    longitudinal_min_panel_distance: f64 = -100.0 => "LongitudinalMinPanelDistance",
});

configuration!(TransitionConfig {
    enabled: Option<bool> = None => "Enabled",
    steps_per_transition_min: i32 = -1 => "StepsPerTransitionMin",
    steps_per_transition_max: i32 = -1 => "StepsPerTransitionMax",
    minimum_pad_width: i32 = -1 => "MinimumPadWidth",
    transition_cutoff_percentage: f64 = -1.0 => "TransitionCutoffPercentage",
});

configuration!(PerformedConfig {
    transitions: TransitionConfig = TransitionConfig::default() => "Transitions",
    facing: FacingConfig = FacingConfig::default() => "Facing",
    lateral_tightening: LateralConfig = LateralConfig::default() => "LateralTightening",
    step_tightening: StepConfig = StepConfig::default() => "StepTightening",
    arrow_weights: BTreeMap<String, Vec<i32>> = BTreeMap::new() => "ArrowWeights",
});

configuration!(PatternConfig {
    beat_sub_division: u32 = 4 => "BeatSubDivision",
    starting_foot_choice: StartingFootChoice = StartingFootChoice::Random => "StartingFootChoice",
    starting_foot_specified: i32 = 0 => "StartingFootSpecified",
    left_foot_start_choice: StartFootChoice = StartFootChoice::AutomaticSameLane => "LeftFootStartChoice",
    left_foot_start_lane_specified: i32 = 0 => "LeftFootStartLaneSpecified",
    left_foot_end_choice: EndFootChoice = EndFootChoice::AutomaticIgnoreFollowingSteps => "LeftFootEndChoice",
    left_foot_end_lane_specified: i32 = 0 => "LeftFootEndLaneSpecified",
    right_foot_start_choice: StartFootChoice = StartFootChoice::AutomaticSameLane => "RightFootStartChoice",
    right_foot_start_lane_specified: i32 = 0 => "RightFootStartLaneSpecified",
    right_foot_end_choice: EndFootChoice = EndFootChoice::AutomaticIgnoreFollowingSteps => "RightFootEndChoice",
    right_foot_end_lane_specified: i32 = 0 => "RightFootEndLaneSpecified",
    same_arrow_step_weight: i32 = 0 => "SameArrowStepWeight",
    new_arrow_step_weight: i32 = 0 => "NewArrowStepWeight",
    step_type_check_period: i32 = 0 => "StepTypeCheckPeriod",
    limit_same_arrows_in_a_row_per_foot: bool = false => "LimitSameArrowsInARowPerFoot",
    max_same_arrows_in_a_row_per_foot: i32 = 0 => "MaxSameArrowsInARowPerFoot",
});

impl ExpressedConfig {
    pub fn validate(&self) -> Result<(), Error> {
        nonnegative(
            "BalancedBracketsPerMinuteForAggressiveBrackets",
            self.balanced_bpm_for_aggressive,
        )?;
        nonnegative(
            "BalancedBracketsPerMinuteForNoBrackets",
            self.balanced_bpm_for_no_brackets,
        )?;
        if self.balanced_bpm_for_aggressive != 0.0
            && self.balanced_bpm_for_no_brackets != 0.0
            && self.balanced_bpm_for_aggressive <= self.balanced_bpm_for_no_brackets
        {
            return invalid("Aggressive bracket threshold must exceed the no-bracket threshold");
        }
        Ok(())
    }
}

impl PerformedConfig {
    pub fn initialize(&mut self) {
        if self.step_tightening.lateral_min_panel_distance == -100.0 {
            self.step_tightening.lateral_min_panel_distance = 0.5;
        }
        if self.step_tightening.longitudinal_min_panel_distance == -100.0 {
            self.step_tightening.longitudinal_min_panel_distance = 0.5;
        }
    }

    /// Retain the reference override sentinels before initialization. As in
    /// Config.SetAsOverrideOf, transition policy is not inherited implicitly.
    pub fn override_of(&mut self, base: &Self) {
        macro_rules! inherit {
            ($group:ident, $sentinel:expr, $($field:ident),+ $(,)?) => { $(if self.$group.$field == $sentinel { self.$group.$field = base.$group.$field; })+ };
        }
        inherit!(
            facing,
            -1.0,
            max_inward_percentage,
            inward_percentage_cutoff,
            max_outward_percentage,
            outward_percentage_cutoff
        );
        inherit!(lateral_tightening, None, enabled);
        inherit!(lateral_tightening, -1.0, relative_nps, absolute_nps, speed);
        inherit!(
            step_tightening,
            None,
            speed_tightening_enabled,
            distance_tightening_enabled,
            stretch_tightening_enabled
        );
        inherit!(
            step_tightening,
            -1.0,
            speed_min_time_seconds,
            speed_max_time_seconds,
            speed_tightening_min_distance,
            distance_min,
            distance_max,
            stretch_distance_min,
            stretch_distance_max
        );
        inherit!(
            step_tightening,
            -100.0,
            lateral_min_panel_distance,
            longitudinal_min_panel_distance
        );
        for (layout, weights) in &base.arrow_weights {
            self.arrow_weights
                .entry(layout.clone())
                .or_insert_with(|| weights.clone());
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        for (name, value) in [
            ("MaxInwardPercentage", self.facing.max_inward_percentage),
            (
                "InwardPercentageCutoff",
                self.facing.inward_percentage_cutoff,
            ),
            ("MaxOutwardPercentage", self.facing.max_outward_percentage),
            (
                "OutwardPercentageCutoff",
                self.facing.outward_percentage_cutoff,
            ),
        ] {
            percentage(name, value)?;
        }
        let step = &self.step_tightening;
        if step.speed_tightening_enabled == Some(true) {
            interval(
                "Speed time",
                step.speed_min_time_seconds,
                step.speed_max_time_seconds,
            )?;
            nonnegative(
                "SpeedTighteningMinDistance",
                step.speed_tightening_min_distance,
            )?;
        }
        if step.distance_tightening_enabled == Some(true) {
            interval("Distance", step.distance_min, step.distance_max)?;
        }
        if step.stretch_tightening_enabled == Some(true) {
            interval(
                "Stretch distance",
                step.stretch_distance_min,
                step.stretch_distance_max,
            )?;
        }
        if !step.lateral_min_panel_distance.is_finite()
            || !step.longitudinal_min_panel_distance.is_finite()
        {
            return invalid("Panel compensation must be finite");
        }
        if self.lateral_tightening.enabled == Some(true) {
            nonnegative("RelativeNPS", self.lateral_tightening.relative_nps)?;
            nonnegative("AbsoluteNPS", self.lateral_tightening.absolute_nps)?;
            nonnegative("Speed", self.lateral_tightening.speed)?;
        }
        if self.transitions.enabled == Some(true) {
            if self.transitions.steps_per_transition_min < 0
                || self.transitions.minimum_pad_width < 0
            {
                return invalid("Transition counts and width must be non-negative");
            }
            percentage(
                "TransitionCutoffPercentage",
                self.transitions.transition_cutoff_percentage,
            )?;
        }
        Ok(())
    }

    pub fn normalized_weights(&self, layout: &str) -> Result<Vec<f64>, Error> {
        let weights = self
            .arrow_weights
            .get(layout)
            .ok_or_else(|| Error::InvalidEdit(format!("No arrow weights for {layout}")))?;
        let columns = layout_columns(layout)
            .ok_or_else(|| Error::Unsupported(format!("Unknown generator layout {layout}")))?;
        if weights.len() != usize::from(columns) || weights.iter().any(|&weight| weight < 0) {
            return invalid("Arrow weights must be non-negative and match the lane count");
        }
        let sum: i64 = weights.iter().map(|&weight| i64::from(weight)).sum();
        if sum <= 0 {
            return invalid("At least one arrow weight must be positive");
        }
        Ok(weights
            .iter()
            .map(|&weight| f64::from(weight) / sum as f64)
            .collect())
    }

    pub fn balanced() -> Self {
        let weights: &[(&str, &[i32])] = &[
            ("dance-single", &[25, 25, 25, 25]),
            ("dance-double", &[6, 12, 10, 22, 22, 12, 10, 6]),
            ("dance-solo", &[13, 12, 25, 25, 12, 13]),
            ("dance-threepanel", &[25, 50, 25]),
            ("pump-single", &[17, 16, 34, 16, 17]),
            ("pump-halfdouble", &[25, 12, 13, 13, 12, 25]),
            ("pump-double", &[4, 4, 17, 12, 13, 13, 12, 17, 4, 4]),
            ("smx-beginner", &[25, 50, 25]),
            ("smx-single", &[25, 21, 8, 21, 25]),
            ("smx-dual", &[8, 17, 25, 25, 17, 8]),
            ("smx-full", &[6, 8, 7, 8, 22, 22, 8, 7, 8, 6]),
        ];
        Self {
            facing: FacingConfig {
                max_inward_percentage: 1.0,
                inward_percentage_cutoff: 0.34,
                max_outward_percentage: 1.0,
                outward_percentage_cutoff: 0.34,
            },
            lateral_tightening: LateralConfig {
                enabled: Some(true),
                relative_nps: 1.65,
                absolute_nps: 12.0,
                speed: 3.0,
            },
            step_tightening: StepConfig {
                speed_tightening_enabled: Some(true),
                speed_min_time_seconds: 120.0 / (4.0 * 170.0),
                speed_max_time_seconds: 120.0 / (4.0 * 125.0),
                speed_tightening_min_distance: 0.0,
                distance_tightening_enabled: Some(true),
                distance_min: 1.4,
                distance_max: 2.333333,
                stretch_tightening_enabled: Some(true),
                stretch_distance_min: 2.333333,
                stretch_distance_max: 3.333333,
                lateral_min_panel_distance: 0.166667,
                longitudinal_min_panel_distance: -0.125,
            },
            transitions: TransitionConfig {
                enabled: Some(false),
                steps_per_transition_min: 0,
                steps_per_transition_max: 1024,
                minimum_pad_width: 5,
                transition_cutoff_percentage: 0.5,
            },
            arrow_weights: weights
                .iter()
                .map(|&(name, weights)| (name.to_owned(), weights.to_vec()))
                .collect(),
        }
    }

    pub fn preset(preset: PerformedPreset) -> Self {
        let mut config = Self::balanced();
        match preset {
            PerformedPreset::Balanced => {}
            PerformedPreset::StaminaSingle => {
                config.step_tightening.speed_tightening_enabled = Some(false);
                config.step_tightening.distance_tightening_enabled = Some(false);
                config.step_tightening.stretch_tightening_enabled = Some(false);
                config.lateral_tightening.enabled = Some(false);
                config.transitions.enabled = Some(false);
            }
            PerformedPreset::StaminaDouble => {
                config.step_tightening.speed_max_time_seconds = 120.0 / (4.0 * 99.0);
                config.transitions.enabled = Some(true);
                config.transitions.steps_per_transition_min = 16;
                config.lateral_tightening.absolute_nps = 26.666667;
            }
            PerformedPreset::PatternBalanced
            | PerformedPreset::PatternNoCandles
            | PerformedPreset::PatternNoInward
            | PerformedPreset::PatternNoTransitionLimit => {
                config.transitions.enabled =
                    Some(preset != PerformedPreset::PatternNoTransitionLimit);
                if preset != PerformedPreset::PatternNoTransitionLimit {
                    config.transitions.steps_per_transition_min = 16;
                }
                config.lateral_tightening.enabled = Some(false);
                config.step_tightening.speed_tightening_enabled = Some(false);
                config.step_tightening.distance_tightening_enabled = Some(true);
                config.step_tightening.distance_min = if preset == PerformedPreset::PatternNoCandles
                {
                    0.74
                } else {
                    1.2
                };
                if preset == PerformedPreset::PatternNoInward {
                    config.facing.max_inward_percentage = 0.0;
                }
            }
        }
        config
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerformedPreset {
    Balanced,
    StaminaSingle,
    StaminaDouble,
    PatternBalanced,
    PatternNoCandles,
    PatternNoInward,
    PatternNoTransitionLimit,
}

impl PatternConfig {
    pub fn sixteenths() -> Self {
        Self {
            starting_foot_choice: StartingFootChoice::Automatic,
            left_foot_start_choice: StartFootChoice::AutomaticSameOrNewLane,
            right_foot_start_choice: StartFootChoice::AutomaticSameOrNewLane,
            left_foot_end_choice: EndFootChoice::AutomaticSameOrNewLaneAsFollowing,
            right_foot_end_choice: EndFootChoice::AutomaticSameOrNewLaneAsFollowing,
            same_arrow_step_weight: 25,
            new_arrow_step_weight: 75,
            step_type_check_period: 16,
            limit_same_arrows_in_a_row_per_foot: true,
            max_same_arrows_in_a_row_per_foot: 3,
            ..Self::default()
        }
    }
    pub fn eighths() -> Self {
        Self {
            beat_sub_division: 2,
            ..Self::sixteenths()
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if ![1, 2, 3, 4, 6, 8, 12, 16, 48].contains(&self.beat_sub_division) {
            return invalid("Invalid beat subdivision");
        }
        if self.same_arrow_step_weight < 0 || self.new_arrow_step_weight < 0 {
            return invalid("Pattern step weights must be non-negative");
        }
        if ![0, 1].contains(&self.starting_foot_specified) {
            return invalid("Starting foot must be 0 (left) or 1 (right)");
        }
        if [
            self.left_foot_start_lane_specified,
            self.right_foot_start_lane_specified,
            self.left_foot_end_lane_specified,
            self.right_foot_end_lane_specified,
        ]
        .iter()
        .any(|&lane| lane < 0)
        {
            return invalid("Specified pattern lanes must be non-negative");
        }
        Ok(())
    }
    pub fn normalized_step_weights(&self) -> [f64; 2] {
        let total = i64::from(self.same_arrow_step_weight) + i64::from(self.new_arrow_step_weight);
        if total > 0 {
            [
                f64::from(self.same_arrow_step_weight) / total as f64,
                f64::from(self.new_arrow_step_weight) / total as f64,
            ]
        } else {
            [0.0, 0.0]
        }
    }
}

fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.to_owned()))
}
fn nonnegative(name: &str, value: f64) -> Result<(), Error> {
    if !value.is_finite() || value < 0.0 {
        return invalid(&format!("{name} must be finite and non-negative"));
    }
    Ok(())
}
fn percentage(name: &str, value: f64) -> Result<(), Error> {
    nonnegative(name, value)?;
    if value > 1.0 {
        return invalid(&format!("{name} must be at most 1"));
    }
    Ok(())
}
fn interval(name: &str, minimum: f64, maximum: f64) -> Result<(), Error> {
    nonnegative(name, minimum)?;
    nonnegative(name, maximum)?;
    if minimum > maximum {
        return invalid(&format!("{name} minimum exceeds maximum"));
    }
    Ok(())
}
