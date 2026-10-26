#!/usr/bin/env python3
"""Regenerate attack expectations by executing the pinned Fumen parser."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
LIB_PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", default="dotnet")
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
    env = dict(os.environ, DOTNET_NOLOGO="1", DOTNET_CLI_TELEMETRY_OPTOUT="1")
    subprocess.run([args.dotnet, "build", str(project / "EditorOracle.csproj"), "-c", "Release", f"-p:ReferenceRoot={reference}", "--ignore-failed-sources"], check=True, env=env)
    fixtures = ROOT / "crates/deadsync-editor/tests/fixtures/grooveauthor"
    request = fixtures / "attack-events.request.json"
    result = subprocess.check_output([args.dotnet, str(project / "bin/Release/net10.0/EditorOracle.dll"), str(reference), str(request)], env=env)
    cases = json.loads(result)
    (fixtures / "attack-events.expected.json").write_text(json.dumps(cases, indent=2, ensure_ascii=False) + "\n")
    source = reference / "Fumen/Fumen/Converters/SMPropertyParsers.cs"
    manifest = {
        "StepManiaLibrary": LIB_PIN, "Fumen": FUMEN_PIN,
        "ParserSourceSHA256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "AdapterSHA256": hashlib.sha256((project / "Program.cs").read_bytes()).hexdigest(),
        "RequestSHA256": hashlib.sha256(request.read_bytes()).hexdigest(),
        "Cases": len(cases),
        "Scope": "Unmodified Fumen AttackPropertyParser with MSD parameter adapter; no native gameplay or row/time conversion claim",
    }
    (fixtures / "attack-events-reference.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Generated {len(cases)} Fumen attack parser cases")

if __name__ == "__main__":
    main()
