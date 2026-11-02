//! Pinned source inspector enable/visibility rules. These inspect raw draft
//! values: a nullable enable flag is false here, not inherited from a preset.
use super::{Editor, Kind};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Active,
    Disabled,
    Hidden,
}
impl Editor {
    pub fn availability(&self, index: usize) -> Option<Availability> {
        let field = self.fields.get(index)?;
        let enabled = |path: &str| {
            self.value
                .pointer(path)
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        };
        let choice = |path: &str, value: &str| {
            self.value.pointer(path).and_then(serde_json::Value::as_str) == Some(value)
        };
        let active = match (self.kind, field.path.as_slice()) {
            (Kind::Expressed, [name]) if name == "DefaultBracketParsingMethod" => {
                choice("/BracketParsingDetermination", "useDefaultMethod")
            }
            (Kind::Expressed, [name])
                if matches!(
                    name.as_str(),
                    "MinLevelForBrackets"
                        | "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets"
                        | "BalancedBracketsPerMinuteForAggressiveBrackets"
                        | "BalancedBracketsPerMinuteForNoBrackets"
                ) =>
            {
                !choice("/BracketParsingDetermination", "useDefaultMethod")
            }
            (Kind::Pattern, [name]) if name == "MaxSameArrowsInARowPerFoot" => {
                enabled("/LimitSameArrowsInARowPerFoot")
            }
            (Kind::Pattern, [name]) if name == "StartingFootSpecified" => {
                return Some(if choice("/StartingFootChoice", "specified") {
                    Availability::Active
                } else {
                    Availability::Hidden
                });
            }
            (Kind::Pattern, [name])
                if matches!(
                    name.as_str(),
                    "LeftFootStartLaneSpecified"
                        | "RightFootStartLaneSpecified"
                        | "LeftFootEndLaneSpecified"
                        | "RightFootEndLaneSpecified"
                ) =>
            {
                let selector = format!("/{}Choice", name.trim_end_matches("LaneSpecified"));
                return Some(if choice(&selector, "specifiedLane") {
                    Availability::Active
                } else {
                    Availability::Hidden
                });
            }
            (Kind::Performed, [group, name])
                if group == "StepTightening"
                    && matches!(name.as_str(), "StretchDistanceMin" | "StretchDistanceMax") =>
            {
                enabled("/StepTightening/StretchTighteningEnabled")
            }
            (Kind::Performed, [group, name])
                if group == "StepTightening"
                    && matches!(name.as_str(), "DistanceMin" | "DistanceMax") =>
            {
                enabled("/StepTightening/DistanceTighteningEnabled")
            }
            (Kind::Performed, [group, name])
                if group == "StepTightening"
                    && matches!(
                        name.as_str(),
                        "SpeedMinTimeSeconds"
                            | "SpeedMaxTimeSeconds"
                            | "SpeedTighteningMinDistance"
                    ) =>
            {
                self.source_travel_speed_enabled()
            }
            (Kind::Performed, [group, name])
                if group == "LateralTightening" && name != "Enabled" =>
            {
                enabled("/LateralTightening/Enabled")
            }
            (Kind::Performed, [group, name]) if group == "Transitions" && name != "Enabled" => {
                enabled("/Transitions/Enabled")
            }
            _ => true,
        };
        Some(if active {
            Availability::Active
        } else {
            Availability::Disabled
        })
    }
    pub fn source_travel_speed_enabled(&self) -> bool {
        self.kind == Kind::Performed
            && self
                .value
                .pointer("/StepTightening/SpeedTighteningEnabled")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::profiles::Config;
    fn editor(kind: Kind) -> Editor {
        Editor::new(&Config::editor_defaults(kind)).unwrap()
    }
    fn set(editor: &mut Editor, path: &str, value: serde_json::Value) {
        *editor.value.pointer_mut(path).unwrap() = value;
    }
    fn state(editor: &Editor, path: &str) -> Availability {
        let index = editor
            .fields
            .iter()
            .position(|field| field.pointer == path)
            .unwrap();
        editor.availability(index).unwrap()
    }
    #[test]
    fn source_expressed_default_and_dynamic_branches_gate_their_actual_fields() {
        let mut editor = editor(Kind::Expressed);
        for (default, mode) in [
            (true, "useDefaultMethod"),
            (false, "chooseMethodDynamically"),
        ] {
            set(
                &mut editor,
                "/BracketParsingDetermination",
                serde_json::json!(mode),
            );
            assert_eq!(
                state(&editor, "/DefaultBracketParsingMethod"),
                if default {
                    Availability::Active
                } else {
                    Availability::Disabled
                }
            );
            for name in [
                "MinLevelForBrackets",
                "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets",
                "BalancedBracketsPerMinuteForAggressiveBrackets",
                "BalancedBracketsPerMinuteForNoBrackets",
            ] {
                assert_eq!(
                    state(&editor, &format!("/{name}")),
                    if default {
                        Availability::Disabled
                    } else {
                        Availability::Active
                    }
                );
            }
            assert_eq!(
                state(&editor, "/BracketParsingDetermination"),
                Availability::Active
            );
        }
    }
    #[test]
    fn source_nullable_enable_methods_treat_null_and_false_as_disabled_without_inheritance() {
        let groups = [
            (
                "StepTightening",
                "StretchTighteningEnabled",
                vec!["StretchDistanceMin", "StretchDistanceMax"],
            ),
            (
                "StepTightening",
                "DistanceTighteningEnabled",
                vec!["DistanceMin", "DistanceMax"],
            ),
            (
                "StepTightening",
                "SpeedTighteningEnabled",
                vec![
                    "SpeedMinTimeSeconds",
                    "SpeedMaxTimeSeconds",
                    "SpeedTighteningMinDistance",
                ],
            ),
            (
                "LateralTightening",
                "Enabled",
                vec!["Speed", "RelativeNPS", "AbsoluteNPS"],
            ),
            (
                "Transitions",
                "Enabled",
                vec![
                    "StepsPerTransitionMin",
                    "StepsPerTransitionMax",
                    "MinimumPadWidth",
                    "TransitionCutoffPercentage",
                ],
            ),
        ];
        for (group, flag, fields) in groups {
            let mut editor = editor(Kind::Performed);
            for value in [
                serde_json::Value::Null,
                serde_json::json!(false),
                serde_json::json!(true),
            ] {
                set(&mut editor, &format!("/{group}/{flag}"), value.clone());
                let enabled = value == true;
                for field in &fields {
                    assert_eq!(
                        state(&editor, &format!("/{group}/{field}")),
                        if enabled {
                            Availability::Active
                        } else {
                            Availability::Disabled
                        },
                        "{group}/{field}: {value}"
                    );
                }
                assert_eq!(
                    state(&editor, &format!("/{group}/{flag}")),
                    Availability::Active
                );
                if flag == "SpeedTighteningEnabled" {
                    assert_eq!(editor.source_travel_speed_enabled(), enabled);
                }
            }
        }
    }
    #[test]
    fn source_pattern_specified_controls_are_hidden_and_repetition_value_is_disabled() {
        let mut editor = editor(Kind::Pattern);
        for (value, visible) in [("random", false), ("automatic", false), ("specified", true)] {
            set(&mut editor, "/StartingFootChoice", serde_json::json!(value));
            assert_eq!(
                state(&editor, "/StartingFootSpecified"),
                if visible {
                    Availability::Active
                } else {
                    Availability::Hidden
                }
            );
        }
        for foot in ["LeftFoot", "RightFoot"] {
            for end in ["Start", "End"] {
                let selector = format!("/{foot}{end}Choice");
                let field = format!("/{foot}{end}LaneSpecified");
                let choices = if end == "Start" {
                    vec![
                        "automaticSameLane",
                        "automaticNewLane",
                        "automaticSameOrNewLane",
                        "specifiedLane",
                    ]
                } else {
                    vec![
                        "automaticIgnoreFollowingSteps",
                        "automaticSameLaneToFollowing",
                        "automaticNewLaneToFollowing",
                        "automaticSameOrNewLaneAsFollowing",
                        "specifiedLane",
                    ]
                };
                for choice in choices {
                    set(&mut editor, &selector, serde_json::json!(choice));
                    assert_eq!(
                        state(&editor, &field),
                        if choice == "specifiedLane" {
                            Availability::Active
                        } else {
                            Availability::Hidden
                        }
                    );
                }
            }
        }
        for value in [false, true] {
            set(
                &mut editor,
                "/LimitSameArrowsInARowPerFoot",
                serde_json::json!(value),
            );
            assert_eq!(
                state(&editor, "/MaxSameArrowsInARowPerFoot"),
                if value {
                    Availability::Active
                } else {
                    Availability::Disabled
                }
            );
        }
    }
    #[test]
    fn gate_reads_do_not_clamp_or_delete_dormant_imported_values_and_raw_api_remains_raw() {
        let mut editor = editor(Kind::Pattern);
        set(
            &mut editor,
            "/LimitSameArrowsInARowPerFoot",
            serde_json::json!(false),
        );
        set(
            &mut editor,
            "/StartingFootChoice",
            serde_json::json!("automatic"),
        );
        let repeat = editor
            .fields
            .iter()
            .position(|field| field.name() == "MaxSameArrowsInARowPerFoot")
            .unwrap();
        editor.set_text(repeat, "999").unwrap();
        let foot = editor
            .fields
            .iter()
            .position(|field| field.name() == "StartingFootSpecified")
            .unwrap();
        let before = editor.value.clone();
        assert_eq!(editor.availability(repeat), Some(Availability::Disabled));
        assert_eq!(editor.availability(foot), Some(Availability::Hidden));
        assert_eq!(editor.value, before);
        assert_eq!(editor.value(repeat).unwrap().as_i64(), Some(999));
        set(
            &mut editor,
            "/LimitSameArrowsInARowPerFoot",
            serde_json::json!(true),
        );
        assert_eq!(editor.value(repeat).unwrap().as_i64(), Some(999));
    }
}
