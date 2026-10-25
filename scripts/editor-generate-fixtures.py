#!/usr/bin/env python3
"""Generate fixtures with pinned libraries and unchanged GrooveAuthor helpers.

The .NET tool is development-only. Committed expected files let ordinary Rust
tests run without .NET, network access, or the source reference checkout.
"""
import argparse
import copy
import json
import hashlib
import re
import os
from pathlib import Path
import subprocess

PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
EDITOR_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
ROOT = Path(__file__).resolve().parents[1]


def extract_member(source, signature):
    match = re.search(signature, source)
    if match is None: raise SystemExit(f"Missing original member {signature}")
    start = source.index("{", match.start())
    level, end = 1, start + 1
    while level:
        if source[end] == "{": level += 1
        elif source[end] == "}": level -= 1
        end += 1
    return source[match.start():end]


def prepare_timing_reference(editor):
    def read(path):
        return subprocess.check_output(["git", "show", f"{EDITOR_PIN}:StepManiaEditor/{path}"], cwd=editor).decode("utf-8-sig")
    chart = read("EditorChart.cs")
    rate = read("EditorEvents/EditorRateAlteringEvent.cs")
    stop = read("EditorEvents/EditorStopEvent.cs")
    density = read("EditorActions/ActionAutoGeneratePatterns.cs")
    deletion = read("EditorActions/ActionDeletePatternNotes.cs")
    pattern_event = read("EditorEvents/EditorPatternEvent.cs")
    groups = {
        "CHART_METHODS": [extract_member(chart, r"\tprivate void " + name + r"\(") for name in ["RefreshEventTimingData", "RefreshRateAlteringEvents"]]
            + [extract_member(chart, r"\tpublic double " + name + r"\(") for name in ["GetStartChartTime", "GetEndChartTime"]]
            + [extract_member(chart, r"\tpublic bool TryGetTimeFromChartPosition\(")],
        "RATE_METHODS": [extract_member(rate, r"\tpublic double " + name + r"\(") for name in ["GetChartTimeFromPosition", "GetChartPositionFromTime"]],
        "STOP_METHODS": [extract_member(stop, r"\tpublic void RefreshEndChartPosition\(")],
        "FAKE_METHOD": [extract_member(chart,r"\tpublic bool IsEventInFake\(")],
        "REGION_METHODS": [extract_member(chart, r"\tpublic \(List<Event>, List<EditorEvent>\) GetEventsInRangeForPattern\(")],
        "PATTERN_BOUNDS": [extract_member(pattern_event, r"\tpublic int " + name + r"\(") for name in ["GetFirstStepRow", "GetLastStepRow", "GetNumSteps", "GetNumStepsBeforeRow"]],
        "DELETION_METHODS": [extract_member(deletion, r"\tpublic class Alterations\s*"), extract_member(deletion, r"\tpublic static Alterations DeleteEventsOverlappingPatterns\(")],
        "RANGE_METHODS": [extract_member(density, r"\tprivate static \(int start, int end\) GetRowRangeToConsiderForPattern\(")],
        "DENSITY_METHODS": [extract_member(density, r"\tprivate double GetNps\(")],
        "SCHEDULE_METHODS": [extract_member(density,r"\tprivate int GetTotalStepsFromAllPatterns\("),
            extract_member(density,r"\tprivate double GetLastPatternStepTime\("),
            extract_member(density,r"\tprivate double GetNps\("),
            extract_member(density,r"\tprivate static void RemoveEventsWhichOverlapNextPattern\("),
            extract_member(density,r"\tprivate static void UpdatePrecedingLaneCounts\(")],
    }
    project = ROOT / "scripts/editor-oracle"
    template = (project / "EditorTimingReference.cs.template").read_text()
    original = []
    for key, methods in groups.items():
        text = "\n\n".join(methods)
        template = template.replace("    // " + key, text)
        original.append(text)
    (project / "obj/EditorTimingReference.cs").write_text(template)
    return hashlib.sha256("\n".join(original).encode()).hexdigest()


def prepare_metadata_reference(editor):
    def read(path):
        return subprocess.check_output(["git", "show", f"{EDITOR_PIN}:StepManiaEditor/{path}"], cwd=editor).decode("utf-8-sig")
    definition=extract_member(read("EditorEvents/EditorPatternEvent.cs"),r"\tpublic class Definition\s*")
    custom=extract_member(read("EditorChart.cs"),r"\tprivate class CustomSaveDataV1\s*")
    project=ROOT/"scripts/editor-oracle"
    text=(project/"MetadataReference.cs.template").read_text().replace("// DEFINITION",definition).replace("// CUSTOM_DATA",custom)
    offset=extract_member(read("EditorChart.cs"),r"\tpublic double GetMusicOffset\(")
    text=text.replace("// MUSIC_OFFSET",offset)
    (project/"obj/MetadataReference.cs").write_text(text)
    return hashlib.sha256((definition+"\n"+custom+"\n"+offset).encode()).hexdigest()



def prepare_profiles_reference(editor):
    def read(path):
        return subprocess.check_output(["git", "show", f"{EDITOR_PIN}:StepManiaEditor/{path}"],cwd=editor).decode("utf-8-sig")
    pattern=read("AutogenConfig/EditorPatternConfig.cs");performed=read("AutogenConfig/EditorPerformedChartConfig.cs")
    pieces={
        "SUBDIVISION":extract_member(read("SubdivisionType.cs"),r"public enum SubdivisionType\s*"),
        "BEAT_METHOD":extract_member(read("Utils.cs"),r"\tpublic static int GetBeatSubdivision\("),
        "PATTERN_TYPE":extract_member(pattern,r"\tpublic SubdivisionType PatternType\s*"),
        "START_FOOT":extract_member(pattern,r"\tpublic Editor.Foot StartingFootSpecified\s*"),
        "SPEED_INDEX":extract_member(performed,r"\tpublic int TravelSpeedNoteTypeDenominatorIndex\s*"),
        "SPEED_MIN":extract_member(performed,r"\tpublic int TravelSpeedMinBPM\s*")+" = 1;",
        "SPEED_MAX":extract_member(performed,r"\tpublic int TravelSpeedMaxBPM\s*")+" = 1;",
        "UPDATE_SPEED":extract_member(performed,r"\tprivate void UpdateStepTighteningSpeed\("),
    }
    project=ROOT/"scripts/editor-oracle";text=(project/"ProfilesReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/ProfilesReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_context_reference(editor: Path):
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=editor, text=True).strip()
    if commit != EDITOR_PIN:
        raise SystemExit(f"Editor reference must be pinned at {EDITOR_PIN}, found {commit}")
    source = subprocess.check_output(["git", "show", f"{EDITOR_PIN}:StepManiaEditor/EditorActions/ActionAutoGeneratePatterns.cs"], cwd=editor).decode("utf-8-sig")
    methods = []
    for name in ["GetFollowingFooting", "GetPrecedingFootingAndTransitions", "GetPrecedingFooting", "CheckAndUpdateCurrentSteppedLanes", "CheckAndUpdateFooting"]:
        match = re.search(r"\tprivate static void " + name + r"\(", source)
        if match is None: raise SystemExit(f"Missing original context method {name}")
        start = source.index("{", match.start())
        level, end = 1, start + 1
        while level:
            if source[end] == "{": level += 1
            elif source[end] == "}": level -= 1
            end += 1
        methods.append(source[match.start():end])
    original = "\n\n".join(methods)
    project = ROOT / "scripts/editor-oracle"
    output = project / "obj/PatternContextReference.cs"
    output.parent.mkdir(parents=True, exist_ok=True)
    template = (project / "PatternContextReference.cs.template").read_text()
    output.write_text(template.replace("    // ORIGINAL_METHODS", original))
    return hashlib.sha256(original.encode()).hexdigest()


def configurations():
    # EditorPerformedChartConfig defaults, 48e18f71. These are input values;
    # expected patterns are produced only by the original C# implementation.
    performed = {
        "Facing": {"MaxInwardPercentage": 1.0, "InwardPercentageCutoff": 0.34, "MaxOutwardPercentage": 1.0, "OutwardPercentageCutoff": 0.34},
        "StepTightening": {
            "SpeedTighteningEnabled": True, "SpeedMinTimeSeconds": 120 / (4 * 170), "SpeedMaxTimeSeconds": 120 / (4 * 125),
            "SpeedTighteningMinDistance": 0, "DistanceTighteningEnabled": True, "DistanceMin": 1.4, "DistanceMax": 2.333333,
            "StretchTighteningEnabled": True, "StretchDistanceMin": 2.333333, "StretchDistanceMax": 3.333333,
            "LateralMinPanelDistance": 0.166667, "LongitudinalMinPanelDistance": -0.125,
        },
        "LateralTightening": {"Enabled": True, "RelativeNPS": 1.65, "AbsoluteNPS": 12, "Speed": 3},
        "Transitions": {"Enabled": False, "StepsPerTransitionMin": 0, "StepsPerTransitionMax": 1024, "MinimumPadWidth": 5, "TransitionCutoffPercentage": 0.5},
        "ArrowWeights": {
            "dance-single": [25, 25, 25, 25], "dance-double": [6, 12, 10, 22, 22, 12, 10, 6],
            "dance-solo": [13, 12, 25, 25, 12, 13], "dance-threepanel": [25, 50, 25],
            "pump-single": [17, 16, 34, 16, 17], "pump-halfdouble": [25, 12, 13, 13, 12, 25],
            "pump-double": [4, 4, 17, 12, 13, 13, 12, 17, 4, 4], "smx-beginner": [25, 50, 25],
            "smx-single": [25, 21, 8, 21, 25], "smx-dual": [8, 17, 25, 25, 17, 8],
            "smx-full": [6, 8, 7, 8, 22, 22, 8, 7, 8, 6],
        },
    }
    pattern = {
        "BeatSubDivision": 4, "StartingFootChoice": "automatic", "StartingFootSpecified": 0,
        "LeftFootStartChoice": "automaticSameOrNewLane", "LeftFootStartLaneSpecified": 0,
        "LeftFootEndChoice": "automaticSameOrNewLaneAsFollowing", "LeftFootEndLaneSpecified": 0,
        "RightFootStartChoice": "automaticSameOrNewLane", "RightFootStartLaneSpecified": 0,
        "RightFootEndChoice": "automaticSameOrNewLaneAsFollowing", "RightFootEndLaneSpecified": 0,
        "SameArrowStepWeight": 25, "NewArrowStepWeight": 75, "StepTypeCheckPeriod": 16,
        "LimitSameArrowsInARowPerFoot": True, "MaxSameArrowsInARowPerFoot": 3,
    }
    return performed, pattern


def requests(reference: Path):
    performed, pattern = configurations()
    def case(layout="dance-single", **values):
        definition = json.loads((reference / "StepManiaLibrary" / f"{layout}.json").read_text())
        footing = definition["StartingPositions"][0][0]
        return {"Operation": "Pattern", "Layout": layout, "Start": 0, "End": 192, "Seed": 42,
                "PreviousFoot": 1, "PreviousFooting": footing, "FollowingFooting": footing,
                "Bpms": [[0, 150]], "PerformedConfig": copy.deepcopy(performed), "PatternConfig": copy.deepcopy(pattern), **values}
    cases = {"single-balanced": case(), "single-seed-zero": case(Seed=0), "single-minimum-seed": case(Seed=-2147483648),
             "one-step": case(End=0), "double-context": case("dance-double", Start=192, End=960, Seed=31,
                 LaneCounts=[5, 9, 8, 13, 11, 7, 8, 4], PreviousTimes=[1.5, 1.6], PreviousSameCounts=[2, 1], TotalSteps=65,
                 LastTransitionSteps=48, LastTransitionLeft=True, Bpms=[[0, 180], [384, 240]], Nps=10.0),
             "pump-balanced": case("pump-single", End=384, Seed=9), "smx-balanced": case("smx-single", Seed=-7),
             "eighths-random-foot": case(Seed=19), "specified-footing": case(Seed=123, End=384), "invalid-root": case(PreviousFooting=[-1, -1])}
    cases["double-context"]["PerformedConfig"]["Transitions"].update(Enabled=True, StepsPerTransitionMin=16, StepsPerTransitionMax=32)
    cases["double-balanced"] = case("dance-double", End=960, Seed=31)
    for option in ["LaneCounts", "PreviousTimes", "PreviousSameCounts", "TotalSteps", "Bpms"]:
        isolated = case("dance-double", Start=192, End=960, Seed=31)
        isolated[option] = copy.deepcopy(cases["double-context"][option])
        cases["double-with-" + option.lower()] = isolated
    cases["double-transitions"] = case("dance-double", Start=192, End=960, Seed=31)
    cases["double-transitions"]["PerformedConfig"]["Transitions"].update(Enabled=True, StepsPerTransitionMin=16, StepsPerTransitionMax=32)
    cases["eighths-random-foot"]["PatternConfig"].update(BeatSubDivision=2, StartingFootChoice="random")
    cases["specified-footing"]["PatternConfig"].update(StartingFootChoice="specified", StartingFootSpecified=1,
        LeftFootStartChoice="specifiedLane", LeftFootStartLaneSpecified=0, RightFootStartChoice="specifiedLane", RightFootStartLaneSpecified=3,
        LeftFootEndChoice="specifiedLane", LeftFootEndLaneSpecified=1, RightFootEndChoice="specifiedLane", RightFootEndLaneSpecified=3)
    cases["invalid-root"]["PatternConfig"].update(LeftFootStartChoice="specifiedLane", LeftFootStartLaneSpecified=99,
        RightFootStartChoice="specifiedLane", RightFootStartLaneSpecified=99)
    cases["timing-stops-delays"] = case(End=768, Stops=[[96, 0.3], [384, -0.25]], Delays=[[192, 0.2], [384, 0.1]])
    cases["timing-overlapping-warps"] = case(End=768, Warps=[[96, 144], [192, 192]], Bpms=[[0, 150], [144, 180], [384, 120]])
    cases["timing-simultaneous"] = case(End=768, Stops=[[192, 0.3], [384, -0.8]], Delays=[[192, 0.2]], Warps=[[192, 96]], Bpms=[[0, 150], [192, 200]])
    for layout in performed["ArrowWeights"]:
        cases["layout-" + layout] = case(layout, End=384, Seed=71)
    cases["interpret-single-sustains"] = {
        "Operation": "InterpretMatches", "Layout": "dance-single",
        "Frames": [{"State": state, "Release": release} for state, release in [
            ([4, 0, 0, 0], False), ([5, 0, 0, 6], False),
            ([0, 0, 0, 7], True), ([0, 2, 0, 7], False),
            ([0, 0, 0, 0], True), ([0, 0, 3, 0], False),
            ([1, 0, 0, 1], False), ([1, 1, 0, 1], False),
        ]]}
    for layout, weights in performed["ArrowWeights"].items():
        definition = json.loads((reference / "StepManiaLibrary" / f"{layout}.json").read_text())
        left, right = definition["StartingPositions"][0][0]
        def state(*entries):
            values = [0] * len(weights)
            for lane, value in entries: values[lane] = value
            return values
        cases["interpret-" + layout] = {
            "Operation": "InterpretMatches", "Layout": layout,
            "Frames": [{"State": values, "Release": release} for values, release in [
                (state((left, 4)), False), (state((left, 5), (right, 6)), False),
                (state((right, 7)), True), (state((left, 2), (right, 7)), False),
                (state(), True), (state((right, 3)), False),
                (state((left, 1), (right, 1)), False),
            ]]}
    for method in ["Balanced", "Aggressive", "NoBrackets"]:
        request = copy.deepcopy(cases["interpret-single-sustains"])
        request.update(Operation="InterpretPaths", Method=method)
        for values in [[1, 1, 0, 0], [0, 0, 1, 0], [0, 1, 0, 1], [1, 0, 0, 0],
                       [0, 0, 1, 1], [0, 1, 0, 0], [1, 0, 1, 0], [0, 0, 0, 1]] * 2:
            request["Frames"].append({"State": values, "Release": False})
        cases["interpret-paths-" + method.lower()] = request
    for layout, weights in performed["ArrowWeights"].items():
        definition = json.loads((reference / "StepManiaLibrary" / f"{layout}.json").read_text())
        left, right = definition["StartingPositions"][0][0]
        free = next(lane for lane in range(len(weights)) if lane not in [left, right])
        notes = [[0, left, "2"], [12, right, "4"], [24, free, "M"], [36, left, "3"],
                 [36, free, "F"], [48, right, "3"], [48, left, "M"], [60, right, "L"],
                 [72, left, "1"], [72, free, "1"], [84, right, "1"], [96, free, "1"],
                 [108, free, "M"], [120, left, "1"], [132, right, "1"],
                 [144, left, "1"], [144, free, "1"], [144, right, "1"], [156, free, "K"]]
        for method in ["Balanced", "Aggressive", "NoBrackets"]:
            cases["chart-" + layout + "-" + method.lower()] = {
                "Operation": "InterpretChart", "Layout": layout, "Method": method, "Notes": notes}
    default_expression = {
        "DefaultBracketParsingMethod": "balanced", "BracketParsingDetermination": "chooseMethodDynamically",
        "MinLevelForBrackets": 9, "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets": False,
        "BalancedBracketsPerMinuteForAggressiveBrackets": 10000.0, "BalancedBracketsPerMinuteForNoBrackets": 0.0,
    }
    for variant, changes in {
        "balanced": {}, "difficulty": {}, "forced": {"UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets": True},
        "rate-aggressive": {"BalancedBracketsPerMinuteForAggressiveBrackets": 0.01},
        "rate-no-brackets": {"BalancedBracketsPerMinuteForAggressiveBrackets": 1000000.0, "BalancedBracketsPerMinuteForNoBrackets": 100000.0},
        "specified": {"BracketParsingDetermination": "useDefaultMethod", "DefaultBracketParsingMethod": "noBrackets"},
        "zero-duration": {"BalancedBracketsPerMinuteForAggressiveBrackets": 0.01}, "empty": {}, "only-mines": {},
        "no-brackets": {"BalancedBracketsPerMinuteForAggressiveBrackets": 1000000.0, "BalancedBracketsPerMinuteForNoBrackets": 100000.0},
    }.items():
        request = copy.deepcopy(cases["chart-dance-single-balanced"])
        request["ExpressedConfig"] = {**default_expression, **changes}
        request["Difficulty"] = 5.0 if variant == "difficulty" else 12.0
        if variant == "zero-duration": request["TimeScale"] = 0.0
        if variant == "empty": request["Notes"] = []
        if variant == "only-mines": request["Notes"] = [[0, 0, "M"], [12, 1, "M"]]
        if variant == "no-brackets": request["Notes"] = [[0, 0, "1"], [12, 3, "1"], [24, 0, "1"], [36, 3, "1"]]
        cases["policy-" + variant] = request
    cases["mine-association"] = {
        "Operation": "InterpretChart", "Layout": "dance-single", "Method": "Balanced",
        "Notes": [[0, 0, "M"], [0, 2, "M"], [12, 0, "1"], [24, 0, "M"], [24, 2, "M"],
                  [36, 0, "1"], [36, 3, "1"], [48, 0, "M"], [48, 3, "M"], [60, 1, "1"],
                  [72, 0, "M"], [84, 3, "M"], [96, 0, "1"], [108, 1, "M"]]}
    fallback_cases = []
    for step in range(58):
        for foot in ["Left", "Right"]:
            for action in range(3):
                fallback_cases.append({"Label": f"step-{step}-{foot}-{action}", foot: {"Step": step, "Action": action, "Instance": 1 if action == 1 else 2}})
            fallback_cases.append({"Label": f"mixed-{step}-{foot}", foot: {"Step": step, "Action": 1, "Instance": 1, "ToeAction": 0, "ToeInstance": 3}})
    for left, right in [(1, 1), (17, 17), (22, 24), (8, 39), (42, 47), (13, 15), (27, 30), (33, 36)]:
        fallback_cases.append({"Label": f"jump-{left}-{right}", "Left": {"Step": left, "Action": 1, "Instance": 1, "ToeAction": 0, "ToeInstance": 2},
            "Right": {"Step": right, "Action": 0, "Instance": 3, "ToeAction": 2, "ToeInstance": 0}})
    fallback_cases.extend([{"Label": "blank"}, {"Label": "disabled-blank", "Disabled": True},
        {"Label": "disabled-step", "Disabled": True, "Right": {"Step": 57, "Action": 0, "Instance": 3}}])
    cases["fallback-replacements"] = {"Operation": "Fallbacks", "Cases": fallback_cases}
    sort_cases = []
    for count in [0, 1, 2, 3, 15, 16, 17, 31, 64, 257]:
        sort_cases.extend([[i % 4 for i in range(count)], [0] * count, list(range(count - 1, -1, -1)), [(i * 7) % 13 for i in range(count)]])
    cases["reference-sort"] = {"Operation": "Sort", "Cases": sort_cases}
    conversion_pairs = [("dance-single", "dance-double", "none"), ("dance-double", "dance-single", "default"),
        ("dance-single", "pump-single", "default"), ("pump-single", "dance-single", "default"),
        ("smx-single", "dance-double", "default"), ("dance-double", "pump-double", "default"),
        ("pump-double", "smx-full", "default"), ("dance-single", "dance-single", "none")]
    for source, target, mode in conversion_pairs:
        for seed in [42, -7]:
            original = cases["chart-" + source + "-balanced"]
            cases[f"convert-{source}-to-{target}-{seed}"] = {"Operation": "Convert", "SourceLayout": source, "Layout": target,
                "Notes": copy.deepcopy(original["Notes"]), "Seed": seed, "FallbackMode": mode, "PerformedConfig": copy.deepcopy(performed)}
    def manual_link(step, foot=0):
        link = [0] * 16
        link[foot * 8:foot * 8 + 4] = [1, step, 0, 0]
        return link
    for variant, steps, mode in [("blank-carry", [8, 1, 1], "identity"), ("missing-final", [8], "none"),
                                 ("missing-path", [8, 1], "none"), ("root-fallback", [2, 0], "none")]:
        cases["convert-manual-" + variant] = {"Operation": "Convert", "Layout": "dance-single", "Seed": 31,
            "FallbackMode": mode, "PerformedConfig": copy.deepcopy(performed),
            "LinkEvents": [{"Row": i * 12, "Time": i * 0.1, "Link": manual_link(step, i % 2)} for i, step in enumerate(steps)]}
    for label, original in [("associations", cases["mine-association"]), ("only-mines", cases["policy-only-mines"]), ("empty", cases["policy-empty"])]:
        for target in ["dance-double", "pump-single"]:
            cases[f"convert-mines-{label}-to-{target}"] = {"Operation": "Convert", "SourceLayout": "dance-single", "Layout": target,
                "Notes": copy.deepcopy(original["Notes"]), "Seed": 19, "FallbackMode": "default", "PerformedConfig": copy.deepcopy(performed)}
    for seed in [42, -7]:
        cases[f"convert-manual-mine-recency-{seed}"] = {"Operation": "Convert", "Layout": "dance-double", "Seed": seed,
            "FallbackMode": "none", "PerformedConfig": copy.deepcopy(performed),
            "LinkEvents": [{"Row": i * 12, "Time": i * 0.1, "Link": manual_link(0)} for i in range(6)],
            "LinkMines": [{"Row": 96, "Time": 0.8, "Lane": 0, "Kind": 0, "Nth": 9, "Foot": 0}]}
    cases["random-order"] = {"Operation": "RandomOrder", "Cases": [{"Seed": seed, "Length": length}
        for seed in [0, 42, -7] for length in [0, 1, 2, 3, 8, 128]]}
    cases["convert-mines-simultaneous-no-arrow"] = {"Operation": "Convert", "SourceLayout": "dance-single", "Layout": "dance-double",
        "Notes": [[0, 0, "M"], [0, 1, "M"], [12, 2, "M"]], "Seed": 19, "FallbackMode": "default", "PerformedConfig": copy.deepcopy(performed)}
    cases["pad-containment"] = {"Operation": "PadFit", "Layouts": list(performed["ArrowWeights"]) + ["dance-routine", "pump-routine", "smx-team"]}
    for label, values in {
        "delay": {"Delays": [[48, 0.5], [96, 0.25]]},
        "tempo-delay": {"Bpms": [[0, 120], [48, 180], [96, 240]], "Delays": [[48, 0.5], [96, 0.25]]},
        "mixed": {"Bpms": [[0, 120], [48, 180], [96, 240]], "Delays": [[48, 0.5], [96, -0.25]], "Stops": [[48, 0.75], [72, -0.3]], "Warps": [[48, 72], [96, 96]]},
        "negative-stops": {"Stops": [[0, -1.5], [48, 0.25], [72, -0.5]], "Delays": [[48, 0.5]]},
        "overlap": {"Warps": [[24, 96], [48, 192]], "Bpms": [[0, 150], [72, 200]], "Stops": [[96, 0.2]], "Delays": [[120, 0.4]]},
    }.items():
        cases["clock-" + label] = {"Operation": "Clock", "Bpms": [[0, 120]],
            "Rows": [0, 12, 24, 48, 48, 72, 96, 120, 192, 240, 384], **values}
    note_rows = [["0"] * 4 for _ in range(192)]
    for row, lane, symbol in [(0, 0, "2"), (48, 0, "3"), (48, 1, "1"), (48, 2, "M"), (72, 2, "M"), (96, 3, "4"), (144, 3, "3"), (180, 0, "F"), (180, 2, "L"), (191, 1, "K")]:
        note_rows[row][lane] = symbol
    body = "\n".join("".join(row) for row in note_rows)
    base = "#TITLE:Timing fixture;#ARTIST:Oracle;#OFFSET:0.15;#BPMS:0=120,1=180,2=240;#DELAYS:1=0.5,2=0.25;#STOPS:1=0.3,1.5=-0.2;#WARPS:1=1.5,2=2;"
    for label, extension, version, extra_song, extra_chart in [
        ("sm", "sm", "", "", ""),
        ("inherited", "ssc", "0.83", "", ""),
        ("own", "ssc", "0.83", "", "#BPMS:0=150,2=200;#DELAYS:1=0.4,2=0.3;"),
        ("offset-only", "ssc", "0.83", "", "#OFFSET:0.2;"),
        ("metadata", "ssc", "0.83", "#TIMESIGNATURES:0=4=4,8=3=4;#LABELS:10=End;#TICKCOUNTS:6=8;#COMBOS:7=2=3;#FAKES:9=1;#SCROLLS:11=0.5;#SPEEDS:12=1=0=0;", ""),
        ("ending-delay", "ssc", "0.83", "#DELAYS:6=0.4;#LABELS:6=End;", ""),
        ("ending-scroll-delay", "ssc", "0.83", "#DELAYS:6=0.4;#SCROLLS:6=0.5;", ""),
    ]:
        content = (base.replace("#DELAYS:1=0.5,2=0.25;", "") if label.startswith("ending-") else base) + extra_song
        if extension == "ssc":
            content = "#VERSION:" + version + ";" + content + "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;" + extra_chart + "#NOTES:\n" + body + "\n;"
        else:
            content += "#NOTES:dance-single:Oracle:Hard:9:0,0,0,0,0:\n" + body + "\n;"
        cases["file-clock-" + label] = {"Operation": "FileClock", "Format": extension, "Chart": 0, "Content": content, "TargetLayout": "dance-double", "PerformedConfig": copy.deepcopy(performed), "Seed": 42}
    for layout in performed["ArrowWeights"]:
        definition = json.loads((reference / "StepManiaLibrary" / f"{layout}.json").read_text())
        left, right = definition["StartingPositions"][0][0]
        for variant in ["empty", "following-even", "following-odd", "preceding", "jacks", "jump", "hold", "late", "bracket", "overlap-order", "equal-time", "mine-key", "release-same-row", "transitions"]:
            before = [[0, left, "1"], [12, right, "1"], [24, left, "1"], [36, right, "1"]]
            after = [[600, left, "1"], [612, right, "1"], [624, left, "1"], [636, right, "1"]]
            count = 16 if variant == "following-even" else 17
            if variant in ["empty", "following-even", "following-odd"]: before = []
            if variant == "empty": after = []
            if variant == "jacks": before = [[i * 12, left, "1"] for i in range(10)]
            if variant == "jump": before = [[0, left, "1"], [0, right, "1"]]; after = [[600, left, "1"], [600, right, "1"]]
            if variant in ["hold", "overlap-order"]: before = [[0, left, "2"], [12, right, "4"], [96, left, "3"], [108, right, "3"]]
            if variant == "equal-time": before = [[0, left, "1"], [12, right, "1"]]
            if variant == "mine-key": before = [[0, left, "M"], [12, right, "K"], [24, left, "1"], [36, right, "F"], [48, left, "L"]]
            if variant == "release-same-row": before = [[0, left, "2"], [24, left, "3"], [24, right, "1"], [48, left, "1"]]
            if variant == "transitions": before = [[i * 12, lane % len(performed["ArrowWeights"][layout]), "1"] for i, lane in enumerate([0, 1, 2, 3, 0, 1, 4, 5, 6, 7, 4, 5])]
            if variant == "late": before = [[1200 + i * 12, left if i % 2 == 0 else right, "1"] for i in range(40)]
            if variant == "bracket": before = [[0, left, "1"], [0, right, "1"], [0, next(lane for lane in range(len(performed["ArrowWeights"][layout])) if lane not in [left, right]), "1"]]
            start = 1800 if variant == "late" else 192
            if variant == "late": after = [[2400, left, "1"], [2412, right, "1"]]
            pc = {"Operation": "PatternContext", "Layout": layout, "Start": start, "End": start + (count - 1) * 12,
                "NumSteps": count, "Seed": 42, "BeforeNotes": before, "AfterNotes": after,
                "RangeStartsAfterChart": variant == "late", "TotalSteps": 512 if variant == "late" else len([n for n in before if n[2] != "3"]),
                "LaneCounts": [i + 1 for i in range(len(performed["ArrowWeights"][layout]))], "Nps": 8.0,
                "PerformedConfig": copy.deepcopy(performed), "PatternConfig": copy.deepcopy(pattern)}
            pc["PerformedConfig"]["Transitions"].update(Enabled=True, StepsPerTransitionMin=40, StepsPerTransitionMax=128)
            if variant == "equal-time": pc["TimeScale"] = 0.0
            if variant == "overlap-order": pc["BeforeEditorEvents"] = [[12, right, True], [0, left, True]]
            cases[f"context-{layout}-{variant}"] = pc
    clear_notes = [[0, 0, "2", 180], [47, 1, "4", 144], [48, 2, "M"], [60, 3, "K"], [72, 2, "F"], [84, 3, "L"], [96, 2, "1"], [120, 3, "2", 240], [288, 0, "1"]]
    # Raw start, length, spacing, start-inclusive, end-inclusive.
    clear_cases = {
        "inclusive": [[48, 48, 12, True, True]],
        "exclusive": [[48, 48, 12, False, False]],
        "start-exclusive": [[48, 48, 12, False, True]],
        "end-exclusive": [[48, 48, 12, True, False]],
        "empty": [[48, 0, 12, False, False]],
        "single": [[48, 0, 12, True, True]],
        "off-grid": [[49, 46, 12, True, False]],
        "spacing-one": [[48, 48, 1, True, True]],
        "spacing-forty-eight": [[48, 96, 48, True, True]],
        "overlapping-unsorted": [[72, 96, 12, True, True], [48, 48, 12, True, True]],
        "successive-hold-trims": [[48, 12, 12, True, True], [24, 12, 12, True, True]],
        "empty-at-origin": [[0, 0, 12, True, False]],
    }
    for label, markers in clear_cases.items():
        cases["clear-pattern-" + label] = {"Operation":"EditorTiming", "Timing":[[0,"signature",4,4],[0,"tempo",120]],
            "Notes":copy.deepcopy(clear_notes), "Rows":[], "NumSteps":0, "LastStep":0, "ClearPatterns":markers}
    schedule_cases = {
        "disjoint": [[48,48,12,True,True],[144,48,24,True,True]],
        "adjacent": [[48,48,12,True,True],[96,48,12,True,True]],
        "overlap": [[48,144,12,True,True],[96,144,24,True,True]],
        "nested-short": [[48,240,12,True,True],[96,24,12,True,True]],
        "empty-next": [[48,144,12,True,True],[96,0,12,False,False],[144,48,12,True,True]],
        "empty-final": [[48,144,12,True,True],[96,0,12,True,False]],
        "all-empty": [[48,0,12,False,False],[96,0,24,True,False]],
        "none": [],
        "unsorted": [[192,48,24,True,True],[48,96,12,False,False],[120,48,12,True,True]],
        "repeated": [[48,48,12,True,True],[48,48,12,True,True],[144,48,12,True,True]],
        "same-row-exclusive": [[48,48,12,False,True],[48,48,24,True,False]],
        "off-grid": [[49,46,12,True,False],[84,49,24,False,True]],
        "single-row": [[48,0,12,True,True],[96,0,24,True,True]],
        "spacing-one": [[48,9,1,True,False],[54,7,1,False,True]],
        "delayed-origin": [[0,0,12,False,False],[24,48,12,True,True]],
    }
    for label,markers in schedule_cases.items():
        cases["pattern-schedule-"+label] = {"Operation":"EditorTiming","Timing":[[0,"signature",4,4],[0,"tempo",120]],
            "Notes":[[0,0,"1"],[12,1,"F"],[24,2,"L"],[36,3,"M"],[47,0,"1"],[60,1,"2",156],[240,2,"1"]],
            "Rows":[],"NumSteps":0,"LastStep":0,"Schedule":markers}
    cases["pattern-schedule-delayed-origin"]["Timing"] += [[0,"delay",0.4],[72,"tempo",180],[96,"stop",-0.3],[144,"warp",48]]
    editor_notes = [[0, 0, "1"], [12, 1, "1"], [24, 2, "F"], [36, 3, "L"], [48, 0, "2", 96], [120, 2, "M"], [144, 3, "K"], [192, 1, "1"]]
    editor_cases = {
        "baseline": {},
        "stop": {"Extra": [[48, "stop", 0.4]]},
        "delay": {"Extra": [[48, "delay", 0.4]]},
        "simultaneous": {"Extra": [[48, "tempo", 180], [48, "delay", 0.25], [48, "stop", 0.4]]},
        "warp": {"Extra": [[48, "warp", 96]]},
        "warp-delay": {"Extra": [[48, "delay", 0.0], [48, "warp", 96]]},
        "warp-stop": {"Extra": [[48, "stop", 0.25], [48, "warp", 96]]},
        "overlapping-warps": {"Extra": [[48, "warp", 144], [96, "warp", 240], [144, "tempo", 187.3]]},
        "negative-stop": {"Extra": [[48, "stop", -0.4]]},
        "stacked-negative-stops": {"Extra": [[48, "stop", -1.0], [96, "stop", -0.3], [144, "stop", 0.2]]},
        "negative-stop-delay": {"Extra": [[48, "delay", 0.1], [48, "stop", -0.4]]},
        "signature-in-warp": {"Extra": [[24, "warp", 240], [37, "signature", 3, 4], [89, "tempo", 187.3]]},
        "nested-fakes": {"Extra": [[0, "fake", 384], [48, "fake", 48]]},
        "last-fake": {"Extra": [[384, "fake", 384]]},
        "last-fake-and-label": {"Extra": [[384, "fake", 384], [384, "label", 0]]},
        "last-stop": {"Extra": [[512, "stop", 0.75]]},
        "last-delay": {"Extra": [[512, "delay", 0.75]]},
        "last-warp": {"Extra": [[512, "warp", 384]]},
        "last-long-hold": {"Notes": [[0, 0, "2", 960]], "LastStep": 0},
        "hold-before-metadata": {"Notes": [[0, 0, "2", 960]], "Extra": [[192, "label", 0]]},
        "region-end": {"Notes": [], "Region": [96, 385], "LastStep": 384},
        "region-delay": {"Notes": [], "Region": [96, 384], "Extra": [[384, "delay", 0.4]]},
        "no-tempo": {"Timing": [[0, "signature", 4, 4]]},
        "empty": {"Notes": [], "LastStep": 0},
        "scroll-in-negative-stop": {"Extra": [[48, "stop", -1.0], [72, "scroll", 0.5], [96, "tempo", 187.3], [120, "scroll", 2.0]]},
        "zero-duration": {"Notes": [[0, 0, "1"]], "LastStep": 0},
        "last-tick": {"Extra": [[384, "tick", 4]]},
        "last-combo": {"Extra": [[384, "combo", 1]]},
        "last-speed": {"Extra": [[384, "speed", 1]]},
        "mixed-row-order": {"Notes": [[0, 0, "2", 96], [0, 3, "4", 96], [0, 1, "1"], [0, 2, "M"], [108, 0, "L"], [108, 1, "1"], [108, 2, "F"]]},
        "dense-warp-rounding": {"Notes": [[row, row % 4, "1"] for row in range(384)], "Timing": [[0, "signature", 4, 4], [0, "tempo", 187.3], [5, "warp", 300], [37, "signature", 3, 4]]},
    }
    for label, values in editor_cases.items():
        values = copy.deepcopy(values)
        extra = values.pop("Extra", [])
        cases["editor-timing-" + label] = {"Operation": "EditorTiming", "Timing": [[0, "signature", 4, 4], [0, "tempo", 150]] + extra,
            "Notes": copy.deepcopy(editor_notes), "Rows": [0, 12, 24, 48, 72, 96, 120, 144, 192, 240, 384, 768], "NumSteps": 17, "LastStep": 384, **values}
        cases["editor-timing-" + label]["Conversion"] = {"PerformedConfig": copy.deepcopy(performed)}
        cases["editor-timing-" + label]["PatternConfig"] = copy.deepcopy(pattern)
        cases["editor-timing-" + label]["Projection"] = {"Start": 48, "End": 96, "Spacing": 12, "Region": [48, 97], "NumSteps": 5, "Ranges": [[0, 47], [97, 768], [0, -1]]}
    for label, region, first, last, inclusive, notes in [
        ("pattern-start", [0, 48], 0, 48, [True, True], editor_notes),
        ("pattern-exclusive", [48, 96], 60, 84, [False, False], editor_notes),
        ("pattern-single", [48, 48], 48, 48, [True, True], editor_notes),
        ("pattern-empty-chart", [0, 48], 0, 48, [True, True], []),
    ]:
        case = copy.deepcopy(cases["editor-timing-baseline"])
        case["Notes"] = copy.deepcopy(notes)
        case["Inclusive"] = inclusive
        case["Projection"].update(Start=first, End=last, Region=region, NumSteps=(last-first)//12+1)
        cases["editor-timing-" + label] = case
    for label, base, markers in [
        ("loaded-flat", "baseline", [[48,97],[192,400]]),
        ("loaded-negative", "stacked-negative-stops", [[48,97],[55,250],[96,192]]),
        ("loaded-warp", "dense-warp-rounding", [[37,250],[48,97],[300,768]]),
        ("loaded-mixed", "mixed-row-order", [[0,48],[48,97]]),
    ]:
        case=copy.deepcopy(cases["editor-timing-"+base]);case["LoadedRegions"]=markers
        cases["editor-timing-"+label]=case
    for index,bpm in enumerate([187.3,157.7,120.1,243.45]):
        item=copy.deepcopy(cases["editor-timing-baseline"])
        item["Timing"]=[[0,"signature",4,4],[0,"tempo",bpm],[48,"stop",-60.0/bpm]]
        cases["editor-insertion-negative-end-rounding-"+str(index)]=item
    insertion_bases=[name for name in cases if name.startswith("editor-timing-") or name.startswith("editor-insertion-negative-end-rounding-")]
    for name in insertion_bases:
        item=copy.deepcopy(cases[name])
        item["Insertions"]=[[[0,2],[24,0],[48,1],[72,2],[96,1],[144,0]],[[49,3],[95,1],[192,3],[288,1],[384,0]]]
        item["InsertionClear"]=[[row,0,12,True,True] for row in sorted({note[0] for stage in item["Insertions"] for note in stage})]
        item["InsertionRanges"]=[[0,47],[48,96],[97,768],[0,-1]]
        cases[name.replace("editor-timing-","editor-insertion-")]=item
    metadata={"MusicOffset":-0.125,"ShouldUseChartMusicOffset":True,"ExpressedChartConfig":"A19D532E-B0CE-4759-AD1C-02ECBBDF2EFD","MaxPlayerIndex":1,
        "Patterns":{"48":{"PatternConfigGuid":"03de57bd-4329-4de6-a722-a171c11fdd16","PerformedChartConfigGuid":"41a7f76e-596a-4fbe-bf6e-6a77a898e4a1","Length":192,"RandomSeed":-2147483648,"StartPositionInclusive":True,"EndPositionInclusive":False,"IgnorePrecedingDistribution":True},"384":{"Length":0}}}
    for label,content in {"defaults":"{}","complete":json.dumps(metadata),"null-patterns":'{"Patterns":null}',"comments":'/* original */ {"Patterns": {"0": {/* marker */ "Length":48}}}',"trailing-comma":'{"Patterns":{},}',"invalid-guid":'{"ExpressedChartConfig":"invalid"}',"braced-guid":'{"ExpressedChartConfig":"{a19d532e-b0ce-4759-ad1c-02ecbbdf2efd}"}'}.items():
        cases["metadata-"+label]={"Operation":"Metadata","Content":content}
    for label,song,offset,use in [("song-positive",0.25,-0.125,False),("chart-negative",0.25,-0.125,True),("song-negative",-0.375,0.5,False),("chart-zero",0.25,0.0,True),("song-zero",0.0,-0.125,False),("chart-positive",-0.5,0.125,True)]:
        cases["chart-offset-"+label]={"Operation":"Metadata","Content":json.dumps({"MusicOffset":offset,"ShouldUseChartMusicOffset":use}),"SongOffset":song}
    profile_guid="12345678-9abc-4def-8123-456789abcdef"
    profile_base={"Guid":profile_guid,"Name":"Custom","Description":"Preserve"}
    profile_cases={
        "pattern":("Pattern",{**profile_base,"PatternType":"sixteenthNotes","Config":copy.deepcopy(pattern)}),
        "pattern-after":("Pattern",{**profile_base,"Config":copy.deepcopy(pattern),"PatternType":"eighthNotes","StartingFootSpecified":"right"}),
        "pattern-before":("Pattern",{**profile_base,"PatternType":"eighthNotes","StartingFootSpecified":"right","Config":copy.deepcopy(pattern)}),
        "performed-after":("Performed",{**profile_base,"Config":copy.deepcopy(performed),"TravelSpeedNoteTypeDenominatorIndex":2,"TravelSpeedMinBPM":110,"TravelSpeedMaxBPM":190}),
        "performed-before":("Performed",{**profile_base,"TravelSpeedNoteTypeDenominatorIndex":2,"TravelSpeedMinBPM":110,"TravelSpeedMaxBPM":190,"Config":copy.deepcopy(performed)}),
        "index-negative":("Performed",{**profile_base,"Config":copy.deepcopy(performed),"TravelSpeedNoteTypeDenominatorIndex":-50,"TravelSpeedMinBPM":110,"TravelSpeedMaxBPM":190}),
        "index-large":("Performed",{**profile_base,"Config":copy.deepcopy(performed),"TravelSpeedNoteTypeDenominatorIndex":50,"TravelSpeedMinBPM":110,"TravelSpeedMaxBPM":190}),
        "expressed":("Expressed",{**profile_base,"Config":{"DefaultBracketParsingMethod":"noBrackets","BracketParsingDetermination":"useDefaultMethod"}}),
        "unknown":("Pattern",{**profile_base,"Unknown":{"keep":True},"Config":{**copy.deepcopy(pattern),"Future":123}}),
        "invalid-subdivision":("Pattern",{**profile_base,"Config":copy.deepcopy(pattern),"PatternType":-1}),
        "empty-guid":("Pattern",{**profile_base,"Guid":"00000000-0000-0000-0000-000000000000","Config":copy.deepcopy(pattern)}),
        "invalid-config":("Pattern",{**profile_base,"Config":{**copy.deepcopy(pattern),"BeatSubDivision":5}}),
    }
    for label,(kind,body) in profile_cases.items():
        cases["profile-"+label]={"Operation":"Profile","Kind":kind,"Content":json.dumps(body)}
    profile_cases["enum-whitespace"]=("Pattern",{**profile_base,"Config":copy.deepcopy(pattern),"PatternType":" EIGHTHNOTES "})
    mixed_speed=copy.deepcopy(performed);mixed_speed["StepTightening"].update(SpeedMinTimeSeconds=0.4,SpeedMaxTimeSeconds=0.6)
    cases["profile-duplicate-config"]={"Operation":"Profile","Kind":"Pattern","Content":'{"Guid":'+json.dumps(profile_guid)+',"Config":'+json.dumps(pattern)+',"PatternType":"eighthNotes","Config":'+json.dumps({**pattern,"BeatSubDivision":3})+'}'}
    cases["profile-interleaved-speed"]={"Operation":"Profile","Kind":"Performed","Content":'{"Guid":'+json.dumps(profile_guid)+',"Config":'+json.dumps(performed)+',"TravelSpeedNoteTypeDenominatorIndex":3,"TravelSpeedMinBPM":110,"Config":'+json.dumps(mixed_speed)+',"TravelSpeedMaxBPM":190}'}
    kind,body=profile_cases["enum-whitespace"];cases["profile-enum-whitespace"]={"Operation":"Profile","Kind":kind,"Content":json.dumps(body)}
    combined=copy.deepcopy(pattern);combined["LeftFootStartChoice"]="automaticNewLane, automaticSameOrNewLane"
    cases["profile-enum-combinations"]={"Operation":"Profile","Kind":"Pattern","Content":json.dumps({**profile_base,"Config":combined,"PatternType":"eighthNotes, eighthNoteTriplets","StartingFootSpecified":"left, right"})}
    numeric=copy.deepcopy(pattern);numeric.update(StartingFootChoice="1",LeftFootStartChoice="3")
    cases["profile-enum-numeric-strings"]={"Operation":"Profile","Kind":"Pattern","Content":json.dumps({**profile_base,"Config":numeric,"PatternType":"2","StartingFootSpecified":"-1"})}
    huge=copy.deepcopy(pattern);huge["FutureHuge"]=0
    content=json.dumps({**profile_base,"Config":huge,"FutureInteger":123456789012345678901234567890}).replace('"FutureHuge": 0','"FutureHuge": 1e999')
    cases["profile-unknown-big-numbers"]={"Operation":"Profile","Kind":"Pattern","Content":content}
    cases["profile-utf8-bom"]={"Operation":"Profile","Kind":"Pattern","Content":"\ufeff"+json.dumps({**profile_base,"Config":pattern})}
    cases["profile-comments"]={"Operation":"Profile","Kind":"Pattern","Content":"/* profile */ "+json.dumps({**profile_base,"Config":pattern})[:-1]+",}"}
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", type=Path, required=True)
    parser.add_argument("--editor-root", type=Path, default=ROOT.parent / "grooveauthor")
    parser.add_argument("--only", action="append", help="Regenerate only names starting with this prefix (repeatable)")
    args = parser.parse_args()
    reference = args.reference_root.resolve()
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=reference, text=True).strip()
    if commit != PIN:
        raise SystemExit(f"Reference must be pinned at {PIN}, found {commit}")
    env = {**os.environ, "DOTNET_CLI_TELEMETRY_OPTOUT": "1", "DOTNET_SKIP_FIRST_TIME_EXPERIENCE": "1"}
    context_hash = prepare_context_reference(args.editor_root.resolve())
    timing_hash = prepare_timing_reference(args.editor_root.resolve())
    metadata_hash = prepare_metadata_reference(args.editor_root.resolve())
    profiles_hash = prepare_profiles_reference(args.editor_root.resolve())
    project = ROOT / "scripts/editor-oracle/EditorOracle.csproj"
    build = subprocess.run([str(args.dotnet), "build", str(project), f"-p:ReferenceRoot={reference}", "-c", "Release"], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if build.returncode:
        raise SystemExit(build.stdout)
    oracle = project.parent / "bin/Release/net10.0/EditorOracle.dll"
    output = ROOT / "crates/deadsync-editor/tests/fixtures/grooveauthor"
    output.mkdir(parents=True, exist_ok=True)
    cases = requests(reference)
    for seed in [0, 42, -7, -2147483648, 2147483647]:
        cases[f"random-{seed}"] = {"Operation": "Random", "Seed": seed, "Count": 64}
    for name, request in cases.items():
        if args.only and not any(name.startswith(prefix) for prefix in args.only):
            continue
        path = output / f"{name}.request.json"
        path.write_text(json.dumps(request, indent=2) + "\n")
        result = subprocess.check_output([str(args.dotnet), str(oracle), str(reference), str(path)], env=env, text=True)
        # Parse before publishing; diagnostic output must not become a baseline.
        parsed = json.loads(result)
        (output / f"{name}.expected.json").write_text(json.dumps(parsed, indent=2) + "\n")
        print(name)
    (output / "provenance.json").write_text(json.dumps({
        "reference": "https://github.com/PerryAsleep/StepManiaLibrary", "commit": commit,
        "editor_commit": EDITOR_PIN, "editor_context_methods_sha256": context_hash, "editor_timing_methods_sha256": timing_hash, "editor_metadata_types_sha256": metadata_hash, "editor_profile_setters_sha256": profiles_hash,
        "fumen_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=reference / "Fumen", text=True).strip(),
        "dotnet_sdk": subprocess.check_output([str(args.dotnet), "--version"], env=env, text=True).strip(),
        "dotnet_runtimes": [line.split(" [", 1)[0] for line in subprocess.check_output([str(args.dotnet), "--list-runtimes"], env=env, text=True).splitlines()],
        "sort_source": "https://github.com/dotnet/runtime/blob/v10.0.13/src/libraries/System.Private.CoreLib/src/System/Collections/Generic/ArraySortHelper.cs",
        "notice": "Expected results come from pinned C# libraries and unchanged GrooveAuthor helper bodies, not from the Rust port. Editor adapters provide explicit input events; they do not run the full editor.",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
