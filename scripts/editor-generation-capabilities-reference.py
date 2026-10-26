#!/usr/bin/env python3
"""Extract the pinned original generation capability contract (static evidence).

This records unchanged source members and their directly declared layout/default
inventories. It does not execute GrooveAuthor or stand in for an interpreter
oracle. No graph aliases or multiplayer output semantics are invented here.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

EDITOR_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
LIBRARY_PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--editor-root", required=True, type=Path)
parser.add_argument("--library-root", required=True, type=Path)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()

def source(root, pin, path):
    return subprocess.check_output(["git", "-C", str(root), "show", f"{pin}:{path}"], text=True, encoding="utf-8-sig")

def member(text, signature):
    start = text.index(signature)
    brace = text.index("{", start)
    depth = 0
    for end in range(brace, len(text)):
        if text[end] == "{": depth += 1
        elif text[end] == "}": depth -= 1
        if depth == 0: return text[start:end + 1]
    raise ValueError(signature)

editor = source(args.editor_root, EDITOR_PIN, "StepManiaEditor/Editor.cs")
chart = source(args.editor_root, EDITOR_PIN, "StepManiaEditor/EditorChart.cs")
common = source(args.library_root / "Fumen", FUMEN_PIN, "Fumen/Converters/SMCommon.cs")
arrays = {}
for name in ["SupportedSinglePlayerChartTypes", "SupportedChartTypes"]:
    match = re.search(r"public static readonly ChartType\[\] " + name + r"\s*=\s*\[(.*?)\];", editor, re.S)
    assert match
    arrays[name] = match.group()
    content = re.sub(r"//[^\n]*", "", match[1])
    arrays[name + "Values"] = [value.replace("_", "-") for value in re.findall(r"ChartType\.(\w+)", content)]
methods = {name: member(chart, signature) for name, signature in [
    ("IsMultiPlayer", "public bool IsMultiPlayer()"),
    ("SupportsAutogenFeatures", "public bool SupportsAutogenFeatures()")
]}
assert re.sub(r"\s+", "", methods["IsMultiPlayer"]).endswith("{returnDefaultNumPlayers>1;}")
assert re.sub(r"\s+", "", methods["SupportsAutogenFeatures"]).endswith("{return!IsMultiPlayer();}")
methods["GetStepGraph"] = member(editor, "public bool GetStepGraph(ChartType chartType,")
assert "StepGraphByChartType.TryGetValue(chartType, out stepGraph)" in methods["GetStepGraph"]
assert "foreach (var chartType in SupportedSinglePlayerChartTypes)\n\t\t\tStepGraphByChartType[chartType] = null;" in editor
properties = {name.replace("_", "-"): [int(lanes), int(players), variable == "true"]
    for name, lanes, players, variable in re.findall(r"Properties\[\(int\)ChartType\.(\w+)\] = new ChartProperties\((\d+), (\d+), (true|false)\)", common)}
all_layouts = arrays["SupportedChartTypesValues"] + ["dance-couple", "pump-couple"]
single = arrays["SupportedSinglePlayerChartTypesValues"]
cases = [{"layout": layout, "default_players": properties[layout][1], "columns": properties[layout][0],
    "supported_editor": layout in arrays["SupportedChartTypesValues"],
    "graph_registered": layout in single,
    "autogen_feature": properties[layout][1] <= 1} for layout in all_layouts]
output = {"single_player_graphs": single, "cases": cases, "source_arrays": arrays, "source_methods": methods,
    "provenance": {"editor_commit": EDITOR_PIN, "library_commit": LIBRARY_PIN, "fumen_commit": FUMEN_PIN,
        "editor_sha256": hashlib.sha256(editor.encode()).hexdigest(), "chart_sha256": hashlib.sha256(chart.encode()).hexdigest(),
        "common_sha256": hashlib.sha256(common.encode()).hexdigest(),
        "scope": "Static extraction of original arrays, unchanged capability methods, graph registration and layout defaults. Not an executed GUI/.NET or generation oracle."}}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
