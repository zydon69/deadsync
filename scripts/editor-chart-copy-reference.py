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
project = repo / "scripts/editor-chart-copy-oracle"

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
pattern = read("EditorEvents/EditorPatternEvent.cs")
pieces = {
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
    "OriginalActionCopyEventsBetweenCharts.cs": read("EditorActions/ActionCopyEventsBetweenCharts.cs"),
    "OriginalForceAddSideEffect.cs": read("EditorActions/ForceAddSideEffect.cs"),
    "Adapter.cs": adapter,
}
for name, content in originals.items(): (generated / name).write_text(content)

def definition(seed=42, length=96):
    return dict(PatternConfigGuid="03de57bd-4329-4de6-a722-a171c11fdd16", PerformedChartConfigGuid="41a7f76e-596a-4fbe-bf6e-6a77a898e4a1", Length=length, RandomSeed=seed, StartPositionInclusive=True, EndPositionInclusive=False, IgnorePrecedingDistribution=True)

def marker(row, seed=42, length=96): return dict(Kind="Pattern", Row=row, Definition=definition(seed, length))
def tap(row, lane=0, player=0): return dict(Kind="Tap", Row=row, Lane=lane, Player=player)
def tempo(row, bpm): return dict(Kind="Tempo", Row=row, Bpm=bpm)
def signature(row,n,d): return dict(Kind="Signature", Row=row,N=n,D=d)
def chart(events,layout="dance-single",players=1): return dict(Layout=layout,Players=players,Events=events)
label=lambda row: dict(Kind="Label", Row=row)
delay=lambda row: dict(Kind="Delay",Row=row)
cases=[]
def add(name,types,source,destinations): cases.append(dict(Name=name,Types=types,Source=chart(source),Destinations=destinations))
add("replace-selected-retain-unselected",["Label"],[label(48),tempo(0,120)],[chart([label(96),tempo(0,90),delay(144)])])
add("empty-source-clears-selected-type",["Label"],[tempo(0,120)],[chart([tempo(0,90),label(96)])])
add("tempo-first-value-and-following-replacement",["Tempo"],[tempo(0,180),tempo(192,240)],[chart([tempo(0,90),tempo(96,110),label(96)])])
add("signature-written-ratio-retained",["Signature"],[signature(0,2,2),signature(192,3,4)],[chart([signature(0,4,4),signature(96,5,8),tempo(0,90)])])
add("first-event-destination-position-preserved",["Tempo","Signature"],[tempo(16,180),tempo(192,240),signature(16,2,2),signature(384,3,4)],[chart([tempo(48,90),tempo(96,110),signature(48,4,4),signature(192,5,8)])])
add("patterns-replace-whole-type-not-just-head",["Pattern"],[marker(48)],[chart([marker(96,7),marker(192,8),label(192)])])
add("overlapping-source-regions-retained",["Pattern"],[marker(48,length=192),marker(96,7,192)],[chart([marker(0,9)])])
add("multidestination-mixed-includes-routine-one",["Pattern","Label","Tempo"],[marker(48),label(96),tempo(0,180),tempo(192,240)],[chart([marker(0,7),label(0),tempo(0,90)]),chart([marker(0,8),label(192),tempo(0,100)],"dance-routine",1),chart([marker(0,9),tempo(0,110)],"pump-double")])
add("unsupported-patterns-remove-existing",["Pattern"],[marker(48)],[chart([marker(0,7)],"dance-routine",1),chart([marker(96,8)],"dance-couple",2)])
add("delay-and-attack-retain-rows",["Delay","Attack"],[delay(48),dict(Kind="Attack",Row=96)],[chart([delay(192),dict(Kind="Attack",Row=288),label(96)])])
request = dict(Cases=cases)
request_path = args.output.with_suffix(".request.json")
expected_path = args.output.with_suffix(".expected.json")
request_path.parent.mkdir(parents=True, exist_ok=True)
request_path.write_text(json.dumps(request, indent=2) + "\n")
subprocess.run([str(args.dotnet), "build", str(project / "ChartCopyOracle.csproj"), "--configuration", "Release", "--nologo", "--verbosity", "quiet", f"-p:ReferenceRoot={args.reference_root}"], check=True)
assembly = project / "bin/Release/net10.0/ChartCopyOracle.dll"
result = subprocess.run([str(args.dotnet), str(assembly), str(request_path)], check=True, capture_output=True, text=True)
expected = json.loads(result.stdout)
expected_path.write_text(json.dumps(expected, indent=2) + "\n")
digest = lambda content: hashlib.sha256(content).hexdigest()
provenance = dict(editor_commit=EDITOR_PIN, library_commit=LIBRARY_PIN, fumen_commit=FUMEN_PIN,
    sdk=subprocess.check_output([str(args.dotnet), "--version"], text=True).strip(),
    scope="Executed whole original ActionCopyEventsBetweenCharts unchanged, original IsMultiPlayer and Pattern.Definition/Clone; actual Fumen/SML event comparer. Event-container, BPM/signature value accessors, base-event clone, input-output and inert timing-callback adapters. Reused ForceAdd helpers are compiled but not used by this action. No original GUI, writer execution, profile observers or timing dependency math.",
    extracted_sha256={name: digest(body.encode()) for name, body in pieces.items()},
    sources_sha256={name: digest(content.encode()) for name, content in originals.items()},
    adapter_template_sha256=digest((project / "Adapter.cs.template").read_bytes()),
    program_sha256=digest((project / "Program.cs").read_bytes()),
    assembly_sha256=digest(assembly.read_bytes()), request_sha256=digest(request_path.read_bytes()), expected_sha256=digest(expected_path.read_bytes()))
args.output.with_suffix(".reference.json").write_text(json.dumps(provenance, indent=2) + "\n")
