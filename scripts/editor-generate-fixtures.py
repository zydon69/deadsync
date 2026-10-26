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
    attack_ui = read("UI/UIAttackEvent.cs")
    groups = {
        "ATTACK_METHODS": [extract_member(attack_ui, r"\tprivate void " + name + r"\(") for name in ["SetModLengthFromCurrentTime", "JumpToModEnd"]],
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
    song=extract_member(read("EditorSong.cs"),r"\tprivate class CustomSaveDataV1\s*")
    position="\n\n".join(extract_member(read("EditorPosition.cs"),r"\tpublic static double "+name+r"\(") for name in ["GetSongTimeFromChartTime","GetChartTimeFromSongTime"])
    text=text.replace("// SONG_DATA",song).replace("// POSITION_METHODS",position)
    (project/"obj/MetadataReference.cs").write_text(text)
    return hashlib.sha256((definition+"\n"+custom+"\n"+offset+"\n"+song+"\n"+position).encode()).hexdigest()



def prepare_tempo_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorTempoEvent.cs"],cwd=editor).decode("utf-8-sig")
    minimum=re.search(r"public const double MinTempo = [^;]+;",source).group()
    setter=extract_member(source,r"\tpublic double DoubleValue\s*")
    chart=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorChart.cs"],cwd=editor).decode("utf-8-sig")
    movement="\n\n".join(extract_member(chart,r"\tpublic bool "+name+r"\(") for name in ["CanEventExistAtRow","CanEventBeMovedToRow"])
    project=ROOT/"scripts/editor-oracle"
    template=(project/"TempoReference.cs.template").read_text().replace("// MINIMUM",minimum).replace("// SETTER",setter).replace("// MOVEMENT",movement)
    (project/"obj/TempoReference.cs").write_text(template)
    return hashlib.sha256((minimum+"\n"+setter+"\n"+movement).encode()).hexdigest()


def prepare_pause_reference(editor):
    def read(path):
        return subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/{path}"],cwd=editor).decode("utf-8-sig")
    stop=read("EditorEvents/EditorStopEvent.cs");delay=read("EditorEvents/EditorDelayEvent.cs");chart=read("EditorChart.cs")
    pieces={"VALIDATION":extract_member(stop,r"\tpublic static bool IsStopLengthValid\("),
        "STOP_SETTER":extract_member(stop,r"\tpublic double DoubleValue\s*"),
        "DELAY_SETTER":extract_member(delay,r"\tpublic double DoubleValue\s*"),
        "MOVEMENT":"\n\n".join(extract_member(chart,r"\tpublic bool "+name+r"\(") for name in ["CanEventExistAtRow","CanEventBeMovedToRow"])}
    project=ROOT/"scripts/editor-oracle"
    template=(project/"PauseReference.cs.template").read_text()
    for name,body in pieces.items():template=template.replace("// "+name,body)
    (project/"obj/PauseReference.cs").write_text(template)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_region_reference(editor):
    def read(path):
        return subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/{path}"],cwd=editor).decode("utf-8-sig")
    warp=read("EditorWarpEvent.cs");fake=read("EditorFakeSegmentEvent.cs")
    pieces={"WARP_MINIMUM":re.search(r"public const int MinLength = [^;]+;",warp).group(),
        "FAKE_MINIMUM":re.search(r"public const int MinFakeSegmentLength = [^;]+;",fake).group(),
        "WARP_SETTER":extract_member(warp,r"\tpublic int IntValue\s*"),"FAKE_SETTER":extract_member(fake,r"\tpublic int IntValue\s*")}
    project=ROOT/"scripts/editor-oracle";template=(project/"RegionReference.cs.template").read_text()
    for name,body in pieces.items():template=template.replace("// "+name,body)
    (project/"obj/RegionReference.cs").write_text(template)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_pattern_navigation_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/Editor.cs"],cwd=editor).decode("utf-8-sig")
    pieces={"PREVIOUS_PATTERN":extract_member(source,r"\tprivate void OnMoveToPreviousPattern\(double chartPosition\)"),
        "NEXT_PATTERN":extract_member(source,r"\tprivate void OnMoveToNextPattern\(double chartPosition\)")}
    project=ROOT/"scripts/editor-oracle";text=(project/"PatternNavigationReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/PatternNavigationReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_label_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorLabelEvent.cs"],cwd=editor).decode("utf-8-sig")
    pieces={"VALIDATOR":extract_member(source,r"\tpublic static \(bool, string\) IsValidLabelString\("),"SETTER":extract_member(source,r"\tpublic string StringValue\s*")}
    tree=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/Containers/EventTree.cs"],cwd=editor).decode("utf-8-sig")
    pieces["PREVIOUS"]=extract_member(tree,r"\tpublic EditorEvent FindPreviousEventWithLooping\(")
    pieces["NEXT"]=extract_member(tree,r"\tpublic EditorEvent FindNextEventWithLooping\(")
    project=ROOT/"scripts/editor-oracle";text=(project/"LabelReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/LabelReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_speed_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorInterpolatedRateAlteringEvent.cs"],cwd=editor).decode("utf-8-sig")
    pieces={"VALIDATOR":extract_member(source,r"\tpublic static \(bool, double, int, double, bool\) IsValidScrollRateInterpolationString\("),"SETTER":extract_member(source,r"\tpublic string StringValue\s*"),"ROW_INTERPOLATION":extract_member(source,r"\tpublic double GetInterpolatedScrollRateFromRow\("),"TIME_INTERPOLATION":extract_member(source,r"\tpublic double GetInterpolatedScrollRateFromTime\(")}
    project=ROOT/"scripts/editor-oracle";text=(project/"SpeedReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/SpeedReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_combo_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorMultipliersEvent.cs"],cwd=editor).decode("utf-8-sig")
    pieces={"VALIDATOR":extract_member(source,r"\tpublic static \(bool, int, int\) IsValidMultipliersString\("),"GETTER":extract_member(source,r"\tpublic string GetMultipliersString\("),"SETTER":extract_member(source,r"\tpublic string StringValue\s*")}
    project=ROOT/"scripts/editor-oracle";text=(project/"ComboReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/ComboReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_tick_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorTickCountEvent.cs"],cwd=editor).decode("utf-8-sig")
    setter=extract_member(source,r"\tpublic int IntValue\s*")
    project=ROOT/"scripts/editor-oracle";text=(project/"TickReference.cs.template").read_text().replace("// SETTER",setter)
    (project/"obj/TickReference.cs").write_text(text)
    return hashlib.sha256(setter.encode()).hexdigest()


def prepare_scroll_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorScrollRateEvent.cs"],cwd=editor).decode("utf-8-sig")
    setter=extract_member(source,r"\tpublic double DoubleValue\s*")
    project=ROOT/"scripts/editor-oracle";text=(project/"ScrollReference.cs.template").read_text().replace("// SETTER",setter)
    (project/"obj/ScrollReference.cs").write_text(text)
    return hashlib.sha256(setter.encode()).hexdigest()


def prepare_signature_reference(editor):
    source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/EditorEvents/EditorTimeSignatureEvent.cs"],cwd=editor).decode("utf-8-sig")
    pieces={"VALIDATOR":extract_member(source,r"\tpublic static \(bool, Fraction\) IsValidTimeSignatureString\("),"SETTER":extract_member(source,r"\tpublic string StringValue\s*")}
    editor_source=subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/Editor.cs"],cwd=editor).decode("utf-8-sig")
    pieces["PREVIOUS_MEASURE"]=extract_member(editor_source,r"\tprivate void OnMoveToPreviousMeasure\(")
    pieces["NEXT_MEASURE"]=extract_member(editor_source,r"\tprivate void OnMoveToNextMeasure\(")
    project=ROOT/"scripts/editor-oracle";text=(project/"SignatureReference.cs.template").read_text()
    for name,body in pieces.items():text=text.replace("// "+name,body)
    (project/"obj/SignatureReference.cs").write_text(text)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


def prepare_source_timing_reference(editor):
    def read(path):return subprocess.check_output(["git","show",f"{EDITOR_PIN}:StepManiaEditor/{path}"],cwd=editor).decode("utf-8-sig")
    chart=read("EditorChart.cs");display=read("DisplayTempo.cs")
    pieces={"DISPLAY_MODE":extract_member(display,r"internal enum DisplayTempoMode\s*"),"DISPLAY_TYPE":extract_member(display,r"internal sealed class DisplayTempo\s*"),
        "DEFAULT_TEMPO":re.search(r"public const double DefaultTempo = [^;]+;",chart).group(),
        "STARTING_TEMPO":extract_member(read("EditorSong.cs"),r"\tpublic double GetBestChartStartingTempo\("),
        "DEFAULT_SIGNATURE":re.search(r"public static readonly Fraction DefaultTimeSignature = [^;]+;",chart).group(),
        "STARTING_SIGNATURE":extract_member(read("EditorSong.cs"),r"\tpublic Fraction GetBestChartStartingTimeSignature\("),
        "TEMPO_CLAMP":extract_member(chart,r"if \(t.TempoBPM < EditorTempoEvent.MinTempo\)"),
        "WARP_CLAMP":re.search(r"if \(WarpEvent.LengthIntegerPosition < MinLength\)\s*WarpEvent.LengthIntegerPosition = MinLength;",read("EditorEvents/EditorWarpEvent.cs")).group(),
        "FAKE_CLAMP":re.search(r"if \(FakeSegmentEvent.LengthIntegerPosition < MinFakeSegmentLength\)\s*FakeSegmentEvent.LengthIntegerPosition = MinFakeSegmentLength;",read("EditorEvents/EditorFakeSegmentEvent.cs")).group()}
    project=ROOT/"scripts/editor-oracle";template=(project/"SourceTimingReference.cs.template").read_text()
    for name,body in pieces.items():template=template.replace("// "+name,body)
    (project/"obj/SourceTimingReference.cs").write_text(template)
    return hashlib.sha256("\n".join(pieces.values()).encode()).hexdigest()


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
    for label,markers in schedule_cases.items():
        if label in ["repeated","same-row-exclusive","none"]: continue
        item=copy.deepcopy(cases["pattern-schedule-"+label])
        item.update(BatchPatterns=True,PerformedConfig=copy.deepcopy(performed),PatternConfig=copy.deepcopy(pattern))
        cases["pattern-batch-"+label]=item
    for label,extra in [("empty-next-context",[[264,72,12,True,True],[384,96,12,True,True]]),("empty-next-double",[[240,0,12,False,False],[264,96,12,True,True],[400,48,12,True,True]])]:
        item=copy.deepcopy(cases["pattern-batch-empty-next"]);item["Schedule"]+=extra
        cases["pattern-batch-"+label]=item
    item=copy.deepcopy(cases["pattern-batch-delayed-origin"])
    item["Timing"]=[event for event in item["Timing"] if event[1]!="warp"]
    cases["pattern-batch-delayed-origin-sm"]=item
    item=copy.deepcopy(cases["pattern-batch-disjoint"]);item["IgnoreRows"]=[144]
    cases["pattern-batch-ignore-distribution"]=item
    item=copy.deepcopy(cases["pattern-batch-empty-next-context"])
    item["PerformedConfig"]["Transitions"].update(Enabled=True,StepsPerTransitionMin=2,StepsPerTransitionMax=8,MinimumPadWidth=1)
    cases["pattern-batch-shared-transitions"]=item
    for name,item in cases.items():
        if name.startswith("pattern-batch-"):
            item["Formats"]=["ssc"] if any(event[1]=="warp" for event in item["Timing"]) else ["sm","ssc"]
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
    tempo_cases={
        "basic":[[0,120],[4,180]],"unsorted":[[4,180],[0,120],[2,150]],
        "duplicate-beat":[[0,120],[0,180],[4,190],[4,200]],
        "row-collision":[[0,120],[1.001,170],[1.009,190],[1.005,180]],
        "half-row":[[0,120],[0.5/48,130],[1.5/48,140],[2.5/48,150],[47.5/48,160]],
        "negative-rows":[[-2,90],[-0.1,100],[-0.00001,110],[4,180]],
        "invalid-first":[[0,-1],[2,-10],[4,180],[8,200]],
        "invalid-all":[[0,0],[2,-1],[4,-180]],"below-min":[[0,0.00000001],[4,0.0000001]],
        "empty":[],"future-first":[[4,180],[8,200]],
        "fractional":[[0,187.3],[0.3333333333333333,157.7],[2.0000000001,143.456789123]],
        "ulp":[[0,120]],"large-row":[[0,120],[2147483647/48,180]],
    }
    for label,segments in tempo_cases.items():
        cases["tempo-edit-"+label]={"Operation":"Tempo","Segments":segments,"Fallback":120.0,
            "Operations":[["set",0,150],["set",96,180],["set",96,180],["move",96,144],["move",144,0],["remove",0],["remove",144],["remove",144],["set",48,0.0000001]]}
    cases["tempo-edit-ulp"]["Operations"]=[["set",0,value] for value in [float.fromhex("0x1.e000000000001p+6"),float.fromhex("0x1.e000000000002p+6"),120.0]]
    cases["tempo-edit-empty-display"]=copy.deepcopy(cases["tempo-edit-empty"]);cases["tempo-edit-empty-display"]["Fallback"]=187.3
    pause_cases={
        "basic":[[0,0.5],[2,1.25],[4,-0.25]],"empty":[],
        "unsorted":[[4,0.5],[0,1],[2,0.25]],"duplicates":[[0,0.25],[0,0.5],[2,1],[2,-1]],
        "row-conflict":[[1.001,1],[1.005,2],[1.009,0]],
        "invalid-masks-valid":[[1.001,1],[1.009,-0.5]],
        "half-row":[[0.5/48,0.25],[1.5/48,0.5],[2.5/48,1],[47.5/48,2]],
        "negative-position":[[-2,1],[-0.1,2],[-0.00001,3],[4,4]],
        "zeros":[[0,0.0],[1,-0.0],[2,float.fromhex("0x0.0000000000001p-1022")]],
        "fractional":[[0,0.123456789123],[1/3,0.25],[2,1.0000000000000002]],
        "large-row":[[2147483647/48,1]],
    }
    for kind in ["Stop","Delay"]:
        for label,segments in pause_cases.items():
            cases[f"pause-edit-{kind.lower()}-{label}"]={"Operation":"Pause","Kind":kind,"Segments":segments,
                "Operations":[["set",0,0.5],["set",96,1.25],["set",96,1.25],["move",0,48],["move",48,96],["move",48,72],["remove",0],["remove",72],["remove",72],["set",192,-0.5],["set",192,0.0],["set",192,0.75],["set",192,float.fromhex("0x1.8000000000001p-1")]]}
    region_cases={
        "basic":[[0,1],[2,2],[8,0.5]],"empty":[],
        "unsorted":[[8,2],[0,1],[2,0.5]],"duplicates":[[0,1],[0,2],[2,3],[2,4]],
        "row-conflict":[[1.001,1],[1.005,2],[1.009,3]],
        "invalid-masks-valid":[[1.001,1],[1.009,0],[2,-1]],
        "half-row":[[0.5/48,0.5/48],[1.5/48,1.5/48],[2.5/48,2.5/48],[47.5/48,47.5/48]],
        "negative-position":[[-2,1],[-0.1,2],[-0.00001,3],[4,4]],
        "tiny-lengths":[[0,0.0000001],[2,0.00001],[4,0],[6,-1]],
        "fractional":[[0,1/3],[1/3,2/3],[2.000000001,0.123456789]],
        "overlapping":[[0,8],[1,2],[4,8],[8,1]],
        "large-row":[[2147483646/48,1/48]],
    }
    for kind in ["Warp","Fake"]:
        for label,segments in region_cases.items():
            cases[f"region-edit-{kind.lower()}-{label}"]={"Operation":"Region","Kind":kind,"Segments":segments,
                "Operations":[["set",0,48],["set",96,96],["set",96,96],["move",0,48],["move",48,96],["move",48,72],["remove",0],["remove",72],["remove",72],["set",192,0],["set",192,1],["set",192,1]]}
    timing_fields={"OFFSET":"0.25","BPMS":"0=120,4=180","STOPS":"2=0.25","DELAYS":"3=0.125","WARPS":"8=1","TIMESIGNATURES":"0=4=4,16=3=4","TICKCOUNTS":"0=4","COMBOS":"0=1=1","SPEEDS":"0=1=0=0,6=2=1=0","SCROLLS":"0=1,4=0.5","FAKES":"12=1","LABELS":"1=Café"}
    def tags(fields):return "".join(f"#{tag}:{value};" for tag,value in fields.items())
    def chart(fields=None):return "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;"+tags(fields or {})+"#NOTES:1000\n0100\n0010\n0001;"
    own={"BPMS":"0=150","OFFSET":"-0.125","STOPS":"1=0.1"}
    root="#VERSION:0.83;#TITLE:Timing;"+tags(timing_fields)
    content=root+chart()+chart(own)+chart()
    edits={"inherited":("Chart",0,root+chart({**timing_fields,"BPMS":"0=160,2=200,4=180"})+chart(own)+chart(),[["set",0,160],["set",96,200]]),
        "own":("Chart",1,root+chart()+chart({**own,"BPMS":"0=175"})+chart(),[["set",0,175]]),
        "song":("Song",0,"#VERSION:0.83;#TITLE:Timing;"+tags({**timing_fields,"BPMS":"0=160,2=200,4=180"})+chart()+chart(own)+chart(),[["set",0,160],["set",96,200]])}
    for label,(scope,index,edited,operations) in edits.items():
        cases["tempo-file-"+label]={"Operation":"TimingFile","Format":"ssc","Scope":scope,"Chart":index,"Content":content,"EditedContent":edited,"Operations":operations}
    cases["tempo-file-offset"]={"Operation":"TimingFile","Format":"ssc","Scope":"Offset","Chart":0,"Content":content,
        "EditedContent":root+chart({**timing_fields,"OFFSET":"0.5"})+chart(own)+chart(),"Operations":[["offset",0,0.5]]}
    sm="#TITLE:Timing;#OFFSET:0.25;#BPMS:0=120,4=180;#STOPS:2=0.25;#DELAYS:3=0.125;"+"#NOTES:dance-single::Hard:9::1000\n0100\n0010\n0001;"*2
    cases["tempo-file-sm"]={"Operation":"TimingFile","Format":"sm","Scope":"Song","Chart":0,"Content":sm,"EditedContent":sm.replace("#BPMS:0=120,4=180;","#BPMS:0=160,2=200,4=180;"),"Operations":[["set",0,160],["set",96,200]]}
    pause_edits={
        "inherited":("Chart",0,root+chart({**timing_fields,"STOPS":"1=0.5,3=0.25","DELAYS":"1=0.25"})+chart(own)+chart(),
            [["set","Stop",48,0.5],["set","Delay",48,0.25],["move","Stop",96,144],["remove","Delay",144]]),
        "owned-sign-change":("Chart",1,root+chart()+chart({**own,"STOPS":"1=-0.5","DELAYS":"1=0.25"})+chart(),
            [["set","Stop",48,-0.5],["set","Delay",48,0.25]]),
        "song":("Song",0,"#VERSION:0.83;#TITLE:Timing;"+tags({**timing_fields,"STOPS":"1=0.5,2=0.25","DELAYS":"1=0.25,3=0.125"})+chart()+chart(own)+chart(),
            [["set","Stop",48,0.5],["set","Delay",48,0.25]]),
        "empty-set":("Chart",0,root+chart({**timing_fields,"STOPS":"","DELAYS":""})+chart(own)+chart(),
            [["remove","Stop",96],["remove","Delay",144]]),
    }
    for label,(scope,index,edited,operations) in pause_edits.items():
        cases["pause-file-"+label]={"Operation":"PauseFile","Format":"ssc","Scope":scope,"Chart":index,"Content":content,"EditedContent":edited,"Operations":operations}
    cases["pause-file-sm"]={"Operation":"PauseFile","Format":"sm","Scope":"Song","Chart":0,"Content":sm,
        "EditedContent":sm.replace("#STOPS:2=0.25;","#STOPS:1=-0.25,2=0.25;").replace("#DELAYS:3=0.125;","#DELAYS:1=0.5,3=0.125;"),
        "Operations":[["set","Stop",48,-0.25],["set","Delay",48,0.5]]}
    region_fields={"OFFSET":"0.25","BPMS":"0=120,4=180","STOPS":"1=0.25","DELAYS":"3=0.125","WARPS":"1=1,3=1","FAKES":"0=6,1=1","TIMESIGNATURES":"0=4=4","TICKCOUNTS":"0=4","COMBOS":"0=1=1","SPEEDS":"0=1=0=0","SCROLLS":"0=1","LABELS":"1=Keep"}
    note_lines=["".join("1" if lane==i%4 else "0" for lane in range(4)) for i in range(32)]
    region_notes="\n".join(note_lines[:16])+"\n,\n"+"\n".join(note_lines[16:])
    def region_chart(fields=None):return "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;"+tags(fields or {})+"#NOTES:"+region_notes+";"
    region_own={"BPMS":"0=150","OFFSET":"-0.125","WARPS":"2=1"}
    region_root="#VERSION:0.83;#TITLE:Regions;"+tags(region_fields)
    region_source=region_root+region_chart()+region_chart(region_own)+region_chart()
    region_edits={
        "inherited":("Chart",0,region_root+region_chart({**region_fields,"WARPS":"1=1,2=2,3=1","FAKES":"0=6,1=1,3=0.5"})+region_chart(region_own)+region_chart(),
            [["set","Warp",96,96],["set","Fake",144,24]]),
        "owned":("Chart",1,region_root+region_chart()+region_chart({**region_own,"WARPS":"1=1","FAKES":"0=3,1=1"})+region_chart(),
            [["move","Warp",96,48],["set","Fake",0,144],["set","Fake",48,48]]),
        "song":("Song",0,"#VERSION:0.83;#TITLE:Regions;"+tags({**region_fields,"WARPS":"2=1,3=1","FAKES":"0=6,1=1,3=0.5"})+region_chart()+region_chart(region_own)+region_chart(),
            [["move","Warp",48,96],["set","Fake",144,24]]),
        "empty-set":("Chart",0,region_root+region_chart({**region_fields,"WARPS":"","FAKES":""})+region_chart(region_own)+region_chart(),
            [["remove","Warp",48],["remove","Warp",144],["remove","Fake",0],["remove","Fake",48]]),
    }
    for label,(scope,index,edited,operations) in region_edits.items():
        cases["region-file-"+label]={"Operation":"RegionFile","Format":"ssc","Scope":scope,"Chart":index,"Content":region_source,"EditedContent":edited,"Operations":operations}
    source_base={"BPMS":"0=120,4=180","TIMESIGNATURES":"0=4=4"}
    source_cases={
        "half-rows":{**source_base,"BPMS":"0=120,0.010416666666666666=150,0.03125=180,0.052083333333333336=210","STOPS":"0.010416666666666666=0.25,0.03125=-0.125","DELAYS":"0.052083333333333336=0.25","WARPS":"1.0104166666666667=0.010416666666666666","FAKES":"2.03125=0.03125"},
        "invalid-masks":{**source_base,"BPMS":"0=-1,2=-10,4=180","STOPS":"1.001=0.5,1.009=0,2=-0.25","DELAYS":"1.001=0.5,1.009=-1","WARPS":"1.001=2,1.009=0","FAKES":"0=6,1.001=1,1.009=-1"},
        "tiny-regions":{**source_base,"WARPS":"1=0.000001,2=0.001","FAKES":"3=0.000001,4=0.001"},
        "below-min-bpm":{**source_base,"BPMS":"0=0.0000001,4=120"},
        "empty-bpm":{"BPMS":"","DISPLAYBPM":"143.5:190"},
        "future-bpm":{"BPMS":"4=180","DISPLAYBPM":"143.5:190"},
        "display-invalid-range":{"DISPLAYBPM":"143.5:bad"},
        "negative-positions":{**source_base,"STOPS":"-2=1,-0.1=0.5,2=0.25","DELAYS":"-2=1,1=-1,2=0","WARPS":"-2=1,1=-1,2=1","FAKES":"-2=1,3=1"},
        "metadata-conflicts":{**source_base,"TIMESIGNATURES":"0=4=4,1.001=3=4,1.009=0=4,2=3=5,3=3=4","TICKCOUNTS":"1.001=4,1.009=8","COMBOS":"1.001=1,1.009=2=3","LABELS":"1.001=First,1.009=Last","SCROLLS":"1.001=1,1.009=0.5","SPEEDS":"1.001=1=0=0,1.009=2=1=0","WARPS":"0=4"},
        "malformed":{**source_base,"BPMS":"bad,0=120,1=nope,2=180","STOPS":"bad,1=0.25,2=nope","DELAYS":"1=bad,2=0.25","WARPS":"bad,1=1","FAKES":"bad,2=1"},
    }
    for label,fields in source_cases.items():
        cases["source-timing-"+label]={"Operation":"NormalizedFile","Format":"ssc","Content":"#VERSION:0.83;"+tags(fields)+region_chart()}
    for label in ["half-rows","tiny-regions"]:cases["source-timing-"+label]["PerformedConfig"]=copy.deepcopy(performed)
    cases["source-timing-repeat-tags"]={"Operation":"NormalizedFile","Format":"ssc","Content":"#VERSION:0.83;#BPMS:0=120,2=150;#BPMS:4=180,2=200;#STOPS:1=0.25;#STOPS:3=0.5;#FAKES:0=6;#FAKES:1=1;"+region_chart()+region_chart({"BPMS":"0=150"})}
    cases["source-timing-histogram"]={"Operation":"NormalizedFile","Format":"ssc","Content":"#VERSION:0.83;#BPMS:4=180;#DISPLAYBPM:143.5;"+region_chart({"BPMS":"0=150"})+region_chart({"BPMS":"0=180"}).replace("dance-single","pump-single").replace(region_notes,"10000\n01000\n00100\n00010")+region_chart({"BPMS":"0=210"})+region_chart({"OFFSET":"0.25"})}
    cases["source-timing-sm"]={"Operation":"NormalizedFile","Format":"sm","Content":"#BPMS:0=120,0.010416666666666666=180;#FREEZES:1=0.25;#FREEZES:2=0.125;#STOPS:3=0.5;#DELAYS:2=0.125;#WARPS:0=4;#FAKES:0=4;#SCROLLS:0=2;#NOTES:dance-single::Hard:9::"+region_notes+";"}
    legacy_root="#VERSION:0.60;#TITLE:Legacy;"+tags(region_fields)
    legacy_content=legacy_root+region_chart()+region_chart(region_own)+region_chart({"OFFSET":"0.125","STOPS":"1=0.25"})+region_chart()
    cases["legacy-timing-mixed"]={"Operation":"NormalizedFile","Format":"ssc","Content":legacy_content,"PerformedConfig":copy.deepcopy(performed)}
    cases["legacy-timing-missing-version"]={"Operation":"NormalizedFile","Format":"ssc","Content":"#TITLE:Legacy;#BPMS:4=180;#DISPLAYBPM:143.5;#STOPS:1=0.25;"+region_chart({"BPMS":"0=150"})+region_chart({"OFFSET":"0.125"})+region_chart()}
    cases["legacy-timing-fractional"]={"Operation":"NormalizedFile","Format":"ssc","Content":"#VERSION:0.60;"+tags(source_cases["tiny-regions"])+region_chart()+region_chart({"BPMS":"0=0.0000001,4=180","OFFSET":"0.25","WARPS":"1=0.00001"})}
    for label,content in {"defaults":"{}", "positive":'{"SyncOffset":0.009,"DefaultChartType":"dance_single","DefaultChartIndex":1}', "negative":'{"SyncOffset":-0.125}', "comments":'/* song */ {"SyncOffset":0.25}', "zero":'{"SyncOffset":0}'}.items():
        cases["song-sync-"+label]={"Operation":"SongSync", "Content":content, "MusicOffset":0.375, "Times":[-2,-0.5,0,0.009,1,25.5]}
    for label,format,difficulty,rating in [("ssc-decimal","ssc","Hard","9.5"),("ssc-thousands","ssc","Edit","1,000"),("ssc-unknown","ssc","Future","bad"),("ssc-nonfinite","ssc","99","NaN"),("ssc-number","ssc","3","+12"),("sm-decimal","sm","Easy","9.5"),("sm-legacy","sm","Hard","-7")]:
        content=("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:"+difficulty+";#METER:"+rating+";#NOTES:1000;") if format=="ssc" else ("#BPMS:0=120;#NOTES2:dance-single:Desc:"+difficulty+":"+rating+":0,0,0,0,0:1000;")
        cases["chart-identity-"+label]={"Operation":"ChartIdentity","Format":format,"Content":content}
    for label,format,content in [
        ("ssc-fields","ssc","#VERSION:0.83;#BPMS:0=120;#CREDIT:Song author;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:Été 日本語;#DESCRIPTION:A\\:B\\;C\\\\D;#CREDIT:Local;#CHARTSTYLE:Pad;#NOTES:1000;"),
        ("ssc-inherited","ssc","#VERSION:0.83;#BPMS:0=120;#CREDIT:Song author;#NOTEDATA:;#STEPSTYPE:dance-single;#CREDIT:;#NOTES:1000;"),
        ("ssc-missing","ssc","#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"),
        ("sm-fields","sm","#BPMS:0=120;#CREDIT:Song author;#NOTES2:dance-single:Été 日本語 A\\:B:Hard:9::1000;")
    ]:
        cases["chart-text-"+label]={"Operation":"ChartText","Format":format,"Content":content}
    for label,local in [("override","#MUSIC:sub/Été.ogg;"),("empty","#MUSIC:;"),("missing","")]:
        cases["chart-music-"+label]={"Operation":"ChartText","Format":"ssc","Content":"#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;"+local+"#NOTES:1000;"}
    cases["chart-music-sm"]={"Operation":"ChartText","Format":"sm","Content":"#MUSIC:song.ogg;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;"}
    for label,directory,path in [("same","/Songs/Pack/Song","/Songs/Pack/Song/chart.ogg"),("subfolder","/Songs/Pack/Song","/Songs/Pack/Song/audio/Été.ogg"),("outside","/Songs/Pack/Song","/Songs/Pack/Other/chart.ogg"),("dots","/Songs/Pack/Song","/Songs/Pack/Song/audio/../chart.ogg"),("spaces","/Songs/Pack/Song","/Songs/Pack/Song/a b#c.ogg"),("percent","/Songs/Pack/Song","/Songs/Pack/Song/a%20b.ogg")]:
        cases["music-relative-"+label]={"Operation":"MusicRelative","Directory":directory,"Path":path}
    for label,format,properties in [("ssc","ssc","#TITLE:Été 日本語;#SUBTITLE:Version;#ARTIST:Composer;#CREDIT:File author;#GENRE:Dance;#ORIGIN:Arcade;#TITLETRANSLIT:Summer;#SUBTITLETRANSLIT:Mix;#ARTISTTRANSLIT:Latin;#MUSIC:song.ogg;"),("sm","sm","#TITLE:Été 日本語;#SUBTITLE:Version;#ARTIST:Composer;#CREDIT:File author;#GENRE:Dance;#ORIGIN:Arcade;#TITLETRANSLIT:Summer;#SUBTITLETRANSLIT:Mix;#ARTISTTRANSLIT:Latin;#MUSIC:song.ogg;"),("empty","ssc",""),("escaped","ssc",r"#TITLE:A\:B\;C\\D;#ARTIST:Été / 日本語;#CREDIT:Old;#MUSIC:audio/Été%20.ogg;")]:
        chart=("#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#CREDIT:Local;#NOTES:0001;") if format=="ssc" else "#NOTES:dance-single::Hard:9::1000;"
        cases["song-text-"+label]={"Operation":"SongText","Format":format,"Content":"#VERSION:0.83;#BPMS:0=120;"+properties+chart}
    for format in ["ssc", "sm"]:
        chart = "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;" if format == "ssc" else "#NOTES:dance-single::Hard:9::1000;"
        cases["song-text-preview-"+format] = {"Operation":"SongText", "Format":format, "Content":r"#VERSION:0.83;#TITLE:Preview;#BPMS:0=120;#MUSIC:song.ogg;#PREVIEW:audio/Été\;mix.wav;#SAMPLESTART:-0.125;#SAMPLELENGTH:15.75;"+chart}
    def empty_chart(layout="dance-single",bpm=None,signature=None):
        return "#NOTEDATA:;#STEPSTYPE:"+layout+";"+("#BPMS:"+bpm+";" if bpm is not None else "")+("#TIMESIGNATURES:"+signature+";" if signature is not None else "")+"#NOTES:1"+"0"*({"dance-double":8,"dance-solo":6}.get(layout,4)-1)+";"
    for label,content in {
        "empty":"#VERSION:0.83;#TITLE:New;",
        "song-fallback":"#VERSION:0.83;#BPMS:0=137.5;#TIMESIGNATURES:0=3=4;",
        "display-fallback":"#VERSION:0.83;#BPMS:4=190;#DISPLAYBPM:123:150;",
        "majority":"#VERSION:0.83;"+empty_chart(bpm="0=150",signature="0=3=4")+empty_chart("dance-double","0=180","0=7=8")+empty_chart("dance-solo","0=150","0=3=4"),
        "grouped-ties":"#VERSION:0.83;"+empty_chart(bpm="0=100",signature="0=2=2")+empty_chart("dance-double","0=120","0=4=4")+empty_chart(bpm="0=140",signature="0=7=8"),
        "fraction-hash":"#VERSION:0.83;"+empty_chart(bpm="0=150",signature="0=1=4")+empty_chart("dance-double","0=160","0=2=8")+empty_chart("dance-solo","0=170","0=3=4"),
        "missing-initial":"#VERSION:0.83;#BPMS:0=115;"+empty_chart(bpm="2=180",signature="2=3=4")+empty_chart("dance-double","0=170","0=5=8"),
    }.items():
        cases["new-chart-defaults-"+label]={"Operation":"NewChartDefaults","Format":"ssc","Content":content,"Layouts":["dance-single","dance-double","dance-couple","dance-routine","dance-solo","dance-threepanel","pump-single","pump-halfdouble","pump-double","pump-couple","pump-routine","smx-beginner","smx-single","smx-dual","smx-full","smx-team"]}
    for label,starts in {"empty":[],"singleton":[48],"normal":[0,48,96,288],"limits":[0,2147483647]}.items():
        cases["pattern-navigation-"+label]={"Operation":"PatternNavigation","Starts":starts,"Rows":[0,1,47,48,49,72,95,96,97,287,288,289,2147483647]}
    for label,segments in {
        "default":[], "meters":[[0,3,4],[3,7,8],[6.5,2,2]],
        "off-measure":[[0,4,4],[1,3,4],[2,5,8],[8,4,4]],
        "reader-denominators":[[0,5,6],[4,7,192],[8,0,4],[12,4,5]],
        "collisions":[[-1,3,4],[0,2,2],[1.001,3,8],[1.002,0,4],[2,7,16]],
    }.items():
        cases["signature-edit-"+label]={"Operation":"Signature", "Segments":segments,"Rows":[0,1,47,48,49,96,144,191,192,240,312,384,768],
            "Operations":[["set",0,"2/2"],["set",0,"4/4"],["set",24,"3/8"],["set",24,"3/8"],["set",96,"7/16"],["set",120,"5/3"],["set",120,"0/4"],["set",120,"  +5 / +32 "],["set",144,"5/64"],["move",24,96],["move",24,25],["move",0,48],["remove",0],["remove",25],["remove",25]]}
    for label,segments in {"empty":[],"reverse":[[0,1],[2,0],[4,-1],[8,2.5]],"late-first":[[4,-0.5]],"collisions":[[-1,2],[1.001,3],[1.002,-2],[2,1]]}.items():
        cases["scroll-edit-"+label]={"Operation":"Scroll", "Segments":segments,"Operations":[["set",0,0],["set",0,-1],["set",24,1.25],["set",24,1.25],["set",24,1.2500000000000002],["move",0,24],["move",0,12],["remove",12],["remove",12],["move",24,48],["set",48,-2.75],["remove",48]]}
    for label,segments in {"empty":[],"normal":[[0,4],[2,8],[4,0]],"signed-source":[[0,-1],[4,-2]],"collisions":[[-1,4],[1.001,3],[1.002,-2],[2,2147483647]]}.items():
        cases["tick-edit-"+label]={"Operation":"Tick", "Segments":segments,"Operations":[["set",0,0],["set",0,0],["set",24,8],["set",24,-1],["set",96,2147483647],["move",0,24],["move",0,12],["remove",12],["remove",12],["move",24,48],["remove",48]]}
    for label,source in {"empty":"","normal":"0=1=1,4=2=3,8=0=0","optional-and-malformed":"0=3,1=2.5,2=2147483648,3=4=,4=+2=+3,5=2=3=4","signed-collisions":"-1=2=3,0=-2=3,1.001=2=4,1.002=-1=-3,2=-2147483648=2147483647"}.items():
        cases["combo-edit-"+label]={"Operation":"Combo", "Source":source,"Operations":[["set",0,"0x/0x"],["set",0,"2x/3x"],["set",0,"2x/3x"],["set",24,"1x/4x"],["set",96,"2147483647x/2147483647x"],["set",48,"-1x/1x"],["set",48,"2x/3x\n"],["set",48,"2x/3x\r\n"],["set",48,"2X/3X"],["set",48," 2x/3x"],["set",48,"2/3"],["set",48,"+2x/3x"],["set",48,"2147483648x/1x"],["set",48,"002x/003x"],["move",0,24],["move",0,12],["remove",12],["remove",12],["move",24,72],["remove",72]]}
    for label,source in {"empty":"","rows-and-interruption":"0=1=0=0,1=3=4=0,2=0.5=2=0","seconds":"0=1=0=1,1=4=0.5=1,3=2=0=1","signed-source":"1=-1=-1=0,4=2=-0.25=-3","fractional-and-malformed":"0=1=0=0,1.001=2=0.0001=0,1.002=3=0.75=2,2=bad=1=0,3=4=1"}.items():
        cases["speed-edit-"+label]={"Operation":"Speed", "Source":source,"Queries":[[-48,-0.5],[0,0],[24,0.25],[48,0.5],[60,0.625],[72,0.75],[96,1],[120,1.25],[144,1.5],[192,2],[240,2.5],[384,4]],
            "Operations":[["set",0,"1x/0rows"],["set",48,"2x/48rows"],["set",48,"2x/48rows"],["set",96,".5x/.25s"],["set",144,"2.x/0.s"],["set",144,"-1x/1s"],["set",144,"2x/1.5rows"],["set",144,"1e2x/1s"],["set",144,"2x/1s\n"],["set",144,"2x/1s\r\n"],["move",0,48],["move",0,12],["remove",12],["move",48,72],["remove",72]]}
    cases["source-timing-escaped-labels"]={"Operation":"NormalizedFile","Format":"ssc","Content":"#VERSION:0.83;#BPMS:0=120;#LABELS:0=Legacy\\/\\/intro,16=End;"+region_chart()}
    for label,source in {"empty":"","unicode":"0=Pré-refrain,4=日本語 🎵","legacy-text":"0= URL//intro ,4=Legacy:label,8=Ends;ignored","collisions":"-1=skip,0=,1.001=A,1.002=B,2=bad=record"}.items():
        cases["label-edit-"+label]={"Operation":"Label","Source":source,"Sanitize":["","Pré-refrain 日本語 🎵","a\r\n\tb#c;d:e\\f/g,h","A=B","👩‍👩‍👧‍👦"],"Operations":[["set",0,"Refrain 🎵"],["set",0,"Refrain 🎵"],["set",24,"a\r\n\tb#c;d:e\\f/g,h"],["move",0,24],["move",0,12],["remove",12],["set",48,""],["move",24,72],["remove",72],["set",240," tail "]]}
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
    for name,events in {
        "single":[[0,"tempo",120]],
        "rounding-history":[[row*48,"tempo",[120.123,120.12300000001,240.246][row%3]] for row in range(201)],
        "tie":[[0,"tempo",120],[96,"tempo",240],[288,"tempo",180]],
        "repeat":[[0,"tempo",120],[96,"tempo",240],[192,"tempo",120],[384,"scroll",1]],
        "last-rate":[[0,"tempo",120],[48,"tempo",240],[480,"scroll",1]],
        "late-first":[[48,"tempo",180],[144,"scroll",1]],
        "warp-stop":[[0,"tempo",120],[48,"warp",192],[96,"tempo",240],[192,"stop",-0.5],[288,"scroll",-1],[384,"tempo",120],[480,"delay",0.5]],
    }.items():
        cases["waveform-tempo-"+name]={"Operation":"EditorTiming","TempoSummary":True,"Timing":[[0,"signature",4,4]]+events,"Notes":[[768,0,"1"]]}
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
    tempo_hash = prepare_tempo_reference(args.editor_root.resolve())
    pause_hash = prepare_pause_reference(args.editor_root.resolve())
    region_hash = prepare_region_reference(args.editor_root.resolve())
    source_timing_hash = prepare_source_timing_reference(args.editor_root.resolve())
    signature_hash = prepare_signature_reference(args.editor_root.resolve())
    scroll_hash = prepare_scroll_reference(args.editor_root.resolve())
    tick_hash = prepare_tick_reference(args.editor_root.resolve())
    combo_hash = prepare_combo_reference(args.editor_root.resolve())
    speed_hash = prepare_speed_reference(args.editor_root.resolve())
    label_hash = prepare_label_reference(args.editor_root.resolve())
    pattern_navigation_hash = prepare_pattern_navigation_reference(args.editor_root.resolve())
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
        # System.Text.Json writes negative zero as -0. Preserve its sign when
        # converting the oracle response back to committed JSON.
        parsed = json.loads(result, parse_int=lambda token: -0.0 if token == "-0" else int(token))
        (output / f"{name}.expected.json").write_text(json.dumps(parsed, indent=2) + "\n")
        print(name)
    (output / "provenance.json").write_text(json.dumps({
        "reference": "https://github.com/PerryAsleep/StepManiaLibrary", "commit": commit,
        "editor_commit": EDITOR_PIN, "editor_timing_adapter_sha256": hashlib.sha256((ROOT / "scripts/editor-oracle/EditorTimingReference.cs.template").read_bytes()).hexdigest(), "editor_context_methods_sha256": context_hash, "editor_timing_methods_sha256": timing_hash, "editor_metadata_types_sha256": metadata_hash, "editor_profile_setters_sha256": profiles_hash, "editor_tempo_methods_sha256": tempo_hash, "editor_pause_methods_sha256": pause_hash, "editor_region_methods_sha256": region_hash, "editor_source_timing_methods_sha256": source_timing_hash, "editor_signature_methods_sha256": signature_hash, "editor_scroll_methods_sha256": scroll_hash, "editor_tick_methods_sha256": tick_hash, "editor_combo_methods_sha256": combo_hash, "editor_speed_methods_sha256": speed_hash, "editor_label_methods_sha256": label_hash, "editor_pattern_navigation_methods_sha256": pattern_navigation_hash,
        "fumen_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=reference / "Fumen", text=True).strip(),
        "dotnet_sdk": subprocess.check_output([str(args.dotnet), "--version"], env=env, text=True).strip(),
        "dotnet_runtimes": [line.split(" [", 1)[0] for line in subprocess.check_output([str(args.dotnet), "--list-runtimes"], env=env, text=True).splitlines()],
        "sort_source": "https://github.com/dotnet/runtime/blob/v10.0.13/src/libraries/System.Private.CoreLib/src/System/Collections/Generic/ArraySortHelper.cs",
        "notice": "Expected results come from pinned C# libraries and unchanged GrooveAuthor helper bodies, not from the Rust port. Editor adapters provide explicit input events; they do not run the full editor.",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
