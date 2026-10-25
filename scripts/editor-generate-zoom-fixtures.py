#!/usr/bin/env python3
"""Run unchanged pinned GrooveAuthor ZoomManager with Fumen for Rust fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
LIB_PIN = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--editor-root", type=Path, required=True)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", default="dotnet")
    args = parser.parse_args()
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.reference_root, text=True).strip()
    if actual != LIB_PIN:
        raise SystemExit("StepManiaLibrary reference pin mismatch")
    fumen_pin = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.reference_root / "Fumen", text=True).strip()
    if fumen_pin != FUMEN_PIN:
        raise SystemExit("Fumen reference pin mismatch")
    source = subprocess.check_output(["git", "show", f"{PIN}:StepManiaEditor/ZoomManager.cs"], cwd=args.editor_root)
    project = ROOT / "scripts/editor-zoom-oracle"
    (project / "obj").mkdir(exist_ok=True)
    (project / "obj/ZoomManager.cs").write_bytes(source)
    fumen = args.reference_root / "Fumen/Fumen/bin/Release/net10.0/Fumen.dll"
    if not fumen.is_file():
        raise SystemExit("Build the pinned Fumen development reference for net10.0 first")
    env = dict(os.environ, DOTNET_CLI_TELEMETRY_OPTOUT="1", DOTNET_NOLOGO="1")
    subprocess.run([args.dotnet, "build", str(project / "ZoomOracle.csproj"), "-c", "Release",
                    f"-p:ReferenceRoot={args.reference_root.resolve()}", "--ignore-failed-sources"], check=True, env=env)
    destination = ROOT / "crates/deadsync-editor/tests/fixtures/grooveauthor"
    requests = sorted(destination.glob("zoom-*.request.json"))
    if not requests:
        raise SystemExit("No committed zoom request fixtures found")
    for path in requests:
        output = subprocess.check_output([args.dotnet, str(project / "bin/Release/net10.0/ZoomOracle.dll"), str(path)], env=env)
        path.with_name(path.name.replace(".request.json", ".expected.json")).write_bytes(output)
    cases = [path.name.removeprefix("zoom-").removesuffix(".request.json") for path in requests]
    manifest = {"GrooveAuthor":PIN,"StepManiaLibrary":LIB_PIN,"Fumen":FUMEN_PIN,"FumenAssemblySHA256":hashlib.sha256(fumen.read_bytes()).hexdigest(),"ZoomManagerSHA256":hashlib.sha256(source).hexdigest(),"Cases":list(cases),"AdapterScope":"Preference notifications and keyboard/mouse flags only; original ZoomManager unchanged"}
    (destination / "zoom-reference.json").write_text(json.dumps(manifest, indent=2)+"\n")
    print(f"Generated {len(cases)} original ZoomManager traces")


if __name__ == "__main__":
    main()
