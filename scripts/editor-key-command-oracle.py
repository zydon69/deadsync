#!/usr/bin/env python3
"""Run the pinned original command manager with host stubs; never rewrite Rust outputs."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
SOURCE = "StepManiaEditor/KeyCommandManager.cs"


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--source", type=Path, required=True, help="GrooveAuthor checkout")
    parser.add_argument("--dotnet", default="dotnet")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = (args.source / SOURCE).read_bytes()
    pinned = subprocess.check_output(["git", "show", f"{PIN}:{SOURCE}"], cwd=args.source)
    if source != pinned:
        raise SystemExit("Source differs from pinned Git blob; refusing oracle attribution")
    template = Path(__file__).with_suffix("")
    with tempfile.TemporaryDirectory(prefix="deadsync-key-command-") as temporary:
        root = Path(temporary)
        for name in ("Program.cs", "Oracle.csproj"):
            shutil.copyfile(template / name, root / name)
        (root / "KeyCommandManager.cs").write_bytes(source)
        process = subprocess.run(
            [args.dotnet, "run", "--project", str(root / "Oracle.csproj"), "--verbosity", "quiet"],
            check=True, capture_output=True, text=True,
        )
    # Require the recorder's structured result; build messages are not test data.
    records = [json.loads(line) for line in process.stdout.splitlines() if line.startswith("{")]
    if len(records) != 2:
        raise SystemExit(f"Unexpected recorder output: {process.stdout}")
    output = {"commit": PIN, "path": SOURCE, "sha256": hashlib.sha256(source).hexdigest(),
              "method": "Unmodified complete class; keyboard/preferences/observer host stubs only",
              "cases": records}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
