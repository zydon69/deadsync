#!/usr/bin/env python3
"""Regenerate attack expectations by executing the pinned Fumen parser."""
import argparse
import hashlib
import json
import os
import runpy
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
LIB_PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", default="dotnet")
    parser.add_argument("--editor-root", type=Path, default=ROOT.parent / "grooveauthor")
    parser.add_argument("--placement", action="store_true", help="Use actual SM/SSC readers for attack placement")
    args = parser.parse_args()
    reference = args.reference_root.resolve()
    for directory, pin in [(reference, LIB_PIN), (reference / "Fumen", FUMEN_PIN)]:
        if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=directory, text=True).strip() != pin:
            raise SystemExit("Reference pin mismatch")
        if subprocess.check_output(["git", "diff", "--", "*.cs"], cwd=directory, text=True):
            raise SystemExit("Reference C# sources have local modifications")
    project = ROOT / "scripts/editor-oracle"
    if not (project / "obj/PatternContextReference.cs").is_file():
        raise SystemExit("Prepare editor-oracle with editor-generate-fixtures.py first")
    timing_hash = None
    if args.placement:
        helpers = runpy.run_path(str(ROOT / "scripts/editor-generate-fixtures.py"))
        timing_hash = helpers["prepare_timing_reference"](args.editor_root)
    env = dict(os.environ, DOTNET_NOLOGO="1", DOTNET_CLI_TELEMETRY_OPTOUT="1")
    subprocess.run([args.dotnet, "build", str(project / "EditorOracle.csproj"), "-c", "Release", f"-p:ReferenceRoot={reference}", "--ignore-failed-sources"], check=True, env=env)
    fixtures = ROOT / "crates/deadsync-editor/tests/fixtures/grooveauthor"
    prefix = "attack-placement" if args.placement else "attack-events"
    request = fixtures / (prefix + ".request.json")
    result = subprocess.check_output([args.dotnet, str(project / "bin/Release/net10.0/EditorOracle.dll"), str(reference), str(request)], env=env)
    cases = json.loads(result)
    regressions = []
    if args.placement:
        for name in ["editor-timing-baseline", "editor-timing-warp", "editor-timing-negative-stop-delay"]:
            regression_request = fixtures / (name + ".request.json")
            actual = json.loads(subprocess.check_output([args.dotnet, str(project / "bin/Release/net10.0/EditorOracle.dll"), str(reference), str(regression_request)], env=env))
            expected = json.loads((fixtures / (name + ".expected.json")).read_text())
            if actual != expected:
                raise SystemExit(f"Existing original timing trace changed: {name}")
            regressions.append(name)
    (fixtures / (prefix + ".expected.json")).write_text(json.dumps(cases, indent=2, ensure_ascii=False) + "\n")
    source = reference / "Fumen/Fumen/Converters/SMPropertyParsers.cs"
    manifest = {
        "StepManiaLibrary": LIB_PIN, "Fumen": FUMEN_PIN,
        "ParserSourceSHA256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "AdapterSHA256": hashlib.sha256((project / "Program.cs").read_bytes()).hexdigest(),
        "RequestSHA256": hashlib.sha256(request.read_bytes()).hexdigest(),
        "Cases": len(cases),
        "Scope": "Unmodified Fumen AttackPropertyParser with MSD parameter adapter; no native gameplay or row/time conversion claim",
    }
    if args.placement:
        manifest["Scope"] = "Unmodified Fumen readers/SetAttackRowsAndTimes followed by original GrooveAuthor timing refresh with event-container adapters"
        manifest["GrooveAuthor"] = helpers["EDITOR_PIN"]
        manifest["UnchangedTimingTraces"] = regressions
        manifest["EditorTimingMethodsSHA256"] = timing_hash
        manifest["EditorTimingAdapterSHA256"] = hashlib.sha256((project / "EditorTimingReference.cs.template").read_bytes()).hexdigest()
        manifest["LoaderSourcesSHA256"] = {
            name: hashlib.sha256((reference / "Fumen/Fumen/Converters" / name).read_bytes()).hexdigest()
            for name in ["SMReader.cs", "SSCReader.cs", "SMCommon.cs", "SMPropertyParsers.cs"]
        }
    (fixtures / (prefix + "-reference.json")).write_text(json.dumps(manifest, indent=2) + "\n")
    kind = "Fumen file-placement/editor-refresh traces" if args.placement else "Fumen attack parser cases"
    print(f"Generated {len(cases)} {kind}")

if __name__ == "__main__":
    main()
