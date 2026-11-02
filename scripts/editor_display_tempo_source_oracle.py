#!/usr/bin/env python3
"""Replay pinned GrooveAuthor DisplayTempo and real Fumen writer/readback cases.

The complete graphical Editor is not compiled: its load-copy/save routing is
wired from the cited source, while the actual value model and Fumen execute.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile

SOURCE_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
MODEL = "StepManiaEditor/DisplayTempo.cs"
ROUTING = {
    "StepManiaEditor/Editor.cs": "431-432: InvariantCulture startup",
    "StepManiaEditor/EditorSong.cs": "734-779: per-chart copy; 1803-1812: fallback SM / remove global SSC",
    "StepManiaEditor/EditorChart.cs": "3197-3200: new DisplayTempo copy; 3886: chart.Tempo serialization",
    "StepManiaEditor/ImGui/ImGuiLayoutUtils.cs": "2332-2363: chart-local display UI callback",
}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def capture(arguments):
    return subprocess.check_output([str(value) for value in arguments])

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, required=True, help="GrooveAuthor Git checkout containing SOURCE_PIN")
    p.add_argument("--fumen", type=Path, required=True, help="Fumen Git checkout containing FUMEN_PIN")
    p.add_argument("--dotnet", default="dotnet", help=".NET10 SDK command")
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--provenance", type=Path, required=True)
    args = p.parse_args()
    model = capture(["git", "-C", args.source, "show", f"{SOURCE_PIN}:{MODEL}"])
    routing = {path: {"source_sha256": sha(capture(["git", "-C", args.source, "show", f"{SOURCE_PIN}:{path}"])), "wiring": explanation} for path, explanation in ROUTING.items()}
    # Build actual Fumen from an immutable archive, not an unverified prebuilt
    # DLL or the caller's potentially modified checkout. Its project remains
    # byte-identical to the pin, including its Concentus package reference.
    archive = capture(["git", "-C", args.fumen, "archive", "--format=tar", FUMEN_PIN, "Fumen"])
    harness_path = Path(__file__).with_suffix(".cs")
    harness = harness_path.read_bytes()
    with tempfile.TemporaryDirectory(prefix="deadsync-display-source-") as folder:
        root = Path(folder)
        with tarfile.open(fileobj=io.BytesIO(archive)) as source_tar:
            source_tar.extractall(root / "fumen-source", filter="data")
        (root / "DisplayTempo.cs").write_bytes(model)
        (root / "Program.cs").write_bytes(harness)
        project = root / "Oracle.csproj"
        project.write_text('''<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>enable</Nullable><EnableDefaultCompileItems>false</EnableDefaultCompileItems></PropertyGroup>
  <ItemGroup><Compile Include="Program.cs"/><Compile Include="DisplayTempo.cs"/><ProjectReference Include="fumen-source/Fumen/Fumen.csproj"/></ItemGroup>
</Project>''')
        build = subprocess.run([args.dotnet, "build", str(project), "--configuration", "Release", "--nologo", "--verbosity", "quiet", "-p:RestoreIgnoreFailedSources=true"], text=True, capture_output=True)
        if build.returncode:
            raise RuntimeError(build.stdout + build.stderr)
        executed = subprocess.run([args.dotnet, str(root / "bin/Release/net10.0/Oracle.dll")], text=True, capture_output=True)
        if executed.returncode:
            raise RuntimeError(executed.stdout + executed.stderr)
        result = json.loads(executed.stdout)
        if len(result["cases"]) != 5 or not all(case["independent"] for case in result["cases"]):
            raise RuntimeError("Incomplete source display-tempo matrix")
        fixture = {"sourcePin": SOURCE_PIN, "fumenPin": FUMEN_PIN, "culture": "InvariantCulture (Editor.cs431-432)", "executedParts": ["actual pinned DisplayTempo.cs constructors, FromString, ToString", "actual pinned Fumen SMReader/SMWriter/SSCReader/SSCWriter", "EditorSong load-copy/save routing wired from cited source, not the full graphical Editor runtime"], **result}
        rendered = (json.dumps(fixture, ensure_ascii=False, indent=2) + "\n").encode()
        provenance = {"source_pin": SOURCE_PIN, "fumen_pin": FUMEN_PIN, "model_path": MODEL, "model_sha256": sha(model), "routing_source": routing, "fumen_source_archive_sha256": sha(archive), "fumen_project_sha256": sha((root / "fumen-source/Fumen/Fumen.csproj").read_bytes()), "harness_sha256": sha(harness), "fixture_sha256": sha(rendered), "dotnet_sdk": capture([args.dotnet, "--version"]).decode().strip(), "cases": len(result["cases"]), "culture": "InvariantCulture as Editor startup", "scope": "Actual pinned DisplayTempo and actual Fumen; source EditorSong routing reproduced, no full graphical Editor execution"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(rendered)
    args.provenance.parent.mkdir(parents=True, exist_ok=True)
    args.provenance.write_text(json.dumps(provenance, indent=2) + "\n")
    print(f"Executed 5 pinned display-tempo source/writer cases: {args.output}")

if __name__ == "__main__":
    main()
