#!/usr/bin/env python3
"""Run unchanged pinned clipboard actions and ForceAdd/definition helpers.

Adapters supply the event container, input/output projection and inert timing/
selection notifications. The actual original action, side-effect undo, ForceAdd,
Pattern.Definition clone/match and Fumen/StepManiaLibrary comparison execute.
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
project = repo / "scripts/editor-pattern-clipboard-oracle"

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
    "OriginalActionPasteEvents.cs": read("EditorActions/ActionPasteEvents.cs"),
    "OriginalForceAddSideEffect.cs": read("EditorActions/ForceAddSideEffect.cs"),
    "Adapter.cs": adapter,
}
for name, content in originals.items(): (generated / name).write_text(content)

def definition(seed=42, length=96):
    return dict(PatternConfigGuid="03de57bd-4329-4de6-a722-a171c11fdd16", PerformedChartConfigGuid="41a7f76e-596a-4fbe-bf6e-6a77a898e4a1", Length=length, RandomSeed=seed, StartPositionInclusive=True, EndPositionInclusive=False, IgnorePrecedingDistribution=True)

def marker(row, seed=42, length=96): return dict(Kind="Pattern", Row=row, Definition=definition(seed, length))
def tap(row, lane=0, player=0): return dict(Kind="Tap", Row=row, Lane=lane, Player=player)
cases = []
def add(name, copied, destination=None, row=192, layout="dance-single", players=1, player=0):
    cases.append(dict(Name=name, Layout=layout, Players=players, ActivePlayer=player, PasteRow=row, Copied=copied, Destination=destination or []))
add("pattern-shift", [marker(48)])
add("pattern-replaces-same-head", [marker(48)], [marker(192, 7), dict(Kind="Label", Row=192)])
add("overlapping-regions-remain", [marker(48, length=144)], [marker(144, 7, 192)])
add("mixed-earliest-note-origin", [tap(0), marker(48)], row=96)
add("two-marker-relative-heads", [marker(16, 42, 0), marker(80, -7, 128)], row=48)
for layout, players in [("dance-routine", 2), ("dance-routine", 1), ("pump-routine", 1), ("smx-team", 2), ("dance-couple", 2)]:
    add(f"skip-pattern-{layout}-{players}", [marker(48), tap(48)], layout=layout, players=players)
add("ordinary-double-accepts-pattern", [marker(48)], layout="pump-double")
add("cross-layout-skips-only-invalid-lanes", [tap(0, 7), marker(0), tap(48, 1)], layout="dance-single")
add("mixed-owner-clamping", [tap(0, 0, 0), marker(0), tap(48, 1, 3)], layout="dance-routine", players=2, player=1)
add("single-owner-destination-player", [tap(0, 0, 0), marker(0)], layout="dance-routine", players=2, player=1)
add("actual-library-event-order", [dict(Kind="Hold", Row=0, Lane=1, End=96), marker(0), tap(0), dict(Kind="Delay", Row=0), dict(Kind="Attack", Row=0)], row=48)
request = dict(Cases=cases)
request_path = args.output.with_suffix(".request.json")
expected_path = args.output.with_suffix(".expected.json")
request_path.parent.mkdir(parents=True, exist_ok=True)
request_path.write_text(json.dumps(request, indent=2) + "\n")
subprocess.run([str(args.dotnet), "build", str(project / "PatternClipboardOracle.csproj"), "--configuration", "Release", "--nologo", "--verbosity", "quiet", f"-p:ReferenceRoot={args.reference_root}"], check=True)
assembly = project / "bin/Release/net10.0/PatternClipboardOracle.dll"
result = subprocess.run([str(args.dotnet), str(assembly), str(request_path)], check=True, capture_output=True, text=True)
expected = json.loads(result.stdout)
expected_path.write_text(json.dumps(expected, indent=2) + "\n")
digest = lambda content: hashlib.sha256(content).hexdigest()
provenance = dict(editor_commit=EDITOR_PIN, library_commit=LIBRARY_PIN, fumen_commit=FUMEN_PIN,
    sdk=subprocess.check_output([str(args.dotnet), "--version"], text=True).strip(),
    scope="Executed original ActionPasteEvents and ForceAddSideEffect unchanged; original ForceAddEventInternal, CanEventExistAtRow, IsMultiPlayer, Pattern.Definition/Clone/Matches; actual Fumen/SML event comparer. Event-container/base-event clone/input-output/timing-notification adapters. No original GUI, profile observers or timing dependency math.",
    extracted_sha256={name: digest(body.encode()) for name, body in pieces.items()},
    sources_sha256={name: digest(content.encode()) for name, content in originals.items()},
    adapter_template_sha256=digest((project / "Adapter.cs.template").read_bytes()),
    program_sha256=digest((project / "Program.cs").read_bytes()),
    assembly_sha256=digest(assembly.read_bytes()), request_sha256=digest(request_path.read_bytes()), expected_sha256=digest(expected_path.read_bytes()))
args.output.with_suffix(".reference.json").write_text(json.dumps(provenance, indent=2) + "\n")
