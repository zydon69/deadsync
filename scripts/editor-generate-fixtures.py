#!/usr/bin/env python3
"""Generate differential fixtures with pinned, unmodified StepManiaLibrary.

The .NET tool is development-only. Committed expected files let ordinary Rust
tests run without .NET, network access, or the source reference checkout.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess

PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
ROOT = Path(__file__).resolve().parents[1]


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
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", type=Path, required=True)
    args = parser.parse_args()
    reference = args.reference_root.resolve()
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=reference, text=True).strip()
    if commit != PIN:
        raise SystemExit(f"Reference must be pinned at {PIN}, found {commit}")
    env = {**os.environ, "DOTNET_CLI_TELEMETRY_OPTOUT": "1", "DOTNET_SKIP_FIRST_TIME_EXPERIENCE": "1"}
    project = ROOT / "scripts/editor-oracle/EditorOracle.csproj"
    subprocess.run([str(args.dotnet), "build", str(project), f"-p:ReferenceRoot={reference}", "-c", "Release"], env=env, check=True, stdout=subprocess.DEVNULL)
    oracle = project.parent / "bin/Release/net10.0/EditorOracle.dll"
    output = ROOT / "crates/deadsync-editor/tests/fixtures/grooveauthor"
    output.mkdir(parents=True, exist_ok=True)
    cases = requests(reference)
    for seed in [0, 42, -7, -2147483648, 2147483647]:
        cases[f"random-{seed}"] = {"Operation": "Random", "Seed": seed, "Count": 64}
    for name, request in cases.items():
        path = output / f"{name}.request.json"
        path.write_text(json.dumps(request, indent=2) + "\n")
        result = subprocess.check_output([str(args.dotnet), str(oracle), str(reference), str(path)], env=env, text=True)
        # Parse before publishing; diagnostic output must not become a baseline.
        parsed = json.loads(result)
        (output / f"{name}.expected.json").write_text(json.dumps(parsed, indent=2) + "\n")
        print(name)
    (output / "provenance.json").write_text(json.dumps({
        "reference": "https://github.com/PerryAsleep/StepManiaLibrary", "commit": commit,
        "fumen_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=reference / "Fumen", text=True).strip(),
        "dotnet_sdk": subprocess.check_output([str(args.dotnet), "--version"], env=env, text=True).strip(),
        "notice": "Expected results are produced by the unmodified C# library, not by the Rust port.",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
