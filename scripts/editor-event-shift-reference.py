#!/usr/bin/env python3
"""Execute unchanged pinned chart-wide typed-copy action.

Adapters provide event storage, value accessors and inert timing notifications;
the original action controls replacement, initial-node updates and undo/redo.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

EDITOR_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
LIBRARY_PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--editor-root", type=Path, required=True)
parser.add_argument("--reference-root", type=Path, required=True)
parser.add_argument("--dotnet", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
for root, expected in [(args.reference_root, LIBRARY_PIN), (args.reference_root / "Fumen", FUMEN_PIN)]:
    actual = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    if actual != expected: raise ValueError(f"Expected reference pin {expected}, found {actual}")
repo = Path(__file__).resolve().parent.parent
project = repo / "scripts/editor-event-shift-oracle"

def read(path):
    return subprocess.check_output(["git", "-C", str(args.editor_root), "show", f"{EDITOR_PIN}:StepManiaEditor/{path}"], text=True, encoding="utf-8-sig")

def member(text, signature):
    start = text.index(signature)
    brace = text.index("{", start)
    depth = 0
    for end in range(brace, len(text)):
        if text[end] == "{": depth += 1
        elif text[end] == "}": depth -= 1
        if depth == 0: return text[start:end + 1]
    raise ValueError(signature)

chart = read("EditorChart.cs")
event = read("EditorEvents/EditorEvent.cs")
pattern = read("EditorEvents/EditorPatternEvent.cs")
pieces = {
    "ORIGINAL_REPOSITIONABLE": member(event,"public bool CanBeRepositioned()"),
    "ORIGINAL_DEFINITION": member(pattern, "public class Definition"),
    "ORIGINAL_PATTERN_CLONE": member(pattern, "public override EditorEvent Clone("),
    "ORIGINAL_IS_MULTIPLAYER": member(chart, "public bool IsMultiPlayer()"),
    "ORIGINAL_CAN_EXIST": member(chart, "public bool CanEventExistAtRow("),
    "ORIGINAL_FORCE_ADD": member(chart, "private void ForceAddEventInternal("),
}
adapter = (project / "Adapter.cs.template").read_text()
for name, body in pieces.items(): adapter = adapter.replace("// " + name, body)
generated = project / "obj"
generated.mkdir(exist_ok=True)
originals = {
    "OriginalActionShiftSelectionRow.cs": read("EditorActions/ActionShiftSelectionRow.cs"),
    "OriginalForceAddSideEffect.cs": read("EditorActions/ForceAddSideEffect.cs"),
    "Adapter.cs": adapter,
}
for name, content in originals.items(): (generated / name).write_text(content)

def definition(seed=42, length=96):
    return dict(PatternConfigGuid="03de57bd-4329-4de6-a722-a171c11fdd16", PerformedChartConfigGuid="41a7f76e-596a-4fbe-bf6e-6a77a898e4a1", Length=length, RandomSeed=seed, StartPositionInclusive=True, EndPositionInclusive=False, IgnorePrecedingDistribution=True)

def marker(row, seed=42, length=96): return dict(Kind="Pattern", Row=row, Definition=definition(seed, length))
def tap(row, lane=0, player=0): return dict(Kind="Tap", Row=row, Lane=lane, Player=player)
def tempo(row,bpm): return dict(Kind="Tempo",Row=row,Bpm=bpm)
def signature(row,n,d): return dict(Kind="Signature",Row=row,N=n,D=d)
def unselected(value): return dict(value,Selected=False)
def label(row): return dict(Kind="Label",Row=row)
def hold(row,end,lane=0,player=0): return dict(Kind="Hold",Row=row,End=end,Lane=lane,Player=player)
cases=[]
def add(name,events,rows,layout="dance-single",players=1): cases.append(dict(Name=name,Events=events,Rows=rows,Layout=layout,Players=players))
add("mixed-later-protects-first-nodes",[tempo(0,120),signature(0,4,4),label(48),dict(Kind="Attack",Row=96),marker(144),tap(192,1)],48)
add("negative-rows-delete-not-clamp",[tap(0),label(16),marker(48),dict(Kind="Attack",Row=96)],-48)
add("first-nodes-immutable-later-nodes-move",[tempo(0,120),tempo(192,180),signature(0,4,4),signature(384,3,4),unselected(tempo(96,90))],96)
add("same-type-misc-replacement",[label(48),unselected(label(96)),unselected(dict(Kind="Delay",Row=96))],48)
add("same-head-pattern-replacement",[marker(48),unselected(marker(96,7))],48)
add("overlapping-pattern-regions-remain",[marker(48,length=192),unselected(marker(144,7,192))],48)
add("tap-trims-existing-hold",[unselected(hold(0,192)),tap(240)],-144)
add("tap-short-hold-becomes-tap",[unselected(hold(0,36)),tap(48)],-36)
add("hold-force-add-deletes-overlaps",[hold(0,48),unselected(tap(96)),unselected(tap(120)),unselected(tap(144))],96)
add("mixed-owner-notes-preserve-owners",[tap(48,0,0),tap(96,1,1),label(96)],48,"dance-routine",2)
add("unsupported-routine-pattern-is-deleted",[marker(48),tap(48,1)],48,"dance-routine",1)
add("zero-shift-preserves-events",[tempo(0,120),signature(0,4,4),label(48),marker(96),tap(144,1)],0)
request = dict(Cases=cases)
request_path = args.output.with_suffix(".request.json")
expected_path = args.output.with_suffix(".expected.json")
request_path.parent.mkdir(parents=True, exist_ok=True)
request_path.write_text(json.dumps(request, indent=2) + "\n")
subprocess.run([str(args.dotnet), "build", str(project / "EventShiftOracle.csproj"), "--configuration", "Release", "--nologo", "--verbosity", "quiet", f"-p:ReferenceRoot={args.reference_root}"], check=True)
assembly = project / "bin/Release/net10.0/EventShiftOracle.dll"
result = subprocess.run([str(args.dotnet), str(assembly), str(request_path)], check=True, capture_output=True, text=True)
expected = json.loads(result.stdout)
expected_path.write_text(json.dumps(expected, indent=2) + "\n")
digest = lambda content: hashlib.sha256(content).hexdigest()
provenance = dict(editor_commit=EDITOR_PIN, library_commit=LIBRARY_PIN, fumen_commit=FUMEN_PIN,
    sdk=subprocess.check_output([str(args.dotnet), "--version"], text=True).strip(),
    scope="Executed whole unchanged ActionShiftSelectionRow and ForceAddSideEffect, original ForceAddEventInternal/CanEventExistAtRow/CanBeRepositioned, Pattern definition and real Fumen/SML event comparison. Explicit event-container/base-event row and clone adapters, initial first-tempo/signature flags, selection notification projections, inert timing dependencies. No original GUI, writer or full clock recalculation.",
    extracted_sha256={name: digest(body.encode()) for name, body in pieces.items()},
    sources_sha256={name: digest(content.encode()) for name, content in originals.items()},
    adapter_template_sha256=digest((project / "Adapter.cs.template").read_bytes()),
    program_sha256=digest((project / "Program.cs").read_bytes()),
    assembly_sha256=digest(assembly.read_bytes()), request_sha256=digest(request_path.read_bytes()), expected_sha256=digest(expected_path.read_bytes()))
args.output.with_suffix(".reference.json").write_text(json.dumps(provenance, indent=2) + "\n")
